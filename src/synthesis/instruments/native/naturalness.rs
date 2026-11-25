pub(super) fn pitch(players: usize) -> f32 {
    let value = (0.5f64.powi(players as i32 - 1) * 0.3 + 0.2) as f32;
    value * 0.5
}

pub(super) fn timbre(players: usize) -> f32 {
    (0.5f64.powi(players as i32) as f32) * 0.5
}
