// SPDX-License-Identifier: MIT
use super::{Error, Phrase, Result, score::TICKS_PER_QUARTER};
const STEP: i64 = 8_820_000;
const PI: f64 = f64::from_bits(0x400921f9f01b866e);
struct Record {
    origin: i64,
    duration: i64,
    pitch: f64,
    rest: bool,
    left: i64,
    right: i64,
    left_depth: f64,
    right_depth: f64,
    delay: i64,
    attack: i64,
    release: i64,
    rate: f64,
}
pub(super) struct PitchBaseline {
    points: Vec<(f64, f32)>,
}
impl PitchBaseline {
    pub fn new(phrase: &Phrase) -> Result<Self> {
        phrase.validate()?;
        let mut records = Vec::new();
        let mut previous = 0;
        for note in &phrase.notes {
            if note.local_tick > previous {
                let bpm = tempo(phrase, phrase.anchor_tick + previous);
                let ratio = bpm / 120.0;
                records.push(Record {
                    origin: previous,
                    duration: note.local_tick - previous,
                    pitch: -1.0,
                    rest: true,
                    left: (ratio * 141120000.0) as i64,
                    right: (ratio * 141120000.0) as i64,
                    left_depth: 0.5,
                    right_depth: 0.5,
                    delay: (ratio * 235200000.0) as i64,
                    attack: (ratio * 235200000.0) as i64,
                    release: (ratio * 235200000.0) as i64,
                    rate: (120.0 / bpm) * f64::from_bits(0x3e2e6f52eaa653cb),
                });
            }
            let bpm = tempo(phrase, phrase.anchor_tick + note.local_tick);
            let timing = |bits| {
                ((f64::from(f32::from_bits(bits)) * bpm / 60.0) * TICKS_PER_QUARTER as f64) as i64
            };
            records.push(Record {
                origin: note.local_tick,
                duration: note.duration_ticks,
                pitch: f64::from(note.midi_pitch),
                rest: false,
                left: timing(0x3d8f5c29),
                right: timing(0x3d8f5c29),
                left_depth: f64::from(f32::from_bits(0x3e19999a)),
                right_depth: f64::from(f32::from_bits(0x3e19999a)),
                delay: timing(0x3e800000),
                attack: timing(0x3e4ccccd),
                release: timing(0x3e4ccccd),
                rate: f64::from(5.5f32 * 60.0) / bpm / TICKS_PER_QUARTER as f64,
            });
            previous = note.local_tick + note.duration_ticks;
        }
        let first = phrase.notes[0].local_tick;
        let extent = previous - first;
        let count = (extent / STEP) as usize;
        if count == 0 || count > 1_000_000 {
            return Err(Error::Unsupported("default pitch raster extent"));
        }
        let values = evaluate(&records, first, extent, count)?;
        let points = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                Ok((
                    phrase.seconds_at(phrase.anchor_tick + first + i as i64 * STEP)?,
                    v.abs() as f32,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { points })
    }
    pub fn sample(&self, seconds: f64) -> f64 {
        let at = self.points.partition_point(|p| p.0 <= seconds);
        if at == 0 {
            return f64::from(self.points[0].1);
        }
        if at == self.points.len() {
            return f64::from(self.points[at - 1].1);
        }
        let (a, x) = self.points[at - 1];
        let (b, y) = self.points[at];
        f64::from(x) + f64::from(y - x) * ((seconds - a) / (b - a))
    }
}
fn tempo(phrase: &Phrase, tick: i64) -> f64 {
    phrase
        .tempos
        .iter()
        .rev()
        .find(|p| p.tick <= tick)
        .unwrap_or(&phrase.tempos[0])
        .bpm
}
fn width(ticks: i64) -> i64 {
    (ticks as f64 * 0.3).max(2756250.0) as i64
}
fn logistic(x: f64) -> f64 {
    if x < -10.0 {
        0.0
    } else if x > 10.0 {
        1.0
    } else {
        1.0 / ((-x).exp() + 1.0)
    }
}
fn evaluate(records: &[Record], start_tick: i64, extent: i64, count: usize) -> Result<Vec<f64>> {
    let step = extent / count as i64;
    let half = step / 2;
    let halo = TICKS_PER_QUARTER / step;
    let bin = |tick: i64| {
        ((half as i128 - start_tick as i128 + tick as i128) * count as i128 / extent as i128) as i64
    };
    let tick = |i: i64| start_tick + (extent as i128 * i as i128 / count as i128) as i64;
    let mut result = vec![0.0; count];
    for (j, r) in records.iter().enumerate() {
        let last = j + 1 == records.len();
        let next = &records[(j + 1).min(records.len() - 1)];
        let end = r.origin + r.duration;
        let next_end = end
            .checked_add(next.duration)
            .ok_or(Error::Format("pitch provider pair overflow"))?;
        let (a, b, c) = (bin(r.origin), bin(end), bin(next_end));
        let (left_count, right_count) = ((b - a) as usize, (c - b) as usize);
        let (lw, rw) = (
            width(r.right),
            if last { 22050000 } else { width(next.left) },
        );
        let (mut ld, mut rd) = (r.right_depth, if last { 0.0 } else { next.left_depth });
        let mut p = r.pitch;
        let mut q = next.pitch;
        if r.rest {
            p = if j == 0 { q } else { records[j - 1].pitch };
        } else if next.rest {
            q = records[(j + 2).min(records.len() - 1)].pitch;
        }
        let mut shift = 0;
        if r.rest {
            ld = 0.0;
            rd = -rd;
            shift = (rw as f64 * 1.5) as i64;
        } else if next.rest || last {
            ld = -ld;
            rd = 0.0;
            shift = (-(lw as f64) * 1.5) as i64;
        } else if q > p {
            ld = -ld;
        } else {
            rd = -rd;
        }
        let (lc, rc) = (lw as f64 * 1.5, rw as f64 * 1.5);
        for i in a.max(0)..c.min(count as i64) {
            let k = (i - a) as usize;
            let t0 = tick(i) - r.origin - r.duration;
            let mut value = if r.rest {
                if k < left_count {
                    p + (q - p) * logistic((k as f64 / left_count as f64 - 0.5) * 20.0)
                } else {
                    q
                }
            } else if next.rest {
                if k < left_count {
                    p
                } else {
                    p + (q - p)
                        * logistic(((k - left_count) as f64 / right_count as f64 - 0.5) * 20.0)
                }
            } else {
                p + (q - p) * logistic(t0 as f64 * (3.0 / (lw + rw) as f64))
            };
            let t = t0 + shift;
            let u = ((t as f64 + lc) as i64) as f64 / lw as f64;
            let v = ((t as f64 - rc) as i64) as f64 / rw as f64;
            let right = ((-v * v) * 0.5).exp() * rd;
            let left = ((-u * u) * 0.5).exp() * ld;
            value = right + (left + value);
            let weight = if i < b {
                if j == 0 {
                    1.0
                } else {
                    let fraction = (tick(i) - r.origin) as f64 / r.duration as f64;
                    (fraction * PI + PI).cos() * 0.5 + 0.5
                }
            } else if last {
                1.0
            } else {
                let fraction = (tick(i) - r.origin - r.duration) as f64 / next.duration as f64;
                (fraction * PI).cos() * 0.5 + 0.5
            };
            result[i as usize] += weight * value;
        }
    }
    for (j, r) in records.iter().enumerate() {
        let (a, b) = (bin(r.origin), bin(r.origin + r.duration));
        if r.rest {
            for i in a.max(0)..b.min(count as i64) {
                result[i as usize] = if i - a <= halo || b - i <= halo {
                    -result[i as usize]
                } else {
                    0.0
                };
            }
        }
        if j + 1 == records.len() {
            for i in b.max(0)..count as i64 {
                result[i as usize] = if i - b <= halo {
                    -result[i as usize]
                } else {
                    0.0
                };
            }
        }
    }
    let mut vibrato = vec![0.0; count];
    for r in records {
        if r.rest || r.duration <= r.delay {
            continue;
        }
        let start = r.origin + r.delay;
        let end = r.origin + r.duration;
        let first = ((start as i128 - start_tick as i128) * count as i128 / extent as i128) as i64;
        let stop = ((end as i128 - start_tick as i128) * count as i128 / extent as i128) as i64;
        let attack = r.attack.max(5512500) as f64;
        let release = r.release.max(5512500) as f64;
        for i in first.max(0)..stop.min(count as i64) {
            let elapsed = tick(i) - start;
            let envelope = (elapsed as f64 / attack)
                .min((end - start - elapsed) as f64 / release)
                .min(1.0);
            let phase = (elapsed as f64 + elapsed as f64) * PI * r.rate;
            vibrato[i as usize] = phase.sin() * envelope * 0.5; //resolved depth1/phase0.
        }
    }
    for (value, residual) in result.iter_mut().zip(vibrato) {
        if *value != 0.0 {
            *value += if *value > 0.0 { residual } else { -residual };
        }
        if !value.is_finite() {
            return Err(Error::Format("default pitch finite result"));
        }
    }
    Ok(result)
}
