// SPDX-License-Identifier: MIT
use super::super::{Error, Result, frames::NoiseWave, lf::fast_sin};

pub(super) fn shape(source: &NoiseWave) -> Result<(f32,usize)> {
    let n=source.samples.len();let ratio=44100.0/source.rate;
    if n==0 || n>8192 || !source.rate.is_finite() || source.rate<=0.0
        || !source.offset.is_finite() || !source.gain.is_finite()
        || !ratio.is_finite() || ratio<=0.0 || ratio>2.0
        || source.samples.iter().any(|v|!v.is_finite()) {
        return Err(Error::Unsupported("embedded wave sample/rate profile"));
    }
    let count=if ratio==1.0 {n} else {(n as f32*ratio).round() as usize};
    if !(1..=8192).contains(&count) { return Err(Error::Unsupported("embedded wave output extent")); }
    Ok((ratio,count))
}
fn design(cutoff: f32) -> [f32;32] {
    let q=f64::from(cutoff*32.0);let lo=q.floor() as usize;let hi=q.ceil() as usize;
    let mut frequency=[0.0f32;32];frequency[..=lo].fill(1.0);
    frequency[hi]=(q-lo as f64) as f32;
    frequency[16]=frequency[15];for i in 17..32 {frequency[i]=frequency[32-i];}
    let mut work=[0.0f32;32];
    for i in 0..32 {
        let step=i as f32*f32::from_bits(0x3e490fdb);let mut lanes=[0.0f32;4];
        for k in 0..32 {lanes[k%4]+=(f64::from(k as f32*step).cos() as f32)*frequency[k];}
        work[i]=((lanes[2]+lanes[0])+(lanes[3]+lanes[1]))*0.03125;
    }
    std::array::from_fn(|i| {
        let cosine=(i as f64*std::f64::consts::TAU/31.0).cos();
        let window=(f64::from_bits(0x3fe147ae147ae148)-cosine*f64::from_bits(0x3fdd70a3d70a3d71)) as f32;
        window*work[(i+16)%32]
    })
}
fn filter(input: &[f32], taps: &[f32;32]) -> Vec<f32> {
    let mut padded=vec![0.0;input.len()+63];padded[31..31+input.len()].copy_from_slice(input);
    (0..input.len()+31).map(|i| {
        let mut a=[0.0f32;4];let mut b=[0.0f32;4];
        for k in (0..32).step_by(8) { for lane in 0..4 {
            a[lane]+=padded[i+31-k-lane]*taps[k+lane];
            b[lane]+=padded[i+27-k-lane]*taps[k+4+lane];
        }}
        for lane in 0..4 {a[lane]+=b[lane];}
        (a[2]+a[0])+(a[3]+a[1])
    }).collect()
}
fn lanczos(x: f32) -> f32 {
    if x==0.0 {return 1.0;}
    if x.abs()>4.0 {return 0.0;}
    let t=f64::from(x)*std::f64::consts::PI;let reciprocal=1.0/f64::from(x);
    let product=(fast_sin((t*0.25) as f32)*fast_sin(t as f32))*4.0;
    (f64::from(product)*f64::from_bits(0x3fb9f02f6222c721)*(reciprocal*reciprocal)) as f32
}
pub(super) fn convert(source: &NoiseWave) -> Result<Vec<f32>> {
    let (ratio,n)=shape(source)?;
    if ratio==1.0 {return Ok(source.samples.to_vec());}
    let filtered=if ratio<1.0 {Some(filter(&source.samples,&design(ratio*0.5)))} else {None};
    let values=filtered.as_deref().unwrap_or(&source.samples);let shift=if filtered.is_some(){16}else{0};
    let inverse=1.0/ratio;let mut output=vec![0.0f32;n];
    for (i,value) in output.iter_mut().enumerate() {
        let position=i as f32*inverse;let index=position.floor() as isize;
        let fraction=position-index as f32;
        for offset in -3..=4 {
            let at=index+offset;
            if at>0 && at<source.samples.len() as isize {
                *value+=lanczos(fraction-offset as f32)*values[at as usize+shift];
            }
        }
    }
    if output.iter().any(|v|!v.is_finite()) {return Err(Error::Format("embedded wave resample finite output"));}
    Ok(output)
}
