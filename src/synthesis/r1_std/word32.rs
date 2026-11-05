// SPDX-License-Identifier: MIT

use std::{cmp::Ordering, error::Error, fmt};

const SIGN: u32 = 0x8000_0000;
const FRACTION: u32 = 0x007f_ffff;
const EXPONENT: u32 = 0x7f80_0000;
const HIDDEN: u32 = 1 << 23;
const CARRY: u32 = 1 << 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StdWord32(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StdNumericError {
    NonfiniteInput { bits: u32 },
    UnsupportedRatio { bits: u32 },
    TruncateI32Overflow { bits: u32 },
    UnsupportedNoisePowerInput { bits: u32 },
    UnsupportedLogInput { bits: u32 },
    ZeroDivisor { bits: u32 },
    FiniteOverflow,
    IntegerBound,
}

impl fmt::Display for StdNumericError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonfiniteInput { bits } => write!(f, "nonfinite binary32 input {bits:08x}"),
            Self::UnsupportedRatio { bits } => write!(f, "unsupported ratio word {bits:08x}"),
            Self::TruncateI32Overflow { bits } => write!(f, "checked i32 truncation overflow word {bits:08x}"),
            Self::UnsupportedNoisePowerInput { bits } => write!(f, "unsupported noise power input word {bits:08x}"),
            Self::UnsupportedLogInput { bits } => write!(f, "unsupported log2 input word {bits:08x}"),
            Self::ZeroDivisor { bits } => write!(f, "zero divisor word {bits:08x}"),
            Self::FiniteOverflow => f.write_str("finite binary32 result overflow"),
            Self::IntegerBound => f.write_str("binary32 integer representation bound exceeded"),
        }
    }
}

impl Error for StdNumericError {}

#[derive(Clone, Copy)]
struct Parts {
    negative: bool,
    significand: u32,
    exponent: i32,
}

impl StdWord32 {
    pub(crate) fn numeric_cmp(self, rhs: Self) -> Ordering {
        if (self.0 & !SIGN) == 0 && (rhs.0 & !SIGN) == 0 {
            return Ordering::Equal;
        }
        let left_negative = (self.0 & SIGN) != 0;
        let right_negative = (rhs.0 & SIGN) != 0;
        if left_negative != right_negative {
            if left_negative { Ordering::Less } else { Ordering::Greater }
        } else if left_negative {
            rhs.0.cmp(&self.0)
        } else {
            self.0.cmp(&rhs.0)
        }
    }

    pub(crate) fn maximum(self, rhs: Self) -> Self {
        if (self.0 & !SIGN) == 0 && (rhs.0 & !SIGN) == 0 {
            return Self(self.0 & rhs.0);
        }
        if self.numeric_cmp(rhs) == Ordering::Less { rhs } else { self }
    }

    pub(crate) fn trunc_i32(self) -> Result<i32, StdNumericError> {
        let p = self.parts();
        if p.significand == 0 {
            return Ok(0);
        }
        let magnitude = if p.exponent >= 0 {
            if p.exponent > 31 {
                return Err(StdNumericError::TruncateI32Overflow { bits: self.0 });
            }
            u64::from(p.significand) << p.exponent as u32
        } else {
            let shift = (-p.exponent) as u32;
            if shift >= 64 { 0 } else { u64::from(p.significand) >> shift }
        };
        let limit = if p.negative { 0x8000_0000_u64 } else { 0x7fff_ffff_u64 };
        if magnitude > limit {
            return Err(StdNumericError::TruncateI32Overflow { bits: self.0 });
        }
        let signed = if p.negative { -(magnitude as i64) } else { magnitude as i64 };
        i32::try_from(signed).map_err(|_| StdNumericError::TruncateI32Overflow { bits: self.0 })
    }

    pub(crate) fn from_i32(value: i32) -> Result<Self, StdNumericError> {
        let negative = value < 0;
        let wide = i64::from(value);
        let magnitude = (if negative { -wide } else { wide }) as u64;
        pack(negative, Magnitude::shifted(magnitude, 0)?, 0)
    }

    pub(crate) fn from_bits(bits: u32) -> Result<Self, StdNumericError> {
        if bits & EXPONENT == EXPONENT {
            return Err(StdNumericError::NonfiniteInput { bits });
        }
        Ok(Self(bits))
    }

    pub(crate) const fn bits(self) -> u32 {
        self.0
    }

    fn parts(self) -> Parts {
        let field = (self.0 >> 23) & 255;
        Parts {
            negative: self.0 & SIGN != 0,
            significand: (self.0 & FRACTION) | if field == 0 { 0 } else { HIDDEN },
            exponent: if field == 0 { -149 } else { field as i32 - 150 },
        }
    }

    pub(crate) fn add(self, rhs: Self) -> Result<Self, StdNumericError> {
        Self::add_parts(self.parts(), rhs.parts())
    }

    pub(crate) fn sub(self, rhs: Self) -> Result<Self, StdNumericError> {
        let mut right = rhs.parts();
        right.negative = !right.negative;
        Self::add_parts(self.parts(), right)
    }

    pub(crate) fn mul(self, rhs: Self) -> Result<Self, StdNumericError> {
        let left = self.parts();
        let right = rhs.parts();
        let product = u64::from(left.significand) * u64::from(right.significand);
        pack(
            left.negative ^ right.negative,
            Magnitude::shifted(product, 0)?,
            left.exponent + right.exponent,
        )
    }

    pub(crate) fn div(self, rhs: Self) -> Result<Self, StdNumericError> {
        let left = self.parts();
        let right = rhs.parts();
        if right.significand == 0 {
            return Err(StdNumericError::ZeroDivisor { bits: rhs.0 });
        }
        let sign = ((left.negative ^ right.negative) as u32) << 31;
        if left.significand == 0 {
            return Ok(Self(sign));
        }
        let mut ratio_exponent = (32 - left.significand.leading_zeros()) as i32
            - (32 - right.significand.leading_zeros()) as i32;
        let below = if ratio_exponent >= 0 {
            u64::from(left.significand) < (u64::from(right.significand) << ratio_exponent as u32)
        } else {
            (u64::from(left.significand) << (-ratio_exponent) as u32) < u64::from(right.significand)
        };
        if below {
            ratio_exponent -= 1;
        }
        let difference = left.exponent - right.exponent;
        let mut top_exponent = difference + ratio_exponent;
        if top_exponent < -126 {
            let significand = rounded_ratio(left.significand, right.significand, difference + 149)?;
            if significand > HIDDEN {
                return Err(StdNumericError::IntegerBound);
            }
            return Ok(Self(sign | significand));
        }
        let mut significand = rounded_ratio(left.significand, right.significand, 23 - ratio_exponent)?;
        if significand == CARRY {
            significand >>= 1;
            top_exponent += 1;
        }
        if top_exponent > 127 {
            return Err(StdNumericError::FiniteOverflow);
        }
        if !(HIDDEN..CARRY).contains(&significand) {
            return Err(StdNumericError::IntegerBound);
        }
        Ok(Self(sign | (((top_exponent + 127) as u32) << 23) | (significand - HIDDEN)))
    }

    fn add_parts(left: Parts, right: Parts) -> Result<Self, StdNumericError> {
        let exponent = left.exponent.min(right.exponent);
        let a = Magnitude::shifted(u64::from(left.significand), (left.exponent - exponent) as u32)?;
        let b = Magnitude::shifted(u64::from(right.significand), (right.exponent - exponent) as u32)?;
        let (negative, magnitude) = if left.negative == right.negative {
            (left.negative, a.add(b)?)
        } else {
            match a.compare(b) {
                Ordering::Greater => (left.negative, a.sub(b)?),
                Ordering::Less => (right.negative, b.sub(a)?),
                Ordering::Equal => (false, Magnitude::ZERO),
            }
        };
        if magnitude.bit_length() == 0 {
            return Ok(Self(((left.negative && right.negative) as u32) << 31));
        }
        pack(negative, magnitude, exponent)
    }
}

#[derive(Clone, Copy)]
struct Magnitude([u64; 5]);

impl Magnitude {
    const ZERO: Self = Self([0; 5]);

    fn shifted(value: u64, shift: u32) -> Result<Self, StdNumericError> {
        if shift >= 320 {
            return Err(StdNumericError::IntegerBound);
        }
        let mut out = Self::ZERO;
        let limb = (shift / 64) as usize;
        let offset = shift % 64;
        out.0[limb] = value << offset;
        if offset != 0 {
            let high = value >> (64 - offset);
            if high != 0 {
                if limb + 1 >= 5 {
                    return Err(StdNumericError::IntegerBound);
                }
                out.0[limb + 1] = high;
            }
        }
        Ok(out)
    }

    fn compare(self, rhs: Self) -> Ordering {
        for i in (0..5).rev() {
            let order = self.0[i].cmp(&rhs.0[i]);
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }

    fn add(self, rhs: Self) -> Result<Self, StdNumericError> {
        let mut out = Self::ZERO;
        let mut carry = false;
        for i in 0..5 {
            let (sum, first) = self.0[i].overflowing_add(rhs.0[i]);
            let (sum, second) = sum.overflowing_add(carry as u64);
            out.0[i] = sum;
            carry = first || second;
        }
        if carry {
            return Err(StdNumericError::IntegerBound);
        }
        Ok(out)
    }

    fn sub(self, rhs: Self) -> Result<Self, StdNumericError> {
        let mut out = Self::ZERO;
        let mut borrow = false;
        for i in 0..5 {
            let (difference, first) = self.0[i].overflowing_sub(rhs.0[i]);
            let (difference, second) = difference.overflowing_sub(borrow as u64);
            out.0[i] = difference;
            borrow = first || second;
        }
        if borrow {
            return Err(StdNumericError::IntegerBound);
        }
        Ok(out)
    }

    fn bit_length(self) -> u32 {
        for i in (0..5).rev() {
            if self.0[i] != 0 {
                return i as u32 * 64 + 64 - self.0[i].leading_zeros();
            }
        }
        0
    }

    fn bit(self, index: u32) -> bool {
        index < 320 && ((self.0[(index / 64) as usize] >> (index % 64)) & 1) != 0
    }

    fn any_below(self, exclusive: u32) -> bool {
        (0..exclusive.min(320)).any(|i| self.bit(i))
    }

    fn any_from(self, inclusive: u32) -> bool {
        (inclusive.min(320)..320).any(|i| self.bit(i))
    }

    fn rounded_integer(self, scale: i32) -> Result<u32, StdNumericError> {
        if scale >= 0 {
            let shift = scale as u32;
            if self.bit_length() + shift > 25 {
                return Err(StdNumericError::IntegerBound);
            }
            let mut result = 0_u32;
            for i in 0..self.bit_length() {
                if self.bit(i) {
                    result |= 1_u32 << (i + shift);
                }
            }
            return Ok(result);
        }
        let right = (-scale) as u32;
        if self.any_from(right + 25) {
            return Err(StdNumericError::IntegerBound);
        }
        let mut quotient = 0_u32;
        for i in 0..25 {
            if self.bit(right + i) {
                quotient |= 1_u32 << i;
            }
        }
        let guard = self.bit(right - 1);
        let sticky = self.any_below(right - 1);
        if guard && (sticky || quotient & 1 != 0) {
            quotient = quotient.checked_add(1).ok_or(StdNumericError::IntegerBound)?;
        }
        Ok(quotient)
    }
}

fn pack(negative: bool, magnitude: Magnitude, exponent: i32) -> Result<StdWord32, StdNumericError> {
    let sign = (negative as u32) << 31;
    let length = magnitude.bit_length();
    if length == 0 {
        return Ok(StdWord32(sign));
    }
    let mut top_exponent = length as i32 - 1 + exponent;
    if top_exponent < -126 {
        let significand = magnitude.rounded_integer(exponent + 149)?;
        if significand > HIDDEN {
            return Err(StdNumericError::IntegerBound);
        }
        return Ok(StdWord32(sign | significand));
    }
    let mut significand = magnitude.rounded_integer(exponent - (top_exponent - 23))?;
    if significand == CARRY {
        significand >>= 1;
        top_exponent += 1;
    }
    if top_exponent > 127 {
        return Err(StdNumericError::FiniteOverflow);
    }
    if !(HIDDEN..CARRY).contains(&significand) {
        return Err(StdNumericError::IntegerBound);
    }
    let field = (top_exponent + 127) as u32;
    Ok(StdWord32(sign | (field << 23) | (significand - HIDDEN)))
}

fn rounded_ratio(numerator: u32, denominator: u32, scale: i32) -> Result<u32, StdNumericError> {
    if numerator == 0 || denominator == 0 || numerator >= CARRY || denominator >= CARRY
        || !(-104..=47).contains(&scale)
    {
        return Err(StdNumericError::IntegerBound);
    }
    let (n, d) = if scale >= 0 {
        (u128::from(numerator).checked_shl(scale as u32).ok_or(StdNumericError::IntegerBound)?, u128::from(denominator))
    } else {
        (u128::from(numerator), u128::from(denominator).checked_shl((-scale) as u32).ok_or(StdNumericError::IntegerBound)?)
    };
    let mut quotient = n / d;
    let remainder = n % d;
    let complement = d - remainder;
    if remainder > complement || (remainder == complement && quotient & 1 != 0) {
        quotient = quotient.checked_add(1).ok_or(StdNumericError::IntegerBound)?;
    }
    u32::try_from(quotient).map_err(|_| StdNumericError::IntegerBound)
}

