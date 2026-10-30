use std::{
    borrow::Cow,
    collections::HashMap,
    fs,
    ops::Range,
    path::Path,
    sync::{Arc, OnceLock},
};

use num_bigint::BigUint;
use xxhash_rust::xxh3::{Xxh3, xxh3_128};

use super::{Result, VoiceError, array, format::FormatContext, product, range, stream, u32_at};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct ResourceId(pub [u8; 16]);

pub(super) struct Resource {
    pub id: ResourceId,
    pub flag: u32,
    pub range: Range<usize>,
    nonce: [u8; 12],
    digest: [u8; 16],
    verified: OnceLock<Result<()>>,
}

pub(super) struct Archive {
    bytes: Arc<[u8]>,
    constants: [u8; 16],
    material: [u8; 32],
    pub resources: Vec<Resource>,
    indices: HashMap<ResourceId, usize>,
}

impl Archive {
    pub fn load(path: &Path, context: &FormatContext) -> Result<Self> {
        let bytes: Arc<[u8]> = fs::read(path)?.into();
        if bytes.len() < 0x149 || bytes[..8] != [0xff, 0, 0xca, 0x7f, 3, 0, 0, 0] {
            return Err(VoiceError::new("Invalid DNNI v3 magic or truncated header"));
        }
        verify_signature(&bytes, context)?;
        let nonce = array(&bytes, 0x118)?;
        let material: [u8; 32] = stream::decode(
            &bytes[0x124..0x144],
            &context.constants,
            &context.initializer,
            &nonce,
            0,
        )?
        .try_into()
        .expect("32 decoded bytes");
        let count = u32_at(
            &stream::decode(
                &bytes[0x144..0x148],
                &context.constants,
                &material,
                &nonce,
                0,
            )?,
            0,
        )? as usize;
        if count > 0x07ff_ffff {
            return Err(VoiceError::new("DNNI resource count exceeds format limit"));
        }
        let table_range = range(0x148, product(count, 56)?, bytes.len())?;

        let checked_len = 0x118 + table_range.len();
        let digest_length = u32::try_from(checked_len)
            .map_err(|_| VoiceError::new("DNNI index digest length exceeds u32"))?;
        let mut digest = Xxh3::new();
        digest.update(&digest_length.to_le_bytes());
        digest.update(&nonce);
        digest.update(&material);
        digest.update(&bytes[range(0x18, checked_len, bytes.len())?]);
        digest.update(&material);
        digest.update(&nonce);
        if digest.digest128().to_le_bytes() != bytes[8..24] {
            return Err(VoiceError::new("DNNI index digest mismatch"));
        }
        let table = stream::decode(
            &bytes[table_range],
            &context.constants,
            &material,
            &nonce,
            4,
        )?;
        let mut resources = Vec::with_capacity(count);
        let mut indices = HashMap::with_capacity(count);
        for index in 0..count {
            let record = &table[index * 56..(index + 1) * 56];
            let id = ResourceId(array(record, 16)?);
            let flag = u32_at(record, 32)?;
            if flag > 1 || indices.insert(id, index).is_some() {
                return Err(VoiceError::new(format!(
                    "Invalid or duplicate DNNI resource at index {index}"
                )));
            }
            resources.push(Resource {
                id,
                flag,
                range: range(
                    u32_at(record, 48)? as usize,
                    u32_at(record, 52)? as usize,
                    bytes.len(),
                )?,
                nonce: array(record, 36)?,
                digest: array(record, 0)?,
                verified: OnceLock::new(),
            });
        }
        Ok(Self {
            bytes,
            constants: context.constants,
            material,
            resources,
            indices,
        })
    }

    pub fn file_bytes(&self) -> usize {
        self.bytes.len()
    }

    pub fn index(&self, id: ResourceId) -> Option<usize> {
        self.indices.get(&id).copied()
    }

    pub fn verify_all(&self) -> Result<()> {
        for index in 0..self.resources.len() {
            self.verify(index)?;
        }
        Ok(())
    }

    pub fn read(&self, index: usize, expected_length: Option<u32>) -> Result<Cow<'_, [u8]>> {
        let resource = self.verify(index)?;
        if expected_length.is_some_and(|len| len as usize != resource.range.len()) {
            return Err(VoiceError::new(
                "DNNI resource length differs from expected length",
            ));
        }
        let source = &self.bytes[resource.range.clone()];
        if resource.flag == 0 {
            return Ok(Cow::Borrowed(source));
        }
        Ok(Cow::Owned(stream::decode(
            source,
            &self.constants,
            &self.material,
            &resource.nonce,
            0,
        )?))
    }

    pub fn prefix(&self, index: usize, maximum: usize) -> Result<Cow<'_, [u8]>> {
        let resource = self.verify(index)?;
        let source = &self.bytes
            [resource.range.start..resource.range.start + maximum.min(resource.range.len())];
        if resource.flag == 0 {
            return Ok(Cow::Borrowed(source));
        }
        Ok(Cow::Owned(stream::decode(
            source,
            &self.constants,
            &self.material,
            &resource.nonce,
            0,
        )?))
    }

    fn verify(&self, index: usize) -> Result<&Resource> {
        let resource = self
            .resources
            .get(index)
            .ok_or_else(|| VoiceError::new("Unknown DNNI resource index"))?;
        resource
            .verified
            .get_or_init(|| {
                let length = u32::try_from(resource.range.len())
                    .ok()
                    .and_then(|value| value.checked_add(20))
                    .ok_or_else(|| VoiceError::new("DNNI resource digest length overflow"))?;
                let mut digest = Xxh3::new();
                digest.update(&length.to_le_bytes());
                digest.update(&resource.nonce);
                digest.update(&self.material);
                digest.update(&resource.id.0);
                digest.update(&resource.flag.to_le_bytes());
                digest.update(&self.bytes[resource.range.clone()]);
                digest.update(&self.material);
                digest.update(&resource.nonce);
                if digest.digest128().to_le_bytes() != resource.digest {
                    return Err(VoiceError::new(format!(
                        "DNNI resource digest mismatch at index {index}"
                    )));
                }
                Ok(())
            })
            .clone()?;
        Ok(resource)
    }
}

fn verify_signature(bytes: &[u8], context: &FormatContext) -> Result<()> {
    let mut message = Vec::with_capacity(48);
    message.push(0);
    message.extend_from_slice(&bytes[0x118..0x124]);
    message.extend_from_slice(&[0xff, 0xfe]);
    message.extend_from_slice(&bytes[0x124..0x144]);
    message.push(1);
    let signature = BigUint::from_bytes_le(&bytes[0x18..0x118]);
    let decoded = signature
        .modpow(&BigUint::from(3u32), &context.modulus)
        .to_bytes_le();
    let mut low = [0u8; 16];
    let count = decoded.len().min(16);
    low[..count].copy_from_slice(&decoded[..count]);
    if low != xxh3_128(&message).to_le_bytes() {
        return Err(VoiceError::new(
            "DNNI header signature digest-equality branch rejected",
        ));
    }
    Ok(())
}
