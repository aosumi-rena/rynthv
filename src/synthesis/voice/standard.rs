use std::{fs,path::{Path,PathBuf}};
use serde::Deserialize;
use super::{Result,VoiceError,VoiceFamily,VoiceCatalogEntry,VoiceAvailability,VoiceRevision,config::VoiceConfiguration};
mod configuration;
mod discovery;
mod r1;
mod r2;
mod resources;

pub(crate) const PROFILE_NAME:&str="std-voice.json";

#[derive(Clone,Debug)]
pub(crate) enum StdResources {
    R1 {metadata:PathBuf,data:PathBuf,language_directory:PathBuf,renderer_library:PathBuf},
    R2 {bank:PathBuf,classifier_directory:PathBuf,f0_model:PathBuf},
}
impl StdResources {
    pub fn family(&self)->VoiceFamily {match self {Self::R1{..}=>VoiceFamily::R1Std,Self::R2{..}=>VoiceFamily::R2Std}}
}

#[derive(Clone,Debug)]
pub(crate) struct StdVoice {
    pub profile:PathBuf,
    pub database_id:String,
    pub configuration:VoiceConfiguration,
    pub resources:StdResources,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    format:String,id:String,name:String,vendor:String,language:String,revision:u32,
    generation:String,
    metadata:Option<PathBuf>,data:Option<PathBuf>,language_directory:Option<PathBuf>,renderer_library:Option<PathBuf>,
    bank:Option<PathBuf>,classifier_directory:Option<PathBuf>,f0_model:Option<PathBuf>,
}

pub(crate) fn profile_path(path:&Path)->PathBuf {if path.is_dir(){path.join(PROFILE_NAME)}else{path.to_owned()}}
pub(crate) fn is_profile(path:&Path)->bool {
    let path=profile_path(path);
    path.file_name().is_some_and(|name|name==PROFILE_NAME)&&path.is_file()
}

pub(crate) fn native_entry(path:&Path)->Result<Option<(VoiceCatalogEntry,Vec<String>)>> {
    Ok(discovery::inspect(path)?.map(|bank|{let entry=bank.entry();(entry,bank.warnings)}))
}
pub(crate) fn native_voice(path:&Path)->Result<Option<StdVoice>> {
    discovery::inspect(path)?.map(|bank|bank.voice()).transpose()
}

impl StdVoice {
    pub fn load(path:&Path)->Result<Self> {
        if !is_profile(path) {
            return native_voice(path)?.ok_or_else(||VoiceError::new("Select an original R1 master.s5l/master.s5d bank directory, an R2 STD NOFS file, or an explicit std-voice.json profile"));
        }
        let path=profile_path(path).canonicalize()?;
        if fs::metadata(&path)?.len()>131072 {return Err(VoiceError::new("STD voice profile is too large"));}
        let profile:Profile=serde_json::from_slice(&fs::read(&path)?).map_err(|error|VoiceError::new(error.to_string()))?;
        if profile.format!="rynthv-std-1" || profile.id.is_empty() || profile.name.is_empty() || profile.language.is_empty() {
            return Err(VoiceError::new("Invalid STD voice profile"));
        }
        let directory=path.parent().ok_or_else(||VoiceError::new("STD profile has no directory"))?;
        let resolve=|value:Option<PathBuf>,label:&str,is_directory:bool|->Result<PathBuf> {
            let value=value.ok_or_else(||VoiceError::new(format!("Missing STD resource: {label}")))?;
            let full=if value.is_absolute(){value}else{directory.join(value)}.canonicalize()?;
            if (is_directory&&!full.is_dir())||(!is_directory&&!full.is_file()) {return Err(VoiceError::new(format!("Invalid STD resource: {label}")));}
            Ok(full)
        };
        let mut resources=match profile.generation.as_str() {
            "r1"=>{
                if profile.bank.is_some()||profile.classifier_directory.is_some()||profile.f0_model.is_some(){return Err(VoiceError::new("R1 profile contains R2 resources"));}
                StdResources::R1 {metadata:resolve(profile.metadata,"metadata",false)?,data:resolve(profile.data,"data",false)?,
                    language_directory:PathBuf::new(),renderer_library:resolve(profile.renderer_library,"renderer library",false)?}
            },
            "r2"=>{
                if profile.metadata.is_some()||profile.data.is_some()||profile.language_directory.is_some()||profile.renderer_library.is_some(){return Err(VoiceError::new("R2 profile contains R1 resources"));}
                StdResources::R2 {bank:resolve(profile.bank,"bank",false)?,classifier_directory:PathBuf::new(),f0_model:resolve(profile.f0_model,"F0 model",false)?}
            },
            _=>return Err(VoiceError::new("Unknown STD generation")),
        };
        let settings=crate::synthesis::settings::SynthesisSettings::load().map_err(VoiceError::new)?;
        match &mut resources {
            StdResources::R1{language_directory,..}=>*language_directory=settings.require_phoneme_directory(crate::synthesis::settings::PhonemeVersion::R1Std).map_err(VoiceError::new)?.to_owned(),
            StdResources::R2{classifier_directory,..}=>*classifier_directory=settings.require_phoneme_directory(crate::synthesis::settings::PhonemeVersion::R2Std).map_err(VoiceError::new)?.to_owned(),
        }
        let family=resources.family();
        Ok(Self {profile:path,database_id:format!("{}-std:{}",profile.generation,profile.id),
            configuration:VoiceConfiguration::standard(family,profile.name,profile.revision.to_string(),profile.vendor,profile.language),resources})
    }

    pub fn catalog_entry(&self)->VoiceCatalogEntry {
        let revision=self.configuration.version.parse::<VoiceRevision>().ok()
            .or_else(||self.configuration.version.parse::<u32>().ok().map(VoiceRevision::release)).unwrap_or(VoiceRevision::release(0));
        VoiceCatalogEntry {database_id:self.database_id.clone(),revision,
            model_version:Some(self.configuration.model_version),family:Some(self.resources.family()),directory:self.profile.clone(),
            display_name:self.configuration.name.clone(),vendor:self.configuration.vendor.clone(),localized_names:Default::default(),localized_vendors:Default::default(),genres:Vec::new(),pitch_range:None,
            languages:self.configuration.supported_languages.clone(),icon:None,portrait:None,availability:VoiceAvailability::Loadable,
            configuration:Some(self.configuration.clone())}
    }
}
