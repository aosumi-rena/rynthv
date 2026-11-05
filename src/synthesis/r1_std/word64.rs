// SPDX-License-Identifier: MIT

use std::{cmp::Ordering, error::Error, fmt};
use super::word32::{StdNumericError, StdWord32};

const SIGN: u64 = 0x8000_0000_0000_0000;
const FRACTION: u64 = 0x000f_ffff_ffff_ffff;
const EXPONENT: u64 = 0x7ff0_0000_0000_0000;
const HIDDEN: u64 = 1 << 52;
const CARRY: u64 = 1 << 53;
const LIMBS: usize = 34;
const CAPACITY: u32 = 2176;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StdWord64(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StdWideError {
    Word32(StdNumericError),
    NonfiniteInput64 { bits: u64 },
    ZeroDivisor64 { bits: u64 },
    RoundAwayBound64 { bits: u64 },
    FiniteOverflow64,
    FiniteOverflow32,
    IntegerBound64,
}

impl From<StdNumericError> for StdWideError {
    fn from(value: StdNumericError) -> Self { Self::Word32(value) }
}

impl fmt::Display for StdWideError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Word32(error) => write!(f, "binary32: {error}"),
            Self::NonfiniteInput64 { bits } => write!(f, "nonfinite binary64 input {bits:016x}"),
            Self::ZeroDivisor64 { bits } => write!(f, "zero binary64 divisor {bits:016x}"),
            Self::RoundAwayBound64 { bits } => write!(f, "round-away magnitude exceeds 2^31 {bits:016x}"),
            Self::FiniteOverflow64 => f.write_str("finite binary64 result overflow"),
            Self::FiniteOverflow32 => f.write_str("finite binary32 narrowing overflow"),
            Self::IntegerBound64 => f.write_str("binary64 integer representation bound exceeded"),
        }
    }
}

impl Error for StdWideError {}

#[derive(Clone, Copy)]
struct Parts { negative: bool, significand: u64, exponent: i32 }

impl StdWord64 {
    pub(crate) fn from_bits(bits: u64) -> Result<Self, StdWideError> {
        if bits & EXPONENT == EXPONENT {
            return Err(StdWideError::NonfiniteInput64 { bits });
        }
        Ok(Self(bits))
    }

    pub(crate) const fn bits(self) -> u64 { self.0 }

    fn parts(self) -> Parts {
        let field = (self.0 >> 52) & 2047;
        Parts {
            negative: self.0 & SIGN != 0,
            significand: (self.0 & FRACTION) | if field == 0 { 0 } else { HIDDEN },
            exponent: if field == 0 { -1074 } else { field as i32 - 1075 },
        }
    }

    pub(crate) fn add(self, rhs: Self) -> Result<Self, StdWideError> {
        Self::add_parts(self.parts(), rhs.parts())
    }

    pub(crate) fn sub(self, rhs: Self) -> Result<Self, StdWideError> {
        let mut right = rhs.parts();
        right.negative = !right.negative;
        Self::add_parts(self.parts(), right)
    }

    fn add_parts(left: Parts, right: Parts) -> Result<Self, StdWideError> {
        let exponent = left.exponent.min(right.exponent);
        let a = Magnitude::shifted(left.significand, (left.exponent - exponent) as u32)?;
        let b = Magnitude::shifted(right.significand, (right.exponent - exponent) as u32)?;
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
            return Ok(Self(((left.negative && right.negative) as u64) << 63));
        }
        pack64(negative, magnitude, exponent)
    }

    pub(crate) fn mul(self, rhs: Self) -> Result<Self, StdWideError> {
        let a = self.parts();
        let b = rhs.parts();
        let product = u128::from(a.significand) * u128::from(b.significand);
        pack64(a.negative ^ b.negative, Magnitude::from_u128(product), a.exponent + b.exponent)
    }

    pub(crate) fn div(self, rhs: Self) -> Result<Self, StdWideError> {
        let a = self.parts();
        let b = rhs.parts();
        if b.significand == 0 { return Err(StdWideError::ZeroDivisor64 { bits: rhs.0 }); }
        let sign = ((a.negative ^ b.negative) as u64) << 63;
        if a.significand == 0 { return Ok(Self(sign)); }
        let mut ratio_exponent = (64 - a.significand.leading_zeros()) as i32
            - (64 - b.significand.leading_zeros()) as i32;
        let below = if ratio_exponent >= 0 {
            u128::from(a.significand) < (u128::from(b.significand) << ratio_exponent as u32)
        } else {
            (u128::from(a.significand) << (-ratio_exponent) as u32) < u128::from(b.significand)
        };
        if below { ratio_exponent -= 1; }
        let difference = a.exponent - b.exponent;
        let mut top_exponent = difference + ratio_exponent;
        if top_exponent < -1022 {
            let significand = rounded_ratio(a.significand, b.significand, difference + 1074)?;
            if significand > HIDDEN { return Err(StdWideError::IntegerBound64); }
            return Ok(Self(sign | significand));
        }
        let mut significand = rounded_ratio(a.significand, b.significand, 52 - ratio_exponent)?;
        if significand == CARRY { significand >>= 1; top_exponent += 1; }
        if top_exponent > 1023 { return Err(StdWideError::FiniteOverflow64); }
        if !(HIDDEN..CARRY).contains(&significand) { return Err(StdWideError::IntegerBound64); }
        Ok(Self(sign | (((top_exponent + 1023) as u64) << 52) | (significand - HIDDEN)))
    }

    pub(crate) fn widen(value: StdWord32) -> Result<Self, StdWideError> {
        let bits = value.bits();
        let field = (bits >> 23) & 255;
        let significand = (bits & 0x007f_ffff) | if field == 0 { 0 } else { 1 << 23 };
        let exponent = if field == 0 { -149 } else { field as i32 - 150 };
        pack64(bits >> 31 != 0, Magnitude::shifted(u64::from(significand), 0)?, exponent)
    }

    pub(crate) fn narrow(self) -> Result<StdWord32, StdWideError> {
        let p = self.parts();
        let magnitude = Magnitude::shifted(p.significand, 0)?;
        let sign = (p.negative as u32) << 31;
        let length = magnitude.bit_length();
        if length == 0 { return Ok(StdWord32::from_bits(sign)?); }
        let mut top_exponent = length as i32 - 1 + p.exponent;
        if top_exponent < -126 {
            let significand = magnitude.rounded_integer(p.exponent + 149, 25)?;
            if significand > (1 << 23) { return Err(StdWideError::IntegerBound64); }
            return Ok(StdWord32::from_bits(sign | significand as u32)?);
        }
        let mut significand = magnitude.rounded_integer(p.exponent - (top_exponent - 23), 25)?;
        if significand == (1 << 24) { significand >>= 1; top_exponent += 1; }
        if top_exponent > 127 { return Err(StdWideError::FiniteOverflow32); }
        if !((1 << 23)..(1 << 24)).contains(&significand) { return Err(StdWideError::IntegerBound64); }
        Ok(StdWord32::from_bits(sign | (((top_exponent + 127) as u32) << 23) | (significand as u32 - (1 << 23)))?)
    }

    pub(crate) const fn abs(self) -> Self { Self(self.0 & !SIGN) }

    pub(crate) fn round_away(self) -> Result<Self, StdWideError> {
        if self.0 & !SIGN > 0x41e0_0000_0000_0000 {
            return Err(StdWideError::RoundAwayBound64 { bits: self.0 });
        }
        let p = self.parts();
        if p.significand == 0 { return Ok(self); }
        if p.exponent >= 0 { return Err(StdWideError::IntegerBound64); }
        let right = (-p.exponent) as u32;
        let integer = if right >= 64 {
            0 // <=53-bit significand is strictly below half a unit here.
        } else {
            let quotient = p.significand >> right;
            let remainder = p.significand & ((1_u64 << right) - 1);
            quotient + u64::from(remainder >= (1_u64 << (right - 1)))
        };
        if integer > (1 << 31) { return Err(StdWideError::IntegerBound64); }
        pack64(p.negative, Magnitude::shifted(integer, 0)?, 0)
    }
}

#[derive(Clone, Copy)]
struct Magnitude([u64; LIMBS]);

impl Magnitude {
    const ZERO: Self = Self([0; LIMBS]);

    fn from_u128(value: u128) -> Self {
        let mut out = Self::ZERO;
        out.0[0] = value as u64;
        out.0[1] = (value >> 64) as u64;
        out
    }

    fn shifted(value: u64, shift: u32) -> Result<Self, StdWideError> {
        Self::from_u128(u128::from(value)).shift_left(shift)
    }

    fn shift_left(self, shift: u32) -> Result<Self, StdWideError> {
        let length = self.bit_length();
        if shift >= CAPACITY || length + shift > CAPACITY { return Err(StdWideError::IntegerBound64); }
        let mut out = Self::ZERO;
        for i in 0..length {
            if self.bit(i) {
                let at = i + shift;
                out.0[(at / 64) as usize] |= 1_u64 << (at % 64);
            }
        }
        Ok(out)
    }

    fn compare(self, rhs: Self) -> Ordering {
        for i in (0..LIMBS).rev() {
            let order = self.0[i].cmp(&rhs.0[i]);
            if order != Ordering::Equal { return order; }
        }
        Ordering::Equal
    }

    fn add(self, rhs: Self) -> Result<Self, StdWideError> {
        let mut out = Self::ZERO;let mut carry = false;
        for i in 0..LIMBS {
            let (sum, first) = self.0[i].overflowing_add(rhs.0[i]);
            let (sum, second) = sum.overflowing_add(carry as u64);
            out.0[i] = sum;carry = first || second;
        }
        if carry { return Err(StdWideError::IntegerBound64); }
        Ok(out)
    }

    fn sub(self, rhs: Self) -> Result<Self, StdWideError> {
        let mut out = Self::ZERO;let mut borrow = false;
        for i in 0..LIMBS {
            let (difference, first) = self.0[i].overflowing_sub(rhs.0[i]);
            let (difference, second) = difference.overflowing_sub(borrow as u64);
            out.0[i] = difference;borrow = first || second;
        }
        if borrow { return Err(StdWideError::IntegerBound64); }
        Ok(out)
    }

    fn bit_length(self) -> u32 {
        for i in (0..LIMBS).rev() {
            if self.0[i] != 0 { return i as u32 * 64 + 64 - self.0[i].leading_zeros(); }
        }
        0
    }

    fn bit(self, index: u32) -> bool {
        index < CAPACITY && ((self.0[(index / 64) as usize] >> (index % 64)) & 1) != 0
    }
    fn any_below(self, exclusive: u32) -> bool { (0..exclusive.min(CAPACITY)).any(|i| self.bit(i)) }
    fn any_from(self, inclusive: u32) -> bool { (inclusive.min(CAPACITY)..CAPACITY).any(|i| self.bit(i)) }

    fn rounded_integer(self, scale: i32, bits: u32) -> Result<u64, StdWideError> {
        if bits == 0 || bits > 54 { return Err(StdWideError::IntegerBound64); }
        if scale >= 0 {
            let shift = scale as u32;
            if self.bit_length() + shift > bits { return Err(StdWideError::IntegerBound64); }
            let mut result = 0_u64;
            for i in 0..self.bit_length() { if self.bit(i) { result |= 1_u64 << (i + shift); } }
            return Ok(result);
        }
        let right = (-scale) as u32;
        if self.any_from(right + bits) { return Err(StdWideError::IntegerBound64); }
        let mut quotient = 0_u64;
        for i in 0..bits { if self.bit(right + i) { quotient |= 1_u64 << i; } }
        let guard = self.bit(right - 1);
        let sticky = self.any_below(right - 1);
        if guard && (sticky || quotient & 1 != 0) {
            quotient = quotient.checked_add(1).ok_or(StdWideError::IntegerBound64)?;
        }
        Ok(quotient)
    }
}

fn pack64(negative: bool, magnitude: Magnitude, exponent: i32) -> Result<StdWord64, StdWideError> {
    let sign = (negative as u64) << 63;
    let length = magnitude.bit_length();
    if length == 0 { return Ok(StdWord64(sign)); }
    let mut top_exponent = length as i32 - 1 + exponent;
    if top_exponent < -1022 {
        let significand = magnitude.rounded_integer(exponent + 1074, 54)?;
        if significand > HIDDEN { return Err(StdWideError::IntegerBound64); }
        return Ok(StdWord64(sign | significand));
    }
    let mut significand = magnitude.rounded_integer(exponent - (top_exponent - 52), 54)?;
    if significand == CARRY { significand >>= 1; top_exponent += 1; }
    if top_exponent > 1023 { return Err(StdWideError::FiniteOverflow64); }
    if !(HIDDEN..CARRY).contains(&significand) { return Err(StdWideError::IntegerBound64); }
    Ok(StdWord64(sign | (((top_exponent + 1023) as u64) << 52) | (significand - HIDDEN)))
}

fn rounded_ratio(numerator: u64, denominator: u64, scale: i32) -> Result<u64, StdWideError> {
    if numerator == 0 || denominator == 0 || numerator >= CARRY || denominator >= CARRY || !(-971..=105).contains(&scale) {
        return Err(StdWideError::IntegerBound64);
    }
    let (mut remainder, divisor) = if scale >= 0 {
        (Magnitude::shifted(numerator, scale as u32)?, Magnitude::shifted(denominator, 0)?)
    } else {
        (Magnitude::shifted(numerator, 0)?, Magnitude::shifted(denominator, (-scale) as u32)?)
    };
    let mut quotient = 0_u64;
    if remainder.compare(divisor) != Ordering::Less {
        let high = remainder.bit_length() - divisor.bit_length();
        if high >= 64 { return Err(StdWideError::IntegerBound64); }
        for at in (0..=high).rev() {
            let aligned = divisor.shift_left(at)?;
            if remainder.compare(aligned) != Ordering::Less {
                remainder = remainder.sub(aligned)?;
                quotient |= 1_u64 << at;
            }
        }
    }
    let complement = divisor.sub(remainder)?;
    let order = remainder.compare(complement);
    if order == Ordering::Greater || (order == Ordering::Equal && quotient & 1 != 0) {
        quotient = quotient.checked_add(1).ok_or(StdWideError::IntegerBound64)?;
    }
    Ok(quotient)
}
