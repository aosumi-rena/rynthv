mod basis;

use super::{Result, VoiceError, array, range, sum, u32_at};
use std::collections::BTreeMap;

pub const PHONETIC_MAP_TYPE_ID: [u8; 16] = [
    0x1a, 0xd8, 0xac, 0x33, 0x42, 0xf3, 0x0a, 0xed, 0x5b, 0x30, 0xff, 0x9b, 0xa9, 0xf0, 0x78, 0x21,
];
const FIELDS: [&str; 16] = [
    "system_category",
    "voicing",
    "obstruent",
    "rhoticity",
    "nasality",
    "v_height",
    "v_front",
    "rounding",
    "c_stricture",
    "c_place",
    "c2_place",
    "aspiration",
    "tenseness",
    "v2_height",
    "v2_front",
    "v2_rounding",
];

pub struct PhoneticMap {
    pub languages: Vec<String>,
    entries: BTreeMap<String, [f32; 16]>,
}

impl PhoneticMap {

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if array::<16>(bytes, 0)? != PHONETIC_MAP_TYPE_ID {
            return Err(VoiceError::new("Resource is not a phonetic descriptor map"));
        }
        let mut reader = Reader { bytes, offset: 16 };
        let languages = reader.strings()?;
        let fields = reader.strings()?;
        if languages.is_empty() || fields.len() != FIELDS.len() {
            return Err(VoiceError::new(
                "Phonetic descriptor schema has invalid dimensions",
            ));
        }
        let indices = FIELDS
            .iter()
            .map(|name| {
                fields
                    .iter()
                    .position(|f| f == name)
                    .ok_or_else(|| VoiceError::new(format!("Missing phonetic descriptor {name}")))
            })
            .collect::<Result<Vec<_>>>()?;
        let enum_count = reader.count(8)?;
        let mut enums = BTreeMap::new();
        for _ in 0..enum_count {
            let name = reader.string()?;
            let count = reader.count(8)?;
            let mut values = BTreeMap::new();
            for _ in 0..count {
                let label = reader.string()?;
                let value = reader.scalar()?;
                if !value.is_finite() || values.insert(label, value).is_some() {
                    return Err(VoiceError::new("Invalid phonetic descriptor enumeration"));
                }
            }
            if enums.insert(name, values).is_some() {
                return Err(VoiceError::new("Duplicate phonetic descriptor enumeration"));
            }
        }

        if reader.word()? != 1000 {
            return Err(VoiceError::new("Unknown phonetic descriptor quantization"));
        }
        let count = reader.count(4 + 16 * 4)?;
        let mut entries = BTreeMap::new();
        for _ in 0..count {
            let key = reader.string()?;
            let mut stored = [0.0; 16];
            for value in &mut stored {
                *value = reader.scalar()?;
            }
            let row = std::array::from_fn(|i| stored[indices[i]]);
            if !key.contains('*') || entries.insert(key, row).is_some() {
                return Err(VoiceError::new(
                    "Invalid or duplicate phonetic descriptor key",
                ));
            }
        }
        if reader.offset != bytes.len() || entries.is_empty() {
            return Err(VoiceError::new(
                "Phonetic descriptor map has trailing or missing entries",
            ));
        }
        Ok(Self { languages, entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn encode(&self, language: &str, token: &str) -> Result<[f32; 73]> {
        let token=super::resource_phone_symbol(token).unwrap_or(token);
        let key = format!("{token}*{language}");
        let common = format!("{token}*");
        let row = self
            .entries
            .get(&key)
            .or_else(|| self.entries.get(&common))
            .ok_or_else(|| VoiceError::new(format!("No phonetic descriptors for {key}")))?;
        basis::encode(row)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn word(&mut self) -> Result<u32> {
        let v = u32_at(self.bytes, self.offset)?;
        self.offset = sum(self.offset, 4)?;
        Ok(v)
    }
    fn count(&mut self, minimum_bytes: usize) -> Result<usize> {
        let count = self.word()? as usize;
        if count > (self.bytes.len() - self.offset) / minimum_bytes {
            return Err(VoiceError::new(
                "Phonetic metadata count exceeds remaining bytes",
            ));
        }
        Ok(count)
    }
    fn string(&mut self) -> Result<String> {
        let n = self.word()? as usize;
        let bounds = range(self.offset, n, self.bytes.len())?;
        let s = std::str::from_utf8(&self.bytes[bounds.clone()])
            .map_err(|_| VoiceError::new("Phonetic metadata contains invalid UTF-8"))?;
        self.offset = bounds.end;
        Ok(s.to_owned())
    }
    fn strings(&mut self) -> Result<Vec<String>> {
        let n = self.count(4)?;
        (0..n).map(|_| self.string()).collect()
    }
    fn scalar(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.word()?))
    }
}
