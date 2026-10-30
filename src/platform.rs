use std::path::PathBuf;

#[cfg(windows)]
mod windows;

pub fn context_directory() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        windows::roaming_directory().map(|root| root.join("RynthV"))
    }
    #[cfg(not(windows))]
    {
        config_directory()
    }
}

pub fn config_directory() -> Result<PathBuf, String> {
    config_root().map(|root| root.join("RynthV"))
}

#[cfg(windows)]
fn config_root() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "LOCALAPPDATA 未设置，无法定位用户配置目录。".into())
}

#[cfg(target_os = "linux")]
fn config_root() -> Result<PathBuf, String> {

    if let Some(root) = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Ok(root);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|home| home.join(".config"))
        .ok_or_else(|| "XDG_CONFIG_HOME 和 HOME 未设置为绝对路径，无法定位用户配置目录。".into())
}

#[cfg(target_os = "macos")]
fn config_root() -> Result<PathBuf, String> {
    // macOS convention: ~/Library/Application Support. $HOME is always absolute.
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|home| home.join("Library").join("Application Support"))
        .ok_or_else(|| "HOME 未设置为绝对路径，无法定位用户配置目录。".into())
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn config_root() -> Result<PathBuf, String> {
    Err("当前平台尚未支持用户配置目录。".into())
}
