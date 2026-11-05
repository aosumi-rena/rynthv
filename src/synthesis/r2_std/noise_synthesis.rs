// SPDX-License-Identifier: MIT
use super::{
    DecodedFrame, Error, Result,
    spectrum::uniform,
    transforms::{Complex, fft},
};
pub(super) fn shape(
    frame: &DecodedFrame, wave: &[f32], window: &[f32], nyquist: f32,
) -> Result<Vec<f32>> {
        const N: usize = 1024;
        if wave.len()!=window.len() || wave.len()<2 || wave.len()>N {
            return Err(Error::Unsupported("native noise window"));
        }
        let count=wave.len();
        let start=N/2-count/2;
        let energy: f32=window.iter().map(|x|x*x).sum();
        let mut bins = vec![Complex::default(); N];
        for i in 0..count {
            bins[start + i].re = f64::from(wave[i] * window[i]);
        }
        if frame
            .noise_psd_db
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
            < -100.0
        {
            return Ok(vec![0.0; N]);
        }
        fft(&mut bins, false)?;
        let measured: Vec<f32> = bins[..=N / 2]
            .iter()
            .map(|v| ((v.re * v.re + v.im * v.im) / f64::from(energy)) as f32)
            .collect();
        let mut acc = Vec::with_capacity(519);
        let mut cumulative = measured[0];
        acc.push(cumulative);
        for _ in 0..3 {
            cumulative += measured[0];
            acc.push(cumulative);
        }
        for value in &measured[1..] {
            cumulative += value;
            acc.push(cumulative);
        }
        for _ in 0..3 {
            cumulative += measured[512];
            acc.push(cumulative);
        }
        for i in 0..512 {
            let envelope = (acc[i + 6] - acc[i]) * (1.0 / 6.0);
            let hz = i as f32 * (22050.0 / 512.0);
            let target = uniform(&frame.noise_psd_db, nyquist, hz);
            let gain = (f64::from(target) * f64::from_bits(0x3fbd791c6109b02d)).exp()
                / (f64::from(envelope) + 1e-8).sqrt();
            bins[i].re *= gain;
            bins[i].im *= gain;
        }
        bins[512] = bins[511];
        for i in 513..N {
            bins[i] = Complex {
                re: bins[N - i].re,
                im: -bins[N - i].im,
            };
        }
        fft(&mut bins, true)?;
        let mut out: Vec<f32> = bins.iter().map(|v| v.re as f32).collect();
        for i in 0..16 {
            let ratio = i as f32 * (1.0 / 16.0);
            out[i] *= ratio;
            out[1023 - i] *= 1.0 - ratio;
        }
        super::directional_model::finite(&out)?;
        Ok(out)
    }
