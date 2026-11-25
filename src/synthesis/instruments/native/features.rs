use super::{score::{self, Note, Record}, tensor::Tensor};

pub(super) fn vibrato(timing: &Tensor, row: usize, note: &Note) -> Option<(f32, f32)> {
    let x = timing.get(row * 5 + 2);
    let y = timing.get(row * 5 + 3);
    Some(match (note.vibrato_hz, note.vibrato_phase) {
        (Some(hz), Some(phase)) => (hz, phase),
        (Some(hz), None) => (hz, 0.),
        (None, Some(phase)) => (5., phase),
        (None, None) => {
            if x.hypot(y) <= 0.5 { return None; }
            (timing.get(row * 5 + 4).clamp(4., 8.), y.atan2(x))
        }
    })
}

pub(super) fn pitch(notes: &[Note], records: &[Record], encoded: &Tensor,
    timing: &Tensor, trill_vibrato: bool, players: usize) -> (Tensor, Tensor) {
    let frames = records.last().unwrap().start + records.last().unwrap().length;
    let expanded = score::expand(encoded, records, frames);
    let mut input = vec![0.; frames * 523];
    let mut seeds = vec![0.; frames * 4];
    for t in 0..frames {
        input[t * 523..t * 523 + 512].copy_from_slice(&expanded.data[t * 512..(t + 1) * 512]);
        input[t * 523 + 512] = 0.5;
        input[t * 523 + 517] = super::naturalness::pitch(players);
    }
    for (begin, end, i, context) in score::segments(notes, frames) {
        let row = records.iter().position(|r| r.note == Some(i)).unwrap();
        let note = &notes[i];
        let draws = score::segment_seeds(notes, i, context, 5, end - begin);
        let trill = note.articulations.iter().find_map(|name| match name.as_str() {
            "Trill Minor" => Some(0.5), "Trill Major" => Some(1.), _ => None,
        });
        let vibrato = vibrato(timing, row, note);
        let automatic_vibrato = timing.get(row * 5 + 2).hypot(timing.get(row * 5 + 3)) > 0.5;
        for t in begin..end {
            let output = &mut input[t * 523..(t + 1) * 523];
            output[512] = note.dynamic;
            output[513] = timing.get(row * 5 + 1);
            let local = t.saturating_sub(note.start).min(note.length - 1);
            let envelope = note.vibrato_envelope.get(local).copied().unwrap_or(1.);
            if trill.is_none() || trill_vibrato { output[518] = note.vibrato_depth * envelope - 1.; }
            let draw = if context == 1 { end - 1 - t } else { t - begin };
            seeds[t * 4..(t + 1) * 4].copy_from_slice(&draws[draw * 4..(draw + 1) * 4]);
            if context != 0 { continue; }
            let local = t - begin;
            output[514] = 1.;
            output[519] = local as f32;
            output[520] = (end - t - 1) as f32;
            if let Some(interval) = trill && local % 30 >= 14 { output[521] = interval; }
            if (trill.is_none() || trill_vibrato) && note.length > 20 && local >= 10 && local < note.length - 10
                && let Some((hz, phase)) = vibrato {
                let lead = ((local as f32 - 10.) / 10.).clamp(0., 1.);
                let tail = ((note.length as f32 - 10. - local as f32) / 10.).clamp(0., 1.);
                let depth = note.vibrato_depth * envelope - 1.;
                let gain = (lead * tail) * (depth + if automatic_vibrato { 1. } else { 0. }).clamp(0., 1.);
                let angle = (local as f64 * (f64::from(hz) * std::f64::consts::TAU)) / 200. - f64::from(phase);
                output[522] = (angle.cos() * f64::from(gain)) as f32;
            }
        }
    }
    (Tensor::new(vec![1, frames, 523], input), Tensor::new(vec![1, frames, 4], seeds))
}

pub(super) fn phase(pitch: &Tensor, seed: u32) -> Tensor {
    use super::random::{M, pcg};
    let state = u64::from(seed).wrapping_mul(M).wrapping_add(0xc294138371cfd702);
    let next = state.wrapping_mul(M).wrapping_add(0xaca090a7);
    let uniform = (f64::from(pcg(next)) * 4294967296. + f64::from(pcg(state))) * 2f64.powi(-64);
    let tau = std::f64::consts::TAU;
    let mut previous = f64::from(pitch.get(0));
    let mut angle = super::oscillators::remainder(previous * tau / 48000. + (uniform * 840.) * std::f64::consts::PI, tau);
    let mut data = Vec::with_capacity(pitch.len());
    data.push(angle as f32);
    for t in 1..pitch.len() {
        let current = f64::from(pitch.get(t));
        angle = super::oscillators::remainder((previous + current) * 753.9822368615503 / 48000. + angle, tau);
        data.push(angle as f32);
        previous = current;
    }
    Tensor::new(pitch.shape.clone(), data)
}
