// SPDX-License-Identifier: MIT
use super::{
    Error, Result, StdBank, UnitCostModel,
    archive::Reader,
    selection::{Handle, Selection, Target, bounds},
};
use std::collections::HashMap;

pub(super) struct Packet {
    pub begin: usize,
    pub count: usize,
    pub duration: f64,
    pub weights: Vec<f64>,
}
pub(super) struct PhoneTiming {
    pub duration: f64,
    pub next_source_duration: f64,
}
pub(super) fn allocate(
    bank: &StdBank,
    model: &UnitCostModel,
    targets: &[Target],
    selection: &Selection,
    packets: &[Packet],
) -> Result<Vec<PhoneTiming>> {
    if selection.handles.len() != targets.len() {
        return Err(Error::Format("timing selection cardinality"));
    }
    let scale = bank
        .configuration
        .optional_first_number("dur_offset_svwl")?
        .unwrap_or(0.0);
    let mut output = Vec::with_capacity(targets.len());
    let mut secondary = HashMap::new();
    let mut carry = 0.0;
    for packet in packets {
        if packet.begin != output.len()
            || packet.count == 0
            || packet.weights.len() != packet.count
            || packet.begin + packet.count > targets.len()
            || !packet.duration.is_finite()
            || packet.duration <= 0.0
        {
            return Err(Error::Format("duration packet shape"));
        }
        let mut total = packet.duration;
        if carry > 0.0 && total > carry + carry {
            total -= carry;
            carry = 0.0;
        }
        let mut raw = Vec::with_capacity(packet.count);
        let mut kinds = Vec::with_capacity(packet.count);
        let mut next = Vec::with_capacity(packet.count);
        for i in 0..packet.count {
            let t = packet.begin + i;
            let current = selection.handles[t];
            let previous = if packet.begin == 0 {
                current
            } else {
                let h = selection.handles[t - 1];
                Handle {
                    unit: h.unit,
                    phone: h.phone + 1,
                }
            };
            let (cs, ce) = bounds(bank, current);
            let (ps, pe) = bounds(bank, previous);
            let cd = ce - cs;
            let pd = pe - ps;
            let matches =
                |h: Handle| bank.units[h.unit].timeline[h.phone].symbol == targets[t].context[2];
            let cm = matches(current);
            let pm = matches(previous);
            let mut value = ((if pm { pd } else { 0.0 }) + (if cm { cd } else { 0.0 }))
                / ((usize::from(pm) + usize::from(cm)) as f64 + f64::from_bits(0x3ee4f8b588e368f1));
            if value == 0.0 {
                value = (pd + cd) * 0.5;
            }
            let kind = model.category(&targets[t].context[2]);
            if kind == 2 || kind == 3 {
                let c = midpoint(bank, current, &mut secondary)? - cs;
                let p = midpoint(bank, previous, &mut secondary)? - ps;
                value = (cd + if p > 0.0 { p } else { 0.0 }) - if c > 0.0 { c } else { 0.0 };
            }
            if kind == 8 && i == packet.count - 1 {
                if !scale.is_finite() || scale < 0.0 {
                    return Err(Error::Format("semivowel duration scale"));
                }
                let delta = value * scale;
                total += delta;
                carry = delta;
            }
            raw.push(value);
            kinds.push(kind);
            let (_, right_end) = bounds(
                bank,
                Handle {
                    unit: current.unit,
                    phone: current.phone + 1,
                },
            );
            next.push(right_end - ce);
        }
        let durations = allocation(total, &raw, &kinds, &packet.weights)?;
        for (duration, next_source_duration) in durations.into_iter().zip(next) {
            output.push(PhoneTiming {
                duration,
                next_source_duration,
            });
        }
        if !carry.is_finite() || carry < 0.0 {
            return Err(Error::Format("duration carry"));
        }
    }
    if output.len() != targets.len() {
        return Err(Error::Format("duration packet coverage"));
    }
    Ok(output)
}
fn midpoint(bank: &StdBank, h: Handle, cache: &mut HashMap<usize, Vec<f64>>) -> Result<f64> {
    if !cache.contains_key(&h.unit) {
        let unit = &bank.units[h.unit];
        let mut name = unit.directory.clone();
        name.extend_from_slice(b"\x82\xa1\x82");
        name.extend_from_slice(&if unit.entry_name.is_empty() {
            unit.ordinal.to_string().into_bytes()
        } else {
            unit.entry_name.clone()
        });
        name.extend_from_slice(b"\x81txtp");
        let mut ends = Vec::new();
        if let Some(bytes) = bank.archive.named_optional(&name)? {
            if !bytes.is_empty() {
                let mut reader = Reader::new(bytes);
                if reader.u32()? != 0xfcfc {
                    return Err(Error::Format("secondary timeline header"));
                }
                let count = reader.u32()? as usize;
                if count > reader.remaining() / 11 || count != 0 && count != unit.timeline.len() {
                    return Err(Error::Format("secondary timeline extent"));
                }
                let mut last = 0.0;
                for _ in 0..count {
                    let end = reader.f64()?;
                    reader.u8()?;
                    reader.string_bytes()?;
                    if !end.is_finite() || end < last {
                        return Err(Error::Format("secondary timeline endpoint"));
                    }
                    ends.push(end);
                    last = end;
                }
                reader.end()?;
            }
        }
        cache.insert(h.unit, ends);
    }
    let ends = &cache[&h.unit];
    let (start, end) = bounds(bank, h);
    Ok(if ends.is_empty() {
        (end + start) * 0.5
    } else if h.phone == 0 {
        0.0
    } else {
        ends[h.phone - 1]
    })
}
fn allocation(duration: f64, raw: &[f64], kinds: &[u8], weights: &[f64]) -> Result<Vec<f64>> {
    if !duration.is_finite()
        || duration <= 0.0
        || raw.is_empty()
        || raw.len() != kinds.len()
        || raw.len() != weights.len()
    {
        return Err(Error::Format("phone allocation shape"));
    }
    let set: Vec<_> = kinds.iter().map(|&k| k <= 1 || k == 9 || k == 10).collect();
    let mut sums = [0.0; 2];
    for (i, &value) in raw.iter().enumerate() {
        if !value.is_finite() || value < 0.0 || !weights[i].is_finite() || weights[i] < 0.0 {
            return Err(Error::Format("nonnegative allocation values"));
        }
        sums[usize::from(set[i])] += value;
    }
    let a = (duration - sums[0]).max(duration * 0.3);
    let b = if sums[1] == 0.0 {
        duration
    } else {
        duration - a
    };
    let mut values = Vec::with_capacity(raw.len());
    let mut total = 0.0;
    for i in 0..raw.len() {
        let group = usize::from(set[i]);
        if sums[group] <= 0.0 {
            return Err(Error::Format("allocation group denominator"));
        }
        let x = ((if set[i] { a } else { b }) / sums[group] * raw[i]) * weights[i];
        values.push(x);
        total += x;
    }
    if !total.is_finite() || total <= 0.0 {
        return Err(Error::Format("allocation weighted total"));
    }
    let factor = duration / total;
    for value in &mut values {
        *value *= factor;
        if !value.is_finite() || *value < 0.0 {
            return Err(Error::Format("finite allocation result"));
        }
    }
    Ok(values)
}
