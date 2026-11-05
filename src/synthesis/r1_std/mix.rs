// SPDX-License-Identifier: MIT
use super::word32::{StdNumericError, StdWord32};

pub(crate) fn mix(
    a: StdWord32,
    b: StdWord32,
    ratio: StdWord32,
) -> Result<StdWord32, StdNumericError> {
    if ratio.bits() > 0x3f80_0000 {
        return Err(StdNumericError::UnsupportedRatio { bits: ratio.bits() });
    }
    let delta = b.sub(a)?;
    let weighted = delta.mul(ratio)?;
    a.add(weighted)
}
