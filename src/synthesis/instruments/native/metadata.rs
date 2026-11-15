use super::{
    Result,
    archive::Archive,
    fail, hex, id,
    random::{M, pcg},
    u32at, u64at,
};
use std::collections::BTreeMap;
pub const ROOT: &str = "c64191888d316216b2eb44877a9ed141";
pub const EXTRA: &str = "8dd57dda5aa9a8c4c954661897b2bf65";
#[derive(Clone, Debug)]
#[allow(dead_code)] // Preserve the complete typed metadata, including unselected fields.
pub enum Value {
    Map(BTreeMap<String, Value>),
    Array(Vec<Value>),
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Id([u8; 16]),
}
pub fn description(a: &Archive) -> Result<(Value, u64)> {
    let wire = a.read(&id(ROOT))?;
    if wire.len() < 24 || u32at(&wire, 0) != 1 || u32at(&wire, 4) != 0 {
        return fail("Instrument description header");
    }
    let n = u32at(&wire, 16) as usize;
    let extra = u32at(&wire, 20) as usize;
    if n > 0x40000 || extra != 0 || wire.len() != 24 + n {
        return fail("Instrument description extent");
    }
    let nonce = u64at(&wire, 8);
    let mut input = hex("bee232b2c7c7d2c12d371d33e2653b8696641eb8dd0db088caff785fc28d22fd");
    input.extend(nonce.to_le_bytes());
    let h = xxhash_rust::xxh3::xxh3_128(&input);
    let lo = h as u64;
    let hi = (h >> 64) as u64;
    let twice = hi.wrapping_mul(2).wrapping_add(1);
    let initial = lo.wrapping_add(twice);
    let mut plain = Vec::new();
    for (i, c) in wire[24..].chunks(8).enumerate() {
        let state = initial
            .wrapping_add((i as u64).wrapping_mul(0x9e3779b97f4a7c15))
            .wrapping_mul(M)
            .wrapping_add(twice);
        let mut mask = pcg(state).to_le_bytes().to_vec();
        mask.extend(pcg(state.wrapping_mul(M).wrapping_add(hi | 1)).to_le_bytes());
        plain.extend(c.iter().zip(mask).map(|(a, b)| a.wrapping_sub(b)));
    }
    let (v, end) = parse(&plain, 0, false, 0)?;
    if end != plain.len() {
        return fail("Instrument description trailing bytes");
    }
    Ok((v, nonce))
}
fn parse(b: &[u8], at: usize, named: bool, depth: usize) -> Result<(Value, usize)> {
    if depth > 64 || at + 8 > b.len() {
        return fail("Truncated instrument metadata");
    }
    let tag = u32at(b, at);
    let n = u32at(b, at + 4) as usize;
    let a = at + 8;
    let end = a
        .checked_add(n)
        .filter(|e| *e <= b.len())
        .ok_or("Instrument metadata bounds")?;
    let p = &b[a..end];
    let v = match tag {
        0 => Value::Map(mapping(p, named, depth + 1)?),
        1 => {
            if n < 4 {
                return fail("Instrument metadata array");
            }
            let count = u32at(p, 0) as usize;
            if count > 65535 {
                return fail("Instrument metadata array count");
            }
            let mut at = 4;
            let mut out = Vec::new();
            for _ in 0..count {
                let (v, e) = parse(p, at, named, depth + 1)?;
                at = e;
                out.push(v);
            }
            if at != p.len() {
                return fail("Instrument metadata array suffix");
            }
            Value::Array(out)
        }
        2 => Value::String(String::from_utf8(p.to_vec()).map_err(|_| "Instrument metadata UTF-8")?),
        3 if n == 8 => Value::Int(u64at(p, 0) as i64),
        4 if n == 8 => Value::Float(f64::from_bits(u64at(p, 0))),
        5 if n == 1 && p[0] < 2 => Value::Bool(p[0] != 0),
        6 if n == 16 => Value::Id(p.try_into().unwrap()),
        _ => return fail("Instrument metadata value type"),
    };
    Ok((v, end))
}
fn mapping(p: &[u8], named: bool, depth: usize) -> Result<BTreeMap<String, Value>> {
    if p.len() < 4 {
        return fail("Instrument metadata map");
    }
    let count = u32at(p, 0) as usize;
    if count > 65535 {
        return fail("Instrument metadata map count");
    }
    let mut at = 4;
    let mut out = BTreeMap::new();
    for _ in 0..count {
        let key = if named {
            if at + 4 > p.len() {
                return fail("Instrument field name");
            }
            let n = u32at(p, at) as usize;
            at += 4;
            if at + n > p.len() {
                return fail("Instrument field name bounds");
            }
            let s =
                String::from_utf8(p[at..at + n].to_vec()).map_err(|_| "Instrument field UTF-8")?;
            at += n;
            s
        } else {
            if at + 16 > p.len() {
                return fail("Instrument hashed field bounds");
            }
            let s = p[at..at + 16].iter().map(|x| format!("{x:02x}")).collect();
            at += 16;
            s
        };
        let (v, e) = parse(p, at, named, depth + 1)?;
        at = e;
        if out.insert(key, v).is_some() {
            return fail("Duplicate instrument metadata field");
        }
    }
    if at != p.len() {
        return fail("Instrument metadata map suffix");
    }
    Ok(out)
}
pub fn extra(a: &Archive) -> Result<BTreeMap<String, Value>> {
    let b = a.read(&id(EXTRA))?;
    if b.len() < 16
        || u32at(&b, 0) != 1
        || u32at(&b, 4) != 0
        || u32at(&b, 8) as usize != b.len() - 12
    {
        return fail("Instrument named metadata header");
    }
    mapping(&b[12..], true, 0)
}
pub fn name(root: &Value, nonce: u64) -> Result<String> {
    let mut b = id("e091377ac9797061a2f235693211d62f").to_vec();
    b.extend(nonce.to_le_bytes());
    let k = xxhash_rust::xxh3::xxh3_128(&b)
        .to_le_bytes()
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect::<String>();
    let mut names = Vec::new();
    if let Value::Map(m) = root {
        for v in m.values() {
            if let Value::Map(m) = v {
                if let Some(Value::String(s)) = m.get(&k) {
                    names.push(s.clone());
                }
            }
        }
    }
    if names.len() != 1 {
        return fail("Instrument name absent/ambiguous");
    }
    Ok(names.remove(0))
}
pub fn microphones(root: &Value) -> Result<BTreeMap<String, [u8; 16]>> {
    let mut candidates = Vec::new();
    if let Value::Map(m) = root {
        for v in m.values() {
            if let Value::Array(a) = v {
                if a.is_empty() {
                    continue;
                }
                let mut rows = BTreeMap::new();
                for v in a {
                    let Value::Map(m) = v else { break };
                    if m.len() != 4 {
                        break;
                    }
                    let mut name = None;
                    let mut id = None;
                    let mut ints = 0;
                    for v in m.values() {
                        match v {
                            Value::String(s) => name = Some(s.clone()),
                            Value::Id(v) => id = Some(*v),
                            Value::Int(_) => ints += 1,
                            _ => {}
                        }
                    }
                    if let (Some(n), Some(i), 2) = (name, id, ints) {
                        rows.insert(n, i);
                    } else {
                        break;
                    }
                }
                if rows.len() == a.len() {
                    candidates.push(rows);
                }
            }
        }
    }
    if candidates.len() != 1 {
        return fail("Instrument microphone metadata absent/ambiguous");
    }
    Ok(candidates.remove(0))
}
