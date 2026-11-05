use std::{fs::File, io::Read, path::PathBuf};

use serde::Deserialize;
use zeroize::Zeroizing;

use super::{Result, VoiceError};

const MAXIMUM_CONFIG_BYTES: u64 = 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    editor_context: Option<ContextLayout>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextLayout {
    executable_sha256: String,
    singleton_offset: usize,
    identity_pointer_offset: usize,
}

pub(super) struct EditorContext {
    pub executable_sha256: [u8; 32],
    pub singleton_offset: usize,
    pub identity_pointer_offset: usize,
}

fn invalid() -> VoiceError {
    VoiceError::new("Private runtime resources are missing or invalid")
}

fn load() -> Result<Resources> {
    let path = match std::env::var_os("RYNTHV_RUNTIME_RESOURCES") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        Some(_) => return Err(invalid()),
        None => crate::platform::config_directory()
            .map_err(|_| invalid())?
            .join("runtime-resources.json"),
    };
    let file = File::open(path).map_err(|_| invalid())?;
    let length = file.metadata().map_err(|_| invalid())?.len();
    if length == 0 || length > MAXIMUM_CONFIG_BYTES {
        return Err(invalid());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(MAXIMUM_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() as u64 != length {
        return Err(invalid());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}

fn hex_word(text: &str) -> Result<Zeroizing<[u8; 32]>> {
    if text.len() != 64 {
        return Err(invalid());
    }
    fn nibble(byte: u8) -> Result<u8> {
        match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            b'A'..=b'F' => Ok(byte - b'A' + 10),
            _ => Err(invalid()),
        }
    }
    let mut result = Zeroizing::new([0; 32]);
    for (output, pair) in result.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *output = nibble(pair[0])? << 4 | nibble(pair[1])?;
    }
    Ok(result)
}

pub(super) fn editor_context() -> Result<EditorContext> {
    let resources = load()?;
    let layout = resources.editor_context.as_ref().ok_or_else(invalid)?;
    if layout.singleton_offset == 0
        || layout.singleton_offset >= 512 * 1024 * 1024
        || layout.singleton_offset % 8 != 0
        || layout.identity_pointer_offset == 0
        || layout.identity_pointer_offset > 1024 * 1024
        || layout.identity_pointer_offset % 8 != 0
    {
        return Err(invalid());
    }
    Ok(EditorContext {
        executable_sha256: *hex_word(&layout.executable_sha256)?,
        singleton_offset: layout.singleton_offset,
        identity_pointer_offset: layout.identity_pointer_offset,
    })
}
