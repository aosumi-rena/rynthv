use crate::model::Extra;
use serde_json::Value;

pub(super) fn validate_project_mixer(value: Option<&Value>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let object = value.as_object().ok_or("工程 projectMixer 必须为对象")?;
    for (key, value) in object {
        match key.as_str() {
            "linkRoomSettings" if value.is_boolean() => {}
            _ => return Err(format!("暂不支持工程混音设置 projectMixer.{key} = {value}")),
        }
    }
    Ok(())
}

pub(super) fn validate_mixer(mixer: &Extra) -> Result<(), String> {
    for (key, value) in mixer {
        let supported = match key.as_str() {
            "gainDecibel" | "pan" => value.as_f64().is_some_and(f64::is_finite),
            "mute" | "solo" | "display" => value.is_boolean(),
            "fxPresetName" => value.is_string(),
            "micPresetName" => value.as_str()==Some(""),
            "micParams" => value==&serde_json::json!({"instrumentMics":{"mics":[],"mute":false,"solo":false,"link":false},"room":{"name":"","micArrays":[]}}),
            "fxParams" => {
                super::effects::Configuration::read(value)?;
                true
            }
            "scriptData" => true,
            _ => false,
        };
        if !supported {
            return Err(format!("暂不支持混音设置 {key} = {value}"));
        }
    }
    Ok(())
}
pub(super) fn validate_instrument_mixer(mixer:&Extra)->Result<(),String> {
    let mut rest=mixer.clone();
    if let Some(value)=rest.get("fxParams") {
        if super::effects::Configuration::read(value)?.room.is_some() {
            return Err("Room Sound is a voice effect; instruments use their microphone room".into());
        }
    }
    if let Some(value)=rest.remove("micParams") {super::instruments::microphones::Configuration::parse(&value)?;}
    if let Some(value)=rest.remove("micPresetName") {if !value.is_string(){return Err("Invalid microphone preset name".into());}}
    validate_mixer(&rest)
}

pub(super) fn validate_export_bypass(config: &Extra) -> Result<(), String> {
    for (key, label) in [
        ("bypassPan", "绕过声像"),
        ("bypassGain", "绕过音量"),
        ("bypassEffects", "绕过效果"),
    ] {
        match config.get(key) {
            None | Some(Value::Bool(false)) => {}
            Some(Value::Bool(true)) => return Err(format!("暂不支持导出选项“{label}”（{key}）")),
            Some(_) => return Err(format!("导出选项 {key} 必须为布尔值")),
        }
    }
    Ok(())
}
