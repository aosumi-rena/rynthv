// SPDX-License-Identifier: MIT
use super::{
    DecodedFrame, Error, Result,
    lf::Pulse,
    transforms::{Complex, fft},
};
pub(super) struct Harmonics {
    pub amplitude: Vec<f32>,
    pub phase: Vec<f32>,
}
pub(super) fn harmonics(frame: &DecodedFrame, f0: f32, nyquist: f32) -> Result<Harmonics> {
    if f0 == 0.0 || frame.f0_hz == 0.0 {
        return Ok(Harmonics {
            amplitude: Vec::new(),
            phase: Vec::new(),
        });
    }
    if !f0.is_finite() || f0 <= 0.0 || frame.vocal_tract_db.len() != 1025 {
        return Err(Error::Format("harmonic physical input"));
    }
    let count = ((nyquist / f0) as usize)
        .min(120)
        .min(frame.harmonic_phase.len());
    if count == 0 {
        return Err(Error::Unsupported("no harmonics below Nyquist"));
    }
    let pulse = Pulse::new(frame.rd, f0)?;
    let fundamental = pulse.magnitude(f0)?;
    if !fundamental.is_finite() || fundamental <= 0.0 {
        return Err(Error::Format("LF fundamental magnitude"));
    }
    let mut vt = Vec::with_capacity(count);
    let mut magnitude = Vec::with_capacity(count);
    for i in 0..count {
        let hz = ((i + 1) as f32) * f0;
        vt.push(
            (f64::from(uniform(&frame.vocal_tract_db, nyquist, hz))
                * f64::from_bits(0x3fbd791c6109b02d))
            .exp() as f32,
        );
        magnitude.push(if i == 0 {
            1.0
        } else {
            (f64::from(pulse.magnitude(hz)?) / ((i + 1) as f64 * f64::from(fundamental))) as f32
        });
    }
    let minimum = minimum_phase(&vt)?;
    let mut amplitude = Vec::with_capacity(count);
    let mut phase = Vec::with_capacity(count);
    let scale =
        (f64::from(1.5 * f32::from_bits(0x3cda740e)) * f64::from_bits(0x3f4ead761ac9763b)) as f32;
    let c = f32::from_bits(0x3fb87318);
    let c2 = f32::from_bits(0x4004e5a6);
    for i in 0..count {
        let omega = (((i + 1) as f64) * f64::from(f0) * std::f64::consts::TAU) as f32;
        let w = omega * scale;
        let inverse = 1.0 / (w * w + c2);
        let v = w * c;
        let x = (w * v) * inverse;
        let y = (v * c) * inverse;
        amplitude.push((y * y + x * x).sqrt() * (vt[i] * magnitude[i]));
        phase.push(y.atan2(x) + (frame.harmonic_phase[i] + minimum[i]));
    }
    if amplitude.iter().chain(&phase).any(|x| !x.is_finite()) {
        return Err(Error::Format("nonfinite harmonic spectrum"));
    }
    Ok(Harmonics { amplitude, phase })
}
pub(super) fn dense(values: &[f32], top: f32, q: f32) -> f32 {
    let position = q / top * (values.len() as f32);
    let index = (position as usize).min(values.len() - 1);
    if position <= 0.0 {
        return values[0];
    }
    if index + 1 >= values.len() {
        return values[index];
    }
    (values[index + 1] - values[index]) * (position - index as f32) + values[index]
}
pub(super) fn uniform(values: &[f32], top: f32, q: f32) -> f32 {
    dense(
        values,
        top * (values.len() as f32) / ((values.len() - 1) as f32),
        q,
    )
}
fn minimum_phase(magnitude: &[f32]) -> Result<Vec<f32>> {
    let count = magnitude.len();
    let n = (count * 4).next_power_of_two().max(64);
    let half = n / 2;
    let positions: Vec<_> = (0..=count)
        .map(|i| (i as f32) * (n as f32) * 0.5 / ((count + 1) as f32))
        .collect();
    let mut log: Vec<_> = magnitude
        .iter()
        .map(|&v| (f64::from(v) + f64::from_bits(0x3ddb7cdfd9d7bdbb)).ln() as f32)
        .collect();
    log.insert(0, log[0]);
    let end = positions[count] + positions[count] - positions[count - 1];
    let mut full = vec![Complex::default(); n];
    for i in 0..=half {
        full[i].re = f64::from(dense(&log, end, i as f32));
    }
    full[half] = full[half - 1];
    for i in half + 1..n {
        full[i] = full[n - i];
    }
    fft(&mut full, true)?;
    for value in &mut full[1..=half] {
        value.re += value.re;
    }
    for value in &mut full[half + 2..] {
        value.re = 0.0;
    }
    fft(&mut full, false)?;
    let phases: Vec<_> = full[..=half].iter().map(|v| v.im as f32).collect();
    let mut result: Vec<_> = positions
        .iter()
        .map(|&q| dense(&phases, (half + 1) as f32, q))
        .collect();
    for i in 1..count {
        result[i - 1] = result[i];
    }
    result.truncate(count);
    Ok(result)
}
