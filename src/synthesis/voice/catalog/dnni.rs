use super::{
    VoiceAvailability, VoiceCatalog, VoiceCatalogEntry, metadata::MetadataIndex, resource,
};
use crate::synthesis::voice::{Result, VoiceFamily, VoiceRevision, format::FormatContext};
use std::{fs, path::Path};

pub(super) fn scan(
    database: &Path,
    database_id: uuid::Uuid,
    catalog: &mut VoiceCatalog,
    metadata: &MetadataIndex,
) -> Result<()> {
    let context = FormatContext::dnni_v3();
    let revisions = match fs::read_dir(database) {
        Ok(revisions) => revisions,
        Err(error) => {
            catalog
                .warnings
                .push(format!("无法读取声库 {database_id}：{error}"));
            return Ok(());
        }
    };
    for revision in revisions {
        let revision = revision?;
        if !revision.file_type()?.is_dir() {
            continue;
        }
        let Ok(version) = revision
            .file_name()
            .to_string_lossy()
            .parse::<VoiceRevision>()
        else {
            continue;
        };
        let mut entry = VoiceCatalogEntry {
            configuration: None,
            database_id: database_id.to_string(),
            revision: version,
            model_version: None,
            family: None,
            directory: revision.path(),
            display_name: database_id.to_string(),
            vendor: String::new(),
            localized_names: Default::default(),
            localized_vendors: Default::default(),
            genres: Vec::new(),
            pitch_range: None,
            languages: Vec::new(),
            icon: None,
            portrait: None,
            availability: VoiceAvailability::Loadable,
        };
        match resource::configuration(&entry.directory, &context) {
            Ok(configuration) => {
                entry.configuration = Some(configuration.metadata.clone());
                entry.model_version = Some(configuration.model_version);
                let family = if configuration.model_version == 1 {
                    VoiceFamily::Sv2Compatible
                } else {
                    VoiceFamily::Sv2
                };
                entry.family = Some(family);
                let missing = configuration.models.missing(configuration.model_version);
                if configuration.version != version.to_string() {
                    entry.availability =
                        VoiceAvailability::Unsupported("声库配置版本与目录版本不一致".to_owned());
                } else if !missing.is_empty() {
                    entry.availability = VoiceAvailability::Unsupported(format!(
                        "缺少本地{}模型，当前安装内容无法在本机合成",
                        missing.join("、")
                    ));
                }
                entry.display_name = configuration.name;
                entry.vendor = configuration.vendor;
                entry.languages = configuration.languages;
                entry.icon = configuration.icon;
                entry.portrait = configuration.portrait;
                if let Some(display) = metadata.lookup(&entry.display_name, &entry.vendor) {
                    entry.genres.clone_from(&display.genres);
                    entry.pitch_range.clone_from(&display.pitch_range);

                    if !display.languages.is_empty() {
                        entry.languages.clone_from(&display.languages);
                    }
                    if entry.icon.is_none() {
                        entry.icon = display.icon_path.clone().map(|path| super::VoiceArtwork::File {path});
                    }
                }
            }
            Err(error) => {
                entry.availability = VoiceAvailability::Unsupported(error.to_string());
            }
        }
        catalog.entries.push(entry);
    }

    Ok(())
}
