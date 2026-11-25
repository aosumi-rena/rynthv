use super::{
    Result, fail,
    random::{M, Normal, pcg},
    tensor::Tensor,
};
use serde::Deserialize;
#[derive(Clone, Deserialize)]
pub struct Note {
    #[serde(default)]
    pub id: u64,
    pub start: usize,
    pub length: usize,
    pub pitch: f64,
    #[serde(default)]
    pub direction: String,
    #[serde(default = "half")]
    pub dynamic: f32,
    #[serde(default)]
    pub automatic_dynamics: bool,
    #[serde(default)]
    pub automatic_articulations: bool,
    #[serde(default)]
    pub articulations: Vec<String>,
    #[serde(default = "one")]
    pub vibrato_depth: f32,
    #[serde(default)]
    pub vibrato_frequency: f32,
    #[serde(default)]
    pub vibrato_hz: Option<f32>,
    #[serde(default)]
    pub vibrato_phase: Option<f32>,
    #[serde(default)]
    pub vibrato_envelope: Vec<f32>,
    #[serde(default = "four")]
    pub articulation_in: u32,
    #[serde(default = "four")]
    pub articulation_out: u32,
    #[serde(default)]
    pub pitch_seed: u32,
    #[serde(default)]
    pub timbre_seed: u32,
    #[serde(default)]
    pub articulation_seed: u32,
    #[serde(default)]
    pub dynamics_seed: u32,
    #[serde(default)]
    pub duration_ticks: u64,
    pub linked_before: bool,
    pub linked_after: bool,
    #[serde(default = "one")]
    pub time_dispersion: f32,
    #[serde(default)]
    pub pitch_dispersion: f32,
}
fn four() -> u32 {
    4
}
fn half() -> f32 { 0.5 }
fn one() -> f32 { 1.0 }
pub struct Record {
    pub start: usize,
    pub length: usize,
    pub pitch: f64,
    pub note: Option<usize>,
    pub context: u32,
    pub owner: usize,
}
pub fn records(notes: &[Note]) -> Result<Vec<Record>> {
    for note in notes {
        if !note.time_dispersion.is_finite() || !(0.0..=2.0).contains(&note.time_dispersion)
            || !note.pitch_dispersion.is_finite() || !(0.0..=2.0).contains(&note.pitch_dispersion) {
            return fail("Invalid instrument ensemble spread");
        }
        super::super::direction::index(&note.direction)?;
        super::super::articulation::features(&note.articulations)?;
        if !note.vibrato_depth.is_finite() || !(0.0..=2.0).contains(&note.vibrato_depth)
            || !note.vibrato_frequency.is_finite() || !(-1.0..=1.0).contains(&note.vibrato_frequency)
            || note.vibrato_hz.is_some_and(|hz| !hz.is_finite() || !(4.0..=8.0).contains(&hz))
            || note.vibrato_phase.is_some_and(|phase| !phase.is_finite() || !(-1.0..=1.0).contains(&phase))
        {
            return fail("Invalid instrument vibrato depth or frequency");
        }
        if !note.vibrato_envelope.is_empty() && (note.vibrato_envelope.len()!=note.length
            || note.vibrato_envelope.iter().any(|value|!value.is_finite())) {
            return fail("Invalid instrument vibrato envelope");
        }
    }
    if notes.is_empty()
        || notes.iter().any(|n| n.length == 0 || !n.pitch.is_finite() || !n.dynamic.is_finite() || !(0.0..=1.0).contains(&n.dynamic))
        || notes
            .windows(2)
            .any(|p| p[0].start + p[0].length > p[1].start)
        || notes.last().unwrap().start + notes.last().unwrap().length + 120 > 120000
    {
        return fail(
            "Instrument notes must be finite, nonoverlapping, positive length and under ten minutes",
        );
    }
    let mut out = Vec::new();
    let (mut last, mut pitch) = (0, 666.);
    for (i, n) in notes.iter().enumerate() {
        if n.start > last {
            out.push(Record {
                start: last,
                length: n.start - last,
                pitch,
                note: None,
                context: if i == 0 { 1 } else { 2 },
                owner: i.saturating_sub(1),
            });
        }
        out.push(Record {
            start: n.start,
            length: n.length,
            pitch: n.pitch,
            note: Some(i),
            context: 0,
            owner: i,
        });
        last = n.start + n.length;
        pitch = n.pitch;
    }
    out.push(Record {
        start: last,
        length: 120,
        pitch,
        note: None,
        context: 2,
        owner: notes.len() - 1,
    });
    Ok(out)
}
pub fn basic(recs: &[Record], width: usize, notes: &[Note]) -> Tensor {
    let mut a = vec![0.; recs.len() * width];
    for (i, r) in recs.iter().enumerate() {
        a[i * width] = if r.note.is_none() { 1. } else { 0. };
        a[i * width + 1] = r.pitch as f32;
        a[i * width + 2] = r.length as f32;
        if let Some(note)=r.note && let Some(direction)=super::super::direction::index(&notes[note].direction).expect("validated direction") {
            a[i*width+width-5+direction]=1.;
        }
        if width == 29 && let Some(note) = r.note {
            let flags = super::super::articulation::features(&notes[note].articulations).unwrap();
            a[i * width + 3..i * width + 19].copy_from_slice(&flags);
        }
    }
    Tensor::new(vec![1, recs.len(), width], a)
}
pub fn segments(notes: &[Note], frames: usize) -> Vec<(usize, usize, usize, u32)> {
    let mut s = Vec::new();
    for (i, n) in notes.iter().enumerate() {
        let before = if i == 0 {
            0
        } else {
            (notes[i - 1].start + notes[i - 1].length + n.start + 1) / 2
        };
        let after = if i + 1 == notes.len() {
            frames
        } else {
            (n.start + n.length + notes[i + 1].start + 1) / 2
        };
        if before < n.start {
            s.push((before, n.start, i, 1));
        }
        s.push((n.start, n.start + n.length, i, 0));
        if after > n.start + n.length {
            s.push((n.start + n.length, after, i, 2));
        }
    }
    s
}
fn domain(base: u64, seed: u32) -> u64 {
    let inc = 2 * base + 1;
    (seed as u64)
        .wrapping_add(inc)
        .wrapping_mul(M)
        .wrapping_add(inc)
}
fn note_hash(notes: &[Note], index: usize, context: u32, kind: Option<u32>) -> [u8; 16] {
    let mut text = String::new();
    for n in &notes[index.saturating_sub(1)..notes.len().min(index + 2)] {
        text.push_str(&format!(
            "@{:x}>{:x}?{:x}/{:x}",
            n.start, n.length, n.articulation_in, n.articulation_out
        ));
        if let Some(k) = kind {
            text.push_str(&format!("*{:x}", domain(if k == 0 { 1 } else { 3 },
                if k == 0 { n.articulation_seed } else { n.dynamics_seed })));
        }
        text.push_str(&format!(".{context:x}"));
    }
    xxhash_rust::xxh3::xxh3_128(text.as_bytes()).to_le_bytes()
}
fn words(hash: [u8; 16], count: usize) -> Vec<f32> {
    let mut state = u64::from_le_bytes(hash[..8].try_into().unwrap());
    let inc = u64::from_le_bytes(hash[8..].try_into().unwrap()) | 1;
    (0..count * 4)
        .map(|_| {
            let v = pcg(state);
            state = state.wrapping_mul(M).wrapping_add(inc);
            f32::from_bits(v)
        })
        .collect()
}
pub fn record_seeds(notes: &[Note], recs: &[Record], kind: u32) -> Tensor {
    Tensor::new(
        vec![1, recs.len(), 4],
        recs.iter()
            .flat_map(|r| words(note_hash(notes, r.owner, r.context, Some(kind)), 1))
            .collect(),
    )
}
pub fn timing_seeds(notes: &[Note], recs: &[Record], explicit_vibrato: bool) -> Tensor {
    if !explicit_vibrato { return record_seeds(notes, recs, 1); }
    let mut data = Vec::with_capacity(recs.len() * 8);
    for record in recs {
        let dynamics = note_hash(notes, record.owner, record.context, Some(1));
        data.extend(words(dynamics, 1));
        let mut input = dynamics.to_vec();
        input.extend(11u64.to_le_bytes());
        data.extend(words(xxhash_rust::xxh3::xxh3_128(&input).to_le_bytes(), 1));
    }
    Tensor::new(vec![1, recs.len(), 8], data)
}
fn segment_hash(notes: &[Note], index: usize, context: u32, base: u64) -> [u8; 16] {
    let mut h = note_hash(notes, index, context, None).to_vec();
    let seed = if base == 5 {
        notes[index].pitch_seed
    } else {
        notes[index].timbre_seed
    };
    h.extend(domain(base, seed).to_le_bytes());
    xxhash_rust::xxh3::xxh3_128(&h).to_le_bytes()
}
pub(super) fn segment_seeds(notes: &[Note], index: usize, context: u32, base: u64, count: usize) -> Vec<f32> {
    words(segment_hash(notes, index, context, base), count)
}
fn fallback_pitch(notes: &[Note], frames: usize) -> Vec<f32> {
    let mut output = vec![0.; frames];
    for (begin, end, index, context) in segments(notes, frames) {
        let mut input = segment_hash(notes, index, context, 5).to_vec();
        input.extend(0x46304649u64.to_le_bytes());
        let hash = xxhash_rust::xxh3::xxh3_128(&input).to_le_bytes();
        let state = u64::from_le_bytes(hash[..8].try_into().unwrap());
        let increment = u64::from_le_bytes(hash[8..].try_into().unwrap()) | 1;
        let mut random = Normal::new(state, increment, false);
        let mut walk = 0f32;
        for local in 0..end - begin {
            walk += random.next();
            if walk.abs() > 5. { walk = 0.; }
            let frame = if context == 1 { end - 1 - local } else { begin + local };
            output[frame] = walk + 40.;
        }
    }
    output
}
pub fn clock_seeds(notes: &[Note], frames: usize, base: u64, step: usize) -> Tensor {
    let mut a = vec![0.; frames / step * 4];
    for (begin, end, i, context) in segments(notes, frames) {
        let (b, e) = (begin / step, end / step);
        let q = segment_seeds(notes, i, context, base, e - b);
        for t in 0..e - b {
            let s = if context == 1 { e - b - 1 - t } else { t };
            a[(b + t) * 4..(b + t + 1) * 4].copy_from_slice(&q[s * 4..(s + 1) * 4]);
        }
    }
    Tensor::new(vec![1, frames / step, 4], a)
}
pub fn wave_seeds(notes: &[Note], frames: usize, source_seed: u32) -> Tensor {
    let mut output = vec![0.; frames * 4];
    let salt = u64::from(source_seed).wrapping_mul(M).wrapping_add(0x86d247d3a65e6b6a);
    for (begin, end, owner, context) in segments(notes, frames) {
        let mut input = segment_hash(notes, owner, context, 9).to_vec();
        input.extend(salt.to_le_bytes());
        let hash = xxhash_rust::xxh3::xxh3_128(&input).to_le_bytes();
        let values = words(hash, end - begin);
        for frame in begin..end {
            let local = if context == 1 { end - 1 - frame } else { frame - begin };
            output[frame * 4..(frame + 1) * 4].copy_from_slice(&values[local * 4..(local + 1) * 4]);
        }
    }
    Tensor::new(vec![1, frames, 4], output)
}
pub fn expand(encoded: &Tensor, recs: &[Record], frames: usize) -> Tensor {
    let width = encoded.shape[2];
    let quarter = width / 4;
    let mut out = vec![0.; frames * width];
    for (i, r) in recs.iter().enumerate() {
        if r.note.is_none() {
            continue;
        }
        for t in 0..r.length.min(frames.saturating_sub(r.start)) {
            for c in 0..width {
                let j = c % quarter;
                let freq = (1. + (j as f32 * 7.) / (quarter - 1) as f32).round_ties_even();
                let pos = if c < 2 * quarter { t } else { r.length - 1 - t }.min(100) as f32;
                let angle = (((freq / 100.) * pos) * 2.) * std::f32::consts::PI;
                let basis = if c / quarter % 2 == 0 {
                    angle.sin()
                } else {
                    angle.cos()
                };
                out[(r.start + t) * width + c] =
                    half::f16::from_f32(basis * encoded.get(i * width + c)).to_f32();
            }
        }
    }
    Tensor::new(vec![1, frames, width], out)
}
pub fn pitch_features(
    notes: &[Note],
    recs: &[Record],
    encoded: &Tensor,
    timing: &Tensor,
    trill_vibrato: bool,
    players: usize,
) -> (Tensor, Tensor) {
    let frames = recs.last().unwrap().start + recs.last().unwrap().length;
    let e = expand(encoded, recs, frames);
    let mut x = vec![0.; frames * 522];
    let mut seeds = vec![0.; frames * 4];
    for t in 0..frames {
        x[t * 522..t * 522 + 512].copy_from_slice(&e.data[t * 512..(t + 1) * 512]);
        x[t * 522 + 512] = 0.5;
        x[t * 522 + 517] = super::naturalness::pitch(players);
    }
    for (begin, end, i, context) in segments(notes, frames) {
        let row = recs.iter().position(|r| r.note == Some(i)).unwrap();
        let q = segment_seeds(notes, i, context, 5, end - begin);
        for t in begin..end {
            x[t * 522 + 512] = notes[i].dynamic;
            let note = &notes[i];
            let envelope = if note.vibrato_envelope.is_empty() { 1.0 } else {
                note.vibrato_envelope[t.saturating_sub(note.start).min(note.length - 1)]
            };
            let trill = note.articulations.iter().any(|name| name.starts_with("Trill "));
            if !trill || trill_vibrato {
                x[t * 522 + 518] = note.vibrato_depth * envelope - 1.0;
                x[t * 522 + 519] = notes[i].vibrato_frequency;
            }
            x[t * 522 + 513] = timing.get(row * 2 + 1);
            let p = if context == 1 { end - 1 - t } else { t - begin };
            seeds[t * 4..(t + 1) * 4].copy_from_slice(&q[p * 4..(p + 1) * 4]);
            if context == 0 {
                x[t * 522 + 514] = 1.;
                x[t * 522 + 520] = (t - begin) as f32;
                x[t * 522 + 521] = (end - t - 1) as f32;
            }
        }
    }
    (
        Tensor::new(vec![1, frames, 522], x),
        Tensor::new(vec![1, frames, 4], seeds),
    )
}
pub fn latent_features(
    notes: &[Note],
    recs: &[Record],
    encoded: &Tensor,
    timing: &Tensor,
    pitch: &Tensor,
    frames: usize,
    players: usize,
) -> (Tensor, Tensor, Tensor) {
    let e = expand(encoded, recs, frames);
    let mut x = vec![0.; frames * 518];
    let mut controls = vec![0.; frames / 4 * 2];
    for row in controls.chunks_mut(2) {
        row[0] = 0.5;
    }
    for note in notes.iter().filter(|n| n.linked_before && n.linked_after) {
        let begin = (note.start / 4).min(frames / 4);
        let end = ((note.start + note.length) / 4).min(frames / 4);
        for t in begin..end { controls[t * 2] = super::naturalness::timbre(players); }
    }
    for t in 0..frames {
        x[t * 518..t * 518 + 512].copy_from_slice(&e.data[t * 512..(t + 1) * 512]);
        x[t * 518 + 512] = 0.5;
        x[t * 518 + 515] = pitch.get(t);
    }
    for (begin, end, i, context) in segments(notes, frames) {
        let row = recs.iter().position(|r| r.note == Some(i)).unwrap();
        let end = end.min(frames);
        for t in begin..end {
            x[t * 518 + 512] = notes[i].dynamic;
            x[t * 518 + 513] = timing.get(row * timing.shape[2] + 1);
            if context == 0 {
                x[t * 518 + 514] = 1.;
                x[t * 518 + 516] = (t - begin) as f32;
                x[t * 518 + 517] = (end - t - 1) as f32;
            }
        }
    }
    (
        Tensor::new(vec![1, frames, 518], x),
        clock_seeds(notes, frames, 7, 4),
        Tensor::new(vec![1, frames / 4, 2], controls),
    )
}
pub fn assemble(raw: &Tensor, notes: &[Note], frames: usize) -> Tensor {
    let midi: Vec<f32> = (0..raw.len())
        .map(|i| (((raw.get(i) as f64 / 440.).log2() * 12.) + 69.) as f32)
        .collect();
    let mut smooth = midi.clone();
    let weights = [
        0.1464466154575348f32,
        0.4999999701976776,
        0.8535534143447876,
        0.8535534143447876,
        0.4999999701976776,
        0.1464466154575348,
    ];
    for (i, v) in midi.iter().copied().enumerate() {
        if !v.is_finite() {
            continue;
        }
        let (mut sum, mut total) = (0f32, 0f32);
        for (delta, w) in [-3isize, -2, -1, 1, 2, 3].into_iter().zip(weights) {
            let j = i as isize + delta;
            if j < 0 || j >= midi.len() as isize || !midi[j as usize].is_finite() {
                continue;
            }
            let mut diff = midi[j as usize] - v;
            let mag = diff.abs();
            if mag > 0.06 {
                diff = if mag >= 0.14999999105930328 {
                    0.
                } else {
                    (((0.14999999105930328 - mag) * 0.06) / 0.08999998867511749).copysign(diff)
                };
            }
            sum += w * diff;
            total += w;
        }
        smooth[i] = if total > 0. { v + sum / total } else { v };
    }
    let mut sums = vec![0f64; frames];
    let mut ws = vec![0f64; frames];
    for n in notes {
        for t in n.start.saturating_sub(100)..frames.min(n.start + n.length + 100) {
            let w = if t < n.start {
                ((t as f64 - (n.start as f64 - 100.)) / 100.).powi(4)
            } else if t >= n.start + n.length {
                ((n.start + n.length + 100 - t) as f64 / 100.).powi(4)
            } else {
                1.
            };
            let relative = smooth[t.min(smooth.len() - 1)] - n.pitch as f32;
            let restored = relative + n.pitch as f32;
            sums[t] += restored as f64 * w;
            ws[t] += w;
        }
    }
    let fallback = fallback_pitch(notes, frames);
    let out = (0..frames)
        .map(|t| {
            let p = if ws[t] > 1e-5 {
                (sums[t] / ws[t].max(1e-5)) as f32
            } else {
                fallback[t]
            };
            (((p as f64 - 69.) / 12.).exp2() * 440.) as f32
        })
        .collect();
    Tensor::new(vec![1, frames, 1], out)
}
