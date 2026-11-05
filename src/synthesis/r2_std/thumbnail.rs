// SPDX-License-Identifier: MIT
use super::{Error, Result, StdBank, archive::Reader};

pub(super) struct Thumbnail {
    rows: Vec<[f32; 32]>,
    repeats: Vec<i32>,
    hop: f64,
}
pub(super) struct Window<'a> {
    source: &'a Thumbnail,
    rows: Vec<usize>,
    weights: Vec<f64>,
    span: i32,
}
pub(super) struct Join {
    floor: f32,
    radius: f64,
}
impl Thumbnail {
    pub fn read(bank: &StdBank, unit: usize) -> Result<Self> {
        let unit = bank
            .units
            .get(unit)
            .ok_or(Error::Format("thumbnail unit"))?;
        let name = if unit.entry_name.is_empty() {
            unit.ordinal.to_string().into_bytes()
        } else {
            unit.entry_name.clone()
        };
        let mut stored = unit.directory.clone();
        stored.extend_from_slice(b"\x82thumb\x82");
        stored.extend_from_slice(&name);
        stored.extend_from_slice(b"\x81thumb");
        let mut reader = Reader::new(bank.archive.descriptor(&stored)?);
        if reader.string_bytes()? != stored {
            return Err(Error::Format("thumbnail descriptor name"));
        }
        reader.i32()?;
        let link = reader.u64()?;
        if reader.u16()? != 2
            || reader.u8()? != 0x33
            || reader.u8()? != 0
            || reader.i32()? != 32
            || reader.u8()? != 0x12
            || reader.u8()? != 1
            || reader.i32()? != 0
        {
            return Err(Error::Unsupported("raw32/signed16 thumbnail schema"));
        }
        reader.end()?;
        let offsets = bank.archive.frame_offsets(link)?;
        if offsets.is_empty() {
            return Err(Error::Format("nonempty thumbnail source required"));
        }
        let mut rows = Vec::with_capacity(offsets.len());
        let mut repeats = Vec::with_capacity(offsets.len());
        let mut expanded = 0i32;
        for offset in offsets {
            let mut reader = Reader::new(bank.archive.frame(offset)?);
            let mut row = [0.0; 32];
            for value in &mut row {
                *value = reader.f32()?;
                if !value.is_finite() {
                    return Err(Error::Format("finite thumbnail"));
                }
            }
            let repeat = reader.u16()? as i16;
            if repeat <= 0 {
                return Err(Error::Format("positive thumbnail repetition"));
            }
            expanded = expanded
                .checked_add(i32::from(repeat))
                .ok_or(Error::Format("expanded thumbnail length"))?;
            reader.end()?;
            rows.push(row);
            repeats.push(i32::from(repeat));
        }
        Ok(Self {
            rows,
            repeats,
            hop: bank.profile.source_period,
        })
    }
    pub fn window(&self, start: f64, end: f64) -> Result<Window<'_>> {
        if !start.is_finite() || !end.is_finite() || start < 0.0 || end < start {
            return Err(Error::Format("thumbnail window times"));
        }
        fn frame(x: f64) -> Result<i32> {
            if x.is_finite() && (0.0..2147483648.0).contains(&x) {
                Ok(x as i32)
            } else {
                Err(Error::Unsupported("thumbnail window frame domain"))
            }
        }
        let begin = frame((1.0 / self.hop) * start)?;
        let end = frame((1.0 / self.hop) * end)?;
        let mut index = 0;
        let mut next = self.repeats[0];
        while index < self.rows.len() - 1 && next <= begin {
            index += 1;
            next = next
                .checked_add(self.repeats[index])
                .ok_or(Error::Format("thumbnail repeated extent"))?;
        }
        let mut rows = Vec::new();
        let mut weights = Vec::new();
        let mut current = f64::from(begin);
        let mut total = 0.0;
        loop {
            let last = index == self.rows.len() - 1;
            let boundary = if last {
                f64::from(end)
            } else {
                f64::from(next.min(end))
            };
            let weight = boundary - current;
            if weight < 0.0 {
                return Err(Error::Format("negative thumbnail window weight"));
            }
            rows.push(index);
            weights.push(weight);
            total += weight;
            if next >= end || last {
                break;
            }
            current = f64::from(next);
            index += 1;
            next = next
                .checked_add(self.repeats[index])
                .ok_or(Error::Format("thumbnail repeated extent"))?;
        }
        Ok(Window {
            source: self,
            rows,
            weights,
            span: frame(total)?,
        })
    }
}
impl Join {
    pub fn new(bank: &StdBank) -> Result<Self> {
        if bank.configuration.single_text("ac_dtw_window")? != "itakura" {
            return Err(Error::Unsupported("STD Itakura JOIN configuration"));
        }
        fn scalar(bank: &StdBank, key: &str) -> Result<f64> {
            let values = bank.configuration.numbers(key)?;
            if values.len() != 1 || !values[0].is_finite() {
                Err(Error::Format("JOIN scalar configuration"))
            } else {
                Ok(values[0])
            }
        }
        let floor = scalar(bank, "ac_floor")? as f32;
        let radius = scalar(bank, "ac_dtw_radius")?;
        if !floor.is_finite() || !(0.0..=1.0).contains(&radius) {
            return Err(Error::Unsupported("JOIN floor/radius"));
        }
        Ok(Self { floor, radius })
    }
    pub fn cost(&self, a: &Window<'_>, b: &Window<'_>) -> Result<f64> {
        let (a, b) = if b.rows.len() > a.rows.len() {
            (b, a)
        } else {
            (a, b)
        };
        let n = a.rows.len();
        let m = b.rows.len();
        if n == 0 || m == 0 || n > 32767 || n.checked_mul(m).is_none_or(|cells| cells > 1_048_576) {
            return Err(Error::Unsupported("JOIN dimensions"));
        }
        let norm = ((n * n + m * m) as f64).sqrt();
        let slope = ((norm * 0.5) / (n as f64)) * (m as f64);
        let width = slope * self.radius;
        let inverse = 1.0 / norm;
        let aa = ((n / 2) as f64 - (width * (m as f64)) * inverse) as i32;
        let bb = ((width * (n as f64)) * inverse + (m / 2) as f64) as i32;
        let aa = aa.clamp(0, n as i32 - 1) as usize;
        let bb = bb.clamp(0, m as i32 - 1) as usize;
        let tail_n = n - aa - 1;
        let tail_m = m - bb - 1;
        let mut previous = vec![1e8; m + 1];
        let mut current = vec![1e8; m + 1];
        previous[0] = 0.0;
        for i in 0..n {
            current.fill(1e8);
            let high = if i < aa {
                bb * i / aa
            } else {
                bb + (i - aa) * tail_m / tail_n.max(1)
            };
            let low = if i < tail_n {
                tail_m * i / tail_n
            } else {
                tail_m + (i - tail_n) * bb / aa.max(1)
            };
            for j in low.min(m - 1)..=high.min(m - 1) {
                let left = &a.source.rows[a.rows[i]];
                let right = &b.source.rows[b.rows[j]];
                let mut distance = 0.0;
                for k in 0..32 {
                    let x = left[k].max(self.floor);
                    let y = self.floor.max(right[k]);
                    let d = x - y;
                    distance += f64::from(d * d);
                }
                distance /= 32.0;
                let diagonal = previous[j];
                let up = previous[j + 1];
                let side = current[j];
                let (prior, weight) = if !(diagonal >= up) && !(diagonal >= side) {
                    (diagonal, a.weights[i].max(b.weights[j]))
                } else {
                    (
                        if !(up >= side) { up } else { side },
                        a.weights[i].min(b.weights[j]),
                    )
                };
                current[j + 1] = (weight * distance + prior) + 1e-8;
            }
            std::mem::swap(&mut previous, &mut current);
        }
        Ok(previous[m] / f64::from(a.span.min(b.span)))
    }
}
