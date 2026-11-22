use super::{Result, archive::Archive};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};

const CAPACITY: usize = 8;
struct Entry {
    path: PathBuf,
    length: u64,
    modified: SystemTime,
    accept_any_v4_signature: bool,
    archive: Arc<OnceLock<Result<Arc<Archive>>>>,
}
static CACHE: OnceLock<Mutex<VecDeque<Entry>>> = OnceLock::new();

pub fn open(path: &Path, accept_any_v4_signature: bool) -> Result<Arc<Archive>> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let length = metadata.len();
    let modified = metadata.modified().map_err(|e| e.to_string())?;
    let cache = CACHE.get_or_init(|| Mutex::new(VecDeque::new()));
    let slot = {
        let mut entries = cache.lock().map_err(|_| "Instrument archive cache unavailable")?;
        if let Some(index) = entries.iter().position(|entry| {
            entry.path == path && entry.length == length && entry.modified == modified
                && (entry.accept_any_v4_signature == accept_any_v4_signature
                    || (accept_any_v4_signature && matches!(entry.archive.get(), Some(Ok(_)))))
        }) {
            let entry = entries.remove(index).unwrap();
            let slot = entry.archive.clone();
            entries.push_back(entry);
            slot
        } else {
            entries.retain(|entry| entry.path != path
                || (entry.length == length && entry.modified == modified));
            if entries.len() >= CAPACITY {
                entries.pop_front();
            }
            let slot = Arc::new(OnceLock::new());
            entries.push_back(Entry {
                path: path.clone(), length, modified, accept_any_v4_signature,
                archive: slot.clone(),
            });
            slot
        }
    };
    let result = slot.get_or_init(|| Archive::read_file(&path, accept_any_v4_signature)).clone();
    if result.is_err() && let Ok(mut entries) = cache.lock() {
        entries.retain(|entry| !Arc::ptr_eq(&entry.archive, &slot));
    }
    result
}
