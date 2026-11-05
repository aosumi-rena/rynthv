//! Optional exact voice/vendor translations, cached during catalogue discovery.
//! Raw configuration identities never depend on the current display language.
use std::{collections::{BTreeMap, BTreeSet}, fs::File, io::Read, path::{Path, PathBuf}};
use super::VoiceCatalog;

type Translations = BTreeMap<String, BTreeMap<String, String>>;
const FILE_BYTES: u64 = 1_048_576;
const SCAN_BYTES: u64 = 64 * 1_048_576;
const ROOTS: usize = 512;
const LINES: usize = 16384;
const TEXT_BYTES: usize = 4096;

struct Diagnostics<'a> { warnings: &'a mut Vec<String>, count: usize }
impl Diagnostics<'_> {
    fn push(&mut self, path: &Path, reason: &str) {
        if self.count < 32 {
            self.warnings.push(format!("Optional voice translations {}: {reason}", path.display()));
        } else if self.count == 32 {
            self.warnings.push("Further optional voice translation warnings were omitted".into());
        }
        self.count += 1;
    }
}

pub(super) fn localize(selected: &Path, catalog: &mut VoiceCatalog) {
    let targets: BTreeSet<String> = catalog.entries.iter().flat_map(|entry|
        [&entry.display_name, &entry.vendor]).filter(|s| !s.trim().is_empty()).cloned().collect();
    if targets.is_empty() { return; }
    let selected = if selected.is_dir() { selected } else { selected.parent().unwrap_or(selected) };
    let mut cache: BTreeMap<PathBuf, Translations> = BTreeMap::new();
    let mut remaining = SCAN_BYTES;
    let mut diagnostics = Diagnostics { warnings: &mut catalog.warnings, count: 0 };
    for entry in &mut catalog.entries {
        let bank = if entry.directory.is_dir() { entry.directory.as_path() }
            else { entry.directory.parent().unwrap_or(&entry.directory) };
        // Most-specific exact mapping wins. At most bank, parent, install,
        // plus the explicitly selected source root; never a recursive search.
        let mut roots = Vec::new();
        for path in bank.ancestors().take(3).chain(std::iter::once(selected)) {
            let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
            if !roots.contains(&path) { roots.push(path); }
        }
        entry.localized_names.clear();
        entry.localized_vendors.clear();
        for root in roots {
            if !cache.contains_key(&root) {
                if cache.len() >= ROOTS {
                    diagnostics.push(&root, "source-directory scan limit reached");
                    continue;
                }
                let translations = load(&root, &targets, &mut remaining, &mut diagnostics);
                cache.insert(root.clone(), translations);
            }
            for (culture, values) in &cache[&root] {
                if let Some(value) = values.get(&entry.display_name) {
                    entry.localized_names.entry(culture.clone()).or_insert_with(|| value.clone());
                }
                if let Some(value) = values.get(&entry.vendor) {
                    entry.localized_vendors.entry(culture.clone()).or_insert_with(|| value.clone());
                }
            }
        }
    }
}

fn load(root: &Path, targets: &BTreeSet<String>, remaining: &mut u64,
    diagnostics: &mut Diagnostics<'_>) -> Translations {
    let mut result = Translations::new();
    // These are the app's eleven canonical cultures. Language keys already
    // use the reference convention: lower case, '-' rather than '_'. No
    // translated proper name or substitute culture is synthesized here.
    for language in crate::i18n::Language::ALL {
        let culture = language.key();
        let path = root.join("translations").join(format!("{culture}.txt"));
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => { diagnostics.push(&path, "file could not be opened"); continue; }
        };
        let limit = FILE_BYTES.min(*remaining);
        let mut bytes = Vec::new();
        if file.take(limit + 1).read_to_end(&mut bytes).is_err() {
            *remaining = remaining.saturating_sub(bytes.len() as u64);
            diagnostics.push(&path, "file could not be read");
            continue;
        }
        *remaining = remaining.saturating_sub(bytes.len() as u64);
        if bytes.len() as u64 > limit {
            diagnostics.push(&path, "file or total scan byte limit exceeded");
            continue;
        }
        let Ok(text) = std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes)) else {
            diagnostics.push(&path, "file is not UTF-8"); continue;
        };
        let mut values = BTreeMap::new();
        let mut malformed = false;
        for (i, line) in text.lines().enumerate() {
            if i >= LINES { malformed = true; break; }
            let line = line.trim();
            if !line.starts_with('"') { continue; }
            if line.len() > TEXT_BYTES * 4 { malformed = true; continue; }
            let Some((key, value)) = pair(line) else { malformed = true; continue; };
            if key.trim().is_empty() || value.trim().is_empty() { continue; }
            if key.len() > TEXT_BYTES || value.len() > TEXT_BYTES { malformed = true; continue; }
            // Match ordinal keys; a later declaration in the same source file
            // wins, as in the existing VoiceNames reader. Retain relevant names
            // only, not the editor's complete interface translation catalogue.
            if targets.contains(&key) { values.insert(key, value); }
        }
        if malformed { diagnostics.push(&path, "malformed or over-limit entries were ignored"); }
        if !values.is_empty() { result.insert(culture.to_owned(), values); }
    }
    result
}

fn pair(line: &str) -> Option<(String, String)> {
    let (key, rest) = quoted(line)?;
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let (value, rest) = quoted(rest)?;
    let rest = rest.trim_start();
    (rest.is_empty() || rest.starts_with("//")).then_some((key, value))
}

fn quoted(text: &str) -> Option<(String, &str)> {
    if !text.starts_with('"') { return None; }
    let mut escaped = false;
    for (i, byte) in text.bytes().enumerate().skip(1) {
        if escaped { escaped = false; continue; }
        match byte {
            b'\\' => escaped = true,
            b'"' => return serde_json::from_str(&text[..=i]).ok().map(|value| (value, &text[i + 1..])),
            _ => {},
        }
    }
    None
}
