// SPDX-License-Identifier: MIT
use super::{Error, Result, StdBank, UnitCostModel, archive::Reader};

struct Rule {
    target: [Vec<u8>; 5],
    source: [Vec<u8>; 5],
    weight: f64,
}
pub(super) struct TargetRules(Vec<Rule>);
impl TargetRules {
    pub fn read(bank: &StdBank) -> Result<Self> {
        let Some(bytes) = bank.archive.named_optional(&[0x92])? else {
            return Ok(Self(Vec::new()));
        };
        if bytes.is_empty() {
            return Ok(Self(Vec::new()));
        }
        let mut reader = Reader::new(bytes);
        if reader.i32()? != 0xfdfd {
            return Err(Error::Unsupported("FDFD target rules"));
        }
        let count = reader.i32()?;
        if !(0..=4096).contains(&count) || count as usize > reader.remaining() / 32 {
            return Err(Error::Format("target rule count"));
        }
        let mut rules = Vec::with_capacity(count as usize);
        let mut total = 0usize;
        for _ in 0..count {
            let weight = reader.f64()?;
            if reader.i32()? != 5 {
                return Err(Error::Unsupported("target rule width"));
            }
            let mut side = || -> Result<[Vec<u8>; 5]> {
                let mut out = std::array::from_fn(|_| Vec::new());
                for pattern in &mut out {
                    let size = reader.u16()? as usize;
                    total = total
                        .checked_add(size)
                        .ok_or(Error::Format("target rule extent"))?;
                    if size == 0 || size > 32767 || total > 1_048_576 {
                        return Err(Error::Unsupported("target pattern extent"));
                    }
                    *pattern = reader.take(size)?.to_vec();
                }
                Ok(out)
            };
            rules.push(Rule {
                target: side()?,
                source: side()?,
                weight,
            });
        }
        reader.end()?;
        Ok(Self(rules))
    }
    pub fn cost(&self, model: &UnitCostModel, target: [&str; 5], source: [&str; 5]) -> Result<f64> {
        let tc = model.category(target[2]);
        let sc = model.category(source[2]);
        let mut cost = model.cost(target, source, tc == 10)?;
        if (if tc == 1 { 0 } else { tc }) != (if sc == 1 { 0 } else { sc }) {
            cost += 10.0;
        }
        fn literal(pattern: &[u8], value: &[u8]) -> bool {
            let n = (pattern.len() - 1).min(value.len());
            n == value.len() && pattern[1..1 + n] == *value
        }
        for rule in &self.0 {
            if !(0..5)
                .all(|i| rule.target[i] == b"*" || literal(&rule.target[i], target[i].as_bytes()))
            {
                continue;
            }
            if !(0..5).all(|i| match rule.source[i].as_slice() {
                b"*" => true,
                b"=" => source[i] == target[i],
                b"~" => source[i] != target[i],
                p => literal(p, source[i].as_bytes()),
            }) {
                continue;
            }
            cost = if rule.weight > 0.0 {
                rule.weight + cost
            } else {
                rule.weight.exp() * cost
            };
            break;
        }
        if !cost.is_finite() {
            return Err(Error::Format("nonfinite selected target cost"));
        }
        Ok(cost)
    }
}
