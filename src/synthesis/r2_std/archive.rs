// SPDX-License-Identifier: MIT
use super::{Error, Result};
use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::Read,
    path::Path,
    sync::Arc,
};

pub(super) struct Archive {
    data: Arc<[u8]>,
    members: BTreeMap<u32, usize>,
}

impl Archive {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(Error::Format("regular bank file required"));
        }
        let length = metadata.len();
        if !(16..=512 * 1024 * 1024).contains(&length) {
            return Err(Error::Unsupported(
                "bank snapshot must be 16 bytes..512 MiB",
            ));
        }
        let mut bytes = vec![0; length as usize];
        file.read_exact(&mut bytes)?;
        let mut extra = [0];
        if file.read(&mut extra)? != 0 {
            return Err(Error::Format("bank changed during read"));
        }
        Self::from_bytes(bytes.into())
    }

    fn from_bytes(data: Arc<[u8]>) -> Result<Self> {
        let mut header = Reader::new(&data);
        if header.u32()? != 0xf580 || !(1..=0x13).contains(&header.u32()?) {
            return Err(Error::Format("NOFS magic/version"));
        }
        let extent = header.u64()?;
        if extent != 0 && extent != data.len() as u64 {
            return Err(Error::Format("NOFS extent"));
        }
        let mut members = BTreeMap::new();
        let mut visited = HashSet::new();
        let mut offset = 16usize;
        for chain in 0..64 {
            if !visited.insert(offset) {
                return Err(Error::Format("cyclic master index"));
            }
            let block = node(&data, offset, if chain == 0 { 0x1000 } else { 0x2000 })?;
            if !(18..=1024 * 1024).contains(&block.len()) {
                return Err(Error::Format("master index extent"));
            }
            let end = block.len() - 12;
            let mut cursor = 6;
            while cursor + 12 <= end {
                let mut pair = Reader::new(&block[cursor..cursor + 12]);
                let key = pair.u32()?;
                let target = pair.u64()?;
                if target != 0 {
                    let target = checked_offset(&data, target)?;
                    if let Some(old) = members.insert(key, target) {
                        if old != target {
                            return Err(Error::Unsupported(
                                "different-offset member hash collision",
                            ));
                        }
                    }
                }
                cursor += 12;
            }
            let next = Reader::new(&block[end..]).u64()?;
            if next == 0 {
                return Ok(Self { data, members });
            }
            offset = checked_offset(&data, next)?;
        }
        Err(Error::Format("master index chain exceeds 64 blocks"))
    }

    pub fn named(&self, name: &[u8]) -> Result<&[u8]> {
        self.named_optional(name)?
            .ok_or(Error::Format("required named bank member absent"))
    }
    pub fn named_optional(&self, name: &[u8]) -> Result<Option<&[u8]>> {
        let Some(&offset) = self.members.get(&name_key(name)) else {
            return Ok(None);
        };
        let block = node(&self.data, offset, 1)?;
        let mut reader = Reader::new(&block[6..block.len() - 4]);
        if reader.string_bytes()? != name {
            return Err(Error::Format("named member collision"));
        }
        let count = reader.u32()? as usize;
        let bytes = reader.take(count)?;
        reader.end()?;
        Ok(Some(bytes))
    }
    pub fn descriptor(&self, name: &[u8]) -> Result<&[u8]> {
        let &offset = self
            .members
            .get(&name_key(name))
            .ok_or(Error::Format("source descriptor absent"))?;
        let block = node(&self.data, offset, 0x100)?;
        let payload = &block[6..block.len() - 4];
        if Reader::new(payload).string_bytes()? != name {
            return Err(Error::Format("source descriptor name"));
        }
        Ok(payload)
    }
    pub fn frame_offsets(&self, mut link: u64) -> Result<Vec<usize>> {
        let mut visited = HashSet::new();
        let mut offsets = Vec::new();
        let mut stopped = false;
        while link != 0 {
            if !visited.insert(link) {
                return Err(Error::Format("cyclic frame index"));
            }
            let offset = checked_offset(&self.data, link)?;
            let block = node(&self.data, offset, 0x200)?;
            if block.len() < 18 {
                return Err(Error::Format("frame index extent"));
            }
            let end = block.len() - 12;
            let mut at = 6;
            while at + 12 <= end {
                let frame = Reader::new(&block[at..at + 8]).u64()?;
                if frame == 0 {
                    stopped = true;
                }
                if !stopped {
                    offsets.push(checked_offset(&self.data, frame)?);
                }
                if offsets.len() > 1_048_576 {
                    return Err(Error::Unsupported(
                        "source frame count exceeds native-port bound",
                    ));
                }
                at += 8;
            }
            link = Reader::new(&block[end..]).u64()?;
        }
        if offsets.is_empty() {
            return Err(Error::Format("empty source frame index"));
        }
        Ok(offsets)
    }
    pub fn frame(&self, offset: usize) -> Result<&[u8]> {
        let block = node(&self.data, offset, 0x10)?;
        if block.len() > 1_048_576 {
            return Err(Error::Unsupported("source frame exceeds 1 MiB"));
        }
        Ok(&block[6..block.len() - 4])
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

fn checked_offset(data: &[u8], offset: u64) -> Result<usize> {
    let offset =
        usize::try_from(offset).map_err(|_| Error::Format("unaddressable archive offset"))?;
    if offset < 16 || offset > data.len().saturating_sub(10) {
        return Err(Error::Format("archive offset outside snapshot"));
    }
    Ok(offset)
}
fn node(data: &[u8], offset: usize, tag: u16) -> Result<&[u8]> {
    let bytes = data.get(offset..).ok_or(Error::Format("node offset"))?;
    let mut reader = Reader::new(bytes);
    let length = reader.u32()? as usize;
    if length < 10 || reader.u16()? != tag {
        return Err(Error::Format("node type/length"));
    }
    let block = bytes
        .get(..length)
        .ok_or(Error::Format("node exceeds snapshot"))?;
    let footer = Reader::new(&block[length - 4..]).u32()? as usize;
    if footer != length {
        return Err(Error::Archive(format!(
            "node footer at {offset:#x}, tag {tag:#x}, size {length}, trailer {footer}"
        )));
    }
    Ok(block)
}

pub(super) fn name_key(name: &[u8]) -> u32 {
    fn fold(mut hash: u32, bytes: &[u8]) -> u32 {
        for &byte in bytes {
            hash = (hash ^ (byte as i8 as i32 as u32)).wrapping_mul(0x1000193);
        }
        hash
    }
    let key = fold(0x811c9dc5, name);
    if key != 0 {
        key
    } else {
        fold((0x811c9dc5u32 ^ 0x20).wrapping_mul(0x1000193), name)
    }
}

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }
    pub fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(count)
            .ok_or(Error::Format("archive length overflow"))?;
        let bytes = self
            .bytes
            .get(self.at..end)
            .ok_or(Error::Format("truncated archive value"))?;
        self.at = end;
        Ok(bytes)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.u64()?))
    }
    pub fn string_bytes(&mut self) -> Result<&'a [u8]> {
        let n = self.u16()? as usize;
        self.take(n)
    }
    pub fn end(&self) -> Result<()> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(Error::Format("trailing archive bytes"))
        }
    }
}
