//! Original-bank descriptors; no sidecar files are created in bank directories.
use std::{fs,path::{Path,PathBuf}};
use super::{StdResources,StdVoice,resources,r1,r2};
use super::super::{Result,VoiceError,VoiceFamily,VoiceRevision,VoiceCatalogEntry,VoiceAvailability,config::VoiceConfiguration};

pub(super) struct Discovered {
    pub path:PathBuf,pub data:Option<PathBuf>,pub name:String,pub vendor:String,
    pub revision:VoiceRevision,pub version:String,pub language:String,pub family:VoiceFamily,
    pub unsupported:Option<String>,
    pub warnings:Vec<String>,
}
impl Discovered {
    fn configuration(&self)->VoiceConfiguration {
        VoiceConfiguration::standard(self.family,self.name.clone(),self.version.clone(),self.vendor.clone(),self.language.clone())
    }
    fn id(&self)->String {
        let identity=if self.family==VoiceFamily::R2Std{self.path.parent().unwrap_or(&self.path)}else{&self.path};
        let path=identity.to_string_lossy();
        let normalized=if cfg!(windows){path.to_uppercase()}else{path.into_owned()};
        format!("{}-std:{}",if self.family==VoiceFamily::R1Std{"r1"}else{"r2"},super::super::hex(normalized.as_bytes()))
    }
    fn resources(&self)->Result<StdResources> {
        if let Some(reason)=&self.unsupported {return Err(VoiceError::new(reason.clone()));}
        resources::resolve(self)
    }
    pub fn voice(&self)->Result<StdVoice> {
        Ok(StdVoice {profile:self.path.clone(),database_id:self.id(),configuration:self.configuration(),resources:self.resources()?})
    }
    pub fn entry(&self)->VoiceCatalogEntry {
        let availability=match self.resources() {Ok(_)=>VoiceAvailability::Loadable,Err(error)=>VoiceAvailability::Unsupported(error.to_string())};
        let artwork=|name:&str|self.path.parent().map(|p|p.join(name)).filter(|p|p.is_file())
            .map(|path|super::super::catalog::VoiceArtwork::File {path});
        VoiceCatalogEntry {database_id:self.id(),revision:self.revision,model_version:Some(self.family.model_version()),
            family:Some(self.family),directory:self.path.clone(),display_name:self.name.clone(),vendor:self.vendor.clone(),localized_names:Default::default(),localized_vendors:Default::default(),
            genres:Vec::new(),pitch_range:None,languages:vec![self.language.clone()],icon:artwork("profile.png"),portrait:artwork("background.png"),
            availability,configuration:Some(self.configuration())}
    }
}
pub(super) fn inspect(path:&Path)->Result<Option<Discovered>> {
    if path.is_dir() || path.file_name().is_some_and(|n|n.eq_ignore_ascii_case("master.s5l")) {
        if let Some(bank)=r1::inspect(path)? {return Ok(Some(bank));}
    }
    if path.is_file()&&path.extension().is_some_and(|n|n.eq_ignore_ascii_case("nofs")) {return r2::inspect(path);}
    Ok(None)
}
pub(super) fn read_bounded(path:&Path,maximum:u64)->Result<Vec<u8>> {
    use std::io::Read;
    let file=fs::File::open(path)?;
    let length=file.metadata()?.len();
    if length>maximum {return Err(VoiceError::new(format!("Metadata/resource exceeds its read limit: {}",path.display())));}
    let mut bytes=Vec::new();file.take(maximum+1).read_to_end(&mut bytes)?;
    if bytes.len() as u64!=length {return Err(VoiceError::new("Metadata/resource changed during reading"));}
    Ok(bytes)
}
