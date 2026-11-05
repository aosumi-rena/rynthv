// SPDX-License-Identifier: MIT
use super::{StdNumericError, StdWord32};

const LOG_SCALE: u32 = 0x3400_0000;
const LOG_OFFSET: u32 = 0x42f8_7377;
const LOG_LINEAR: u32 = 0x3fbf_bf75;
const LOG_NUMERATOR: u32 = 0x3fdc_e9a3;
const LOG_DENOMINATOR: u32 = 0x3eb4_44f9;

pub(crate) fn log2(value: StdWord32) -> Result<StdWord32, StdNumericError> {
    let bits = value.bits();
    if !(0x0080_0000..0x7f80_0000).contains(&bits) {
        return Err(StdNumericError::UnsupportedLogInput { bits });
    }
    let mantissa = StdWord32::from_bits((bits & 0x007f_ffff) + 0x3f00_0000)?;
    let rounded_bits = StdWord32::from_i32(bits as i32)?;
    let scaled = rounded_bits.mul(StdWord32::from_bits(LOG_SCALE)?)?;
    let shifted = scaled.sub(StdWord32::from_bits(LOG_OFFSET)?)?;
    let linear = StdWord32::from_bits(LOG_LINEAR)?.mul(mantissa)?;
    let left = shifted.sub(linear)?;
    let denominator = mantissa.add(StdWord32::from_bits(LOG_DENOMINATOR)?)?;
    let quotient = StdWord32::from_bits(LOG_NUMERATOR)?.div(denominator)?;
    left.sub(quotient)
}
