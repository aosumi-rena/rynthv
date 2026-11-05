// SPDX-License-Identifier: MIT
use super::{Error, Result, lf::Pulse};

pub(super) fn remap(values: &[f32], old: f32, next: f32, nyquist: f32) -> Result<Vec<f32>> {
    let endpoint = old.max(next) + 500.0;
    let source = [0.0, old, endpoint, nyquist];
    let destination = [0.0, next, endpoint, nyquist];
    let n = values.len();
    let nf = n as f32;
    let inv = 1.0 / nyquist;
    let invn = 1.0 / nf;
    let mut result = vec![0.0; n];
    let mut last = 0;
    for j in 0..3 {
        let a = destination[j];
        let span = destination[j + 1] - a;
        let start = (source[j] * inv) * nf;
        let width = ((source[j + 1] - source[j]) * inv) * nf;
        if span <= 0.0 || width <= 0.0 {
            return Err(Error::Format("release remap positive spans"));
        }
        let lo = bin((a * inv) * nf)?.min(n);
        let hi = bin((destination[j + 1] * inv) * nf)?.min(n);
        let top = (((nf - start - 1.0) / width) * span + a) * inv;
        last = hi.min(bin(top * nf)?);
        for i in lo..last {
            let position = ((((i as f32 * nyquist) * invn - a).max(0.0) / span) * width) + start;
            let k = bin(position)?;
            if k + 1 >= n {
                return Err(Error::Format("release remap source index"));
            }
            result[i] = (values[k + 1] - values[k]) * (position - k as f32) + values[k];
        }
    }
    if last == 0 || last > n {
        return Err(Error::Format("release remap tail"));
    }
    let tail = result[last - 1];
    result[last..].fill(tail);
    Ok(result)
}
fn bin(value: f32) -> Result<usize> {
    if !value.is_finite() || value < 0.0 || value >= 2147483648.0 {
        return Err(Error::Format("release coordinate conversion"));
    }
    Ok(value as usize)
}
fn dense(start: f32, end: f32, values: &[f32], query: f32) -> f32 {
    if query < start {
        return values[0];
    }
    let position = ((query - start) * (1.0 / (end - start))) * values.len() as f32;
    if f64::from(position) + f64::from_bits(0x3ff028f5c28f5c29) >= values.len() as f64 {
        return values[values.len() - 1];
    }
    let index = (position as usize).min(values.len() - 2);
    let fraction = position - index as f32;
    (values[index + 1] - values[index]) * fraction + values[index]
}
pub(super) fn formant(values: &[f32], pitch: f32, factor: f32, nyquist: f32) -> Result<Vec<f32>> {
    let n = values.len();
    let grid: Vec<_> = (0..n).map(|i| i as f32 * nyquist / n as f32).collect();
    let last = grid[n - 1];
    let knot = (factor * 4000.0).clamp(100.0, 7000.0);
    let mut mapped = vec![0.0];
    let x = [0.0, pitch, knot, 8000.0, last];
    let y = [0.0, pitch, 4000.0, 8000.0, last];
    let mut j = 0;
    for &q in &grid[1..] {
        while j < 3 && q > x[j + 1] {
            j += 1;
        }
        let value = if q > x[4] {
            y[4]
        } else {
            ((y[j + 1] - y[j]) * (q - x[j])) / (x[j + 1] - x[j]) + y[j]
        };
        if !value.is_finite() {
            return Err(Error::Format("formant interpolation finite output"));
        }
        mapped.push(value);
    }
    let end = last + last - grid[n - 2];
    Ok(mapped
        .into_iter()
        .map(|q| dense(grid[0], end, values, q))
        .collect())
}
pub(super) fn envelope(rd: f32, pitch: f32, count: usize, nyquist: f32) -> Result<Vec<f32>> {
    let extent = (1.0 / pitch) * nyquist;
    if !extent.is_finite() || !(2.0..1202.0).contains(&extent) {
        return Err(Error::Unsupported("release LF harmonic count2..1201"));
    }
    let n = extent as usize;
    let pulse = Pulse::new(rd, pitch)?;
    let top = n as f32 * pitch;
    let first = pulse.at(pitch)?.0;
    let mut magnitude = vec![0.0; n];
    for (i, value) in magnitude.iter_mut().enumerate().skip(1) {
        let hz = i as f32 * (top - pitch) / (n - 1) as f32 + pitch;
        let ratio = pulse.at(hz)?.0 / ((i + 1) as f32 * first);
        *value = (f64::from(ratio).ln() * f64::from_bits(0x40215f2cec554b28)) as f32;
    }
    Ok((0..count)
        .map(|i| {
            dense(
                pitch,
                (n + 1) as f32 * pitch,
                &magnitude,
                i as f32 * nyquist / count as f32,
            )
        })
        .collect())
}

pub(super) fn equalize(values: &mut [f32], points: &[f64], nyquist: f32) {
    let n = values.len();
    let bin = |f: f64| {
        (((1.0 / f64::from(nyquist)) * f) * n as f64)
            .trunc()
            .clamp(0.0, n as f64) as usize
    };
    for p in (0..points.len()).step_by(2) {
        let begin = bin(points[p]);
        let end = if p + 2 < points.len() {
            bin(points[p + 2])
        } else {
            n
        };
        if end <= begin {
            continue;
        }
        let low = points[p + 1];
        let high = if p + 3 < points.len() {
            points[p + 3]
        } else {
            low
        };
        for i in begin..end {
            let fraction = (i - begin) as f32 / (end - begin) as f32;
            values[i] = (f64::from(fraction) * (high - low) + low + f64::from(values[i])) as f32;
        }
    }
}
