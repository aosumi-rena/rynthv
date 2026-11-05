//! Original seating metadata through the production parser and planner.
use std::{ffi::OsString,fs,path::Path};
use rynthv::synthesis::{unison::UnisonSettings,voice::VoiceInventory};
use serde_json::json;

pub(super) fn run(args:&[OsString])->Result<(),Box<dyn std::error::Error>> {
    if args.len()!=3 {return Err("Usage: --seating VOICE PROJECT NEW_REPORT".into())}
    let inventory=VoiceInventory::load(Path::new(&args[0]))?;
    let config=inventory.configuration.as_ref().ok_or("Voice metadata absent")?;
    let project=rynthv::storage::load(Path::new(&args[1]))?;
    if project.tracks.len()!=1 {return Err("Seating diagnostic requires one explicit track".into())}
    let track=&project.tracks[0];
    if track.is_instrumental()||track.groups.len()>1 {return Err("Seating diagnostic requires one explicit voice reference".into())}
    let reference=track.groups.first().unwrap_or(&track.main_ref);
    let settings=UnisonSettings::from_voice(reference.extra.get("voice"))?;
    let plan=settings.seating(&config.speakers)?;
    let output=json!({"part":plan.part,"rows":plan.rows,"speakers":plan.speakers,
        "first":plan.first,"count":plan.count,"declared_speakers":config.speakers,
        "scope":"Original seating selection and source-index numbering; no audio accuracy claim"});
    use std::io::Write;
    let mut file=fs::OpenOptions::new().write(true).create_new(true).open(Path::new(&args[2]))?;
    file.write_all(&serde_json::to_vec_pretty(&output)?)?;
    Ok(())
}
