use super::{Result, archive::Archive};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};

const CAPACITY: u64 = 192 * 1024 * 1024;
struct Entry {
    path: PathBuf,
    length: u64,
    modified: SystemTime,
    archive: Arc<Archive>,
}
static CACHE: OnceLock<Mutex<VecDeque<Entry>>> = OnceLock::new();

pub fn open(path: &Path) -> Result<Arc<Archive>> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let length = metadata.len();
    let modified = metadata.modified().map_err(|e| e.to_string())?;
    let cache = CACHE.get_or_init(|| Mutex::new(VecDeque::new()));
    if let Ok(mut entries) = cache.lock() {
        if let Some(i) = entries
            .iter()
            .position(|e| e.path == path && e.length == length && e.modified == modified)
        {
            let entry = entries.remove(i).unwrap();
            let result = entry.archive.clone();
            entries.push_back(entry);
            return Ok(result);
        }
        entries.retain(|e| e.path != path);
    }
    let archive = Archive::read_file(&path)?;
    if length <= CAPACITY
        && let Ok(mut entries) = cache.lock()
    {
        entries.retain(|e| e.path != path);
        while entries.len() >= 2
            || entries.iter().map(|e| e.length).sum::<u64>() + length > CAPACITY
        {
            entries.pop_front();
        }
        entries.push_back(Entry {
            path,
            length,
            modified,
            archive: archive.clone(),
        });
    }
    Ok(archive)
}
