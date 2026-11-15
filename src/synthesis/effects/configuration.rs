use super::{compressor, equalizer, reverb, room};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Configuration {
    pub room: Option<room::Parameters>,
    pub equalizer: Option<[equalizer::Parameters; 4]>,
    pub compressor: Option<compressor::Parameters>,
    pub reverb: Option<reverb::Parameters>,
}

fn number(value: &Value, name: &str, default: f32) -> Result<f32, String> {
    match value.get(name) {
        None => Ok(default),
        Some(v) => v.as_f64().map(|x| x as f32).filter(|x| x.is_finite())
            .ok_or_else(|| format!("Invalid effect parameter {name}")),
    }
}

impl Configuration {
    pub fn defaults() -> Value {
        serde_json::json!({
            "postRoomEq":{"enabled":false,"filters":[
                {"freq":117.7_f32,"gain":0.0,"q":0.71_f32},
                {"freq":692.8_f32,"gain":0.0,"q":0.71_f32},
                {"freq":4077.7_f32,"gain":0.0,"q":0.71_f32}],
                "lowShelf":{"freq":25.0,"gain":0.0}},
            "compressor":{"enabled":false,"attack":0.0,"ratio":4.0,"threshold":-10.0},
            "reverb":{"enabled":false,"type":"clean","preDelay":0.05_f32,"decay":1.0,"dryWetRatio":0.5}
        })
    }
    pub fn read(value: &Value) -> Result<Self, String> {
        let object = value.as_object().ok_or("Effect settings must be an object")?;
        let mut result = Self::default();
        for (name, value) in object {
            match value.get("enabled").and_then(Value::as_bool) {
                Some(false) => continue,
                Some(true) => {},
                None => return Err(format!("Invalid effect switch {name}")),
            }
            match name.as_str() {
                "room" => {
                    let p = room::Parameters {
                        position_x: number(value,"positionX",0.0)?,
                        position_y: number(value,"positionY",0.0)?,
                        size: number(value,"size",15.0)?,
                        reflection_gain: number(value,"reflectionGain",0.0)?,
                    };
                    if p.size <= 0.0 { return Err("Invalid room size".into()); }
                    result.room = Some(p);
                }
                "postRoomEq" => {
                    let peaks = value.get("filters").and_then(Value::as_array).filter(|v| v.len()==3)
                        .ok_or("Equalizer requires three filters")?;
                    let shelf = value.get("lowShelf").ok_or("Missing equalizer low shelf")?;
                    let mut filters = Vec::with_capacity(4);
                    for p in peaks {
                        filters.push(equalizer::Parameters { kind: equalizer::Kind::Peak,
                            frequency: number(p,"freq",1000.0)?, gain: number(p,"gain",0.0)?, q: number(p,"q",0.71)? });
                    }
                    filters.push(equalizer::Parameters { kind: equalizer::Kind::LowShelf,
                        frequency: number(shelf,"freq",25.0)?, gain: number(shelf,"gain",0.0)?, q: 1.0 });
                    if filters.iter().any(|p| p.frequency < 0.0 || p.q <= 0.0) { return Err("Invalid equalizer frequency or Q".into()); }
                    result.equalizer = Some(filters.try_into().map_err(|_|"Invalid equalizer count")?);
                }
                "compressor" => {
                    let p = compressor::Parameters { attack: number(value,"attack",0.0)?, ratio: number(value,"ratio",4.0)?, threshold: number(value,"threshold",-10.0)? };
                    if p.attack < 0.0 || p.ratio < 1.0 { return Err("Invalid compressor attack or ratio".into()); }
                    result.compressor = Some(p);
                }
                "reverb" => {
                    let (damping_frequency, directional_gain) = match value.get("type").and_then(Value::as_str).unwrap_or("clean") {
                        "clean" => (3000.0,0.0),
                        "hall" => (10000.0,1.0),
                        "room" => (1000.0,0.3),
                        other => return Err(format!("Unsupported reverb type {other}")),
                    };
                    let p = reverb::Parameters { pre_delay: number(value,"preDelay",0.05)?, decay: number(value,"decay",1.0)?, dry_wet: number(value,"dryWetRatio",0.5)?, damping_frequency, directional_gain };
                    if !(0.0..=0.2).contains(&p.pre_delay) || p.decay<=0.0 || !(0.0..=1.0).contains(&p.dry_wet) {
                        return Err("Invalid reverb parameters".into());
                    }
                    result.reverb = Some(p);
                }
                _ => return Err(format!("Unsupported enabled mixer effect {name}")),
            }
        }
        Ok(result)
    }

    pub fn enabled(&self) -> bool { self.room.is_some() || self.equalizer.is_some() || self.compressor.is_some() || self.reverb.is_some() }
}
