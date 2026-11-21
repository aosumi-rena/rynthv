use crate::model::*;
use serde_json::{Value, json};

pub(super) const VERSION: i32 = 201;

fn takes(value: &mut Value) {
    if let Some(rows) = value
        .get_mut("takes")
        .and_then(|v| v.get_mut("takes"))
        .and_then(Value::as_array_mut)
    {
        for row in rows {
            if let Some(row) = row.as_object_mut() {
                row.entry("seedDuration").or_insert(json!(0));
            }
        }
    }
}

pub(super) fn decode(bytes: &[u8]) -> Result<Project, String> {
    let mut value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if value.get("version").and_then(Value::as_i64) != Some(i64::from(VERSION)) {
        return Err("Unsupported Instrument X project version".into());
    }
    fn group(value: &mut Value) {
        if let Some(notes) = value.get_mut("notes").and_then(Value::as_array_mut) {
            for note in notes {
                if let Some(note) = note.as_object_mut() {
                    note.entry("lyrics").or_insert(json!(""));
                    if let Some(takes) = note
                        .get_mut("takes")
                        .and_then(|v| v.get_mut("takes"))
                        .and_then(Value::as_array_mut)
                    {
                        for take in takes {
                            if let Some(take) = take.as_object_mut() {
                                take.entry("seedDuration").or_insert(json!(0));
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(groups) = value.get_mut("library").and_then(Value::as_array_mut) {
        for item in groups {
            group(item);
        }
    }
    if let Some(tracks) = value.get_mut("tracks").and_then(Value::as_array_mut) {
        for track in tracks {
            if let Some(reference) = track.get_mut("mainRef") {
                takes(reference);
            }
            if let Some(references) = track.get_mut("groups").and_then(Value::as_array_mut) {
                for reference in references {
                    takes(reference);
                }
            }
            if let Some(main) = track.get_mut("mainGroup") {
                group(main);
            }
            if track
                .pointer("/mainRef/database/backendType")
                .and_then(Value::as_str)
                == Some("W")
            {
                track.as_object_mut().unwrap().insert("rynthvInstrument".into(),json!({"selection":null,"ambientSeed":0}));
            }
        }
    }
    serde_json::from_value(value).map_err(|e| format!("Instrument X project: {e}"))
}

pub(super) fn prepare_save(project: &mut Project) {
    for track in &mut project.tracks {
        let Some(instrument) = track.instrument.take() else {
            continue;
        };
        let database = &mut track.main_ref.database;
        if !database.is_object() {
            *database = json!({});
        }
        database["backendType"] = json!("W");
        if let Some(selection) = instrument.selection {
            database["id"] = json!(selection.database_id);
            database["version"] = json!(selection.revision);
        }
    }
}
