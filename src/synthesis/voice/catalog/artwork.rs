use std::{path::PathBuf,sync::Arc};
use serde::Serialize;

pub(super) enum ArtworkRole {
    Avatar,
    Portrait,
}

impl ArtworkRole {
    pub(super) fn from_resource(id: [u8; 16]) -> Option<Self> {
        match u128::from_be_bytes(id) {
            0xb8ee1171c55fb5927807ea16c297a90b
            | 0xa6417d1ec535696a90c40e50c2e08b17
            | 0x99eff93490e55c939ed832f19d9542ec
            | 0x7605328cd3c57e0a0e271b4c590ef368
            | 0xdb7c34f026a76005fbc94064770cf7a9 => Some(Self::Avatar),
            0x175803adc810aba27ec51bb4df92d5e3
            | 0xd9034be2ee55b382bf88c39ef6a56150
            | 0x6548e7845b2ad4599d1416ed08e96272 => Some(Self::Portrait),
            _ => None,
        }
    }
}

#[derive(Clone,Debug,Serialize)]
#[serde(tag="source",rename_all="camelCase")]
pub enum VoiceArtwork {
    File {path:PathBuf},
    Embedded {uri:String,#[serde(skip)] bytes:Arc<[u8]>},
}

impl VoiceArtwork {
    pub(super) fn embedded(bytes:Vec<u8>) -> super::Result<Self> {
        if bytes.len()<24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(super::super::VoiceError::new("声库图像不是 PNG"));
        }
        let width=u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let height=u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        if width==0 || height==0 || width>8192 || height>16384 || u64::from(width)*u64::from(height)>32_000_000 {
            return Err(super::super::VoiceError::new("声库图像尺寸无效或超过限制"));
        }
        let uri=format!("bytes://voice-artwork/{:032x}.png",xxhash_rust::xxh3::xxh3_128(&bytes));
        Ok(Self::Embedded {uri,bytes:bytes.into()})
    }
}
