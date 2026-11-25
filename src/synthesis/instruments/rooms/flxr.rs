use super::wire::Reader;
use std::{collections::BTreeMap, ops::Range, sync::Arc};

struct Field { kind: u32, shape: Vec<usize>, extent: Range<usize> }
pub struct Program { bytes: Arc<[u8]>, fields: BTreeMap<u32, Field> }
impl Program {
    pub fn dimensions(&self, tag:u32) -> Result<&[usize],String> {
        self.fields.get(&tag).map(|v|v.shape.as_slice()).ok_or_else(||format!("FLXR field {tag:x} absent"))
    }
    pub fn read(bytes: Arc<[u8]>, extent: Range<usize>) -> Result<Self, String> {
        let mut r = Reader {bytes: &bytes[extent.clone()], at: 0};
        if r.take(4)? != b"FLXR" || r.u32()? != 3 || r.u32()? != 32 || r.u32()? != 0 {
            return Err("Invalid FLXR room model header".into());
        }
        let length = r.u64()?;
        let checksum = r.u32()?;
        if r.u32()? != 0 || length != (r.bytes.len() - r.at) as u64 {
            return Err("Invalid FLXR room model extent".into());
        }
        if crc32(&r.bytes[r.at..]) != checksum { return Err("FLXR room model CRC32 mismatch".into()); }
        let mut fields = BTreeMap::new();
        while r.at < r.bytes.len() {
            let tag = r.u32()?;
            let kind = r.u32()?;
            let rank = r.count(8)?;
            if r.u32()? != 0 || !(1..=3).contains(&kind) { return Err("Invalid FLXR field type".into()); }
            let mut shape = Vec::with_capacity(rank);
            let mut count = 1usize;
            for _ in 0..rank {
                let n = usize::try_from(r.u64()?).map_err(|_| "FLXR field dimension overflow")?;
                count = count.checked_mul(n).ok_or("FLXR field size overflow")?;
                shape.push(n);
            }
            let length = if kind == 3 {count.checked_add(7).map(|n| n & !7)} else {count.checked_mul(8)}
                .ok_or("FLXR field extent overflow")?;
            let begin = r.at + extent.start;
            r.take(length)?;
            if fields.insert(tag, Field {kind, shape, extent: begin..begin+length}).is_some() {
                return Err("Duplicate FLXR field".into());
            }
        }
        Ok(Self {bytes, fields})
    }
    pub fn values(&self, tag: u32, shape: &[usize]) -> Result<Vec<f64>, String> {
        let field = self.fields.get(&tag).ok_or_else(|| format!("FLXR field {tag:x} absent"))?;
        if field.kind != 1 || field.shape != shape { return Err(format!("Invalid FLXR field {tag:x} shape")); }
        let values: Vec<_> = self.bytes[field.extent.clone()].chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect();
        if values.iter().any(|v| !v.is_finite()) { return Err(format!("Nonfinite FLXR field {tag:x}")); }
        Ok(values)
    }
    pub fn integers(&self, tag: u32, shape: &[usize]) -> Result<Vec<i64>, String> {
        let field = self.fields.get(&tag).ok_or_else(|| format!("FLXR field {tag:x} absent"))?;
        if field.kind != 2 || field.shape != shape { return Err(format!("Invalid FLXR field {tag:x} shape")); }
        Ok(self.bytes[field.extent.clone()].chunks_exact(8)
            .map(|b| i64::from_le_bytes(b.try_into().unwrap())).collect())
    }
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut value = !0u32;
    for byte in bytes {
        value ^= u32::from(*byte);
        for _ in 0..8 { value = (value >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(value & 1)); }
    }
    !value
}
