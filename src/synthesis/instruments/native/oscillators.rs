use super::{Result, fail, tensor::Tensor};
use rayon::prelude::*;

pub(super) fn remainder(value: f64, modulus: f64) -> f64 {
    let r = value % modulus;
    let half = modulus * 0.5;
    if r > half || (r == half && (value / modulus).trunc() as i64 & 1 != 0) { r - modulus }
    else if r < -half || (r == -half && (value / modulus).trunc() as i64 & 1 != 0) { r + modulus }
    else { r }
}

pub(super) fn harmonics(pitch: &Tensor, phases: &Tensor, rate: usize, hop: usize) -> Result<Tensor> {
    if pitch.shape.len() != 3 || pitch.shape[2] != 1 || phases.shape != pitch.shape
        || rate == 0 || hop == 0 || pitch.shape[1] == 0 {
        return fail("Instrument explicit harmonic source shape");
    }
    let (frames, size) = (pitch.shape[1], hop * 2);
    let tau = std::f64::consts::TAU;
    let window: Vec<_> = (0..size).map(|j| ((1. - (j as f64 * tau / size as f64).cos()) * 0.5) as f32).collect();
    let mut output = Vec::with_capacity(pitch.shape[0] * frames * hop);
    for batch in 0..pitch.shape[0] {
        let mut samples = vec![0.; (frames + 2) * hop];
        for begin in (0..=frames).step_by(128) {
        let rows: Vec<_> = (begin..(begin + 128).min(frames + 1)).into_par_iter().map(|frame| {
            let source = batch * frames + frame.min(frames - 1);
            let f = f64::from(pitch.get(source));
            let mut phase = f64::from(phases.get(source));
            if frame == frames { phase += f * tau * hop as f64 / rate as f64; }
            let denominator = ((rate as f64 * 0.5 / f.max(rate as f64 * 0.5 / (size - 1) as f64)) as f32).min((size - 1) as f32);
            let scale = 1. / denominator;
            let omega = tau * f / rate as f64;
            let mut row = vec![0.; size];
            for harmonic in 1..=denominator as usize {
                for (j, sample) in row.iter_mut().enumerate() {
                    *sample += ((harmonic as f64 * (phase - omega * hop as f64 + omega * j as f64)).cos() as f32) * scale;
                }
            }
            row
        }).collect();
        for (local, row) in rows.iter().enumerate() {
            let t = begin + local;
            for j in 0..size { samples[t * hop + j] += row[j] * window[j]; }
        }
        }
        output.extend_from_slice(&samples[hop..(frames + 1) * hop]);
    }
    Ok(Tensor::new(vec![pitch.shape[0], frames * hop, 1], output))
}

pub(super) fn bank(frequency: &Tensor, amplitude: &Tensor, phase: &Tensor,
    rate: usize, hop: usize, linear: bool) -> Result<Tensor> {
    if frequency.shape.len() != 3 || amplitude.shape != frequency.shape || phase.shape != frequency.shape
        || frequency.shape[1] == 0 || rate == 0 || hop == 0 {
        return fail("Instrument sinusoidal source shape");
    }
    let (batch, frames, channels) = (frequency.shape[0], frequency.shape[1], frequency.shape[2]);
    let size = 2 * hop;
    let tau = std::f64::consts::TAU;
    let window: Vec<_> = (0..size).map(|j| if linear {
        1. - (j as f32 - hop as f32).abs() / hop as f32
    } else {
        (1. - (j as f32 * std::f32::consts::TAU / size as f32).cos()) * 0.5
    }).collect();
    let mut output = Vec::with_capacity(batch * (frames - 1) * hop);
    for b in 0..batch {
        let mut samples = vec![0.; (frames + 1) * hop];
        for begin in (0..frames).step_by(128) {
        let rows: Vec<_> = (begin..(begin + 128).min(frames)).into_par_iter().map(|t| {
            let mut row = vec![0f32; size];
            for c in 0..channels {
                let i = (b * frames + t) * channels + c;
                let amplitude = amplitude.get(i);
                if amplitude == 0. { continue; }
                let f = f64::from(frequency.get(i));
                let phase = f64::from(phase.get(i));
                for (j, sample) in row.iter_mut().enumerate() {
                    let angle = (((j as f64 - hop as f64) * 2.) * std::f64::consts::PI * f) / rate as f64 + phase;
                    let angle = remainder(angle, tau) as f32;
                    *sample += angle.cos() * amplitude;
                }
            }
            row
        }).collect();
        for (local, row) in rows.iter().enumerate() {
            let t = begin + local;
            for j in 0..size { samples[t * hop + j] += row[j] * window[j]; }
        }
        }
        output.extend_from_slice(&samples[hop..frames * hop]);
    }
    Ok(Tensor::new(vec![batch, (frames - 1) * hop, 1], output))
}
