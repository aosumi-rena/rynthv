use super::*;
use std::collections::BTreeMap;

impl Editor {
    pub(super) fn capture_note_selection(
        &self,
        parameters: bool,
    ) -> Result<Option<Content>, String> {
        let Some(reference) = self.active_ref() else {
            return Ok(None);
        };
        let mut notes: Vec<_> = self
            .notes()
            .iter()
            .filter(|n| self.selected.contains(&n.id))
            .cloned()
            .collect();
        if notes.is_empty() {
            return Ok(None);
        }
        let mut data = Content {
            position_set: true,
            ..Default::default()
        };
        if parameters {
            let group = self.group().unwrap();
            let ranges = super::regions::selected(
                group
                    .notes
                    .iter()
                    .map(|n| (n.clone(), self.selected.contains(&n.id)))
                    .collect(),
            );
            for (key, curve) in group.parameters.iter().map(|(k, v)| (k.clone(), v)).chain(
                group
                    .vocal_modes
                    .iter()
                    .map(|(k, v)| (format!("vocalMode_{k}"), v)),
            ) {
                if curve.points.is_empty() {
                    continue;
                }
                let mut points = BTreeMap::new();
                for &(begin, end) in &ranges {
                    points.extend(
                        curve
                            .points
                            .iter()
                            .copied()
                            .filter(|p| begin <= p.0 && p.0 <= end),
                    );
                }
                let mut captured = curve.clone();
                captured.points = points.into_iter().collect();
                data.parameters.insert(key, captured);
            }
            for control in &group.pitch_controls {
                let range = match control {
                    PitchControl::Point(p) => Some((p.pos, p.pos)),
                    PitchControl::Curve(c) => c.range(),
                    _ => None,
                };
                if range.is_some_and(|(l, r)| ranges.iter().any(|&(b, e)| l <= e && r >= b)) {
                    let mut control = control.clone();
                    curves::shift_control(&mut control, reference.blick_offset, 0, false)?;
                    data.pitch_controls.push(control);
                }
            }
        }
        for note in &mut notes {
            note.onset = note
                .onset
                .checked_add(reference.blick_offset)
                .ok_or("粘贴位置超出可用范围。")?;
        }
        for curve in data.parameters.values_mut() {
            for (position, _) in &mut curve.points {
                *position = position
                    .checked_add(reference.blick_offset)
                    .ok_or("粘贴位置超出可用范围。")?;
            }
        }
        notes.sort_by_key(|n| (n.onset, n.pitch, n.id));
        data.notes = notes;
        data.validate()?;
        Ok(Some(data))
    }
}
