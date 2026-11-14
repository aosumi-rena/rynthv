use std::sync::Arc;

use crate::{
    model::{Automation, Extra, QUARTER},
    synthesis::{
        plan::{GlobalBlick, OwnerInput},
        voice::{Result, VoiceError},
    },
};

#[derive(Clone, Debug)]
pub(super) struct SavedPitch {
    curve: Arc<Automation>,
    notes: Vec<PitchNote>,
}

#[derive(Clone, Copy, Debug)]
struct PitchNote {
    start: i64,
    end: i64,
    pitch: f64,
    instant: bool,
    offset: f64,
    left: f64,
    right: f64,
    left_depth: f64,
    right_depth: f64,
}

impl SavedPitch {
    pub(super) fn new(input: &OwnerInput) -> Result<Option<Self>> {
        let Some(curve) = input.system_pitch_delta.as_ref() else {
            return Ok(None);
        };
        curve.validate().map_err(VoiceError::new)?;
        let derived = crate::synthesis::plan::note_issues::for_input(input);
        let mut notes = Vec::new();
        for note in &input.notes {
            if note
                .attributes
                .get("muted")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
                || derived.contains_key(&note.id)
            {
                continue;
            }
            let end = note
                .onset
                .0
                .checked_add(note.duration)
                .ok_or_else(|| VoiceError::new("Invalid saved pitch note range"))?;
            let instant = note
                .extensions
                .get("instantMode")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let position = note
                .onset
                .0
                .checked_add(input.content_offset.0)
                .ok_or_else(|| VoiceError::new("Invalid saved pitch note position"))?;
            let start_seconds = input.tempo.seconds_at(GlobalBlick(position));
            let bpm = input
                .tempo
                .points
                .iter()
                .rev()
                .find(|row| row.position.0 <= position)
                .or_else(|| input.tempo.points.first())
                .ok_or_else(|| VoiceError::new("Missing saved pitch tempo"))?
                .bpm;
            let ticks_per_second = bpm * QUARTER as f64 / 60.0;
            if !ticks_per_second.is_finite()
                || ticks_per_second <= 0.0
                || !start_seconds.is_finite()
            {
                return Err(VoiceError::new("Invalid saved pitch tempo"));
            }
            let generated = note
                .extensions
                .get("systemAttributes")
                .and_then(serde_json::Value::as_object);
            let read = |name: &str| read_control(&note.attributes, generated, name);
            let window = |name| {
                let width = read(name)?.map_or(QUARTER as f64 / 5.0, |v| v * ticks_per_second);
                Ok::<_, VoiceError>((width * 0.3).max(QUARTER as f64 / 256.0))
            };
            let detune = note
                .extensions
                .get("detune")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            notes.push(PitchNote {
                start: note.onset.0,
                end,
                pitch: f64::from(note.pitch) + detune / 100.0,
                instant,
                offset: read("tF0Offset")?.unwrap_or(0.0) * ticks_per_second,
                left: window("tF0Left")?,
                right: window("tF0Right")?,
                left_depth: read("dF0Left")?.unwrap_or(0.5),
                right_depth: read("dF0Right")?.unwrap_or(0.5),
            });
        }
        if notes.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            curve: Arc::clone(curve),
            notes,
        }))
    }

    pub(super) fn sample(&self, tick: i64) -> Option<f64> {
        if tick < self.curve.points.first()?.0 || tick > self.curve.points.last()?.0 {
            return None;
        }
        let after = self.notes.partition_point(|note| note.start <= tick);
        let index = after.saturating_sub(1);
        let note = &self.notes[index];
        let previous = index.checked_sub(1).map(|i| &self.notes[i]);
        let next = self.notes.get(index + 1);
        let instant = |note: &PitchNote| f64::from(u8::from(note.instant));
        let between = tick >= note.end && next.is_some_and(|next| tick < next.start);
        let weight = if between {
            let next = next?;
            let fraction = distance(tick, note.end) / distance(next.start, note.end);
            blend(instant(note), instant(next), ease(fraction))
        } else {
            let ramp = (distance(note.end, note.start) * 0.5).min(QUARTER as f64 / 8.0);
            let from_start = distance(tick, note.start);
            let to_end = distance(note.end, tick);
            if ramp > 0.0 && from_start < ramp {
                let prior = previous.filter(|p| p.end == note.start).unwrap_or(note);
                blend(
                    instant(prior),
                    instant(note),
                    ease(0.5 + 0.5 * from_start / ramp),
                )
            } else if ramp > 0.0 && to_end < ramp {
                let following = next.filter(|n| n.start == note.end).unwrap_or(note);
                blend(
                    instant(note),
                    instant(following),
                    ease(0.5 - 0.5 * to_end / ramp),
                )
            } else {
                instant(note)
            }
        };
        if weight <= 0.0 {
            return None;
        }
        let baseline = if between {
            transition(note, next?, tick)
        } else {
            let left_pitch = previous
                .filter(|p| p.end == note.start)
                .map_or(note.pitch, |p| transition(p, note, tick));
            let right_pitch = next
                .filter(|n| n.start == note.end)
                .map_or(note.pitch, |n| transition(note, n, tick));
            if previous.is_none() && next.is_some_and(|n| n.start == note.end) {
                right_pitch
            } else {
                blend(
                    left_pitch,
                    right_pitch,
                    ease(distance(tick, note.start) / distance(note.end, note.start)),
                )
            }
        };
        Some(baseline + f64::from(self.curve.sample(tick, 0.0)) * weight / 100.0)
    }
}

fn read_control(
    attributes: &Extra,
    generated: Option<&serde_json::Map<String, serde_json::Value>>,
    name: &str,
) -> Result<Option<f64>> {
    attributes
        .get(name)
        .or_else(|| generated.and_then(|g| g.get(name)))
        .map(|v| {
            v.as_f64()
                .filter(|v| (*v as f32).is_finite())
                .ok_or_else(|| VoiceError::new(format!("Invalid saved pitch control {name}")))
        })
        .transpose()
}

fn distance(a: i64, b: i64) -> f64 {
    (i128::from(a) - i128::from(b)) as f64
}
fn blend(a: f64, b: f64, weight: f64) -> f64 {
    a + (b - a) * weight
}
fn ease(value: f64) -> f64 {
    (1.0 - (std::f64::consts::PI * value.clamp(0.0, 1.0)).cos()) * 0.5
}

fn transition(left: &PitchNote, right: &PitchNote, tick: i64) -> f64 {
    let gap = distance(right.start, left.end);
    if gap > 0.0 {
        let phase = (distance(tick, left.end) / gap - 0.5) * 20.0;
        return blend(left.pitch, right.pitch, 1.0 / (1.0 + (-phase).exp()));
    }
    let t = distance(tick, left.end) - right.offset;
    let phase = t * 3.0 / (left.right + right.left);
    let base = blend(left.pitch, right.pitch, 1.0 / (1.0 + (-phase).exp()));
    let u = t / left.right + 1.5;
    let v = t / right.left - 1.5;
    let left_depth = if right.pitch > left.pitch {
        -left.right_depth
    } else {
        left.right_depth
    };
    let right_depth = if right.pitch > left.pitch {
        right.left_depth
    } else {
        -right.left_depth
    };
    base + (-0.5 * u * u).exp() * left_depth + (-0.5 * v * v).exp() * right_depth
}
