use super::Extra;
use serde::{Deserialize, Serialize};
mod sample;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Parameter {
    Pitch,
    Vibrato,
    Loudness,
    Tension,
    Breathiness,
    Voicing,
    Gender,
    ToneShift,
    Mouth,
    Rap,
    VocalMode(String),
    Experimental { key:String, label:String },
}
impl Parameter {
    pub const ALL: [Self; 10] = [
        Self::Pitch,
        Self::Vibrato,
        Self::Loudness,
        Self::Tension,
        Self::Breathiness,
        Self::Voicing,
        Self::Gender,
        Self::ToneShift,
        Self::Mouth,
        Self::Rap,
    ];
    pub fn key(&self) -> &str {
        match self {
            Self::Pitch => "pitchDelta",
            Self::Vibrato => "vibratoEnv",
            Self::Loudness => "loudness",
            Self::Tension => "tension",
            Self::Breathiness => "breathiness",
            Self::Voicing => "voicing",
            Self::Gender => "gender",
            Self::ToneShift => "toneShift",
            Self::Mouth => "mouthOpening",
            Self::Rap => "rapIntonation",
            Self::VocalMode(key) => key,
            Self::Experimental {key,..} => key,
        }
    }
    pub fn label(&self) -> &str {
        match self {
            Self::Pitch => "音高偏差",
            Self::Vibrato => "颤音包络",
            Self::Loudness => "响度",
            Self::Tension => "张力",
            Self::Breathiness => "气声",
            Self::Voicing => "发声",
            Self::Gender => "性别",
            Self::ToneShift => "声区转换",
            Self::Mouth => "口型",
            Self::Rap => "说唱语调",
            Self::VocalMode(key) => key.strip_prefix("vocalMode_").unwrap_or(key),
            Self::Experimental {label,..} => label,
        }
    }
    pub fn range(&self) -> (f64, f64) {
        match self {
            Self::Pitch => (-1200.0, 1200.0),
            Self::Loudness => (-48.0, 24.0),
            Self::ToneShift => (-800.0, 800.0),
            Self::Vibrato => (0.0, 2.0),
            Self::Voicing | Self::Rap => (0.0, 1.0),
            Self::Mouth => (-1.0, 1.0),
            Self::VocalMode(_) => {
                let (_,high)=crate::synthesis::experiments::ExperimentalSettings::ui().style_range();
                (-high,high)
            },
            Self::Experimental {key,..} if key=="growl_frequency" => (30.0,90.0),
            Self::Experimental {key,..} if key=="ddsDefaultStyle" => (-3.0,3.0),
            Self::Experimental {key,..} if key.starts_with("ddsPhoneMix_") => (0.0,1.0),
            Self::Experimental {key,..} if key=="growl_amplitude" || key.starts_with("voiceBlend_") => (0.0,1.0),
            Self::Experimental {..} => crate::synthesis::experiments::ExperimentalSettings::ui().style_range(),
            _ => (-2.0, 2.0),
        }
    }
    pub fn default_value(&self) -> f64 {
        if self.key()=="growl_frequency" {return 60.0;}
        if matches!(self, Self::Vibrato | Self::Voicing) { 1.0 } else { 0.0 }
    }
    pub fn is_visible(&self)->bool {
        let settings=crate::synthesis::experiments::ExperimentalSettings::ui();
        match self {
            Self::Experimental {key,..} if matches!(key.as_str(),"growl_frequency"|"growl_amplitude") => settings.active(settings.growl),
            Self::Experimental {key,..} if key=="ddsDefaultStyle" || key.starts_with("ddsPhoneMix_") => settings.active(settings.dds_controls),
            Self::Experimental {key,..} if key.starts_with("voiceBlend_") => settings.active(settings.voice_blending),
            Self::Experimental {..} => settings.active(settings.component_curves),
            _=>true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Automation {
    pub mode: String,
    #[serde(with = "flat_points")]
    pub points: Vec<(i64, f64)>,
    #[serde(flatten)]
    pub extra: Extra,
}
impl Default for Automation {
    fn default() -> Self {
        Self {
            mode: "cubic".into(),
            points: Vec::new(),
            extra: Extra::new(),
        }
    }
}
impl Automation {
    pub fn validate(&self)->Result<(),String> {
        if !["linear","cosine","cubic"].contains(&self.mode.as_str())
            || self.points.iter().any(|(_,v)|!(*v as f32).is_finite())
            || self.points.windows(2).any(|p|p[0].0>=p[1].0) {
            return Err("参数插值模式、控制点顺序或数值无效".into());
        }
        Ok(())
    }
    pub fn insert(&mut self, time: i64, value: f64) {
        match self.points.binary_search_by_key(&time, |p| p.0) {
            Ok(i) => self.points[i].1 = value,
            Err(i) => self.points.insert(i, (time, value)),
        }
    }
}
pub(super) mod flat_points {
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
    use serde_json::Value;
    pub fn serialize<S: Serializer>(
        points: &[(i64, f64)],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let values: Vec<Value> = points
            .iter()
            .flat_map(|(t, v)| [Value::from(*t), Value::from(*v)])
            .collect();
        values.serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<(i64, f64)>, D::Error> {
        let values = Vec::<Value>::deserialize(deserializer)?;
        if !values.len().is_multiple_of(2) {
            return Err(D::Error::custom("参数控制点必须为 time/value 对"));
        }
        values
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                Ok((
                    p[0].as_i64()
                        .ok_or_else(|| D::Error::custom("控制点时间必须为 i64"))?,
                    p[1].as_f64()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| D::Error::custom("控制点值必须为有限数"))?,
                ))
            })
            .collect()
    }
}
