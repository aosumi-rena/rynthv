use std::path::Path;

use super::{VoiceAvailability, VoiceCatalogEntry};
use crate::synthesis::voice::{Result, VoiceFamily, VoiceRevision, nofs};

pub(super) fn entry(path: &Path) -> Result<VoiceCatalogEntry> {
    let icon_path = path
        .parent()
        .map(|directory| directory.join("profile.png"))
        .filter(|path| path.is_file());
    let mut entry = VoiceCatalogEntry {
        configuration: None,
        database_id: nofs::database_id(path)?,
        revision: VoiceRevision::release(0),
        model_version: Some(1),
        family: Some(VoiceFamily::Sv1),
        directory: path.to_owned(),
        display_name: path
            .parent()
            .and_then(Path::file_name)
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        vendor: String::new(),
        genres: Vec::new(),
        pitch_range: None,
        languages: Vec::new(),
        icon: icon_path.map(|path| super::VoiceArtwork::File {path}),
        portrait: None,
        availability: VoiceAvailability::Loadable,
    };
    if crate::synthesis::voice::flat::is_profile(path) {
        return match super::flat::profile_entry(path) {
            Ok(value)=>Ok(value),Err(error)=>{entry.database_id=entry.database_id.replacen("sv1:","flat:",1);entry.availability=VoiceAvailability::Unsupported(error.to_string());Ok(entry)}
        };
    }
    let description =
        nofs::NofsArchive::open(path).and_then(|mut archive| nofs::describe(&mut archive));
    match description {
        Ok(description) => {
            let configuration = description.configuration;
            entry.revision = configuration
                .version
                .parse()
                .expect("NOFS metadata version checked");
            entry.display_name = configuration.name;
            entry.vendor = configuration.vendor;
            entry.languages = vec![configuration.default_language];
        }
        Err(error) => entry.availability = VoiceAvailability::Unsupported(error.to_string()),
    }
    Ok(entry)
}
