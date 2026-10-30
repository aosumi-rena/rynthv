use std::collections::BTreeMap;
use crate::{model::Automation, synthesis::plan::{GlobalBlick,OwnerInput,TempoMapping}};

#[derive(Clone,Debug,PartialEq)]
pub struct PreparedAutomation {
    curves: BTreeMap<String,Automation>,
    tempo: TempoMapping,
    offset: GlobalBlick,
}

impl PreparedAutomation {
    pub(super) fn add_growl(input:&OwnerInput,curves:&mut BTreeMap<String,Automation>)->crate::synthesis::voice::Result<()> {
        use crate::synthesis::{experiments::GrowlControls,voice::VoiceError};
        let controls=GrowlControls::from_voice(&input.voice)?;
        for (key,constant,default,low,high) in [
            ("growl_frequency",controls.frequency,60.0,30.0,90.0),
            ("growl_amplitude",controls.amplitude,0.0,0.0,1.0),
        ] {
            let mut curve=input.automation.get(key).cloned().unwrap_or_default();
            curve.validate().map_err(VoiceError::new)?;
            if curve.points.len()>90000 || curve.extra.keys().any(|k|k!="scriptData")
                || curve.points.iter().any(|(_,v)|!((low as f64)..=(high as f64)).contains(v)) {
                return Err(VoiceError::new("Invalid growl curve"));
            }
            if curve.points.is_empty() {curve.insert(0,f64::from(constant));}
            else {for (_,value) in &mut curve.points {*value=(*value+f64::from(constant-default)).clamp(low as f64,high as f64);}}
            curves.insert(key.into(),curve);
        }
        Ok(())
    }
    pub fn for_experiments(input:&OwnerInput)->Option<Self> {Self::new(input)}
    pub(super) fn new(input:&OwnerInput)->Option<Self> {
        input.automation.values().any(|curve|!curve.points.is_empty()).then(||Self {
            curves:input.automation.clone(),tempo:input.tempo.clone(),offset:input.content_offset,
        })
    }
    pub fn value(&self,key:&str,seconds:f64,default:f32)->f32 {
        let local=(i128::from(self.tempo.position_at(seconds).0)-i128::from(self.offset.0))
            .clamp(i128::from(i64::MIN),i128::from(i64::MAX)) as i64;
        self.curves.get(key).map_or(default,|curve|curve.sample(local,default))
    }
    pub fn has(&self,key:&str)->bool {
        self.curves.get(key).is_some_and(|curve|!curve.points.is_empty())
    }
    pub fn mode_names(&self)->impl Iterator<Item=&str> {
        self.curves.iter().filter(|(_,c)|!c.points.is_empty())
            .filter_map(|(key,_)|key.strip_prefix("vocalMode_").or_else(||key.strip_prefix("vocalModeComponent_").and_then(|v|v.split_once(':').map(|(_,name)|name))))
    }
}
