// SPDX-License-Identifier: MIT
use super::super::{DecodedFrame, Error, Result, lf::fast_sin, spectrum::harmonics};

pub(super) fn rotate(frame: &mut DecodedFrame, phase: f64) {
    let phase = phase as f32;
    let rotate = |values: &mut [f32]| {
        for (i,value) in values.iter_mut().enumerate() {
            let angle = ((i+1) as f64*f64::from(phase)+f64::from(*value)) as f32;
            let cycles = (f64::from(angle*0.5)*std::f64::consts::FRAC_1_PI).round();
            *value = (f64::from(angle)-cycles*std::f64::consts::TAU) as f32;
        }
    };
    rotate(&mut frame.harmonic_phase);
    for envelope in &mut frame.noise_envelopes { rotate(&mut envelope.phase); }
}
pub(super) fn window(count: usize) -> Vec<f32> {
    let inverse = 1.0/(count-1) as f64;
    (0..count).map(|i| {
        let phase = (i as f64*inverse*std::f64::consts::TAU) as f32
            + f32::from_bits(0x3fc90fdb);
        (1.0-fast_sin(phase))*0.5
    }).collect()
}
fn direct(amplitude: &[f32], phase: &[f32], normalized: f32, n: usize) -> Result<Vec<f32>> {
    if n < 2 || phase.len()!=amplitude.len() { return Err(Error::Format("P3 oscillator shape")); }
    let mut output = vec![0.0; n];
    for (k,(&amp,&ph)) in amplitude.iter().zip(phase).enumerate() {
        let frequency = ((k+1) as f64*f64::from(normalized)) as f32;
        let omega = (f64::from(frequency)*std::f64::consts::TAU) as f32;
        let coefficient = (2.0*f64::from(omega).cos()) as f32;
        let mut a = (f64::from(-(n as f32/2.0)*omega+ph).cos()*f64::from(amp)) as f32;
        let mut b = (f64::from((1.0-n as f32/2.0)*omega+ph).cos()*f64::from(amp)) as f32;
        output[0] = a+output[0]; output[1] = b+output[1];
        for value in &mut output[2..] {
            let c = b*coefficient-a; *value = c+*value; a=b; b=c;
        }
    }
    Ok(output)
}
pub(super) fn power(frame: &DecodedFrame, window: &[f32]) -> Result<[Vec<f32>;4]> {
    let mut values = std::array::from_fn(|_| Vec::new());
    for c in 0..4 {
        let envelope = &frame.noise_envelopes[c];
        let mut wave = if frame.f0_hz>0.0 {
            direct(&envelope.amplitude,&envelope.phase,frame.f0_hz/44100.0,window.len())?
        } else { vec![0.0;window.len()] };
        for (i,value) in wave.iter_mut().enumerate() {
            *value = (f64::from(frame.noise_bands[c]+*value).max(1e-8)*f64::from(window[i])) as f32;
        }
        values[c]=wave;
    }
    Ok(values)
}
pub(super) fn classic(frame: &DecodedFrame, window: &[f32], carry: f32, nyquist: f32) -> Result<Vec<f32>> {
    let spectrum = harmonics(frame, frame.f0_hz, nyquist)?;
    let shift = (f64::from(carry+carry)*std::f64::consts::PI*f64::from(frame.f0_hz)) as f32;
    let phase: Vec<_> = spectrum.phase.iter().enumerate()
        .map(|(i,&phase)| (f64::from(phase)-(i+1) as f64*f64::from(shift)) as f32).collect();
    let normalized=frame.f0_hz/44100.0;
    let mut wave = if super::chirp::selects_direct(window.len(),spectrum.amplitude.len()) {
        direct(&spectrum.amplitude,&phase,normalized,window.len())?
    } else {
        super::chirp::evaluate(&spectrum.amplitude,&phase,normalized,window.len())?
    };
    for (value,&window) in wave.iter_mut().zip(window) { *value *= window; }
    Ok(wave)
}
