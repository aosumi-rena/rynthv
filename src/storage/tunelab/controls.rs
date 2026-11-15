use super::*;

pub(super) fn export_config(source: &Value, target: &mut Extra, import: bool) {
    for (lab, native) in [
        ("exportPath", "destination"),
        ("fileName", "filename"),
        ("format", "audioFormat"),
        ("sampleRate", "sampleRate"),
        ("bitDepth", "bitDepth"),
        ("bitrate", "bitRate"),
        ("masterExportEnabled", "exportMixDown"),
        ("masterExportChannels", "numChannels"),
    ] {
        let (from, to) = if import { (lab, native) } else { (native, lab) };
        if let Some(v) = source.get(from) {
            target.insert(to.into(), v.clone());
        }
    }
}
pub(super) fn read_note(row: &Value, n: &mut Note, instrument: bool, version: u64) -> Result<()> {
    let props = &row["properties"];
    let mut phones = Vec::new();
    for key in if version == 0 {
        vec!["phonemes"]
    } else {
        vec!["leadingPhonemes", "bodyPhonemes"]
    } {
        if let Some(items) = row.get(key).and_then(Value::as_array) {
            for item in items {
                phones.push(text(item, "symbol", "").to_owned());
            }
        }
    }
    n.phonemes = phones.join(" ");
    for (src, dst) in [
        ("exp_x", "expValueX"),
        ("exp_y", "expValueY"),
        ("f0_vibrato_modulation", "dF0VbrMod"),
        ("language", "languageOverride"),
        ("phonetic_alphabet", "phoneset"),
    ] {
        if let Some(v) = props.get(src) {
            n.attributes.insert(dst.into(), v.clone());
        }
    }
    if text(props, "f0_mode", "") == "rap" {
        n.musical_type = "rap".into();
    }
    let seed = |key| -> Result<u32> {
        match props.get(key) {
            None => Ok(0),
            Some(v) => {
                let x = v.as_f64().ok_or("Invalid TuneLab retake seed")?;
                if x.fract() != 0. || !(0. ..=u32::MAX as f64).contains(&x) {
                    return Err("Invalid TuneLab retake seed".into());
                }
                Ok(x as u32)
            }
        }
    };
    n.extra.insert("takes".into(),json!({"activeTakeId":0,"takes":[{"id":0,"seedDuration":seed("duration_seed")?,"seedPitch":seed(if instrument{"pitch_seed"}else{"f0_seed"})?,"seedTimbre":seed(if instrument{"timbre_seed"}else{"seed"})?,"liked":false}]}));
    Ok(())
}
fn curve_key(key: &str) -> Option<(String, f64)> {
    Some(match key {
        "tone_shift" => ("toneShift".into(), 1.),
        "mouth_opening" => ("mouthOpening".into(), 1.),
        "PitchDeviation" => ("pitchDelta".into(), 100.),
        "VibratoEnvelope" => ("vibratoEnv".into(), 1.),
        "loudness" | "tension" | "breathiness" | "voicing" | "gender" => (key.into(), 1.),
        _ if key.starts_with("style:") => {
            let style = &key[6..];
            if let Some((name, lane)) = style
                .rsplit_once(':')
                .filter(|(_, l)| ["pitch", "timbre", "pronunciation"].contains(l))
            {
                (format!("vocalModeComponent_{lane}:{name}"), 100.)
            } else {
                (format!("vocalMode_{style}"), 100.)
            }
        }
        _ => return None,
    })
}
pub(super) fn read_curves(part: &Value, group: &mut Group) -> Result<()> {
    if let Some(lines) = part.get("pitch").and_then(Value::as_array) {
        for line in lines {
            let values = line.as_array().ok_or("Invalid TuneLab pitch curve")?;
            if values.len() % 2 != 0 {
                return Err("Odd TuneLab pitch curve".into());
            }
            let points = values
                .chunks_exact(2)
                .map(|p| {
                    Ok((
                        clock(p[0].as_f64().ok_or("Invalid pitch time")?)?,
                        p[1].as_f64().ok_or("Invalid pitch value")?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            if points.len() >= 2 {
                group.pitch_controls.push(PitchControl::Curve(PitchCurve {
                    pos: 0,
                    pitch: 0.,
                    id: uuid::Uuid::new_v4().as_u64_pair().0.to_string(),
                    points,
                    extra: Extra::new(),
                }));
            }
        }
    }
    if let Some(automations) = part.get("automations").and_then(Value::as_object) {
        for (key, value) in automations {
            let Some((native, scale)) = curve_key(key) else {
                continue;
            };
            let mut curve = Automation {
                mode: "linear".into(),
                ..Default::default()
            };
            let values = array(value, "values")?;
            if values.len() % 2 != 0 {
                return Err("Odd TuneLab automation curve".into());
            }
            for p in values.chunks_exact(2) {
                curve.points.push((
                    clock(p[0].as_f64().ok_or("Invalid automation time")?)?,
                    p[1].as_f64().ok_or("Invalid automation value")? * scale,
                ));
            }
            if curve.points.is_empty() {
                curve
                    .points
                    .push((0, number(value, "default", 0.)? * scale));
            }
            curve
                .extra
                .insert("rynthvTuneLabAutomation".into(), json!(key));
            if native.starts_with("vocalMode_") {
                group.vocal_modes.insert(native, curve);
            } else {
                group.parameters.insert(native, curve);
            }
        }
    }
    Ok(())
}
pub(super) fn read_voice(part: &Value, reference: &mut GroupRef) -> Result<()> {
    let props = &part["properties"];
    let mut voice = json!({});
    for (src, dst) in [
        ("consonant_strength", "consonantStrength"),
        ("consonant_duration", "consonantDuration"),
        ("relaxedPronunciation", "relaxedPronunciation"),
        ("europeanPronunciation", "europeanPronunciation"),
    ] {
        if let Some(v) = props.get(src) {
            voice[dst] = v.clone();
        }
    }
    if let Some(styles) = props.get("style_mix").and_then(Value::as_object) {
        let mut modes = json!({});
        for (name, style) in styles {
            if style.get("enabled").and_then(Value::as_bool) != Some(false) {
                modes[name] = json!({"pitch":number(style,"pitch",0.)?,"timbre":number(style,"timbre",0.)?,"pronunciation":number(style,"pronunciation",0.)?});
            }
        }
        voice["vocalModeParams"] = modes;
        voice["vocalModeInherited"] = json!(false);
    }
    reference.extra.insert("voice".into(), voice);
    for (source, target) in [
        ("language", "languageOverride"),
        ("phonetic_alphabet", "phonesetOverride"),
    ] {
        if let Some(v) = props.get(source) {
            reference.database[target] = v.clone();
        }
    }
    Ok(())
}
pub(super) fn write_curves(group: &Group, part: &mut Value) {
    part["pitch"] = json!(
        group
            .pitch_controls
            .iter()
            .filter_map(|c| match c {
                PitchControl::Curve(c) => Some(
                    c.absolute_points()
                        .flat_map(|(x, y)| [tick(x), y])
                        .collect::<Vec<_>>()
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
    );
    if !part["automations"].is_object() {
        part["automations"] = json!({});
    }
    for (native, curve) in group.parameters.iter().chain(&group.vocal_modes) {
        if curve.points.is_empty() {
            continue;
        }
        let key = curve
            .extra
            .get("rynthvTuneLabAutomation")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| match native.as_str() {
                "pitchDelta" => "PitchDeviation".into(),
                "vibratoEnv" => "VibratoEnvelope".into(),
                "toneShift" => "tone_shift".into(),
                "mouthOpening" => "mouth_opening".into(),
                s if s.starts_with("vocalMode_") => format!("style:{}", &s[10..]),
                s if s.starts_with("vocalModeComponent_") => s[19..]
                    .split_once(':')
                    .map(|(lane, name)| format!("style:{name}:{lane}"))
                    .unwrap_or_else(|| s.into()),
                s => s.into(),
            });
        let Some((_, scale)) = curve_key(&key) else {
            continue;
        };
        let mut value = object(&part["automations"][&key]);
        if value.get("default").is_none() {
            value["default"] = json!(if native == "vibratoEnv" { 1. } else { 0. });
        }
        value["values"] = json!(
            curve
                .points
                .iter()
                .flat_map(|(x, y)| [tick(*x), *y / scale])
                .collect::<Vec<_>>()
        );
        part["automations"][key] = value;
    }
}

pub(super) fn write_voice(reference: &GroupRef, part: &mut Value) {
    let props = &mut part["properties"];
    for (src, dst) in [
        ("languageOverride", "language"),
        ("phonesetOverride", "phonetic_alphabet"),
    ] {
        if let Some(value) = reference.database.get(src) {
            props[dst] = value.clone();
        }
    }
    if let Some(voice) = reference.extra.get("voice") {
        for (src, dst) in [
            ("consonantStrength", "consonant_strength"),
            ("consonantDuration", "consonant_duration"),
            ("relaxedPronunciation", "relaxedPronunciation"),
            ("europeanPronunciation", "europeanPronunciation"),
        ] {
            if let Some(value) = voice.get(src) {
                props[dst] = value.clone();
            }
        }
        if let Some(modes) = voice.get("vocalModeParams").and_then(Value::as_object) {
            if modes.is_empty() && props.get("style_mix").is_none() {
                return;
            }
            if !props["style_mix"].is_object() {
                props["style_mix"] = json!({});
            }
            if let Some(styles) = props["style_mix"].as_object_mut() {
                for (name, style) in styles {
                    if !modes.contains_key(name) {
                        style["enabled"] = json!(false);
                    }
                }
            }
            for (name, mode) in modes {
                let mut style = object(&props["style_mix"][name]);
                style["enabled"] = json!(true);
                for lane in ["pitch", "timbre", "pronunciation"] {
                    style[lane] = mode
                        .get(lane)
                        .cloned()
                        .or_else(|| mode.as_f64().map(|v| json!(v)))
                        .unwrap_or(json!(0.));
                }
                props["style_mix"][name] = style;
            }
        }
    }
}
