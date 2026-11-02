use super::*;
use std::collections::BTreeMap;

pub(super) fn paste(
    group: &mut Group,
    data: &Content,
    delta: i64,
    minimum: i64,
) -> Result<usize, String> {
    let mut count = 0;
    let mut notes: Vec<_> = group.notes.iter().cloned().map(|n| (n, false)).collect();
    for source in &data.notes {
        let mut note = source.clone();
        note.onset = note
            .onset
            .checked_add(delta)
            .ok_or("粘贴位置超出可用范围。")?;
        notes.push((note, true));
    }
    let ranges = super::regions::selected(notes);
    for (key, source) in &data.parameters {
        let target = if let Some(mode) = key.strip_prefix("vocalMode_") {
            group.vocal_modes.entry(mode.into()).or_default()
        } else {
            group.parameters.entry(key.clone()).or_default()
        };
        let mut points: BTreeMap<_, _> = target.points.iter().copied().collect();
        if !data.notes.is_empty() {
            let default = Parameter::ALL
                .iter()
                .find(|p| p.key() == key)
                .map_or(0.0, |p| p.default_value()) as f32;
            for &(left, end) in &ranges {
                let right = end.checked_sub(1).ok_or("粘贴位置超出可用范围。")?;
                points.retain(|&position, _| position < left || position >= end);
                points.insert(left, f64::from(target.sample(left, default)));
                points.insert(right, f64::from(target.sample(right, default)));
            }
        }
        for &(position, value) in &source.points {
            let position = position
                .checked_add(delta)
                .ok_or("粘贴位置超出可用范围。")?;
            if position < minimum {
                continue;
            }
            points.insert(position, value);
            count += 1;
        }
        target.points = points.into_iter().collect();
        target.validate()?;
    }
    for source in &data.pitch_controls {
        let mut control = source.clone();
        shift_control(&mut control, delta, 0, true)?;
        group.pitch_controls.push(control);
        count += 1;
    }
    Ok(count)
}

pub(super) fn shift_control(
    control: &mut PitchControl,
    delta: i64,
    pitch_offset: i32,
    renew: bool,
) -> Result<(), String> {
    let (pos, pitch, id) = match control {
        PitchControl::Point(p) => (&mut p.pos, &mut p.pitch, &mut p.id),
        PitchControl::Curve(c) => (&mut c.pos, &mut c.pitch, &mut c.id),
        _ => return Err("剪贴板音高控制点无效。".into()),
    };
    *pos = pos.checked_add(delta).ok_or("粘贴位置超出可用范围。")?;
    *pitch += f64::from(pitch_offset);
    if !pitch.is_finite() {
        return Err("剪贴板音高控制点无效。".into());
    }
    if renew {
        *id = (uuid::Uuid::new_v4().as_u128() as u64).to_string();
    }
    Ok(())
}
