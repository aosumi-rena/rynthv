mod dnni;
mod artwork;
mod metadata;
mod nofs;
mod resource;
mod flat;

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use super::{Result, VoiceFamily, VoiceRevision};
use metadata::MetadataIndex;
pub use artwork::VoiceArtwork;

#[derive(Clone, Debug, Serialize)]
pub enum VoiceAvailability {

    Loadable,
    Unsupported(String),
}

impl VoiceAvailability {
    pub fn is_loadable(&self) -> bool {
        matches!(self, Self::Loadable)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct VoiceCatalogEntry {
    pub configuration: Option<super::config::VoiceConfiguration>,
    pub database_id: String,
    pub revision: VoiceRevision,
    pub model_version: Option<u8>,
    pub family: Option<VoiceFamily>,
    pub directory: PathBuf,
    pub display_name: String,
    pub vendor: String,
    pub genres: Vec<String>,
    pub pitch_range: Option<String>,
    pub languages: Vec<String>,
    pub icon: Option<VoiceArtwork>,
    pub portrait: Option<VoiceArtwork>,
    pub availability: VoiceAvailability,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct VoiceCatalog {
    pub entries: Vec<VoiceCatalogEntry>,
    pub warnings: Vec<String>,
}

impl VoiceCatalog {

    pub fn scan(root: &Path) -> Result<Self> {
        let selected=root.canonicalize()?;
        let databases=selected.join("databases");
        let resolved=if databases.is_dir() {databases.canonicalize()?}else{selected};
        let root=resolved.as_path();
        let mut catalog = Self::default();
        let manifest=if root.is_dir() {root.join("manifest.json")}else{root.to_owned()};
        if super::materialized_flat::is_manifest(&manifest) {
            catalog.entries.push(flat::entry(&manifest)?);return Ok(catalog);
        }
        if super::nofs::is_file(root) {
            catalog.entries.push(nofs::entry(root)?);
            return Ok(catalog);
        }
        if let Some(parent)=root.parent() && root.file_name().and_then(|n|n.to_str()).is_some_and(|n|n.parse::<VoiceRevision>().is_ok())
            && let Some(id)=parent.file_name().and_then(|n|n.to_str()).and_then(|n|uuid::Uuid::parse_str(n).ok()) {
            let metadata=MetadataIndex::load(parent.parent().unwrap_or(parent));
            dnni::scan(parent,id,&mut catalog,&metadata)?;
            catalog.entries.retain(|e|e.directory==root);return Ok(catalog);
        }
        if let Some(id)=root.file_name().and_then(|n|n.to_str()).and_then(|n|uuid::Uuid::parse_str(n).ok()) {
            let metadata=MetadataIndex::load(root.parent().unwrap_or(root));
            dnni::scan(root,id,&mut catalog,&metadata)?;
            if !catalog.entries.is_empty() {return Ok(catalog);}
        }
        for path in super::nofs::paths(root)? {
            catalog.entries.push(nofs::entry(&path)?);
        }
        let metadata = MetadataIndex::load(root);
        for database in fs::read_dir(root)? {
            let database = database?;
            if !database.file_type()?.is_dir() {
                continue;
            }
            let manifest=database.path().join("manifest.json");
            if super::materialized_flat::is_manifest(&manifest) {
                match flat::entry(&manifest) {Ok(entry)=>catalog.entries.push(entry),Err(e)=>catalog.warnings.push(e.to_string())}
                continue;
            }
            if let Ok(database_id) = uuid::Uuid::parse_str(&database.file_name().to_string_lossy())
            {
                dnni::scan(&database.path(), database_id, &mut catalog, &metadata)?;
            }
            match super::nofs::paths(&database.path()) {
                Ok(paths) => {
                    for path in paths {
                        catalog.entries.push(nofs::entry(&path)?);
                    }
                }
                Err(error) => catalog.warnings.push(format!(
                    "无法读取声库目录 {}：{error}",
                    database.path().display()
                )),
            }
        }
        let mut counts = std::collections::BTreeMap::new();
        for entry in &catalog.entries {
            *counts
                .entry((entry.database_id.clone(), entry.revision))
                .or_insert(0usize) += 1;
        }
        for entry in &mut catalog.entries {
            if counts[&(entry.database_id.clone(), entry.revision)] > 1 {
                entry.availability = VoiceAvailability::Unsupported(
                    "相同声库版本存在多个文件，请移除重复项后刷新".to_owned(),
                );
            }
        }
        catalog.entries.sort_by(|left, right| {
            left.display_name.to_lowercase()
                .cmp(&right.display_name.to_lowercase())
                .then_with(||left.display_name.cmp(&right.display_name))
                .then_with(|| left.database_id.cmp(&right.database_id))
                .then_with(|| right.revision.cmp(&left.revision))
        });
        Ok(catalog)
    }
}
