use serde::{Deserialize,Serialize};
use crate::synthesis::{settings::VoiceSelection,voice::{Result,VoiceError}};

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub enum ComponentOutput {#[default] Complete,Harmonics,Aspiration}

pub struct DdsFrameControls {
    pub bank_rows:Option<crate::synthesis::inference::Sequence>,
    pub default_weights:Option<Vec<f32>>,
}

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct DdsControls {
    pub speaker_vectors:bool,
    pub cross_model_vectors:bool,
    pub default_as_style:bool,
    pub default_weight:f32,
    pub vocoder:Option<VoiceSelection>,
    pub output:ComponentOutput,
    pub phonemes:Vec<PhonemeMix>,
}
impl Default for DdsControls {
    fn default()->Self {Self {speaker_vectors:false,cross_model_vectors:false,default_as_style:false,
        default_weight:1.0,vocoder:None,output:ComponentOutput::Complete,phonemes:Vec::new()}}
}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhonemeMix {
    pub id:String,
    pub source:String,
    pub token:String,
    pub language:String,
    pub weight:f32,
}
impl DdsControls {
    pub fn validate(&self)->Result<()> {
        if !self.default_weight.is_finite() || !(-3.0..=3.0).contains(&self.default_weight)
            || self.phonemes.len()>8 || self.phonemes.iter().enumerate().any(|(i,p)|uuid::Uuid::parse_str(&p.id).is_err()
                || self.phonemes[..i].iter().any(|old|old.id==p.id) || p.source.len()>128 || p.token.len()>128 || p.language.len()>128
                || !p.weight.is_finite() || !(0.0..=1.0).contains(&p.weight)) {
            return Err(VoiceError::new("Invalid DDS experimental controls"));
        }
        Ok(())
    }
}
