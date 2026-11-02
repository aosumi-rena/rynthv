//! Actual editor transformations on explicit files; no system/shared clipboard IO.
use crate::{editor::Editor, model, storage};
use rynthv::clipboard::Content;
use serde_json::{Value, json};
use std::{ffi::OsString, fs, io::Write, path::PathBuf};

pub(super) fn run(args: &[OsString]) -> Result<Value, Box<dyn std::error::Error>> {
    if args.len() != 5 {
        return Err("Usage: --clipboard-apply PROJECT PAYLOAD NEW_PROJECT AT_BLICK | --clipboard-capture PROJECT KIND NEW_PAYLOAD AT_BLICK".into());
    }
    let project = storage::load(&PathBuf::from(&args[1]))?;
    let mut editor = Editor::new(project);
    let at = args[4]
        .to_str()
        .ok_or("Invalid placement")?
        .parse::<i64>()?;
    let output = PathBuf::from(&args[3]);
    if output.exists() {
        return Err("Output already exists".into());
    }
    editor.transport.position = at;
    if args[0] == "--clipboard-capture" {
        let kind = args[2].to_str().ok_or("Invalid selection")?;
        let data = match kind {
            "notes" | "notes-only" => {
                editor.select_all_notes();
                editor
                    .copy_content(kind == "notes")?
                    .ok_or("No notes selected")?
            }
            "groups" => {
                editor.group_focus = true;
                editor.select_all_groups();
                editor.copy_content(false)?.ok_or("No groups selected")?
            }
            "pitch" => Content {
                pitch_controls: editor.group().ok_or("No group")?.pitch_controls.clone(),
                position_set: true,
                ..Default::default()
            },
            "parameters" => Content {
                parameters: editor
                    .group()
                    .ok_or("No group")?
                    .parameters
                    .iter()
                    .filter(|(_, c)| !c.points.is_empty())
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                position_set: true,
                ..Default::default()
            },
            _ => return Err("Unknown clipboard selection".into()),
        };
        data.validate()?;
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&output)?
            .write_all(&serde_json::to_vec_pretty(&data)?)?;
        return Ok(
            json!({"clipboard":output,"summary":data.summary(),"system_clipboard_written":false}),
        );
    }
    let bytes = fs::read(PathBuf::from(&args[2]))?;
    let data = Content::decode(&bytes)?;
    let original = editor.project.clone();
    let count = editor.paste_content(data.clone(), at)?;
    let result = editor.project.clone();
    editor.undo();
    let undo = editor.project == original;
    editor.redo();
    let redo = editor.project == result;
    if !undo || !redo {
        return Err("Clipboard operation is not a single reversible edit".into());
    }
    storage::save(&output, &editor.project)?;
    Ok(
        json!({"project":output,"inserted":count,"source_summary":data.summary(),"single_undo":undo,"redo_exact":redo,
        "groups":editor.project.library.len(),"tracks":editor.project.tracks.len(),"system_clipboard_written":false,
        "quarter":model::QUARTER}),
    )
}
