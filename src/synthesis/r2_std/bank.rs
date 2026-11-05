// SPDX-License-Identifier: MIT
use super::{
    Error, Result,
    archive::{Archive, Reader},
};
use std::{collections::BTreeMap, path::Path};

pub(crate) struct BankProfile {
    pub name: String,
    pub version: String,
    pub language: String,
    pub phoneset: String,
    pub encoder_only: bool,
    pub source_period: f64,
    pub bytes: usize,
    pub metadata_warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct TimelinePhone {
    pub end_seconds: f64,
    pub symbol: String,
    pub raw_flags: u8,
}

pub(crate) struct SourceUnit {
    pub ordinal: usize,
    pub logical_group: Vec<u8>,
    pub directory: Vec<u8>,
    pub policy: Vec<u8>,
    pub entry_name: Vec<u8>,
    pub timeline: Vec<TimelinePhone>,
    pub(super) acoustic_name: Vec<u8>,
    pub(super) stored_name: Vec<u8>,
}

pub(crate) struct StdBank {
    pub profile: BankProfile,
    pub units: Vec<SourceUnit>,
    pub(super) archive: Archive,
    pub(super) configuration: Configuration,
}

impl StdBank {
    pub(super) fn require_shared_f0(&self) -> Result<()> {
        for name in [
            "f0model",
            "f0model-g2a",
            "f0model-g2b",
            "f0model-g3",
            "f0model-g4",
            "f0model-g5",
            "f0model-g5d",
            "f0model-dds",
            "f0model-dds2",
            "f0model-dds3",
        ] {
            if self.archive.named_optional(name.as_bytes())?.is_some() {
                return Err(Error::Unsupported(
                    "bank-owned F0 model route cannot be replaced by shared loose F0",
                ));
            }
        }
        Ok(())
    }
    pub fn open(path: &Path) -> Result<Self> {
        let archive = Archive::open(path)?;
        let configuration = Configuration::parse(archive.named(&[0x91])?)?;
        let mut metadata_warnings = Vec::new();
        let mut text = |name: &[u8]| -> String {
            match archive.named_optional(name).and_then(|bytes| {
                std::str::from_utf8(bytes.unwrap_or_default())
                    .map(|v| v.trim_end_matches('\0').to_owned())
                    .map_err(|_| Error::Format("bank metadata UTF-8"))
            }) {
                Ok(value) => value,
                Err(error) => {
                    metadata_warnings.push(format!("{}: {error}", String::from_utf8_lossy(name)));
                    String::new()
                }
            }
        };
        let configured_language = configuration.single_text("language")?;
        let configured_phoneset = configuration.single_text("phoneset")?;
        let language = text(b".language");
        let language = if language.is_empty() {
            configured_language.clone()
        } else {
            language
        };
        let phoneset = text(b".phoneset");
        let phoneset = if phoneset.is_empty() {
            configured_phoneset.clone()
        } else {
            phoneset
        };
        if language != configured_language || phoneset != configured_phoneset {
            return Err(Error::Format("bank/config language or phoneset differs"));
        }
        let encoder_only = configuration.flag("use_enc_only")?;
        if !configuration.flag("ac_enc")?
            || !configuration.flag("ac_thumb")?
            || configuration.single_text("ac_ext")? != "lcod"
            || configuration.single_text("voc_ext")? != "llsm"
            || configuration.numbers("ac_order")? != [72.0]
        {
            return Err(Error::Unsupported(
                "STD LCOD72 unit-source configuration required",
            ));
        }
        if encoder_only {
            if language != "japanese" || phoneset != "romaji" {
                return Err(Error::Unsupported("encoder-only language/phoneset"));
            }
        } else if language != "mandarin" || phoneset != "xsampa" {
            return Err(Error::Unsupported("stored-unit language/phoneset"));
        }
        let models = configuration.strings("unitsynth")?;
        if models.len() != 2 || models.iter().any(|name| name.is_empty() != encoder_only) {
            return Err(Error::Unsupported(
                "directional versus encoder-only model declaration",
            ));
        }
        let hop = configuration.numbers("ac_thop")?;
        if hop.len() != 2
            || !hop.iter().all(|x| x.is_finite() && *x > 0.0)
            || hop[0] >= 2147483648.0
        {
            return Err(Error::Unsupported("acoustic source clock"));
        }
        let source_period = f64::from(hop[0] as i32) / hop[1];
        if !source_period.is_finite() || source_period <= 0.0 {
            return Err(Error::Format("nonpositive source period"));
        }
        let mapping = configuration.strings("group_mapdir")?;
        if mapping.is_empty() || mapping.len() % 3 != 0 {
            return Err(Error::Format("source-directory triplets"));
        }
        let mut units = Vec::new();
        let mut physical = std::collections::HashSet::new();
        for triple in mapping.chunks_exact(3) {
            let [logical, directory, policy] = triple else {
                unreachable!()
            };
            if directory == b"singing" {
                continue;
            }
            if logical.is_empty()
                || directory.is_empty()
                || !physical.insert(directory.clone())
                || directory == b"voice-config"
                || directory == b"target-rules"
            {
                return Err(Error::Unsupported(
                    "empty/reserved/repeated source directory",
                ));
            }
            let mut index_name = directory.clone();
            index_name.extend_from_slice(&[0x82, 0x93]);
            let mut reader = Reader::new(archive.named(&index_name)?);
            if reader.u32()? != 0xfeff
                || reader.u32()? != 1
                || reader.u32()? != 3
                || reader.u32()? != 0
                || decode_key(reader.string_bytes()?)? != "entries"
            {
                return Err(Error::Unsupported("source group index schema"));
            }
            let count = reader.u32()? as usize;
            if count == 0 || count > 4096 || count > reader.remaining() / 2 {
                return Err(Error::Format("source group entry count"));
            }
            for _ in 0..count {
                let name = reader.string_bytes()?.to_vec();
                if name.len() > 4096 || units.len() >= 4096 {
                    return Err(Error::Unsupported("unit catalogue extent"));
                }
                let ordinal = units.len();
                let effective = if name.is_empty() {
                    ordinal.to_string().into_bytes()
                } else {
                    name.clone()
                };
                let timeline_name = role_name(directory, &effective, 0xa1, b"txt");
                let acoustic_name = role_name(directory, &effective, 0xa3, b"lcod");
                let stored_name = role_name(directory, &effective, 0xa2, b"llsm");
                let timeline = read_timeline(archive.named(&timeline_name)?)?;
                archive.descriptor(&acoustic_name)?;
                if !encoder_only {
                    archive.descriptor(&stored_name)?;
                }
                units.push(SourceUnit {
                    ordinal,
                    logical_group: logical.clone(),
                    directory: directory.clone(),
                    policy: policy.clone(),
                    entry_name: name,
                    timeline,
                    acoustic_name,
                    stored_name,
                });
            }
            reader.end()?;
        }
        if units.is_empty() {
            return Err(Error::Format("empty unit catalogue"));
        }
        let profile = BankProfile {
            name: text(b".name"),
            version: text(b".version"),
            language,
            phoneset,
            encoder_only,
            source_period,
            bytes: archive.len(),
            metadata_warnings,
        };
        Ok(Self {
            profile,
            units,
            archive,
            configuration,
        })
    }
}

fn role_name(directory: &[u8], entry: &[u8], role: u8, extension: &[u8]) -> Vec<u8> {
    let mut bytes = directory.to_vec();
    bytes.extend_from_slice(&[0x82, role, 0x82]);
    bytes.extend_from_slice(entry);
    bytes.push(0x81);
    bytes.extend_from_slice(extension);
    bytes
}
fn read_timeline(bytes: &[u8]) -> Result<Vec<TimelinePhone>> {
    let mut reader = Reader::new(bytes);
    if reader.u32()? != 0xfcfc {
        return Err(Error::Format("unit timeline header"));
    }
    let count = reader.u32()? as usize;
    if count == 0 || count > reader.remaining() / 11 {
        return Err(Error::Format("unit timeline count"));
    }
    let mut result = Vec::with_capacity(count);
    let mut previous = 0.0;
    for _ in 0..count {
        let end = reader.f64()?;
        let raw_flags = reader.u8()?;
        let text = reader.string_bytes()?;
        if !end.is_finite() || end < previous || text.is_empty() {
            return Err(Error::Format("unit timeline endpoint/label"));
        }
        let symbol = std::str::from_utf8(text)
            .map_err(|_| Error::Format("timeline label UTF-8"))?
            .to_owned();
        result.push(TimelinePhone {
            end_seconds: end,
            symbol,
            raw_flags,
        });
        previous = end;
    }
    reader.end()?;
    Ok(result)
}

pub(super) enum Values {
    Strings(Vec<Vec<u8>>),
    Numbers(Vec<f64>),
    Raw(Vec<u32>),
}
pub(super) struct Configuration(
    BTreeMap<String, Values>,
    BTreeMap<String, f64>,
    BTreeMap<String, Vec<Vec<Vec<u8>>>>,
);
impl Configuration {
    fn parse(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(bytes);
        if reader.u32()? != 0xfeff {
            return Err(Error::Format("voice configuration header"));
        }
        let count = reader.u32()? as usize;
        if count > reader.remaining() / 14 {
            return Err(Error::Format("configuration row count"));
        }
        let mut fields = BTreeMap::new();
        let mut first_numbers = BTreeMap::new();
        let mut string_rows: BTreeMap<String, Vec<Vec<Vec<u8>>>> = BTreeMap::new();
        for row in 0..count {
            let kind = reader.u32()?;
            if kind > 2 || reader.u32()? as usize != row {
                return Err(Error::Format("configuration row kind/index"));
            }
            let name = decode_key(reader.string_bytes()?)?;
            let n = reader.u32()? as usize;
            if n > reader.remaining()
                / match kind {
                    0 => 2,
                    1 => 8,
                    _ => 4,
                }
            {
                return Err(Error::Format("configuration value count"));
            }
            let value = match kind {
                0 => Values::Strings(
                    (0..n)
                        .map(|_| reader.string_bytes().map(<[u8]>::to_vec))
                        .collect::<Result<_>>()?,
                ),
                1 => Values::Numbers((0..n).map(|_| reader.f64()).collect::<Result<_>>()?),
                _ => Values::Raw((0..n).map(|_| reader.u32()).collect::<Result<_>>()?),
            };
            if let Values::Strings(values) = &value {
                string_rows
                    .entry(name.clone())
                    .or_default()
                    .push(values.clone());
            }
            if let Values::Numbers(values) = &value {
                if let Some(&first) = values.first() {
                    first_numbers.insert(name.clone(), first);
                }
            }
            if let Some(old) = fields.get_mut(&name) {
                match (old, value) {
                    (Values::Strings(a), Values::Strings(b)) => a.extend(b),
                    (Values::Numbers(a), Values::Numbers(b)) => a.extend(b),
                    (Values::Raw(a), Values::Raw(b)) => a.extend(b),
                    _ => return Err(Error::Format("configuration repeated key changes type")),
                }
            } else {
                fields.insert(name, value);
            }
        }
        reader.end()?;
        Ok(Self(fields, first_numbers, string_rows))
    }
    pub fn string_rows(&self, key: &str) -> &[Vec<Vec<u8>>] {
        self.2.get(key).map(Vec::as_slice).unwrap_or_default()
    }
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
    pub fn strings(&self, key: &str) -> Result<&[Vec<u8>]> {
        match self.0.get(key) {
            Some(Values::Strings(v)) => Ok(v),
            _ => Err(Error::Format("required configuration strings absent")),
        }
    }
    pub fn optional_strings(&self, key: &str) -> Result<&[Vec<u8>]> {
        match self.0.get(key) {
            None => Ok(&[]),
            Some(Values::Strings(v)) => Ok(v),
            _ => Err(Error::Format("optional configuration strings type")),
        }
    }
    pub fn numbers(&self, key: &str) -> Result<&[f64]> {
        match self.0.get(key) {
            Some(Values::Numbers(v)) => Ok(v),
            _ => Err(Error::Format("required configuration numbers absent")),
        }
    }
    pub fn optional_first_number(&self, key: &str) -> Result<Option<f64>> {
        match self.0.get(key) {
            None => Ok(None),
            Some(Values::Numbers(values)) if !values.is_empty() => Ok(self.1.get(key).copied()),
            _ => Err(Error::Format("optional numeric configuration type")),
        }
    }
    pub fn single_text(&self, key: &str) -> Result<String> {
        let strings = self.strings(key)?;
        if strings.len() != 1 {
            return Err(Error::Format("configuration string cardinality"));
        }
        std::str::from_utf8(&strings[0])
            .map(str::to_owned)
            .map_err(|_| Error::Format("configuration string UTF-8"))
    }
    pub(super) fn flag(&self, key: &str) -> Result<bool> {
        match self.0.get(key) {
            Some(Values::Raw(v)) if v.len() == 1 => Ok(v[0] & 1 != 0),
            _ => Err(Error::Format("configuration flag cardinality/type")),
        }
    }
}
fn decode_key(bytes: &[u8]) -> Result<String> {
    let mut state = 0xbefcb5e0u32;
    let clear: Vec<u8> = bytes
        .iter()
        .map(|byte| {
            let value = byte ^ (state as u8);
            state = state.wrapping_mul(0x41c64e6d).wrapping_add(0x3039);
            value
        })
        .collect();
    String::from_utf8(clear).map_err(|_| Error::Format("configuration key UTF-8"))
}
