use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    ops::Range,
    path::{Path, PathBuf},
};

use super::super::{Result, VoiceError};

const MAGIC: [u8; 8] = [0x80, 0xf5, 0, 0, 0x0a, 0, 0, 0];
const INDEX_START: usize = 16;
const INDEX_BYTES: usize = 4096;
const RECORDS_START: usize = INDEX_START + INDEX_BYTES;
const INDEX_ENTRIES_START: usize = INDEX_START + 6;
const INDEX_FOOTER: usize = RECORDS_START - 4;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 64 * 1024;

pub(in crate::synthesis::voice) struct NofsRecord {
    pub key: Vec<u8>,
    pub range: Range<u64>,
}

pub(in crate::synthesis::voice) struct NofsArchive {
    pub path: PathBuf,
    pub length: u64,
    pub records: Vec<NofsRecord>,
    file: File,
}

impl NofsArchive {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let length = metadata.len();
        if !metadata.is_file() || !(RECORDS_START as u64..=MAX_FILE_BYTES).contains(&length) {
            return Err(VoiceError::new("Invalid NOFS file size or file type"));
        }
        let header = read_array::<RECORDS_START>(&mut file, 0)?;
        if header[..8] != MAGIC {
            return Err(VoiceError::new("Invalid NOFS signature"));
        }
        if u64::from_le_bytes(header[8..16].try_into().expect("eight bytes")) != length {
            return Err(VoiceError::new(
                "NOFS declared length differs from file size",
            ));
        }

        if header[16..20] != (INDEX_BYTES as u32).to_le_bytes()
            || header[20..22] != 0x1000u16.to_le_bytes()
            || header[INDEX_FOOTER..RECORDS_START] != (INDEX_BYTES as u32).to_le_bytes()
        {
            return Err(VoiceError::new("Unsupported NOFS index block layout"));
        }
        let indexed_offsets = parse_index(&header, length)?;
        let mut keys = HashSet::with_capacity(indexed_offsets.len());
        let mut records = Vec::with_capacity(indexed_offsets.len());
        let mut key_bytes = 0usize;
        // The index is authoritative. Original banks may retain superseded
        // physical records after an in-place metadata update; those records
        // are not members of the current archive and must not be exposed.
        let mut offsets:Vec<_>=indexed_offsets.iter().copied().collect();
        offsets.sort_unstable();
        let mut previous_end=RECORDS_START as u64;
        for offset in offsets {
            if offset<previous_end {return Err(VoiceError::new("NOFS indexed records overlap"));}
            if length - offset < 16 {
                return Err(VoiceError::new("Truncated NOFS record header"));
            }
            let record_header = read_array::<8>(&mut file, offset)?;
            let record_bytes =
                u32::from_le_bytes(record_header[..4].try_into().expect("four bytes")) as u64;
            let kind = u16::from_le_bytes(record_header[4..6].try_into().expect("two bytes"));
            let key_length =
                u16::from_le_bytes(record_header[6..8].try_into().expect("two bytes")) as usize;
            if kind != 1 || record_bytes < 16 + key_length as u64 || record_bytes > length - offset
            {
                return Err(VoiceError::new("Invalid NOFS record kind or bounds"));
            }
            key_bytes += key_length;
            if key_bytes > MAX_METADATA_BYTES {
                return Err(VoiceError::new("NOFS record keys exceed metadata limit"));
            }
            let mut key = vec![0; key_length];
            file.read_exact(&mut key)?;
            if !keys.insert(key.clone()) {
                return Err(VoiceError::new("Duplicate NOFS record key"));
            }
            let value_length =
                u32::from_le_bytes(read_array::<4>(&mut file, offset + 8 + key_length as u64)?)
                    as u64;
            if record_bytes != 16 + key_length as u64 + value_length {
                return Err(VoiceError::new("NOFS record lengths disagree"));
            }
            let end = offset + record_bytes;
            if u32::from_le_bytes(read_array::<4>(&mut file, end - 4)?) as u64 != record_bytes {
                return Err(VoiceError::new("Invalid NOFS record footer"));
            }
            let start = offset + 12 + key_length as u64;
            records.push(NofsRecord {
                key,
                range: start..end - 4,
            });
            previous_end = end;
        }
        if records.len() != indexed_offsets.len() {
            return Err(VoiceError::new(
                "NOFS index contains missing record references",
            ));
        }
        if file.metadata()?.len() != length {
            return Err(VoiceError::new("NOFS file changed while reading its index"));
        }
        Ok(Self {
            path: path.to_owned(),
            length,
            records,
            file,
        })
    }

    pub fn read(&mut self, index: usize, max_bytes: usize) -> Result<Vec<u8>> {
        let range = self.record_range(index)?;
        let length = usize::try_from(range.end - range.start)
            .map_err(|_| VoiceError::new("NOFS value exceeds addressable memory"))?;
        if length > max_bytes {
            return Err(VoiceError::new("NOFS value exceeds requested read limit"));
        }
        self.read_value(range.start, length)
    }

    pub fn prefix(&mut self, index: usize, max_bytes: usize) -> Result<Vec<u8>> {
        if max_bytes > MAX_METADATA_BYTES {
            return Err(VoiceError::new("NOFS prefix exceeds metadata read limit"));
        }
        let range = self.record_range(index)?;
        let length = (range.end - range.start).min(max_bytes as u64) as usize;
        self.read_value(range.start, length)
    }

    fn record_range(&self, index: usize) -> Result<Range<u64>> {
        self.records
            .get(index)
            .map(|record| record.range.clone())
            .ok_or_else(|| VoiceError::new("Unknown NOFS record index"))
    }

    fn read_value(&mut self, offset: u64, length: usize) -> Result<Vec<u8>> {
        if self.file.metadata()?.len() != self.length {
            return Err(VoiceError::new("NOFS file size changed after inventory"));
        }
        self.file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; length];
        self.file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

fn parse_index(header: &[u8; RECORDS_START], file_length: u64) -> Result<HashSet<u64>> {
    let mut offsets = HashSet::new();
    let mut offset = INDEX_ENTRIES_START;
    while offset + 12 <= INDEX_FOOTER {
        let entry = &header[offset..offset + 12];
        if entry.iter().all(|byte| *byte == 0) {
            break;
        }

        let record_offset = u64::from_le_bytes(entry[4..12].try_into().expect("eight bytes"));
        if !(RECORDS_START as u64..file_length).contains(&record_offset)
            || !offsets.insert(record_offset)
        {
            return Err(VoiceError::new("Invalid or duplicate NOFS index reference"));
        }
        offset += 12;
    }
    if header[offset..INDEX_FOOTER].iter().any(|byte| *byte != 0) {
        return Err(VoiceError::new("Nonzero NOFS index padding"));
    }
    Ok(offsets)
}

fn read_array<const N: usize>(file: &mut File, offset: u64) -> Result<[u8; N]> {
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = [0; N];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}
