use super::{Result, fail};
use std::sync::Arc;
use rayon::prelude::*;
#[derive(Clone)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub strides: Vec<isize>,
    pub offset: isize,
    pub data: Arc<Vec<f32>>,
    pub imag: Option<Arc<Vec<f32>>>,
    contiguous: bool,
}
impl Tensor {
    pub fn new(shape: Vec<usize>, data: Vec<f32>) -> Self {
        assert_eq!(shape.iter().product::<usize>(), data.len());
        let mut strides = vec![1; shape.len()];
        for i in (0..shape.len().saturating_sub(1)).rev() {
            strides[i] = strides[i + 1] * shape[i + 1] as isize;
        }
        Self {
            shape,
            strides,
            offset: 0,
            data: Arc::new(data),
            imag: None,
            contiguous: true,
        }
    }
    pub fn len(&self) -> usize {
        self.shape.iter().product()
    }
    pub fn at(&self, coords: &[usize]) -> usize {
        (self.offset
            + coords
                .iter()
                .zip(&self.strides)
                .map(|(i, s)| *i as isize * s)
                .sum::<isize>()) as usize
    }
    pub fn index(&self, mut flat: usize) -> usize {
        if self.contiguous { return self.offset as usize+flat; }
        let mut a = self.offset;
        for i in (0..self.shape.len()).rev() {
            a += (flat % self.shape[i]) as isize * self.strides[i];
            flat /= self.shape[i];
        }
        a as usize
    }
    pub fn get(&self, flat: usize) -> f32 {
        self.data[self.index(flat)]
    }
    pub fn dense(&self) -> Vec<f32> {
        if self.contiguous {return self.data[self.offset as usize..self.offset as usize+self.len()].to_vec();}
        (0..self.len()).map(|i| self.get(i)).collect()
    }
    pub fn real_slice(&self) -> std::borrow::Cow<'_, [f32]> {
        if self.contiguous {
            std::borrow::Cow::Borrowed(&self.data[self.offset as usize..self.offset as usize+self.len()])
        } else {
            std::borrow::Cow::Owned(self.dense())
        }
    }
    pub fn imag_slice(&self)->Option<std::borrow::Cow<'_,[f32]>> {
        self.imag.as_ref().map(|data| {
            if self.contiguous {
                std::borrow::Cow::Borrowed(&data[self.offset as usize..self.offset as usize+self.len()])
            } else {
                std::borrow::Cow::Owned((0..self.len()).map(|i|data[self.index(i)]).collect())
            }
        })
    }
    pub fn finite(&self)->bool {
        if self.contiguous {
            let range=self.offset as usize..self.offset as usize+self.len();
            self.data[range.clone()].iter().all(|v|v.is_finite())&&self.imag.as_ref().is_none_or(|im|im[range].iter().all(|v|v.is_finite()))
        } else {(0..self.len()).all(|i|{let(r,j)=self.complex_at(i);r.is_finite()&&j.is_finite()})}
    }
    pub fn complex_at(&self, flat: usize) -> (f32, f32) {
        let i = self.index(flat);
        (self.data[i], self.imag.as_ref().map_or(0., |x| x[i]))
    }
    pub fn view(&self, shape: Vec<usize>, strides: Vec<isize>, offset: usize) -> Result<Self> {
        let a = self.offset + offset as isize;
        let lo = a + shape
            .iter()
            .zip(&strides)
            .map(|(n, s)| ((*n as isize - 1) * s).min(0))
            .sum::<isize>();
        let hi = a + shape
            .iter()
            .zip(&strides)
            .map(|(n, s)| ((*n as isize - 1) * s).max(0))
            .sum::<isize>();
        if shape.contains(&0) || lo < 0 || hi >= self.data.len() as isize {
            return fail("Instrument tensor view exceeds storage");
        }
        let mut expected=1;let mut contiguous=true;
        for(i,n)in shape.iter().enumerate().rev(){if *n>1&&strides[i]!=expected{contiguous=false;}expected*=*n as isize;}
        Ok(Self {
            shape,
            strides,
            offset: a,
            data: self.data.clone(),
            imag: self.imag.clone(),
            contiguous,
        })
    }
    pub fn map(&self, f: impl Fn(f32) -> f32) -> Self {
        Self::new(
            self.shape.clone(),
            (0..self.len()).map(|i| f(self.get(i))).collect(),
        )
    }
    pub fn coords(shape: &[usize], mut i: usize) -> Vec<usize> {
        let mut c = vec![0; shape.len()];
        for j in (0..shape.len()).rev() {
            c[j] = i % shape[j];
            i /= shape[j];
        }
        c
    }
    pub fn broadcast(&self, c: &[usize]) -> f32 {
        let off = c.len() - self.shape.len();
        let mut a = self.offset;
        for i in 0..self.shape.len() {
            if self.shape[i] != 1 {
                a += c[off + i] as isize * self.strides[i];
            }
        }
        self.data[a as usize]
    }
    pub fn broadcast_complex(&self, c: &[usize]) -> (f32, f32) {
        let off = c.len() - self.shape.len();
        let mut a = self.offset;
        for i in 0..self.shape.len() {
            if self.shape[i] != 1 {
                a += c[off + i] as isize * self.strides[i];
            }
        }
        (
            self.data[a as usize],
            self.imag.as_ref().map_or(0., |v| v[a as usize]),
        )
    }
    pub fn complex_binary(a: &Self, b: &Self, op: u16) -> Result<Self> {
        let shape = Self::broadcast_shape(&[a, b])?;
        let mut re = Vec::new();
        let mut im = Vec::new();
        for i in 0..shape.iter().product() {
            let c = Self::coords(&shape, i);
            let (ar, ai) = a.broadcast_complex(&c);
            let (br, bi) = b.broadcast_complex(&c);
            let (r, j) = match op {
                0x300 => (ar + br, ai + bi),
                0x301 => (ar - br, ai - bi),
                0x302 => (ar * br - ai * bi, ar * bi + ai * br),
                0x303 => {
                    let d = br * br + bi * bi;
                    ((ar * br + ai * bi) / d, (ai * br - ar * bi) / d)
                }
                _ => return fail("Unsupported complex instrument operation"),
            };
            re.push(r);
            im.push(j);
        }
        let mut out = Self::new(shape, re);
        out.imag = Some(Arc::new(im));
        Ok(out)
    }
    pub fn broadcast_shape(xs: &[&Self]) -> Result<Vec<usize>> {
        let rank = xs.iter().map(|x| x.shape.len()).max().unwrap_or(0);
        let mut shape = vec![1; rank];
        for x in xs {
            let off = rank - x.shape.len();
            for (i, n) in x.shape.iter().enumerate() {
                let s = &mut shape[off + i];
                if *s != 1 && *n != 1 && *s != *n {
                    return fail("Instrument tensor broadcasting");
                }
                *s = (*s).max(*n);
            }
        }
        Ok(shape)
    }
    pub fn binary(a: &Self, b: &Self, f: impl Fn(f32, f32) -> f32) -> Result<Self> {
        let shape = Self::broadcast_shape(&[a, b])?;
        if a.shape==b.shape {return Ok(Self::new(shape,(0..a.len()).map(|i|f(a.get(i),b.get(i))).collect()));}
        if a.len()==1&&shape==b.shape {let s=a.get(0);return Ok(b.map(|v|f(s,v)));}
        if b.len()==1&&shape==a.shape {let s=b.get(0);return Ok(a.map(|v|f(v,s)));}
        let data = (0..shape.iter().product())
            .map(|i| {
                let c = Self::coords(&shape, i);
                f(a.broadcast(&c), b.broadcast(&c))
            })
            .collect();
        Ok(Self::new(shape, data))
    }
    pub fn concat(xs: &[&Self], axis: usize) -> Result<Self> {
        if xs.is_empty() || axis >= xs[0].shape.len() {
            return fail("Instrument tensor concatenation axis");
        }
        let rank = xs[0].shape.len();
        if xs.iter().any(|x| x.shape.len() != rank) {
            return fail("Instrument concat rank");
        }
        let mut shape = vec![1; rank];
        for x in xs {
            for i in 0..rank {
                if i == axis {
                    continue;
                }
                if shape[i] != 1 && x.shape[i] != 1 && shape[i] != x.shape[i] {
                    return fail("Instrument concat broadcast");
                }
                shape[i] = shape[i].max(x.shape[i]);
            }
        }
        shape[axis] = xs.iter().map(|x| x.shape[axis]).sum();
        if axis+1==rank&&xs.iter().all(|x|x.contiguous&&x.shape[..axis]==shape[..axis]) {
            let rows:usize=shape[..axis].iter().product();let mut data=Vec::with_capacity(shape.iter().product());
            for row in 0..rows {for x in xs {let width=x.shape[axis];let at=x.offset as usize+row*width;data.extend_from_slice(&x.data[at..at+width]);}}
            return Ok(Self::new(shape,data));
        }
        let data = (0..shape.iter().product())
            .map(|i| {
                let mut c = Self::coords(&shape, i);
                let mut p = c[axis];
                for x in xs {
                    if p < x.shape[axis] {
                        c[axis] = p;
                        return x.broadcast(&c);
                    }
                    p -= x.shape[axis];
                }
                unreachable!()
            })
            .collect();
        Ok(Self::new(shape, data))
    }
}
pub fn affine(
    x: &[f32],
    rows: usize,
    input: usize,
    w: &[f32],
    output: usize,
    bias: Option<&[f32]>,
) -> Vec<f32> {
    assert_eq!(x.len(), rows * input);
    assert_eq!(w.len(), output * input);
    let mut y = vec![0.; rows * output];
    let run=|(block,part):(usize,&mut [f32])| unsafe {
        matrixmultiply::sgemm(
            part.len()/output,
            input,
            output,
            1.,
            x[block*64*input..].as_ptr(),
            input as isize,
            1,
            w.as_ptr(),
            1,
            input as isize,
            0.,
            part.as_mut_ptr(),
            output as isize,
            1,
        );
    };
    if rows>=128 && rows.saturating_mul(input).saturating_mul(output)>=1_000_000 {
        y.par_chunks_mut(64*output).enumerate().for_each(run);
    } else {
        run((0,&mut y));
    }
    if let Some(b) = bias {
        assert_eq!(b.len(), output);
        for row in y.chunks_mut(output) {
            for (v, b) in row.iter_mut().zip(b) {
                *v += b;
            }
        }
    }
    y
}
