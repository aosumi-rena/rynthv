use crate::model::{Automation, Group, Note, PitchControl, QUARTER};
use std::collections::BTreeMap;

/// Keep drag previews independent of redraw frequency.
pub struct NoteMovement {
    parameters: BTreeMap<String, Automation>,
    modes: BTreeMap<String, Automation>,
    pitch: Vec<PitchControl>,
    copied_pitch_ids: Vec<String>,
    ranges: Vec<(i64, i64)>,
    copy: bool,
}

impl NoteMovement {
    pub fn capture(group: &Group, notes: &[Note], copy: bool) -> Self {
        let mut ranges: Vec<_> = notes
            .iter()
            .map(|n| {
                (
                    n.onset.saturating_sub(QUARTER / 2),
                    n.onset
                        .saturating_add(n.duration)
                        .saturating_add(QUARTER / 2),
                )
            })
            .collect();
        ranges.sort_unstable();
        let mut merged: Vec<(i64, i64)> = Vec::new();
        for (begin, end) in ranges {
            if let Some(last) = merged.last_mut()
                && begin <= last.1
            {
                last.1 = last.1.max(end);
            } else {
                merged.push((begin, end));
            }
        }
        let copied_pitch_ids = group
            .pitch_controls
            .iter()
            .map(|_| (uuid::Uuid::new_v4().as_u128() as u64).to_string())
            .collect();
        Self {
            parameters: group.parameters.clone(),
            modes: group.vocal_modes.clone(),
            copied_pitch_ids,
            pitch: group.pitch_controls.clone(),
            ranges: merged,
            copy,
        }
    }

    pub fn apply(&self, group: &mut Group, delta: i64, pitch: i32) {
        group.parameters = self.parameters.clone();
        group.vocal_modes = self.modes.clone();
        group.pitch_controls = self.pitch.clone();
        if delta == 0 && pitch == 0 {
            return;
        }
        if delta != 0 {
            for (key, curve) in &mut group.parameters {
                let default = crate::model::Parameter::ALL
                    .iter()
                    .find(|p| p.key() == key)
                    .map_or(if key == "growl_frequency" { 60.0 } else { 0.0 }, |p| {
                        p.default_value()
                    }) as f32;
                move_curve(curve, &self.ranges, delta, self.copy, default);
            }
            for curve in group.vocal_modes.values_mut() {
                move_curve(curve, &self.ranges, delta, self.copy, 0.0);
            }
        }
        for (index, source) in self.pitch.iter().enumerate() {
            let range = match source {
                PitchControl::Point(p) => Some((p.pos, p.pos)),
                PitchControl::Curve(c) => c.range(),
                PitchControl::Unsupported(_) => None,
            };
            if !range.is_some_and(|(left, right)| {
                self.ranges.iter().any(|&(b, e)| left <= e && right >= b)
            }) {
                continue;
            }
            let mut control = source.clone();
            let (position, value, id) = match &mut control {
                PitchControl::Point(p) => (&mut p.pos, &mut p.pitch, &mut p.id),
                PitchControl::Curve(c) => (&mut c.pos, &mut c.pitch, &mut c.id),
                PitchControl::Unsupported(_) => continue,
            };
            *position = position.saturating_add(delta);
            *value += f64::from(pitch);
            if self.copy {
                *id = self.copied_pitch_ids[index].clone();
                group.pitch_controls.push(control);
            } else {
                group.pitch_controls[index] = control;
            }
        }
    }
}

fn move_curve(curve: &mut Automation, ranges: &[(i64, i64)], delta: i64, copy: bool, default: f32) {
    if curve.points.is_empty() {
        return;
    }
    let original = curve.clone();
    let inside = |x: i64| ranges.iter().any(|&(b, e)| b <= x && x <= e);
    let mut background = original.clone();
    if !copy {
        background.points.retain(|p| !inside(p.0));
    }
    let mut result: BTreeMap<i64, f64> = background.points.iter().copied().collect();
    for &(begin, end) in ranges {
        let (Some(left), Some(right)) = (begin.checked_add(delta), end.checked_add(delta)) else {
            continue;
        };
        result.retain(|&x, _| x < left || x >= right);
        // Preserve the curve outside the moved selection.
        for &(time, value) in &original.points {
            if begin <= time && time <= end {
                result.insert(time.saturating_add(delta), value);
            }
        }
        for time in [
            begin.saturating_add(QUARTER / 8),
            end.saturating_sub(QUARTER / 8).saturating_sub(1),
        ] {
            result.insert(
                time.saturating_add(delta),
                f64::from(original.sample(time, default)),
            );
        }
        result.insert(left, f64::from(background.sample(left, default)));
        result.insert(
            right.saturating_sub(1),
            f64::from(background.sample(right.saturating_sub(1), default)),
        );
    }
    curve.points = result.into_iter().collect();
}
