// SPDX-License-Identifier: MIT
use super::{Result, directional_model::finite, lf::fast_sin};
pub(super) fn repair(rows: &mut [Vec<f32>]) -> Result<()> {
    let threshold = (0.15 * std::f64::consts::PI) as f32;
    for i in 1..rows.len().saturating_sub(1) {
        let count = rows[i]
            .len()
            .min(rows[i - 1].len())
            .min(rows[i + 1].len())
            .min(25);
        for j in 0..count {
            let (c, p, q) = (rows[i][j], rows[i - 1][j], rows[i + 1][j]);
            if distance(c, p).abs() <= threshold || distance(c, q).abs() <= threshold {
                continue;
            }
            let sp = fast_sin(p);
            let sq = fast_sin(q);
            let cp = fast_sin(p + f32::from_bits(0x3fc90fdb));
            let cq = fast_sin(q + f32::from_bits(0x3fc90fdb));
            rows[i][j] = (sp + (sq - sp) * 0.5).atan2(cp + (cq - cp) * 0.5);
        }
        finite(&rows[i])?;
    }
    Ok(())
}
fn distance(c: f32, p: f32) -> f32 {
    let mut adjusted = c;
    if p > c {
        let n = (f64::from((p - c) * 0.5) / std::f64::consts::PI + 1.0) as i32;
        adjusted = (n as f64 * std::f64::consts::TAU + f64::from(c)) as f32;
    }
    ((f64::from(adjusted - p) + std::f64::consts::PI) % std::f64::consts::TAU
        - std::f64::consts::PI) as f32
}
