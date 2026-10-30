use serde::{Deserialize, Serialize};
use crate::model::{Extra, PublicModeMap};
use crate::synthesis::{settings::VoiceSelection, voice::{Result,VoiceError}};

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct ExperimentControls {
    pub extended_expression: bool,
    pub random_enabled: bool,
    pub random_seed: u32,
    pub random_amount: f64,
    pub random_signed: bool,
    pub random_lanes: [bool;3],
    pub acoustic_blend: bool,
    pub voices: Vec<BlendTarget>,
    pub harmonic_gain: f32,
    pub aspiration_gain: f32,
    pub ring_depth: f32,
    pub ring_frequency: f32,
    pub growl: Option<GrowlControls>,
    pub dds:super::DdsControls,
    pub random_voice_enabled:bool,
    pub random_voice_seed:u32,
    pub random_voice_amount:f32,
}
impl Default for ExperimentControls {
    fn default()->Self { Self {extended_expression:false,random_enabled:false,random_seed:0,random_amount:100.0,random_signed:false,
        random_lanes:[true;3],acoustic_blend:true,voices:Vec::new(),harmonic_gain:0.0,
        aspiration_gain:0.0,ring_depth:0.0,ring_frequency:60.0,growl:None,dds:Default::default(),
        random_voice_enabled:false,random_voice_seed:0,random_voice_amount:1.0} }
}
#[derive(Clone,Copy,Debug,PartialEq,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct GrowlControls {
    pub frequency:f32,
    pub amplitude:f32,
}
impl Default for GrowlControls {
    fn default()->Self {Self {frequency:60.0,amplitude:0.0}}
}
impl GrowlControls {
    pub fn valid(self)->bool {
        self.frequency.is_finite() && (30.0..=90.0).contains(&self.frequency)
            && self.amplitude.is_finite() && (0.0..=1.0).contains(&self.amplitude)
    }
    pub fn from_voice(voice:&crate::synthesis::plan::VoiceInput)->Result<Self> {
        let local=ExperimentControls::from_extra(&voice.instance_options)?;
        let value=match local.growl {Some(value)=>value,None=>ExperimentControls::from_extra(&voice.track_default_options)?.growl.unwrap_or_default()};
        if !value.valid() {return Err(VoiceError::new("Invalid growl controls"));}
        Ok(value)
    }
}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlendTarget {
    pub voice: VoiceSelection,
    pub weight: f32,
    #[serde(default)] pub transpose: i32,
    #[serde(default)] pub inherit_modes: bool,
    #[serde(default)] pub speaker_id: Option<i32>,
}
impl BlendTarget {
    pub fn active<'a>(&self,curves:&std::collections::BTreeMap<String,crate::model::Automation>,notes:impl IntoIterator<Item=&'a Extra>)->Result<bool> {
        if self.weight!=0.0 {return Ok(true);}
        let key=format!("{}:{}",self.voice.database_id,self.voice.revision);
        if curves.get(&format!("voiceBlend_{key}")).is_some_and(|c|c.points.iter().any(|(_,v)|*v>0.0)) {return Ok(true);}
        for note in notes {
            let local=LocalBlendWeight::read(note,&key)?;
            if local.note.is_some_and(|v|v>0.0) || local.phones.values().any(|v|*v>0.0) {return Ok(true);}
        }
        Ok(false)
    }
}
#[derive(Clone,Debug,Default,PartialEq,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct LocalBlendWeight {
    pub note: Option<f32>,
    pub phones: std::collections::BTreeMap<usize,f32>,
}
impl LocalBlendWeight {
    pub fn read(extra:&Extra,key:&str)->Result<Self> {
        let values=extra.get("rynthvBlendWeights").map(|v|v.as_object().ok_or_else(||VoiceError::new("Invalid local blend weight object"))).transpose()?;
        let value=values.and_then(|v|v.get(key));
        let result:Self=value.map_or_else(||Ok(Self::default()),|v|serde_json::from_value(v.clone()).map_err(|e|VoiceError::new(e.to_string())))?;
        if result.note.into_iter().chain(result.phones.values().copied()).any(|v|!v.is_finite() || !(0.0..=1.0).contains(&v)) {
            return Err(VoiceError::new("Local blend weight is outside 0..1"));
        }
        Ok(result)
    }
}
impl ExperimentControls {
    pub fn effective(extra:&Extra,settings:super::ExperimentalSettings)->Result<Self> {
        let mut value=Self::from_extra(extra)?;
        value.validate()?;
        if settings.active(settings.random_voices) && settings.voice_blending && value.random_voice_enabled {
            use sha2::{Digest,Sha256};
            let weights:Vec<_>=value.voices.iter().map(|voice| {
                let digest=Sha256::digest(format!("{}/{}/{}",value.random_voice_seed,voice.voice.database_id,voice.voice.revision).as_bytes());
                (f64::from(u32::from_be_bytes(digest[..4].try_into().unwrap()))+1.0)/(f64::from(u32::MAX)+1.0)
            }).collect();
            let total:f64=weights.iter().sum();
            if total>0.0 {for (target,weight) in value.voices.iter_mut().zip(weights) {target.weight=(weight/total*f64::from(value.random_voice_amount)) as f32;}}
        }
        Ok(value)
    }
    pub fn from_extra(extra:&Extra)->Result<Self> {
        extra.get("rynthvExperiments").map_or_else(||Ok(Self::default()),|v|
            serde_json::from_value(v.clone()).map_err(|e|VoiceError::new(format!("Invalid experimental controls: {e}"))))
    }
    pub fn validate(&self)->Result<()> {
        self.dds.validate()?;
        if !self.random_voice_amount.is_finite() || !(0.0..=1.0).contains(&self.random_voice_amount) {return Err(VoiceError::new("Random voice mix amount is outside 0..1"));}
        if !self.random_amount.is_finite() || !(0.0..=300.0).contains(&self.random_amount)
            || self.voices.len()>3 || self.voices.iter().any(|v|!v.weight.is_finite() || !(0.0..=1.0).contains(&v.weight) || !(-24..=24).contains(&v.transpose))
            || !self.harmonic_gain.is_finite() || self.harmonic_gain.abs()>24.0
            || !self.aspiration_gain.is_finite() || self.aspiration_gain.abs()>24.0
            || !self.ring_depth.is_finite() || !(0.0..=1.0).contains(&self.ring_depth)
            || !self.ring_frequency.is_finite() || !(20.0..=150.0).contains(&self.ring_frequency)
            || self.growl.is_some_and(|v|!v.valid()) {
            return Err(VoiceError::new("Experimental controls are outside their supported ranges"));
        }
        for (i,target) in self.voices.iter().enumerate() {
            if self.voices[..i].iter().any(|v|v.voice==target.voice) {return Err(VoiceError::new("Duplicate blend voice revision"));}
        }
        Ok(())
    }
    pub fn random_modes(&self,names:&[String])->PublicModeMap {
        use sha2::{Digest,Sha256};
        let mut sorted=names.to_vec();sorted.sort();sorted.dedup();
        let values:Vec<_>=sorted.iter().map(|name| {
            let hash=Sha256::digest(format!("{}/{name}",self.random_seed).as_bytes());
            let value=u32::from_be_bytes(hash[..4].try_into().unwrap()) as f64/u32::MAX as f64;
            if self.random_signed {value*2.0-1.0}else{value}
        }).collect();
        let total:f64=values.iter().map(|v|v.abs()).sum();
        if total==0.0 {return PublicModeMap::new();}
        sorted.into_iter().zip(values).map(|(name,value)| {
            let lane=|i|Some(if self.random_lanes[i] {value/total*self.random_amount}else{0.0});
            (name,crate::model::PublicModeControl{pitch:lane(0),timbre:lane(1),pronunciation:lane(2)})
        }).collect()
    }
}
