use crate::synthesis::voice::{Result, VoiceError};

pub(super) fn bank(value: f32) -> Result<[f32; 4]> {
    let mut result = [0.0; 4];
    for (lane, output) in result.iter_mut().enumerate() {
        let shift = (lane as f32 - 1.0) * 0.25;
        let angle = (value - shift) * std::f32::consts::TAU;
        *output = (cosine(angle)? + 1.0) * 0.5;
    }
    Ok(result)
}

fn cosine(value: f32) -> Result<f32> {
    let x = f64::from(value);
    let absolute = x.abs();
    if !x.is_finite() || absolute >= f64::from_bits(0x41e921fb60000000) {
        return Err(VoiceError::new("Acoustic phase exceeds its finite range"));
    }
    if absolute <= f64::from_bits(0x3fe921fb54442d18) {
        return Ok(if absolute < f64::from_bits(0x3f20000000000000) {
            1.0
        } else if absolute < f64::from_bits(0x3f80000000000000) {
            (-x * 0.5).mul_add(x, 1.0) as f32
        } else {
            cosine_polynomial(x) as f32
        });
    }
    let count = absolute.mul_add(f64::from_bits(0x3fe45f306dc9c883), 0.5) as i32;
    let high = (-(count as f64)).mul_add(f64::from_bits(0x3ff921fb54400000), absolute);
    let reduced = high - count as f64 * f64::from_bits(0x3dd0b4611a626331);
    let quadrant = count & 3;
    let mut result = if quadrant & 1 != 0 {
        sine_polynomial(reduced)
    } else {
        cosine_polynomial(reduced)
    };
    if ((quadrant + 1) >> 1) & 1 != 0 {
        result = -result;
    }
    Ok(result as f32)
}

fn cosine_polynomial(x: f64) -> f64 {
    let square = x * x;
    let leading = 1.0 - square * 0.5;
    let p = square.mul_add(f64::from_bits(0xbe927e4fb7789f5c), f64::from_bits(0x3efa01a01a01a019));
    let p = p.mul_add(square, f64::from_bits(0xbf56c16c16c16c16));
    let p = p.mul_add(square, f64::from_bits(0x3fa5555555555555));
    p.mul_add(square * square, leading)
}

fn sine_polynomial(x: f64) -> f64 {
    let square = x * x;
    let p = square.mul_add(f64::from_bits(0x3ec71de3a556c734), f64::from_bits(0xbf2a01a01a01a01a));
    let p = p.mul_add(square, f64::from_bits(0x3f81111111111111));
    let p = p.mul_add(square, f64::from_bits(0xbfc5555555555555));
    p.mul_add(x * square, x)
}
