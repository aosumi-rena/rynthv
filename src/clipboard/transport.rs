use std::{fs, io::Read, path::PathBuf};

fn path() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let root = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(target_os = "linux")]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let root: Option<PathBuf> = None;
    root.filter(|p| p.is_absolute())
        .map(|p| p.join("Dreamtonics/shared/clipboard"))
        .ok_or_else(|| "剪贴板目录不可用。".into())
}
pub(super) fn read() -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let file = fs::File::open(path()?).map_err(|_| "剪贴板数据不可用。")?;
    file.take(super::MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "剪贴板数据不可用。")?;
    if bytes.len() as u64 > super::MAX_BYTES {
        return Err("剪贴板数据过大。".into());
    }
    Ok(bytes)
}
pub(super) fn write(bytes: &[u8]) -> Result<(), String> {
    let path = path()?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|_| "剪贴板目录不可用。")?;
    crate::storage::atomic_write(&path, bytes)
}
