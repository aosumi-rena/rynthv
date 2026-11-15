mod imported;
mod online;
#[cfg(windows)]
mod persisted;
mod platform;
mod session;

#[cfg(windows)]
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use zeroize::Zeroize;

use super::{Result, VoiceError};

pub use online::{
    OnlineContextSource, import_online_context, online_context_sources, supports_online_import,
};
pub use session::{import_session_context, supports_session_import, resolve_session_identity};
pub use session::{SessionCondition, SessionProtection, resolve_session_condition};
pub use session::{resolve_session_condition_with_key, import_session_context_with_key};

pub fn saved_identity(_voice_name: &str, _model_version: u8) -> Result<u64> { Ok(0) }

pub fn save_imported_identity(
    voice_name: &str,
    model_version: u8,
    identity: u64,
) -> Result<PathBuf> {
    validate_voice_name(voice_name)?;
    model_suffix(model_version)?;
    imported::save_bound(voice_name, model_version, identity)
}

pub fn import_context_file(voice_name: &str, model_version: u8, path: &Path) -> Result<PathBuf> {
    save_manual_identity(voice_name, model_version, resolve_context_identity(voice_name, model_version, path)?)
}

pub fn resolve_context_identity(voice_name: &str, model_version: u8, path: &Path) -> Result<u64> {
    validate_voice_name(voice_name)?;
    model_suffix(model_version)?;
    let identity = imported::read_source(voice_name, model_version, path)?;
    Ok(identity)
}

pub fn save_manual_identity(voice_name: &str, model_version: u8, identity: u64) -> Result<PathBuf> {
    validate_voice_name(voice_name)?;
    model_suffix(model_version)?;
    imported::save_manual(voice_name, model_version, identity)
}

pub fn parse_identity(text: &str) -> Result<u64> {
    let text = text.trim();
    let (digits, radix) = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .map_or((text, 10), |digits| (digits, 16));
    if digits.is_empty()
        || !digits.bytes().all(|byte| match radix {
            16 => byte.is_ascii_hexdigit(),
            _ => byte.is_ascii_digit(),
        })
    {
        return Err(VoiceError::new(
            "模型 identity 必须是完整的无符号 64 位整数，支持十进制或 0x 开头的十六进制。",
        ));
    }
    u64::from_str_radix(digits, radix)
        .map_err(|_| VoiceError::new("模型 identity 超出无符号 64 位整数范围。"))
}

pub fn parse_session_key(text: &str) -> Result<u64> {
    let text = text.trim();
    let (digits, radix) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
        .map_or((text, 10), |digits| (digits, 16));
    if digits.is_empty() || !digits.bytes().all(|b| if radix == 16 {b.is_ascii_hexdigit()} else {b.is_ascii_digit()}) {
        return Err(VoiceError::new("session 解密密钥必须是完整的无符号 64 位整数，支持十进制或 0x 十六进制。"));
    }
    u64::from_str_radix(digits, radix).map_err(|_| VoiceError::new("session 解密密钥超出无符号 64 位整数范围。"))
}

fn validate_voice_name(voice_name: &str) -> Result<()> {
    if voice_name.trim().is_empty() {
        return Err(VoiceError::new(
            "A voice name is required for identity conditioning",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn derive_local_identity(session_field: &[u8], voice_name: &str, model_version: u8) -> Result<u64> {
    if session_field.is_empty() {
        return Err(VoiceError::new(
            "The installed voice session identity is unavailable",
        ));
    }
    validate_voice_name(voice_name)?;
    let mut machine = platform::machine_identity()?;
    let mut hash = Sha256::new();
    hash.update(session_field);
    hash.update(machine.as_bytes());
    machine.zeroize();
    hash.update(voice_name.as_bytes());
    hash.update(model_suffix(model_version)?);
    let digest = hash.finalize();
    Ok(u64::from_be_bytes(
        digest[..8]
            .try_into()
            .expect("SHA-256 has eight prefix bytes"),
    ))
}

fn model_suffix(model_version: u8) -> Result<&'static [u8]> {

    match model_version {
        1 => Ok(b"v1"),
        2 => Ok(b"v2"),
        _ => Err(VoiceError::new("Unsupported model identity generation")),
    }
}
