use crate::model::{Automation, Note};

pub(super) fn envelope(curve: Option<&Automation>, note: &Note, frames: usize) -> Vec<f32> {
    if frames > 120_000 { return Vec::new(); }
    let Some(curve) = curve.filter(|curve| !curve.points.is_empty()) else {
        return Vec::new();
    };
    (0..frames)
        .map(|frame| {
            let offset = (frame as f64 / frames as f64 * note.duration as f64).round() as i64;
            curve.sample(note.onset.saturating_add(offset), 1.0)
        })
        .collect()
}
