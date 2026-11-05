// SPDX-License-Identifier: MIT
use super::{log2, StdNumericError, StdWord32};
use std::cmp::Ordering;

const TEN: u32 = 0x4120_0000;
const LOG2E: u32 = 0x3fb8_aa3b;
const LOWER: u32 = 0xc2fc_0000;
const UPPER: u32 = 0x42c8_0000;
const OFFSET: u32 = 0x42f2_8c51;
const NUMERATOR: u32 = 0x41dd_d2fe;
const DENOMINATOR: u32 = 0x409a_f5f8;
const LINEAR: u32 = 0x3fbe_bc8d;
const ENCODED_SCALE: u32 = 0x4b00_0000;
const NOISE_LOG_SCALE: u32 = 0x40dd_ce9e;

pub(crate) fn noise_power(value: StdWord32) -> Result<u32, StdNumericError> {
    let normalized = value.div(StdWord32::from_bits(TEN)?)?;
    let scaled = normalized.mul(StdWord32::from_bits(LOG2E)?)?;
    let x = StdWord32::from_bits(LOWER)?.maximum(scaled);
    if x.numeric_cmp(StdWord32::from_bits(UPPER)?) == Ordering::Greater {
        return Err(StdNumericError::UnsupportedNoisePowerInput { bits: value.bits() });
    }
    let negative = x.numeric_cmp(StdWord32::from_bits(0)?) == Ordering::Less;
    let sign = StdWord32::from_i32(if negative { 1 } else { 0 })?;
    let integral = x.trunc_i32()?;
    let whole = StdWord32::from_i32(integral)?;
    let fractional = x.sub(whole)?;
    let fraction = fractional.add(sign)?;
    let shifted = x.add(StdWord32::from_bits(OFFSET)?)?;
    let denominator = StdWord32::from_bits(DENOMINATOR)?.sub(fraction)?;
    let quotient = StdWord32::from_bits(NUMERATOR)?.div(denominator)?;
    let total = shifted.add(quotient)?;
    let linear = fraction.mul(StdWord32::from_bits(LINEAR)?)?;
    let adjusted = total.sub(linear)?;
    let encoded = adjusted.mul(StdWord32::from_bits(ENCODED_SCALE)?)?;
    let integer = encoded.trunc_i32()?;
    Ok(u32::from_le_bytes(integer.to_le_bytes()))
}

pub(crate) fn noise_log(value: StdWord32) -> Result<StdWord32, StdNumericError> {
    let bit_log = log2(value)?;
    bit_log.mul(StdWord32::from_bits(NOISE_LOG_SCALE)?)
}
