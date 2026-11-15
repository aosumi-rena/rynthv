use super::*;
use std::collections::BTreeMap;

pub(super) fn project(p: &Project) -> Result<Value> {
    let mut out = object(p.extra.get(SOURCE).unwrap_or(&Value::Null));
    out["version"] = json!(1);
    out["tempos"] = json!(
        p.time
            .tempo
            .iter()
            .map(|m| json!({"pos":tick(m.position),"bpm":m.bpm}))
            .collect::<Vec<_>>()
    );
    out["timeSignatures"] = json!(
        p.time
            .meter
            .iter()
            .map(
                |m| json!({"barIndex":m.index,"numerator":m.numerator,"denominator":m.denominator})
            )
            .collect::<Vec<_>>()
    );
    let mut config: Extra =
        serde_json::from_value(object(&out["exportConfig"])).map_err(|e| e.to_string())?;
    controls::export_config(&json!(p.render_config), &mut config, false);
    out["exportConfig"] = json!(config);
    let mut tracks: Vec<Value> = Vec::new();
    let mut destinations = BTreeMap::new();
    for t in &p.tracks {
        let mut row = object(t.extra.get(SOURCE).unwrap_or(&Value::Null));
        row["name"] = json!(t.name);
        row["gain"] = t.mixer.get("gainDecibel").cloned().unwrap_or(json!(0.));
        row["pan"] = t.mixer.get("pan").cloned().unwrap_or(json!(0.));
        for key in ["mute", "solo"] {
            row[key] = t.mixer.get(key).cloned().unwrap_or(json!(false));
        }
        let c = t.disp_color.rgba();
        row["color"] = json!(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
        row["exportEnabled"] = t.extra.get("renderEnabled").cloned().unwrap_or(json!(true));
        if row.get("exportChannels").is_none() {
            row["exportChannels"] = json!(1);
        }
        if row.get("asRefer").is_none() {
            row["asRefer"] = json!(true);
        }
        let origin = t
            .extra
            .get("rynthvTuneLabTrackId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("new:{}", t.id));
        let key = format!(
            "{origin}:{}",
            serde_json::to_string(&row).map_err(|e| e.to_string())?
        );
        let index = if let Some(&i) = destinations.get(&key) {
            i
        } else {
            let i = tracks.len();
            row["parts"] = json!([]);
            tracks.push(row);
            destinations.insert(key, i);
            i
        };
        if let Some(clip) = t.audio_clip() {
            let mut part = object(t.main_ref.extra.get(SOURCE).unwrap_or(&Value::Null));
            let anchor = p.time.position_at(clip.onset_seconds);
            let (begin, end) = clip.window();
            part["type"] = json!("audio");
            part["name"] = part.get("name").cloned().unwrap_or(json!(t.name));
            part["path"] = json!(clip.path);
            part["pos"] = json!(tick(anchor));
            part["startOffset"] = json!(tick(p.time.position_at(begin) - anchor));
            part["endOffset"] = json!(tick(p.time.position_at(end) - anchor));
            part["__rynthvOrder"] = t
                .main_ref
                .extra
                .get("rynthvTuneLabPartOrder")
                .cloned()
                .unwrap_or(json!(u64::MAX));
            tracks[index]["parts"].as_array_mut().unwrap().push(part);
        } else {
            for r in t.references() {
                let g = p
                    .group(&r.group_id)
                    .ok_or("TuneLab export: missing group")?;
                if g.notes.is_empty() && !g.extra.contains_key(SOURCE) {
                    continue;
                }
                let mut part = object(g.extra.get(SOURCE).unwrap_or(&Value::Null));
                part["type"] = json!("midi");
                part["name"] = json!(g.name);
                part["pos"] = json!(tick(r.blick_offset));
                part["startOffset"] = json!(tick(r.blick_absolute_begin - r.blick_offset));
                let end = if r.blick_absolute_end >= 0 {
                    r.blick_absolute_end - r.blick_offset
                } else {
                    g.notes
                        .iter()
                        .map(|n| n.onset + n.duration)
                        .max()
                        .unwrap_or(0)
                };
                part["endOffset"] = json!(tick(end));
                part.as_object_mut().unwrap().remove("dur");
                if part.get("gain").is_none() {
                    part["gain"] = json!(0.);
                }
                if !part["properties"].is_object() {
                    part["properties"] = json!({});
                }
                if let Some(inst) = &t.instrument {
                    let id = inst
                        .selection
                        .as_ref()
                        .map(|s| s.database_id.as_str())
                        .unwrap_or(text(&r.database, "id", ""));
                    let rev = inst
                        .selection
                        .as_ref()
                        .map(|s| s.revision.as_str())
                        .unwrap_or(text(&r.database, "version", ""));
                    part["voice"] = json!({"type":"Cordis","id":id,"kind":"instrument"});
                    part["properties"]["instrument_version"] = json!(rev);
                } else {
                    if !part["voice"].is_object() {
                        part["voice"] = json!({"type":"Mortis","kind":"voice"});
                    }
                    if let Some(id) = r.database.get("id") {
                        part["voice"]["id"] = id.clone();
                    } else if part["voice"].get("id").is_none() {
                        part["voice"]["id"] = json!(text(&r.database, "name", ""));
                    }
                    if let Some(rev) = r.database.get("version") {
                        part["properties"]["voice_version"] = rev.clone();
                    }
                }
                part["notes"] = Value::Array(
                    g.notes
                        .iter()
                        .map(|n| note(n, t.is_instrument(), r.pitch_offset))
                        .collect::<Result<_>>()?,
                );
                if !part["vibratos"].is_array() {
                    part["vibratos"] = json!([]);
                }
                controls::write_curves(g, &mut part);
                controls::write_voice(r, &mut part);
                part["__rynthvOrder"] = r
                    .extra
                    .get("rynthvTuneLabPartOrder")
                    .cloned()
                    .unwrap_or(json!(u64::MAX));
                tracks[index]["parts"].as_array_mut().unwrap().push(part);
            }
        }
    }
    for track in &mut tracks {
        let parts = track["parts"].as_array_mut().unwrap();
        parts.sort_by_key(|part| part["__rynthvOrder"].as_u64().unwrap_or(u64::MAX));
        for part in parts {
            part.as_object_mut().unwrap().remove("__rynthvOrder");
        }
    }
    out["tracks"] = json!(tracks);
    Ok(out)
}
fn note(n: &Note, instrument: bool, pitch_offset: i32) -> Result<Value> {
    let mut row = object(n.extra.get(SOURCE).unwrap_or(&Value::Null));
    row["pos"] = json!(tick(n.onset));
    row["dur"] = json!(tick(n.duration));
    row["pitch"] = json!(n.pitch + pitch_offset);
    row["lyric"] = json!(n.lyrics);
    if row.get("pronunciation").is_none() {
        row["pronunciation"] = json!("");
    }
    if !row["properties"].is_object() {
        row["properties"] = json!({});
    }
    for (src, dst) in [
        ("expValueX", "exp_x"),
        ("expValueY", "exp_y"),
        ("dF0VbrMod", "f0_vibrato_modulation"),
        ("languageOverride", "language"),
        ("phoneset", "phonetic_alphabet"),
    ] {
        if let Some(value) = n.attributes.get(src) {
            row["properties"][dst] = value.clone();
        }
    }
    if n.musical_type == "rap" {
        row["properties"]["f0_mode"] = json!("rap");
    }
    let takes = n.extra.get("takes");
    let active = takes
        .and_then(|v| v.get("activeTakeId"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if let Some(take) = takes
        .and_then(|v| v.get("takes"))
        .and_then(Value::as_array)
        .and_then(|a| a.iter().find(|t| t["id"].as_u64() == Some(active)))
    {
        for (src, dst) in [
            ("seedDuration", "duration_seed"),
            (
                "seedPitch",
                if instrument { "pitch_seed" } else { "f0_seed" },
            ),
            (
                "seedTimbre",
                if instrument { "timbre_seed" } else { "seed" },
            ),
        ] {
            let v = take[src].as_u64().unwrap_or(0) as f64;
            if v != 0. || row["properties"].get(dst).is_some() {
                row["properties"][dst] = json!(v);
            }
        }
    }
    if let Some(old) = row.get("phonemes").and_then(Value::as_array).cloned() {
        let mut prefix = 0;
        while prefix < old.len().saturating_sub(1)
            && number(&old[prefix], "startTime", 0.)? + number(&old[prefix], "endTime", 0.)? < 0.
        {
            prefix += 1;
        }
        let mut leading = Vec::new();
        let mut body = Vec::new();
        for (i, phone) in old.iter().enumerate() {
            let p = json!({"symbol":text(phone,"symbol",""),"duration":(number(phone,"endTime",0.)?-number(phone,"startTime",0.)?).max(0.),"stretchWeight":if i==prefix{1.}else{0.},"properties":object(&phone["properties"])});
            if i < prefix {
                leading.push(p);
            } else {
                body.push(p);
            }
        }
        row["bodyOffset"] = json!(
            old.get(prefix)
                .map(|p| number(p, "startTime", 0.))
                .transpose()?
                .unwrap_or(0.)
        );
        row["leadingPhonemes"] = json!(leading);
        row["bodyPhonemes"] = json!(body);
        row.as_object_mut().unwrap().remove("phonemes");
    }
    let stored = ["leadingPhonemes", "bodyPhonemes"]
        .iter()
        .flat_map(|k| row.get(*k).and_then(Value::as_array).into_iter().flatten())
        .map(|p| text(p, "symbol", ""))
        .collect::<Vec<_>>()
        .join(" ");
    if n.phonemes != stored {
        row["leadingPhonemes"] = json!([]);
        row["bodyOffset"] = json!(0.);
        row["bodyPhonemes"] = json!(
            n.phonemes
                .split_whitespace()
                .map(|s| json!({"symbol":s,"duration":0.,"stretchWeight":1.}))
                .collect::<Vec<_>>()
        );
    }
    Ok(row)
}
