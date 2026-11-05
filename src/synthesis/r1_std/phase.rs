// SPDX-License-Identifier: MIT
use std::cmp::Ordering;
use super::{mix::mix, word32::StdWord32, word64::{StdWideError, StdWord64}};

const HALF32: u32 = 0x3f00_0000;
const PI64: u64 = 0x4009_21fb_5444_2d18;

pub(crate) fn phase(a: StdWord32, b: StdWord32, ratio: StdWord32) -> Result<StdWord32, StdWideError> {
    let difference = b.sub(a)?;
    let half = difference.mul(StdWord32::from_bits(HALF32)?)?;
    let wide = StdWord64::widen(half)?;
    let quotient = wide.div(StdWord64::from_bits(PI64)?)?;
    let absolute = quotient.abs();
    let turns = absolute.round_away()?;
    let adjust_b = a.numeric_cmp(b) == Ordering::Greater;
    let moving = if adjust_b { b } else { a };
    let moving_wide = StdWord64::widen(moving)?;
    let twice = turns.add(turns)?;
    let correction = twice.mul(StdWord64::from_bits(PI64)?)?;
    let adjusted = moving_wide.add(correction)?;
    let moved = adjusted.narrow()?;
    Ok(if adjust_b { mix(a, moved, ratio)? } else { mix(moved, b, ratio)? })
}
