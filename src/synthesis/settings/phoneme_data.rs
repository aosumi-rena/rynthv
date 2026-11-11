use super::SynthesisSettings;
use crate::{model::PhonemeFormat, synthesis::voice::{VoiceCatalogEntry, VoiceConfiguration, VoiceFamily, VoiceRevision}};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhonemeVersion { R1Std, R2Std, R2Ai, R2AiFlat, R3Ai, R3Ai2Earlier, R3Ai2, R3Ai2Beta2041, R3Ai2Beta2051, R3Ai2Beta2052 }
impl PhonemeVersion {
    pub const ALL:[Self;10]=[Self::R1Std,Self::R2Std,Self::R2Ai,Self::R2AiFlat,Self::R3Ai,Self::R3Ai2Earlier,Self::R3Ai2,Self::R3Ai2Beta2041,Self::R3Ai2Beta2051,Self::R3Ai2Beta2052];
    pub const BETA:[Self;3]=[Self::R3Ai2Beta2051,Self::R3Ai2Beta2052,Self::R3Ai2Beta2041];
    pub fn label(self)->&'static str {match self {
        Self::R1Std=>"R1 STD",Self::R2Std=>"R2 STD",Self::R2Ai=>"R2 AI",
        Self::R2AiFlat=>"R2 AI Flat",Self::R3Ai=>"R3 AI",
        Self::R3Ai2Earlier=>"R3 AI 2 (<204b1)",Self::R3Ai2=>"R3 AI 2 (≥205)",
        Self::R3Ai2Beta2041=>"R3 AI 2 (204b1)",Self::R3Ai2Beta2051=>"R3 AI 2 (205b1)",Self::R3Ai2Beta2052=>"R3 AI 2 (205b2)"}}
    pub fn entry(entry:&VoiceCatalogEntry)->Option<Self> {
        Self::family(entry.family?,entry.database_id.starts_with("flat:"),entry.revision)
    }
    pub fn configuration(config:&VoiceConfiguration,flat:bool)->Result<Self,String> {
        Self::family(config.family,flat,config.version.parse()?)
            .ok_or_else(||format!("{}: {}",config.version,crate::i18n::t("PHONEME_DATA_REQUIRED")))
    }
    pub fn format(self)->PhonemeFormat {match self {
        Self::R3Ai2Earlier|Self::R3Ai2Beta2041=>PhonemeFormat::Original,
        Self::R3Ai2|Self::R3Ai2Beta2051|Self::R3Ai2Beta2052=>PhonemeFormat::System,
        _=>PhonemeFormat::Auto,
    }}
    pub fn family(family:VoiceFamily,flat:bool,revision:VoiceRevision)->Option<Self> {match family {
        VoiceFamily::R1Std=>Some(Self::R1Std),VoiceFamily::R2Std=>Some(Self::R2Std),
        VoiceFamily::Sv1 if flat=>Some(Self::R2AiFlat),
        VoiceFamily::Sv1=>Some(Self::R2Ai),VoiceFamily::Sv2Compatible=>Some(Self::R3Ai),
        VoiceFamily::Sv2 if revision==VoiceRevision::beta(204,1)=>Some(Self::R3Ai2Beta2041),
        VoiceFamily::Sv2 if revision==VoiceRevision::beta(205,1)=>Some(Self::R3Ai2Beta2051),
        VoiceFamily::Sv2 if revision==VoiceRevision::beta(205,2)=>Some(Self::R3Ai2Beta2052),
        VoiceFamily::Sv2 if revision<VoiceRevision::beta(204,1)=>Some(Self::R3Ai2Earlier),
        VoiceFamily::Sv2 if revision>=VoiceRevision::release(205)=>Some(Self::R3Ai2),
        VoiceFamily::Sv2=>None}}
}
#[derive(Clone,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct PhonemeData {
    pub r1_std:Option<PathBuf>,pub r2_std:Option<PathBuf>,pub r2_ai:Option<PathBuf>,
    pub r2_ai_flat:Option<PathBuf>,pub r3_ai:Option<PathBuf>,
    pub r3_ai_2_earlier:Option<PathBuf>,pub r3_ai_2:Option<PathBuf>,
    pub r3_ai_2_204b1:Option<PathBuf>,pub r3_ai_2_205b1:Option<PathBuf>,pub r3_ai_2_205b2:Option<PathBuf>,
}
impl PhonemeData {
    pub fn get(&self,version:PhonemeVersion)->Option<&Path> {match version {
        PhonemeVersion::R1Std=>&self.r1_std,PhonemeVersion::R2Std=>&self.r2_std,
        PhonemeVersion::R2Ai=>&self.r2_ai,PhonemeVersion::R2AiFlat=>&self.r2_ai_flat,
        PhonemeVersion::R3Ai=>&self.r3_ai,PhonemeVersion::R3Ai2Earlier=>&self.r3_ai_2_earlier,
        PhonemeVersion::R3Ai2=>&self.r3_ai_2,
        PhonemeVersion::R3Ai2Beta2041=>&self.r3_ai_2_204b1,PhonemeVersion::R3Ai2Beta2051=>&self.r3_ai_2_205b1,
        PhonemeVersion::R3Ai2Beta2052=>&self.r3_ai_2_205b2}.as_deref().filter(|p|!p.as_os_str().is_empty())}
    pub fn set(&mut self,version:PhonemeVersion,path:Option<PathBuf>) {let field=match version {
        PhonemeVersion::R1Std=>&mut self.r1_std,PhonemeVersion::R2Std=>&mut self.r2_std,
        PhonemeVersion::R2Ai=>&mut self.r2_ai,PhonemeVersion::R2AiFlat=>&mut self.r2_ai_flat,
        PhonemeVersion::R3Ai=>&mut self.r3_ai,PhonemeVersion::R3Ai2Earlier=>&mut self.r3_ai_2_earlier,
        PhonemeVersion::R3Ai2=>&mut self.r3_ai_2,
        PhonemeVersion::R3Ai2Beta2041=>&mut self.r3_ai_2_204b1,PhonemeVersion::R3Ai2Beta2051=>&mut self.r3_ai_2_205b1,
        PhonemeVersion::R3Ai2Beta2052=>&mut self.r3_ai_2_205b2};*field=path;}
}
impl SynthesisSettings {
    pub fn converts_phones(&self,family:VoiceFamily,revision:VoiceRevision)->bool {
        self.experiments.active(self.experiments.phone_conversion)
            && (family==VoiceFamily::Sv2Compatible || family==VoiceFamily::Sv2
                && matches!(PhonemeVersion::family(family,false,revision),Some(PhonemeVersion::R3Ai2Earlier|PhonemeVersion::R3Ai2Beta2041)))
    }
    pub fn converter_for(&self,entry:&VoiceCatalogEntry)->Result<Option<&Path>,String> {
        let Some(family)=entry.family else {return Ok(None)};
        self.phone_conversion_for(family,entry.revision)
    }
    pub fn phone_conversion_for(&self,family:VoiceFamily,revision:VoiceRevision)->Result<Option<&Path>,String> {
        if !self.converts_phones(family,revision) {return Ok(None)}
        self.phone_conversion.as_deref().filter(|path|path.is_file()).map(Some)
            .ok_or_else(||crate::i18n::t("PHONE_CONVERSION_REQUIRED").into())
    }
    pub fn phoneme_version(&self,entry:&VoiceCatalogEntry)->Option<PhonemeVersion> {
        let version=PhonemeVersion::entry(entry)?;
        if self.experiments.active(self.experiments.phone_conversion)
            && matches!(version,PhonemeVersion::R3Ai|PhonemeVersion::R3Ai2Earlier|PhonemeVersion::R3Ai2
                |PhonemeVersion::R3Ai2Beta2041|PhonemeVersion::R3Ai2Beta2051|PhonemeVersion::R3Ai2Beta2052) {
            Some(PhonemeVersion::R3Ai2)
        } else {Some(version)}
    }
    pub fn phoneme_directory(&self,entry:&VoiceCatalogEntry)->Option<&Path> {
        self.phoneme_data.get(self.phoneme_version(entry)?)
    }
    pub fn require_phoneme_directory(&self,version:PhonemeVersion)->Result<&Path,String> {
        self.phoneme_data.get(version).filter(|p|p.is_dir()).ok_or_else(||format!("{}: {}",version.label(),crate::i18n::t("PHONEME_DATA_REQUIRED")))
    }
}
