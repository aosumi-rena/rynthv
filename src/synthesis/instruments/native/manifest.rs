use blowfish::{
    BlowfishLE,
    cipher::{Block, BlockCipherDecrypt, KeyInit},
};
use std::{collections::BTreeMap, path::Path};
pub fn vendor(directory: &Path, name: &str, revision: &str) -> Option<String> {
    let path = directory.join("wm");
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() == 0 || meta.len() > 65536 || meta.len() % 8 != 0 {
        return None;
    }
    let mut bytes = std::fs::read(&path).ok()?;
    let binding = std::path::absolute(&path).ok()?;
    let binding = binding.to_string_lossy();
    #[cfg(windows)]
    let binding = binding.replace('/', "\\");
    let key = binding
        .bytes()
        .fold(0u64, |h, b| h.wrapping_mul(0x65).wrapping_add(b as u64));
    let cipher = BlowfishLE::new_from_slice(&key.to_le_bytes()).ok()?;
    for block in bytes.chunks_exact_mut(8) {
        cipher.decrypt_block(Block::<BlowfishLE>::from_mut_slice(block));
    }
    let pad = *bytes.last()? as usize;
    if pad == 0
        || pad > 8
        || bytes[bytes.len() - pad..]
            .iter()
            .any(|b| *b as usize != pad)
    {
        return None;
    }
    let text = std::str::from_utf8(&bytes[..bytes.len() - pad]).ok()?;
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (k, v) = line.split_once('=')?;
        if fields.insert(k, v).is_some() {
            return None;
        }
    }
    if fields.get("0") != Some(&name) || fields.get("1") != Some(&revision) {
        return None;
    }
    fields.get("2").map(|v| v.to_string())
}
