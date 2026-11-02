//! Studio's shared JSON clipboard and its system-clipboard summary marker.
mod content;
mod transport;
#[cfg(windows)]
mod windows;
pub use content::Content;
use serde::{Deserialize, Serialize};
pub const PREFIX: &str = "SVR2CLIP\n";
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Summary {
    pub notes: usize,
    pub groups: usize,
    pub has_parameters: bool,
    pub pitch_controls: usize,
}

pub fn can_paste() -> bool {
    #[cfg(windows)]
    {
        windows::summary().is_some_and(|s| {
            s.notes > 0 || s.groups > 0 || s.has_parameters || s.pitch_controls > 0
        })
    }
    #[cfg(not(windows))]
    {
        true
    }
}

pub fn can_paste_groups() -> bool {
    #[cfg(windows)]
    {
        windows::summary().is_some_and(|s| s.groups > 0)
    }
    #[cfg(not(windows))]
    {
        true
    }
}

pub fn read(text: &str) -> Result<Content, String> {
    let header = text
        .strip_prefix(PREFIX)
        .or_else(|| text.strip_prefix("SVR2CLIP\r\n"))
        .ok_or("剪贴板数据无效。")?;
    let summary: Summary = serde_json::from_str(header).map_err(|_| "剪贴板数据无效。")?;
    let content = Content::decode(&transport::read()?)?;
    if content.summary() != summary {
        return Err("剪贴板数据已改变。".into());
    }
    Ok(content)
}

pub fn write(content: &Content) -> Result<String, String> {
    content.validate()?;
    let bytes = serde_json::to_vec_pretty(content).map_err(|_| "剪贴板数据无效。")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("剪贴板数据过大。".into());
    }
    transport::write(&bytes)?;
    Ok(format!(
        "{PREFIX}{}",
        serde_json::to_string(&content.summary()).map_err(|_| "剪贴板数据无效。")?
    ))
}
