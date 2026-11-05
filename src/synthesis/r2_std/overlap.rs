// SPDX-License-Identifier: MIT
use super::{DecodedFrame, Error, Result, lf::fast_sin};
pub(super) type Row = (DecodedFrame, [f32; 64]);
pub(super) struct Weights {
    pub first: Vec<f32>,
    pub second: Vec<f32>,
    pub third: Option<Vec<f32>>,
}
pub(super) struct Roles<'a> {
    pub target: &'a str,
    pub current: &'a str,
    pub previous: &'a str,
    pub category: u8,
    pub start: f64,
    pub mid: f64,
    pub end: f64,
}
pub(super) fn weights(
    roles: Roles<'_>,
    old: &[[f32; 64]],
    next: &[[f32; 64]],
    force: bool,
    lambda: f64,
    hop: f64,
    current_count: usize,
) -> Result<Weights> {
    let n = old.len().min(next.len());
    let stop = matches!(roles.category, 2 | 3);
    let mut mode = if force || roles.target == roles.previous && roles.target == roles.current {
        if stop { 1 } else { 0 }
    } else if roles.target == roles.previous {
        2
    } else if roles.target == roles.current {
        1
    } else {
        0
    };
    if roles.category == 11 {
        mode = 2;
    }
    let radius = (0.3 * n as f64).min(if roles.category <= 1 { 10.0 } else { 5.0 }) as f32;
    let mut first = vec![0.0; n];
    let mut second = vec![0.0; n];
    let mut third = None;
    if n == 0 {
        return Ok(Weights { first, second, third });
    }
    if roles.category == 10 {
        for i in 0..n {
            first[i] = i as f32 / n as f32;
            second[i] = first[i];
        }
    } else if mode == 2 {
        for i in 0..n {
            first[i] = if (i as f32) < n as f32 - radius {
                0.0
            } else {
                (i as f32 - n as f32 + radius) / radius
            };
            second[i] = first[i];
        }
    } else if mode == 1 {
        let fast = |i: usize| {
            if (i as f32) < radius {
                i as f32 / radius
            } else {
                1.0
            }
        };
        if !stop {
            for i in 0..n {
                first[i] = fast(i);
                second[i] = first[i];
            }
        } else if roles.end - roles.start > (current_count + 1) as f64 * hop {
            for i in 0..n {
                first[i] = i as f32 / n as f32;
                second[i] = fast(i);
            }
        } else {
            let delay = (n as i64 - ((roles.end - roles.mid) / hop) as i64 - 2).max(0) as usize;
            third=Some((0..n).map(fast).collect());
            for i in 0..n {
                first[i] = if i < delay {
                    0.0
                } else {
                    (i - delay) as f32 / (n - delay) as f32
                };
                second[i] = if i < delay {
                    i as f32 / delay as f32
                } else {
                    1.0
                };
            }
        }
    } else {
        let count = n.min(256);
        let distance = |j: usize| {
            let mut sums = [0.0f64; 4];
            for i in 0..64 {
                let d = old[j][i].max(-6.0) - next[j][i].max(-6.0);
                sums[i % 4] += f64::from(d * d);
            }
            (((sums[0] + sums[2]) + (sums[1] + sums[3])) / 64.0) as f32
        };
        let mut curve = Vec::with_capacity(count);
        for i in 0..count {
            let q = i as f32 * (1.0 / count as f32);
            let t = n as f32 * q;
            let j0 = (t as usize).min(n - 1);
            let j1 = ((t + 1.0) as usize).min(n - 1);
            let a = distance(j0);
            let b = distance(j1);
            let mut value = (t - j0 as f32) * (b - a) + a;
            if roles.target != roles.current && roles.target == roles.previous && q < 0.5 {
                value += 10.0;
            }
            if roles.target != roles.previous && roles.target == roles.current && q > 0.5 {
                value += 10.0;
            }
            curve.push(value);
        }
        first = super::overlap_fade::compute(&curve, lambda)?;
        if n > 256 {
            let old = first;
            first = (0..n)
                .map(|i| super::spectrum::uniform(&old, n as f32, i as f32))
                .collect();
        }
        second = first.clone();
    }
    if first.iter().chain(&second).any(|v| !v.is_finite()) {
        return Err(Error::Format("overlap weights"));
    }
    Ok(Weights { first, second, third })
}
pub(super) fn blend(a: Row, b: Row, w0: f32, w1: f32, w2: Option<f32>) -> Result<Row> {
    let source_pitch=(b.0.source_pitch-a.0.source_pitch)*f64::from(w0)+a.0.source_pitch;
    if !source_pitch.is_finite() { return Err(Error::Format("source pitch overlap wrapper")); }
    let incoming=b.0.noise_wave.clone();
    let mut output=blend_payload(a,b,w0,w1)?;
    output.0.source_pitch=source_pitch;
    let weight=w2.unwrap_or(w1);
    if let Some(existing)=&mut output.0.noise_wave { existing.gain *= 1.0-weight; }
    if let Some(mut incoming)=incoming {
        if output.0.noise_wave.is_none() || weight>0.5 {
            incoming.gain*=weight;output.0.noise_wave=Some(incoming);
        }
    }
    if output.0.noise_wave.as_ref().is_some_and(|key|!key.gain.is_finite()) {
        return Err(Error::Format("embedded wave overlap gain"));
    }
    Ok(output)
}
fn blend_payload(mut a: Row, b: Row, w0: f32, w1: f32) -> Result<Row> {
    let lerp = |a: f32, b: f32, w: f32| (b - a) * w + a;
    for i in 0..64 {
        a.1[i] = lerp(a.1[i], b.1[i], w0);
    }
    if f64::from(w0) < 0.01 && f64::from(w1) < 0.01 {
        return Ok(a);
    }
    if f64::from(w0) > 0.99 && f64::from(w1) > 0.99 {
        return Ok((b.0, a.1));
    }
    let (lv, rv) = (a.0.f0_hz > 0.0, b.0.f0_hz > 0.0);
    let (lp, rp) = (
        !a.0.harmonic_phase.is_empty(),
        !b.0.harmonic_phase.is_empty(),
    );
    if lv && rv {
        a.0.f0_hz = lerp(a.0.f0_hz, b.0.f0_hz, w0);
        a.0.rd = lerp(a.0.rd, b.0.rd, w0);
        let mut phase = vec![0.0; a.0.harmonic_phase.len().max(b.0.harmonic_phase.len())];
        let common = a.0.harmonic_phase.len().min(b.0.harmonic_phase.len());
        for (i, v) in phase.iter_mut().enumerate().take(common) {
            let (x, y) = (a.0.harmonic_phase[i], b.0.harmonic_phase[i]);
            *v = lerp(fast_sin(x), fast_sin(y), w0).atan2(lerp(
                fast_sin(x + std::f32::consts::FRAC_PI_2),
                fast_sin(y + std::f32::consts::FRAC_PI_2),
                w0,
            ));
        }
        if b.0.harmonic_phase.len() > common {
            phase[common..].copy_from_slice(&b.0.harmonic_phase[common..]);
        }
        a.0.harmonic_phase = phase;
        for (x, y) in a.0.vocal_tract_db.iter_mut().zip(&b.0.vocal_tract_db) {
            *x = lerp(*x, *y, w0);
        }
    } else if rv {
        a.0.f0_hz = b.0.f0_hz;
        a.0.rd = b.0.rd;
        a.0.harmonic_phase = b.0.harmonic_phase.clone();
        a.0.vocal_tract_db = b.0.vocal_tract_db.clone();
    } else if !lv {
        a.0.f0_hz = 0.0;
        a.0.rd = 1.0;
    }
    if lv != rv {
        let amount =
            f64::from(if rv { w0 } else { 1.0 - w0 }).max(f64::from_bits(0x3e45798ee2308c3a));
        let offset =
            (f64::from(super::decoder::fast_log(amount as f32) * f32::from_bits(0x3f317218))
                * f64::from_bits(0x40215f2cec554b28)) as f32;
        for v in &mut a.0.vocal_tract_db {
            *v += offset;
        }
    }
    for v in &mut a.0.vocal_tract_db {
        *v = v.max(-80.0);
    }
    for i in 0..256 {
        let x = (a.0.noise_psd_db[i] * f32::from_bits(0x3e13bb63)).exp2();
        let y = (b.0.noise_psd_db[i] * f32::from_bits(0x3e13bb63)).exp2();
        a.0.noise_psd_db[i] = super::decoder::fast_log(lerp(x, y, w1)) * f32::from_bits(0x40ddce9e);
    }
    for i in 0..4 {
        a.0.noise_bands[i] = lerp(a.0.noise_bands[i], b.0.noise_bands[i], w1);
        let (x, y) = (&a.0.noise_envelopes[i], &b.0.noise_envelopes[i]);
        let common = x.amplitude.len().min(y.amplitude.len());
        let mut amplitude = super::source_frames::mix(&x.amplitude, &y.amplitude, w1, false, false);
        let phase = super::source_frames::mix(&x.phase, &y.phase, w1, true, false);
        let (dl, sr) = (a.0.noise_bands[i], b.0.noise_bands[i]);
        if !lp && dl > sr + sr || !rp && sr > dl + dl {
            amplitude[..common].fill(0.0);
        }
        a.0.noise_envelopes[i] = super::frames::Envelope { amplitude, phase };
    }
    if a.0
        .vocal_tract_db
        .iter()
        .chain(&a.0.noise_psd_db)
        .chain(&a.1)
        .any(|v| !v.is_finite())
    {
        return Err(Error::Format("overlap row output"));
    }
    Ok(a)
}
