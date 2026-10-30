use super::{VoiceCatalogEntry,VoiceAvailability};
use crate::synthesis::voice::{Result,VoiceError,VoiceFamily,materialized_flat,nofs};
use std::path::Path;

pub(super) fn entry(path:&Path)->Result<VoiceCatalogEntry> {
    let configuration=materialized_flat::metadata(path)?;
    configured_entry(path,configuration)
}
pub(super) fn profile_entry(path:&Path)->Result<VoiceCatalogEntry> {
    configured_entry(path,crate::synthesis::voice::flat::Profile::open(path)?.configuration)
}
fn configured_entry(path:&Path,configuration:crate::synthesis::voice::config::VoiceConfiguration)->Result<VoiceCatalogEntry> {
    let artwork=|name:&str|path.parent().map(|p|p.join(name)).filter(|p|p.is_file()).map(|path|super::VoiceArtwork::File{path});
    Ok(VoiceCatalogEntry {database_id:nofs::database_id(path)?.replacen("sv1:","flat:",1),revision:configuration.version.parse().map_err(VoiceError::new)?,
        model_version:Some(1),family:Some(VoiceFamily::Sv1),directory:path.to_owned(),display_name:configuration.name.clone(),
        vendor:configuration.vendor.clone(),genres:Vec::new(),pitch_range:None,languages:configuration.supported_languages.clone(),
        icon:artwork("profile.png"),portrait:artwork("background.png"),configuration:Some(configuration),availability:VoiceAvailability::Loadable})
}
