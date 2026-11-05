// SPDX-License-Identifier: MIT
use super::{Error, Result, StdBank, archive::Reader, transforms::dct_positive};
pub(super) struct PsdResidual {
    lower: Vec<f32>,
    range: Vec<f32>,
    offset: Vec<f32>,
    first: Dense,
    second: Dense,
    floor: f32,
    ceiling: f32,
}
struct Dense {
    rows: usize,
    columns: usize,
    weights: Vec<f32>,
    bias: Vec<f32>,
}
impl PsdResidual {
    pub fn open(bank: &StdBank) -> Result<Self> {
        let name = bank.configuration.single_text("res_generator")?;
        let scalar = |name: &str| -> Result<f32> {
            let x = bank.configuration.numbers(name)?;
            if x.len() != 1 || !x[0].is_finite() || !(x[0] as f32).is_finite() {
                Err(Error::Format("residual clamp configuration"))
            } else {
                Ok(x[0] as f32)
            }
        };
        let ceiling = scalar("res_db_ceil")?.max(0.0);
        let floor = scalar("res_db_floor")?.min(0.0);
        let mut r = Reader::new(bank.archive.named(name.as_bytes())?);
        if shape(&mut r)? != 256 {
            return Err(Error::Unsupported("PSD residual dimension256"));
        }
        let lower = vector(&mut r, 256)?;
        let upper = vector(&mut r, 256)?;
        let offset = vector(&mut r, 256)?;
        let range: Vec<_> = upper.iter().zip(&lower).map(|(a, b)| a - b).collect();
        if range.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(Error::Format("PSD residual normalization range"));
        }
        let first = Dense::read(&mut r, 128, 256)?;
        let second = Dense::read(&mut r, 768, 128)?;
        r.end()?;
        Ok(Self {
            lower,
            range,
            offset,
            first,
            second,
            floor,
            ceiling,
        })
    }
    pub fn sample(&self, psd: &[f32; 256], state: &mut u32) -> Result<[f32; 256]> {
        if psd.iter().any(|x| !x.is_finite()) {
            return Err(Error::Format("residual PSD input"));
        }
        let normalized: Vec<_> = (0..256)
            .map(|i| ((psd[i] - self.lower[i]) / self.range[i] - 0.5).clamp(-0.5, 0.5))
            .collect();
        let hidden: Vec<_> = self
            .first
            .run(&normalized)?
            .into_iter()
            .map(|x| if x > 0.0 { x } else { 0.0 })
            .collect();
        let raw = self.second.run(&hidden)?;
        let mut spectral = vec![0.0; 256];
        for i in (0..256).step_by(2) {
            let mut next = || {
                *state = state.wrapping_mul(214013).wrapping_add(2531011);
                ((*state >> 16) & 32767) as f32
            };
            let (a, b) = gaussian(next(), next());
            spectral[i] = fast_exp(raw[256 + i]) * a + raw[i];
            spectral[i + 1] = fast_exp(raw[256 + i + 1]) * b + raw[i + 1];
        }
        spectral[0] = (f64::from(spectral[0]) * f64::from_bits(0x3fe6a09e65dc27df)) as f32;
        let transformed = dct_positive(&spectral)?;
        let scale = (2.0f64 / 256.0).sqrt() as f32;
        let result = std::array::from_fn(|i| {
            let time = fast_exp(0.1 * raw[512 + i]) * (scale * transformed[i]) + self.offset[i];
            (self.range[i] * time).max(self.floor).min(self.ceiling)
        });
        if result.iter().any(|x| !x.is_finite()) {
            return Err(Error::Format("PSD residual output"));
        }
        Ok(result)
    }
}
impl Dense {
    fn read(r: &mut Reader<'_>, rows: usize, columns: usize) -> Result<Self> {
        if shape(r)? != rows || shape(r)? != columns || shape(r)? != rows {
            return Err(Error::Unsupported("residual affine geometry"));
        }
        Ok(Self {
            rows,
            columns,
            weights: vector(r, rows * columns)?,
            bias: vector(r, rows)?,
        })
    }
    fn run(&self, input: &[f32]) -> Result<Vec<f32>> {
        let mut result = vec![0.0; self.rows];
        for row in 0..self.rows {
            let mut total = 0.0;
            for block in (0..self.columns).step_by(16) {
                let mut partial = 0.0;
                for col in block..block + 16 {
                    partial += self.weights[col * self.rows + row] * input[col];
                }
                total = partial + total;
            }
            result[row] = total + self.bias[row];
        }
        if result.iter().any(|x| !x.is_finite()) {
            return Err(Error::Format("residual affine result"));
        }
        Ok(result)
    }
}
fn shape(r: &mut Reader<'_>) -> Result<usize> {
    let x = r.f32()?;
    if x.is_finite() && (0.0..2147483648.0).contains(&x) && (x as usize) as f32 == x {
        Ok(x as usize)
    } else {
        Err(Error::Format("residual model shape"))
    }
}
fn vector(r: &mut Reader<'_>, count: usize) -> Result<Vec<f32>> {
    if count > r.remaining() / 4 {
        return Err(Error::Format("truncated residual model"));
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let x = r.f32()?;
        if !x.is_finite() {
            return Err(Error::Format("nonfinite residual weights"));
        }
        out.push(x);
    }
    Ok(out)
}
fn fast_exp(value: f32) -> f32 {
    let t = (value * f32::from_bits(0x3f38aa3b)).max(-126.0);
    let t = t * 8388608.0 + f32::from_bits(0x4e7de2a9);
    f32::from_bits((t as i64) as u32)
}
fn gaussian(r1: f32, r2: f32) -> (f32, f32) {
    let u1 = r1 * f32::from_bits(0x38000100) + f32::from_bits(0x322bcc77);
    let u2 = r2 * f32::from_bits(0x38000100);
    let angle = (f64::from(u2) * std::f64::consts::TAU - std::f64::consts::PI) as f32;
    let absolute = angle.abs();
    let c1 = 1.0 - absolute * f32::from_bits(0x3f22f983);
    let c2 = c1 * c1;
    let cosine = ((1.0 - c2) * c1) * f32::from_bits(0x3f0be1bf) + c1;
    let q = angle * f32::from_bits(0x3fa2f983) - (angle * f32::from_bits(0x3ecf817b)) * absolute;
    let coefficient = f32::from_bits((angle.to_bits() & 0x80000000) | 0x3e64706c);
    let sine = (q * coefficient + f32::from_bits(0x3f46bd94)) * q;
    let logarithm = (u1.to_bits() as f32) * f32::from_bits(0x33b17218) - f32::from_bits(0x42affadd);
    let radius = (f64::from(logarithm) * -2.0).max(0.0).sqrt() as f32;
    (cosine * radius, sine * radius)
}
