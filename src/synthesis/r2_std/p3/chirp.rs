// SPDX-License-Identifier: MIT
use super::super::{Error, Result, lf::fast_sin, transforms::{Complex,fft}};
const M: usize=1024;

pub(super) fn selects_direct(n: usize, k: usize) -> bool {
    let log_bits=|v:usize| (v as f32).to_bits() as f32*f32::from_bits(0x33b17218)
        -f32::from_bits(0x42affadd);
    log_bits(n)*f32::from_bits(0x3e8ccccd)>=log_bits(k)-f32::from_bits(0x4010a3d7)
}
fn transform(values: &mut [Complex], positive: bool) -> Result<()> {
    fft(values,positive)?;
    for value in values {
        value.re=f64::from(value.re as f32);value.im=f64::from(value.im as f32);
    }
    Ok(())
}
pub(super) fn evaluate(amplitude:&[f32], phase:&[f32], normalized:f32, n:usize) -> Result<Vec<f32>> {
    if !(257..=512).contains(&n) || amplitude.is_empty() || amplitude.len()>120
        || amplitude.len()!=phase.len() || amplitude.len()>=n || !normalized.is_finite() {
        return Err(Error::Unsupported("classic M1024 spectral profile"));
    }
    let w=(f64::from(normalized)*std::f64::consts::TAU) as f32;
    let scale=-w;
    let mut kernel=vec![Complex::default();M];kernel[0].re=1.0;
    for i in 1..n {
        let angle=(f64::from(scale)*i as f64*(i as f64*-0.5)) as f32;
        if !angle.is_finite() {return Err(Error::Format("classic chirp angle"));}
        let (sine,cosine)=angle.sin_cos();
        kernel[i]=Complex {re:f64::from(cosine),im:f64::from(-sine)};
        kernel[M-i]=kernel[i];
    }
    let mut transformed=kernel.clone();transform(&mut transformed,false)?;
    let mut input=vec![Complex::default();M];
    for i in 0..amplitude.len() {
        let theta=(f64::from(phase[i])-(i+1) as f64*(n/2) as f64*f64::from(w)) as f32;
        let real=(fast_sin(theta+f32::from_bits(0x3fc90fdb))*amplitude[i])*n as f32;
        let imaginary=(fast_sin(theta)*amplitude[i])*n as f32;
        let gr=kernel[i+1].re as f32;let gi=kernel[i+1].im as f32;
        input[i+1]=Complex {re:f64::from(gr*real+gi*imaginary),im:f64::from(gr*imaginary-gi*real)};
    }
    transform(&mut input,false)?;
    for (value,kernel) in input.iter_mut().zip(transformed) {
        let (ar,ai,br,bi)=(value.re as f32,value.im as f32,kernel.re as f32,kernel.im as f32);
        *value=Complex {re:f64::from(ar*br-ai*bi),im:f64::from(ai*br+ar*bi)};
    }
    transform(&mut input,true)?;
    let inverse=1.0/n as f32;
    let output:Vec<_>=(0..n).map(|i| {
        let real=(kernel[i].re as f32)*(input[i].re as f32);
        let imaginary=(kernel[i].im as f32)*(input[i].im as f32);
        (real+imaginary)*inverse
    }).collect();
    if output.iter().any(|v|!v.is_finite()) {return Err(Error::Format("classic chirp finite output"));}
    Ok(output)
}
