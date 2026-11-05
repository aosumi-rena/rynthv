// SPDX-License-Identifier: MIT
use std::cmp::Ordering;

const L: usize = 10; // 640 bits, including exact products and rational numerators.
pub(crate) const PRECISIONS: [u32; 3] = [96, 160, 256];
pub(crate) const MAX_ATTEMPTS: usize = 390;
pub(crate) const MAX_CHAINS: usize = 396;
pub(crate) const MAX_STEPS: usize = 43008;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MathError { Domain, IntegerBound, Capacity, Ambiguous }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Nat([u64; L]);
impl Nat {
    const ZERO: Self = Self([0; L]);
    fn small(x: u64) -> Self { let mut r = Self::ZERO; r.0[0] = x; r }
    fn bit(self, i: u32) -> bool { i < 640 && self.0[i as usize / 64] >> (i % 64) & 1 != 0 }
    fn len(self) -> u32 {
        for i in (0..L).rev() { if self.0[i] != 0 { return i as u32 * 64 + 64 - self.0[i].leading_zeros(); } }
        0
    }
    fn cmp(self, b: Self) -> Ordering {
        for i in (0..L).rev() { let o = self.0[i].cmp(&b.0[i]); if o != Ordering::Equal { return o; } }
        Ordering::Equal
    }
    fn add(self, b: Self) -> Result<Self, MathError> {
        let mut r = Self::ZERO; let mut carry = 0u128;
        for i in 0..L { let t = self.0[i] as u128 + b.0[i] as u128 + carry; r.0[i] = t as u64; carry = t >> 64; }
        if carry != 0 { Err(MathError::IntegerBound) } else { Ok(r) }
    }
    fn sub(self, b: Self) -> Result<Self, MathError> {
        if self.cmp(b) == Ordering::Less { return Err(MathError::IntegerBound); }
        let mut r = Self::ZERO; let mut borrow = false;
        for i in 0..L {
            let (x, a) = self.0[i].overflowing_sub(b.0[i]);
            let (y, c) = x.overflowing_sub(borrow as u64); r.0[i] = y; borrow = a || c;
        }
        if borrow { Err(MathError::IntegerBound) } else { Ok(r) }
    }
    fn shl(self, n: u32) -> Result<Self, MathError> {
        if self.len() == 0 { return Ok(self); }
        if n >= 640 || self.len() + n > 640 { return Err(MathError::IntegerBound); }
        let mut r = Self::ZERO;
        for i in 0..self.len() { if self.bit(i) { let j = i + n; r.0[j as usize / 64] |= 1u64 << (j % 64); } }
        Ok(r)
    }
    fn shr(self, n: u32) -> Self {
        let mut r = Self::ZERO;
        if n < 640 { for i in n..self.len() { if self.bit(i) { let j = i - n; r.0[j as usize / 64] |= 1u64 << (j % 64); } } }
        r
    }
    fn low_nonzero(self, n: u32) -> bool { (0..n.min(640)).any(|i| self.bit(i)) }
    fn mul(self, b: Self) -> Result<Self, MathError> {
        let mut wide = [0u64; 20];
        for i in 0..L {
            let mut carry = 0u128;
            for j in 0..L {
                let k = i + j;
                let t = self.0[i] as u128 * b.0[j] as u128 + wide[k] as u128 + carry;
                wide[k] = t as u64; carry = t >> 64;
            }
            let mut k = i + L;
            while carry != 0 {
                if k >= 20 { return Err(MathError::IntegerBound); }
                let t = wide[k] as u128 + carry; wide[k] = t as u64; carry = t >> 64; k += 1;
            }
        }
        if wide[L..].iter().any(|x| *x != 0) { return Err(MathError::IntegerBound); }
        let mut r = Self::ZERO; r.0.copy_from_slice(&wide[..L]); Ok(r)
    }
    fn div_rem(self, d: Self) -> Result<(Self, Self), MathError> {
        if d.len() == 0 { return Err(MathError::IntegerBound); }
        let mut q = Self::ZERO; let mut rem = Self::ZERO;
        for i in (0..self.len()).rev() {
            rem = rem.shl(1)?.add(Self::small(self.bit(i) as u64))?;
            if rem.cmp(d) != Ordering::Less { rem = rem.sub(d)?; q.0[i as usize / 64] |= 1u64 << (i % 64); }
        }
        Ok((q, rem))
    }
    fn quotient(self, d: Self, up: bool) -> Result<Self, MathError> {
        let (q, r) = self.div_rem(d)?;
        if up && r.len() != 0 { q.add(Self::small(1)) } else { Ok(q) }
    }
    pub(crate) fn hex(self) -> String {
        let mut s = String::new();
        for i in (0..L).rev() {
            if !s.is_empty() { s.push_str(&format!("{:016x}", self.0[i])); }
            else if self.0[i] != 0 { s.push_str(&format!("{:x}", self.0[i])); }
        }
        if s.is_empty() { s.push('0'); } s
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Interval { pub(crate) lo: Nat, pub(crate) hi: Nat }
impl Interval {
    fn exact(x: Nat) -> Self { Self { lo: x, hi: x } }
    fn add(self, b: Self) -> Result<Self, MathError> { Ok(Self { lo: self.lo.add(b.lo)?, hi: self.hi.add(b.hi)? }) }
    fn mul(self, b: Self, p: u32) -> Result<Self, MathError> {
        let den = Nat::small(1).shl(p)?;
        Ok(Self { lo: self.lo.mul(b.lo)?.quotient(den, false)?, hi: self.hi.mul(b.hi)?.quotient(den, true)? })
    }
    fn ratio(self, numerator: u64, denominator: u64) -> Result<Self, MathError> {
        let n = Nat::small(numerator); let d = Nat::small(denominator);
        Ok(Self { lo: self.lo.mul(n)?.quotient(d, false)?, hi: self.hi.mul(n)?.quotient(d, true)? })
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Step { pub(crate) index: u32, pub(crate) term: Interval, pub(crate) sum: Interval }
#[derive(Clone, Debug)]
pub(crate) struct Chain {
    pub(crate) name: &'static str, pub(crate) x: Interval, pub(crate) square: Option<Interval>,
    pub(crate) steps: Vec<Step>, pub(crate) next: Option<Interval>,
    pub(crate) tail_upper: Option<Nat>, pub(crate) enclosure: Option<Interval>,
}
#[derive(Clone, Debug)]
pub(crate) struct Attempt {
    pub(crate) kind: &'static str, pub(crate) input: u64, pub(crate) p: u32,
    pub(crate) reduction: u32, pub(crate) chains: Vec<Chain>,
    pub(crate) enclosure: Option<Interval>, pub(crate) rounded: Option<[u64; 2]>,
    pub(crate) decision: &'static str,
}
#[derive(Default, Debug)]
pub(crate) struct Ledger {
    pub(crate) attempts: Vec<Attempt>, pub(crate) steps: usize, pub(crate) chains: usize,
    pub(crate) identities: Vec<(&'static str, u64, u64)>,
}
fn fixed(bits: u64, p: u32) -> Result<Interval, MathError> {
    let e = (bits >> 52) as i32;
    let sig = (bits & 0x000f_ffff_ffff_ffff) | if e == 0 { 0 } else { 1u64 << 52 };
    let shift = if e == 0 { -1074 } else { e - 1075 } + p as i32;
    let n = Nat::small(sig);
    if shift >= 0 { Ok(Interval::exact(n.shl(shift as u32)?)) }
    else {
        let k = (-shift) as u32; let lo = n.shr(k);
        let hi = if n.low_nonzero(k) { lo.add(Nat::small(1))? } else { lo };
        Ok(Interval { lo, hi })
    }
}
fn endpoint_rne(n: Nat, p: u32) -> Result<u64, MathError> {
    if n.len() == 0 { return Ok(0); }
    let top = n.len() - 1; let mut exponent = top as i32 - p as i32;
    if !(-1022..=1023).contains(&exponent) { return Err(MathError::IntegerBound); }
    let shift = top.saturating_sub(52); let mut q = n.shr(shift).0[0];
    if top < 52 { q <<= 52 - top; }
    if shift > 0 && n.bit(shift - 1) && (n.low_nonzero(shift - 1) || q & 1 != 0) { q += 1; }
    if q == 1u64 << 53 { q >>= 1; exponent += 1; }
    if exponent > 1023 { return Err(MathError::IntegerBound); }
    Ok(((exponent + 1023) as u64) << 52 | (q & 0x000f_ffff_ffff_ffff))
}

impl Ledger {
    fn chain(&mut self, a: usize, name: &'static str, x: Interval, square: Option<Interval>) -> Result<usize, MathError> {
        if self.chains == MAX_CHAINS { return Err(MathError::Capacity); }
        self.chains += 1; let c = self.attempts[a].chains.len();
        self.attempts[a].chains.push(Chain { name, x, square, steps: Vec::new(), next: None, tail_upper: None, enclosure: None }); Ok(c)
    }
    fn step(&mut self, a: usize, c: usize, index: u32, term: Interval, sum: Interval) -> Result<(), MathError> {
        if self.steps == MAX_STEPS { return Err(MathError::Capacity); }
        self.steps += 1; self.attempts[a].chains[c].steps.push(Step { index, term, sum }); Ok(())
    }
    fn atanh_twice(&mut self, a: usize, name: &'static str, z: Interval, p: u32) -> Result<Interval, MathError> {
        let square = z.mul(z, p)?; let c = self.chain(a, name, z, Some(square))?;
        let n = p / 2 + 16; let mut term = z.ratio(2, 1)?; let mut sum = Interval::exact(Nat::ZERO);
        for k in 0..n {
            sum = sum.add(term)?; self.step(a, c, k, term, sum)?;
            term = term.mul(square, p)?.ratio((2 * k + 1) as u64, (2 * k + 3) as u64)?;
        }
        let tail = term.hi.mul(Nat::small(2))?;
        let enclosure = Interval { lo: sum.lo, hi: sum.hi.add(tail)? };
        let row = &mut self.attempts[a].chains[c]; row.next = Some(term); row.tail_upper = Some(tail); row.enclosure = Some(enclosure);
        Ok(enclosure)
    }
    fn ln_attempt(&mut self, a: usize, bits: u64, p: u32) -> Result<Interval, MathError> {
        let k = ((bits >> 52) - 1023) as u32;
        self.attempts[a].reduction = k;
        let m = fixed((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000, p)?.lo;
        let one = Nat::small(1).shl(p)?; let num = m.sub(one)?.shl(p)?; let den = m.add(one)?;
        let z = Interval { lo: num.quotient(den, false)?, hi: num.quotient(den, true)? };
        let local = self.atanh_twice(a, "ln.m", z, p)?;
        if k == 0 { return Ok(local); }
        let z2 = Interval { lo: one.quotient(Nat::small(3), false)?, hi: one.quotient(Nat::small(3), true)? };
        let ln2 = self.atanh_twice(a, "ln.2", z2, p)?.ratio(k as u64, 1)?;
        local.add(ln2)
    }
    fn exp_attempt(&mut self, a: usize, bits: u64, p: u32) -> Result<Interval, MathError> {
        let x = fixed(bits, p)?; let c = self.chain(a, "exp", x, None)?;
        let n = p / 2 + 16; let mut term = Interval::exact(Nat::small(1).shl(p)?);
        let mut sum = Interval::exact(Nat::ZERO);
        for k in 0..=n {
            sum = sum.add(term)?; self.step(a, c, k, term, sum)?;
            term = term.mul(x, p)?.ratio(1, (k + 1) as u64)?;
        }
        let tail = term.hi.mul(Nat::small((n + 2) as u64))?.quotient(Nat::small(n as u64), true)?;
        let enclosure = Interval { lo: sum.lo, hi: sum.hi.add(tail)? };
        let row = &mut self.attempts[a].chains[c]; row.next = Some(term); row.tail_upper = Some(tail); row.enclosure = Some(enclosure);
        Ok(enclosure)
    }
    pub(crate) fn evaluate(&mut self, kind: &'static str, bits: u64) -> Result<u64, MathError> {
        match kind {
            "ln" if (0x3ff0_0000_0000_0000..=0x401c_0000_0000_0000).contains(&bits) => {},
            "exp" if bits <= 0x4000_0000_0000_0000 => {},
            _ => return Err(MathError::Domain),
        }
        if kind == "ln" && bits == 0x3ff0_0000_0000_0000 {
            if self.identities.len() >= 130 { return Err(MathError::Capacity); }
            self.identities.push((kind, bits, 0)); return Ok(0);
        }
        if kind == "exp" && bits == 0 {
            if self.identities.len() >= 130 { return Err(MathError::Capacity); }
            self.identities.push((kind, bits, 0x3ff0_0000_0000_0000)); return Ok(0x3ff0_0000_0000_0000);
        }
        for p in PRECISIONS {
            if self.attempts.len() == MAX_ATTEMPTS { return Err(MathError::Capacity); }
            let a = self.attempts.len();
            self.attempts.push(Attempt { kind, input: bits, p, reduction: 0, chains: Vec::new(), enclosure: None, rounded: None, decision: "started" });
            let value = if kind == "ln" { self.ln_attempt(a, bits, p) } else { self.exp_attempt(a, bits, p) };
            let interval = match value { Ok(v) => v, Err(e) => { self.attempts[a].decision = "typed-refusal"; return Err(e); } };
            self.attempts[a].enclosure = Some(interval);
            let lo = endpoint_rne(interval.lo, p)?; let hi = endpoint_rne(interval.hi, p)?;
            self.attempts[a].rounded = Some([lo, hi]);
            if lo == hi { self.attempts[a].decision = "same-endpoint-RNE"; return Ok(lo); }
            self.attempts[a].decision = "different-endpoint-RNE";
        }
        Err(MathError::Ambiguous)
    }
}
