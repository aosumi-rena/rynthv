use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::Read,
    path::Path,
};

use num_bigint::BigUint;
use xxhash_rust::xxh3::{Xxh3, xxh3_128};
use super::{VoiceArtwork, artwork::ArtworkRole};
mod payload;

use super::super::{
    Result, VoiceError, array, config::CONFIGURATION_ID, format::FormatContext, stream, u32_at,
};

const HEADER_BYTES: usize = 0x148;
const MAX_INDEX_BYTES: usize = 16 * 1024 * 1024;
const MAX_CONFIGURATION_BYTES: usize = 64 * 1024;

pub(super) struct Configuration {
    pub metadata: super::super::config::VoiceConfiguration,
    pub model_version: u8,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub languages: Vec<String>,
    pub models: ModelPresence,
    pub icon: Option<VoiceArtwork>,
    pub portrait: Option<VoiceArtwork>,
}

#[derive(Default)]
pub(super) struct ModelPresence {
    duration: bool,
    f0: [bool; 2],
    timbre: [bool; 2],
    vocoder: bool,
}

impl ModelPresence {
    fn record(&mut self, id: [u8; 16]) {
        match u128::from_le_bytes(id) {
            0xbc72d4467151f7763681c6765678bb13 => self.duration = true,
            0xf82b01e4d6811e3929adc88975d18bdd => self.f0[0] = true,
            0xf1ae8760a5d09483514e0211f299af0f => self.timbre[0] = true,
            0x34620edf25443fa373c2ee09ae6de624 => self.f0[1] = true,
            0xa3d15b952e72382824a13092648ea34e => self.timbre[1] = true,
            0xa95bea71e057b1ae3b7924462d6ca9aa => self.vocoder = true,
            _ => {}
        }
    }

    pub(super) fn missing(&self, model_version: u8) -> Vec<&'static str> {
        let generation = usize::from(model_version - 1);
        [
            (self.duration, "时长"),
            (self.f0[generation], "音高"),
            (self.timbre[generation], "音色"),
            (self.vocoder, "声码器"),
        ]
        .into_iter()
        .filter_map(|(present, role)| (!present).then_some(role))
        .collect()
    }
}

pub(super) fn configuration(directory: &Path, context: &FormatContext) -> Result<Configuration> {
    let mut files = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.retain(|path| {
        path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dnni"))
    });
    files.sort();
    if files.is_empty() {
        return Err(VoiceError::new("该版本目录中没有 DNNI 模型文件"));
    }
    let mut configuration = None;
    let mut models = ModelPresence::default();
    let mut icon = None;
    let mut portrait = None;
    for path in files {
        let metadata = read_metadata(&path, context, &mut models)?;
        if metadata.icon.is_some() {icon=metadata.icon;}
        if metadata.portrait.is_some() {portrait=metadata.portrait;}
        if let Some(bytes) = metadata.configuration {
            if configuration.is_some() {
                return Err(VoiceError::new("声库含有多个配置资源，需要明确模型路由"));
            }
            configuration = Some(parse_configuration(&bytes)?);
        }
    }
    let mut configuration = configuration.ok_or_else(|| VoiceError::new("声库缺少名称配置资源"))?;
    configuration.models = models;
    configuration.icon = icon;
    configuration.portrait = portrait;
    Ok(configuration)
}

fn parse_configuration(bytes: &[u8]) -> Result<Configuration> {
    let text = std::str::from_utf8(bytes).map_err(|_| VoiceError::new("声库配置不是 UTF-8"))?;
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once(' ')
            .filter(|(key, value)| !key.is_empty() && (!value.is_empty() || *key == "style_config"))
            .ok_or_else(|| VoiceError::new("不支持的声库配置行格式"))?;
        if fields.insert(key, value).is_some() {
            return Err(VoiceError::new("声库配置包含重复字段"));
        }
    }
    let required = |key| {
        fields
            .get(key)
            .map(|value| (*value).to_owned())
            .ok_or_else(|| VoiceError::new(format!("声库配置缺少 {key}")))
    };
    Ok(Configuration {
        metadata: super::super::config::VoiceConfiguration::parse(bytes)?,
        model_version: match fields.get("model-version").copied() {
            None | Some("1") => 1,
            Some("2") => 2,
            _ => return Err(VoiceError::new("不支持的声库模型代际")),
        },
        name: required("name")?,
        vendor: required("vendor")?,
        version: required("version")?,
        languages: fields
            .get("default_language")
            .into_iter()
            .map(|value| (*value).to_owned())
            .collect(),
        models: ModelPresence::default(),
        icon: None,
        portrait: None,
    })
}

#[derive(Default)]
struct FileMetadata {
    configuration:Option<Vec<u8>>,
    icon:Option<VoiceArtwork>,
    portrait:Option<VoiceArtwork>,
}

fn read_metadata(
    path: &Path,
    context: &FormatContext,
    models: &mut ModelPresence,
) -> Result<FileMetadata> {
    let mut file = File::open(path)?;
    let file_length = file.metadata()?.len();
    let mut header = [0u8; HEADER_BYTES];
    file.read_exact(&mut header)?;
    if header[..8] != [0xff, 0, 0xca, 0x7f, 3, 0, 0, 0] {
        return Err(VoiceError::new("不支持的 DNNI 容器格式"));
    }
    verify_signature(&header, context)?;
    let nonce: [u8; 12] = array(&header, 0x118)?;
    let material: [u8; 32] = stream::decode(
        &header[0x124..0x144],
        &context.constants,
        &context.initializer,
        &nonce,
        0,
    )?
    .try_into()
    .expect("fixed size material");
    let count = u32_at(
        &stream::decode(&header[0x144..], &context.constants, &material, &nonce, 0)?,
        0,
    )? as usize;
    let table_length = count
        .checked_mul(56)
        .filter(|length| *length <= MAX_INDEX_BYTES)
        .ok_or_else(|| VoiceError::new("声库资源索引超过扫描大小限制"))?;
    if (HEADER_BYTES + table_length) as u64 > file_length {
        return Err(VoiceError::new("声库资源索引被截断"));
    }
    let mut table = vec![0u8; table_length];
    file.read_exact(&mut table)?;
    let checked_length = 0x118 + table_length;
    let mut digest = Xxh3::new();
    digest.update(&(checked_length as u32).to_le_bytes());
    digest.update(&nonce);
    digest.update(&material);

    let header_tail = &header[0x18..];
    digest.update(&header_tail[..checked_length.min(header_tail.len())]);
    if checked_length > header_tail.len() {
        digest.update(&table[..checked_length - header_tail.len()]);
    }
    digest.update(&material);
    digest.update(&nonce);
    if digest.digest128().to_le_bytes() != header[8..24] {
        return Err(VoiceError::new("声库资源索引校验失败"));
    }
    let table = stream::decode(&table, &context.constants, &material, &nonce, 4)?;
    let mut identities = HashSet::with_capacity(count);
    let mut metadata = FileMetadata::default();
    for record in table.as_chunks::<56>().0 {
        let id: [u8; 16] = array(record, 16)?;
        let flag = u32_at(record, 32)?;
        let offset = u32_at(record, 48)? as u64;
        let length = u32_at(record, 52)? as u64;
        if flag > 1 || !identities.insert(id) || offset + length > file_length {
            return Err(VoiceError::new("声库资源索引内容无效"));
        }
        if id == CONFIGURATION_ID.0 {
            metadata.configuration=Some(payload::read(&mut file,record,&material,context,MAX_CONFIGURATION_BYTES)?);
        } else if let Some(role) = ArtworkRole::from_resource(id) {
            let image = VoiceArtwork::embedded(payload::read(
                &mut file, record, &material, context, 8 * 1024 * 1024,
            )?)?;
            match role {
                ArtworkRole::Avatar => metadata.icon = Some(image),
                ArtworkRole::Portrait => metadata.portrait = Some(image),
            }
        }
        models.record(id);
    }
    Ok(metadata)
}

fn verify_signature(header: &[u8], context: &FormatContext) -> Result<()> {
    let mut message = Vec::with_capacity(48);
    message.push(0);
    message.extend_from_slice(&header[0x118..0x124]);
    message.extend_from_slice(&[0xff, 0xfe]);
    message.extend_from_slice(&header[0x124..0x144]);
    message.push(1);
    let decoded = BigUint::from_bytes_le(&header[0x18..0x118])
        .modpow(&BigUint::from(3u32), &context.modulus)
        .to_bytes_le();
    let mut low = [0u8; 16];
    let count = decoded.len().min(low.len());
    low[..count].copy_from_slice(&decoded[..count]);
    if low != xxh3_128(&message).to_le_bytes() {
        return Err(VoiceError::new("声库文件头校验失败"));
    }
    Ok(())
}
