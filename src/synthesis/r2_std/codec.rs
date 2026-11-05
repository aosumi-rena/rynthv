// SPDX-License-Identifier: MIT
use super::{Error, Result, archive::Reader};

pub(super) fn quantized(code: u32, low: f32, high: f32, word: bool) -> Result<f32> {
    if code > if word { 65535 } else { 255 } || !low.is_finite() || !high.is_finite() {
        return Err(Error::Format("quantized source range"));
    }
    let delta = high - low;
    let value = ((f64::from(delta) * f64::from(code))
        * if word { 1.0 / 65535.0 } else { 1.0 / 255.0 }
        + f64::from(low)) as f32;
    if !value.is_finite() {
        return Err(Error::Format("nonfinite quantized source"));
    }
    Ok(value)
}

pub(super) fn predictor8(
    reader: &mut Reader<'_>,
    low: f32,
    high: f32,
    count: usize,
) -> Result<Vec<f32>> {
    if count > 4096 {
        return Err(Error::Unsupported("predictor vector exceeds 4096"));
    }
    let groups = count.div_ceil(8);
    let mut codes = vec![0u16; (groups * 8).max(1)];
    codes[0] = reader.u16()?;
    let mut tags = 0u8;
    for group in 0..groups {
        if group % 2 == 0 {
            tags = reader.u8()?;
        }
        let width = if group % 2 == 0 { tags >> 4 } else { tags & 15 } as usize;
        let packed = reader.take(width)?;
        for lane in 0..8 {
            let mut code = 0i32;
            for bit in 0..width {
                let index = lane * width + bit;
                code = (code << 1) | i32::from((packed[index / 8] >> (7 - index % 8)) & 1);
            }
            let i = group * 8 + lane;
            codes[i] = if width == 8 {
                code as u16
            } else {
                let a = f32::from(codes[i.saturating_sub(1)]) * f32::from_bits(0x3fd33333);
                let b = f32::from(codes[i.saturating_sub(2)]) * f32::from_bits(0x3f266666);
                let residual = (code >> 1) ^ -(code & 1);
                ((a - b).round() as i32).wrapping_add(residual) as u16
            };
        }
    }
    let delta = high - low;
    codes
        .into_iter()
        .take(count)
        .map(|code| {
            let value = ((f32::from(code) * f32::from_bits(0x3b808081)) * delta) + low;
            if value.is_finite() {
                Ok(value)
            } else {
                Err(Error::Format("nonfinite predictor output"))
            }
        })
        .collect()
}
