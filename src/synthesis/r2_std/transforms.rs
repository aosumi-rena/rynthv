// SPDX-License-Identifier: MIT
use super::{Error, Result};
mod plan;

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub(super) struct Complex {
    pub re: f64,
    pub im: f64,
}
impl Complex {
    fn mul(self, b: Self) -> Self {
        Self {
            re: self.re * b.re - self.im * b.im,
            im: self.re * b.im + self.im * b.re,
        }
    }
}
pub(super) fn fft(values: &mut [Complex], inverse: bool) -> Result<()> {
    let n = values.len();
    if n == 0 || !n.is_power_of_two() || n > 65536 {
        return Err(Error::Unsupported("Fourier power-of-two extent"));
    }
    let plan=plan::get(n,inverse);
    for &(i,j) in &plan.swaps {values.swap(i,j);}
    for twiddle in &plan.stages {
        for chunk in values.chunks_exact_mut(twiddle.len()*2) {plan::butterfly(chunk,twiddle);}
    }
    if inverse {
        for value in &mut *values {
            value.re /= n as f64;
            value.im /= n as f64;
        }
    }
    if values
        .iter()
        .any(|v| !v.re.is_finite() || !v.im.is_finite())
    {
        return Err(Error::Format("nonfinite Fourier output"));
    }
    Ok(())
}
pub(super) fn dct_negative(input: &[f32]) -> Result<Vec<f32>> {
    let n = input.len();
    if !matches!(n, 64 | 256 | 1024) || input.iter().any(|x| !x.is_finite()) {
        return Err(Error::Unsupported("STD DCT input"));
    }
    let mut work = vec![Complex::default(); 2 * n];
    for (i, &value) in input.iter().enumerate() {
        work[i].re = f64::from(value);
        work[2 * n - 1 - i].re = f64::from(value);
    }
    fft(&mut work, false)?;
    Ok((0..n)
        .map(|k| {
            let angle = std::f64::consts::PI * (k as f64) / (2 * n) as f64;
            ((work[k].re * angle.cos() + work[k].im * angle.sin()) * 0.5) as f32
        })
        .collect())
}
pub(super) fn dct_positive(input: &[f32]) -> Result<Vec<f32>> {
    let n = input.len();
    if !matches!(n, 64 | 256 | 1024) || input.iter().any(|x| !x.is_finite()) {
        return Err(Error::Unsupported("STD DCT input"));
    }
    let mut work = vec![Complex::default(); 2 * n];
    work[0].re = 4.0 * f64::from(input[0]);
    for k in 1..n {
        let angle = std::f64::consts::PI * (k as f64) / (2 * n) as f64;
        work[k] = Complex {
            re: 2.0 * f64::from(input[k]) * angle.cos(),
            im: 2.0 * f64::from(input[k]) * angle.sin(),
        };
        work[2 * n - k] = Complex {
            re: work[k].re,
            im: -work[k].im,
        };
    }
    fft(&mut work, true)?;
    Ok(work[..n]
        .iter()
        .map(|v| (v.re * (n as f64) * 0.5) as f32)
        .collect())
}
