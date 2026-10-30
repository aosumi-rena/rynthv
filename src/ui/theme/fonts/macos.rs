//! macOS UI font lookup.
//!
//! PingFang SC ships as a collection whose faces follow a fixed order
//! (HK, MO, TC, SC for each weight). The Simplified Chinese face is therefore
//! selected by index rather than by parsing the name table, and the remaining
//! candidates are tried in case a system has been trimmed.

use eframe::egui::FontData;
use std::path::{Path, PathBuf};

/// Collection faces per weight, in the order PingFang ships them.
/// Ultralight, Light, Thin, Regular, Medium, Semibold.
const PINGFANG_SC_FACES: [u32; 6] = [19, 15, 3, 3, 7, 11];
const REGULAR_FACE: usize = 3;
const BOLD_FACE: usize = 5;

/// Fallback sans-serif Chinese fonts, with the collection index of their
/// regular and bold faces. These are single-weight families, so the same face
/// is used for both requests when no bold sibling exists.
const FALLBACKS: [(&str, u32, u32); 3] = [
    ("Hiragino Sans GB.ttc", 0, 2),
    ("STHeiti Medium.ttc", 0, 0),
    ("STHeiti Light.ttc", 0, 0),
];

const SYSTEM_FONT_DIRS: [&str; 2] = ["/System/Library/Fonts", "/Library/Fonts"];

pub(super) fn load(bold: bool) -> Result<FontData, String> {
    let mut attempted: Vec<String> = Vec::new();

    for path in pingfang_candidates() {
        if !path.is_file() {
            continue;
        }
        let index = PINGFANG_SC_FACES[if bold { BOLD_FACE } else { REGULAR_FACE }];
        match super::read_font(&path, index) {
            Ok(font) => return Ok(font),
            Err(error) => attempted.push(error),
        }
    }

    for (name, regular, bold_face) in FALLBACKS {
        for directory in SYSTEM_FONT_DIRS {
            let path = Path::new(directory).join(name);
            if !path.is_file() {
                continue;
            }
            match super::read_font(&path, if bold { bold_face } else { regular }) {
                Ok(font) => return Ok(font),
                Err(error) => attempted.push(error),
            }
        }
    }

    let detail = if attempted.is_empty() {
        String::new()
    } else {
        format!(" ({})", attempted.join("; "))
    };
    Err(format!(
        "No Simplified Chinese UI font found. Install PingFang SC or Hiragino Sans GB{detail}"
    ))
}

/// PingFang is delivered as a downloadable font asset, so its location varies
/// between macOS releases; every candidate is probed before falling back.
fn pingfang_candidates() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/System/Library/Fonts/PingFang.ttc"),
        PathBuf::from("/System/Library/Fonts/Supplemental/PingFang.ttc"),
    ];
    paths.extend(asset_pingfang_paths());
    paths
}

fn asset_pingfang_paths() -> Vec<PathBuf> {
    let root = Path::new("/System/Library/AssetsV2");
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let candidate = path.join("AssetData").join("PingFang.ttc");
        if candidate.is_file() {
            found.push(candidate);
        }
    }
    found
}
