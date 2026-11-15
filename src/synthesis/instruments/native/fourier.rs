use super::{Result, fail, tensor::Tensor};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Arc;
use rayon::prelude::*;
pub fn forward_rows(rows: &[f32], width: usize) -> (Vec<f32>, Vec<f32>) {
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(width);
    let bins=width/2+1;
    let count=rows.len()/width;
    let mut re=vec![0.;count*bins];
    let mut im=vec![0.;count*bins];
    let scratch=|| (vec![Complex::new(0.,0.);width],vec![Complex::new(0.,0.);fft.get_inplace_scratch_len()]);
    let process=|(b,work):&mut (Vec<Complex<f64>>,Vec<Complex<f64>>),((re,im),row):((&mut [f32],&mut [f32]),&[f32])| {
        for (v,x) in b.iter_mut().zip(row) {*v=Complex::new(*x as f64,0.);}
        fft.process_with_scratch(b,work);
        for((re,im),v)in re.iter_mut().zip(im).zip(&b[..bins]){*re=v.re as f32;*im=v.im as f32;}
    };
    if count>=128 {
        re.par_chunks_exact_mut(bins).zip(im.par_chunks_exact_mut(bins)).zip(rows.par_chunks_exact(width))
            .for_each_init(scratch,process);
    } else {
        let mut state=scratch();
        for pair in re.chunks_exact_mut(bins).zip(im.chunks_exact_mut(bins)).zip(rows.chunks_exact(width)){process(&mut state,pair);}
    }
    (re, im)
}
pub fn inverse_rows(re: &[f32], im: &[f32], width: usize) -> Vec<f32> {
    let bins = width / 2 + 1;
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_inverse(width);
    let count=re.len()/bins;
    let mut out=vec![0.;count*width];
    let scratch=|| (vec![Complex::new(0.,0.);width],vec![Complex::new(0.,0.);fft.get_inplace_scratch_len()]);
    let process=|(b,work):&mut (Vec<Complex<f64>>,Vec<Complex<f64>>),(out,(r,i)):(&mut [f32],(&[f32],&[f32]))| {
        for j in 0..bins {b[j]=Complex::new(r[j] as f64,i[j] as f64);}
        for j in 1..(width+1)/2 {b[width-j]=b[j].conj();}
        fft.process_with_scratch(b,work);
        for(v,z)in out.iter_mut().zip(b){*v=(z.re/width as f64) as f32;}
    };
    if count>=128 {
        out.par_chunks_exact_mut(width).zip(re.par_chunks_exact(bins).zip(im.par_chunks_exact(bins)))
            .for_each_init(scratch,process);
    } else {
        let mut state=scratch();
        for pair in out.chunks_exact_mut(width).zip(re.chunks_exact(bins).zip(im.chunks_exact(bins))){process(&mut state,pair);}
    }
    out
}
pub fn rfft(x: &Tensor, exp: bool) -> Result<Tensor> {
    let width = *x.shape.last().ok_or("RFFT rank")?;
    let mut shape = x.shape.clone();
    *shape.last_mut().unwrap() = width / 2 + 1;
    let (mut re, mut im) = forward_rows(&x.dense(), width);
    if exp {
        for (r, i) in re.iter_mut().zip(&mut im) {
            let e = r.exp();
            let a = *i;
            *r = e * a.cos();
            *i = e * a.sin();
        }
    }
    let mut t = Tensor::new(shape, re);
    t.imag = Some(Arc::new(im));
    Ok(t)
}
pub fn stft(x: &Tensor, w: &Tensor, bins: usize, hop: usize) -> Result<Tensor> {
    let size = 2 * (bins - 1);
    let w = w.dense();
    if x.shape.len() != 3 || x.shape[2] != 1 || w.len() > size || hop == 0 || x.shape[1] < size {
        return fail("Instrument STFT layout");
    }
    let frames = (x.shape[1] - size) / hop + 1;
    let mut rows = vec![0.; x.shape[0] * frames * size];
    let pad = (size - w.len()) / 2;
    for b in 0..x.shape[0] {
        for t in 0..frames {
            for j in 0..w.len() {
                rows[(b * frames + t) * size + pad + j] =
                    x.get(b * x.shape[1] + t * hop + pad + j) * w[j];
            }
        }
    }
    let (re, im) = forward_rows(&rows, size);
    let mut out = Tensor::new(vec![x.shape[0], frames, bins], re);
    out.imag = Some(Arc::new(im));
    Ok(out)
}
pub fn istft(x: &Tensor, w: &Tensor, hop: usize) -> Result<Tensor> {
    if x.shape.len() != 3 || hop == 0 {
        return fail("Instrument ISTFT rank");
    }
    let size = 2 * (x.shape[2] - 1);
    let w = w.dense();
    if w.len() != size {
        return fail("Instrument ISTFT window");
    }
    let re = x.dense();
    let im: Vec<f32> = (0..x.len()).map(|i| x.complex_at(i).1).collect();
    let rows = inverse_rows(&re, &im, size);
    let frames = x.shape[1];
    let samples = (frames - 1) * hop + size;
    let mut out = vec![0.; x.shape[0] * (samples - size)];
    for b in 0..x.shape[0] {
        let mut y = vec![0.; samples];
        for t in 0..frames {
            for j in 0..size {
                y[t * hop + j] += rows[(b * frames + t) * size + j] * w[j];
            }
        }
        out[b * (samples - size)..(b + 1) * (samples - size)]
            .copy_from_slice(&y[size / 2..samples - size / 2]);
    }
    Ok(Tensor::new(vec![x.shape[0], samples - size, 1], out))
}
pub fn harmonics(x: &Tensor, rate: usize, hop: usize) -> Result<Tensor> {
    if x.shape.len() != 3 || x.shape[2] != 1 || rate == 0 || hop == 0 {
        return fail("Instrument harmonic source shape");
    }
    let n = 2 * hop;
    let frames = x.shape[1];
    let tau = std::f64::consts::TAU;
    let window: Vec<f32> = (0..n)
        .map(|j| ((1. - (j as f64 * tau / n as f64).cos()) * 0.5) as f32)
        .collect();
    let mut out = Vec::new();
    for b in 0..x.shape[0] {
        let mut y = vec![0.; frames * hop + n];
        let (mut phase, mut previous) = (0., 0.);
        let mut phases=Vec::with_capacity(frames+1);
        for t in 0..=frames {
            let frequency = x.get(b * frames + t.min(frames - 1));
            let current = frequency as f64;
            if previous <= 0. {
                phase = (tau / rate as f64 - tau * hop as f64 / rate as f64) * current;
                previous = current;
            }
            phase += (previous + current) * (std::f64::consts::PI * hop as f64 / rate as f64);
            previous = current;
            let denominator = ((rate as f64 * 0.5
                / (frequency.max((rate as f64 * 0.5 / (n - 1) as f64) as f32) as f64))
                as f32)
                .min((n - 1) as f32);
            let count = denominator as usize;
            let omega = tau * current / rate as f64;
            phases.push((phase,omega,denominator,count));
        }
        for (batch,phases) in phases.chunks(128).enumerate() {
            let rows:Vec<_>=phases.par_iter().map(|&(phase,omega,denominator,count)| {
                let scale=1./denominator;
                let mut row=vec![0f32;n];
                for k in 1..=count {for j in 0..n {
                    row[j]+=((k as f64*(phase-omega*hop as f64+omega*j as f64)).cos() as f32)*scale;
                }}
                row
            }).collect();
            for (i,row) in rows.iter().enumerate() {for j in 0..n {
                y[(batch*128+i)*hop+j]+=row[j]*window[j];
            }}
        }
        out.extend(&y[hop..hop + frames * hop]);
    }
    Ok(Tensor::new(vec![x.shape[0], frames * hop, 1], out))
}
