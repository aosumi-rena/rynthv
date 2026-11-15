use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const MAX_METADATA_BYTES: u64 = 64 * 1024;

#[derive(Debug)]
pub(crate) struct DisplayMetadata {
    pub genres: Vec<String>,
    pub pitch_range: Option<String>,
    pub languages: Vec<String>,
    pub icon_path: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub(crate) struct MetadataIndex {
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    name: String,
    vendor: String,
    display: DisplayMetadata,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataFile {
    name: String,
    vendor: String,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    pitch_range: Option<String>,
    #[serde(default)]
    languages: Vec<String>,
}

impl MetadataIndex {
    pub(crate) fn load(root: &Path) -> Self {
        let Ok(files) = fs::read_dir(root.join("meta")) else {
            return Self::default();
        };
        let entries = files
            .filter_map(Result::ok)
            .filter_map(|file| read_entry(&file.path()))
            .collect();
        Self { entries }
    }

    pub(crate) fn lookup(&self, name: &str, vendor: &str) -> Option<&DisplayMetadata> {
        let matches = self
            .entries
            .iter()
            .filter(|entry| entry.name == name && entry.vendor == vendor);

        let mut illustrated = matches.clone().filter(|entry| entry.display.icon_path.is_some());
        if let Some(entry) = illustrated.next() {
            return illustrated.next().is_none().then_some(&entry.display);
        }
        matches.max_by_key(|entry| entry.display.genres.len()+entry.display.languages.len()
            +usize::from(entry.display.pitch_range.is_some())).map(|entry| &entry.display)
    }
}

fn read_entry(path: &Path) -> Option<Entry> {
    if !path.extension()?.to_str()?.eq_ignore_ascii_case("json") {
        return None;
    }
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_METADATA_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return None;
    }
    let metadata: MetadataFile = serde_json::from_slice(&bytes).ok()?;
    if metadata.name.is_empty() || metadata.vendor.is_empty() {
        return None;
    }
    let icon_path = path.with_extension("png");
    Some(Entry {
        name: metadata.name,
        vendor: metadata.vendor,
        display: DisplayMetadata {
            genres: metadata.genres,
            pitch_range: metadata.pitch_range.filter(|range| !range.is_empty()),
            languages: metadata.languages,
            icon_path: icon_path.is_file().then_some(icon_path),
        },
    })
}
