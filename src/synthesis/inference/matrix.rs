use std::{fmt, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatrixError {
    InvalidShape,
    InvalidLength {
        buffer: &'static str,
        expected: usize,
        actual: usize,
    },
    UnsupportedBits(u32),
    InvalidScale,
    Parallel(String),
    Cancelled,
}

impl fmt::Display for MatrixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidShape => f.write_str("Dense matrix dimensions are empty or overflow"),
            Self::InvalidLength {
                buffer,
                expected,
                actual,
            } => write!(
                f,
                "Invalid {buffer} length: expected {expected}, got {actual}"
            ),
            Self::UnsupportedBits(bits) => {
                write!(f, "Unsupported integer matrix precision: {bits}")
            }
            Self::InvalidScale => {
                f.write_str("Integer matrix scale must be finite and nonnegative")
            }
            Self::Parallel(message) => f.write_str(message),
            Self::Cancelled => f.write_str("合成任务已由更新的编辑替代。"),
        }
    }
}

impl std::error::Error for MatrixError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quantization {
    pub bits: u32,

    pub input_exponent: i32,
}

#[derive(Clone)]
pub struct DenseMatrix {
    rows: usize,
    columns: usize,
    values: Arc<[f32]>,
    quantization: Option<Quantization>,
    integer: Option<IntegerMatrix>,
}

#[derive(Clone)]
struct IntegerMatrix {
    // Input-major, each input's outputs padded with zeros to `stride`.
    weights: Arc<[i16]>,
    stride: usize,
    factors: Arc<[f32]>,
    input_scale: f32,
}

impl IntegerMatrix {
    fn frame(&self,input:&[f32],output:&mut[f32],accumulate:bool) {
        let mut sums=vec![0_i16;self.stride];
        for (&value,weights) in input.iter().zip(self.weights.chunks_exact(self.stride)) {
            let value=quantize_input(value,self.input_scale);
            for (sum,&weight) in sums.iter_mut().zip(weights) {
                *sum=sum.saturating_add(rounded_product(weight,value));
            }
        }
        self.scale(&sums,output,accumulate);
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn frame_avx2(&self,input:&[f32],output:&mut[f32],accumulate:bool) {
        use std::arch::x86_64::*;
        let mut sums=vec![0_i16;self.stride];
        for (&value,weights) in input.iter().zip(self.weights.chunks_exact(self.stride)) {
            let value=_mm256_set1_epi16(quantize_input(value,self.input_scale));
            for (sum,weights) in sums.chunks_exact_mut(16).zip(weights.chunks_exact(16)) {
                unsafe {
                    let product=_mm256_mulhrs_epi16(_mm256_loadu_si256(weights.as_ptr().cast()),value);
                    let total=_mm256_adds_epi16(_mm256_loadu_si256(sum.as_ptr().cast()),product);
                    _mm256_storeu_si256(sum.as_mut_ptr().cast(),total);
                }
            }
        }
        self.scale(&sums,output,accumulate);
    }

    #[inline(always)]
    fn scale(&self,sums:&[i16],output:&mut[f32],accumulate:bool) {
        for ((target,&sum),&factor) in output.iter_mut().zip(sums).zip(self.factors.iter()) {
            if accumulate {*target=(sum as f32).mul_add(factor,*target);}
            else {*target=(sum as f32)*factor;}
        }
    }
}

impl DenseMatrix {

    pub fn from_float(
        rows: usize,
        columns: usize,
        values: Arc<[f32]>,
    ) -> Result<Self, MatrixError> {
        check_length("float weights", elements(rows, columns)?, values.len())?;
        Ok(Self {
            rows,
            columns,
            values,
            quantization: None,
            integer: None,
        })
    }

    pub fn from_quantized(
        rows: usize,
        columns: usize,
        bits: u32,
        input_exponent: i32,
        scales: &[f32],
        values: &[u8],
    ) -> Result<Self, MatrixError> {
        Self::from_quantized_execution(rows,columns,bits,input_exponent,scales,values,false)
    }

    pub fn from_quantized_execution(
        rows:usize, columns:usize, bits:u32, input_exponent:i32,
        scales:&[f32], values:&[u8], integer_execution:bool,
    )->Result<Self,MatrixError> {
        let count = elements(rows, columns)?;
        let (bytes, divisor) = match bits {
            8 => (1, 1.0 / 128.0),
            16 => (2, 1.0 / 32768.0),
            _ => return Err(MatrixError::UnsupportedBits(bits)),
        };
        let expected = count.checked_mul(bytes).ok_or(MatrixError::InvalidShape)?;
        check_length("integer weights", expected, values.len())?;
        check_length("row scales", rows, scales.len())?;
        if scales
            .iter()
            .any(|scale| !scale.is_finite() || *scale < 0.0)
        {
            return Err(MatrixError::InvalidScale);
        }
        let mut decoded = vec![0.0; count];
        for (output, &scale) in scales.iter().enumerate() {
            let factor = scale * divisor;
            for input in 0..columns {
                let index = (input * rows + output) * bytes;
                let signed = if bits == 8 {
                    values[index] as i8 as f32
                } else {
                    i16::from_le_bytes([values[index], values[index + 1]]) as f32
                };
                decoded[output * columns + input] = signed * factor;
            }
        }

        let integer=if integer_execution && bits==16 {
            if !(-30..=30).contains(&input_exponent) {return Err(MatrixError::InvalidScale);}
            let input_scale=2.0_f32.powi(input_exponent);
            let stride=rows.next_multiple_of(16);
            let mut weights=vec![0_i16;columns.checked_mul(stride).ok_or(MatrixError::InvalidShape)?];
            for output in 0..rows {for input in 0..columns {
                let index=(input*rows+output)*bytes;
                weights[input*stride+output]=i16::from_le_bytes([values[index],values[index+1]]);
            }}
            Some(IntegerMatrix {weights:weights.into(),stride,input_scale,
                factors:scales.iter().map(|scale| scale/input_scale).collect()})
        } else {None};
        Ok(Self {
            rows,
            columns,
            values: decoded.into(),
            quantization: Some(Quantization {
                bits,
                input_exponent,
            }),
            integer,
        })
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn quantization(&self) -> Option<Quantization> {
        self.quantization
    }

    pub(super) fn has_integer_execution(&self)->bool {self.integer.is_some()}

    pub(super) fn integer_frame(&self,input:&[f32],output:&mut[f32],accumulate:bool)->bool {
        let Some(integer)=&self.integer else{return false};
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") {
            unsafe {integer.frame_avx2(input,output,accumulate);}
            return true;
        }
        integer.frame(input,output,accumulate);
        true
    }

    pub(super) fn row(&self, output: usize) -> &[f32] {
        &self.values[output * self.columns..(output + 1) * self.columns]
    }
}

#[inline(always)]
fn quantize_input(value:f32,scale:f32)->i16 {
    (value*scale).round_ties_even().clamp(-32768.0,32767.0) as i16
}

#[inline(always)]
fn rounded_product(a:i16,b:i16)->i16 {
    ((i32::from(a)*i32::from(b)+0x4000)>>15) as i16
}

pub(super) fn check_length(
    buffer: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), MatrixError> {
    if actual != expected {
        return Err(MatrixError::InvalidLength {
            buffer,
            expected,
            actual,
        });
    }
    Ok(())
}

fn elements(rows: usize, columns: usize) -> Result<usize, MatrixError> {
    if rows == 0 || columns == 0 {
        return Err(MatrixError::InvalidShape);
    }
    rows.checked_mul(columns).ok_or(MatrixError::InvalidShape)
}
