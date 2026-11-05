//! Read-only pronunciation preparation/profiling on existing scores.
use std::{ffi::OsString,path::PathBuf,time::Instant};
use rynthv::synthesis::{plan::Planner,score::{ExplicitScore,LyricFrontend},voice::VoiceInventory};
use serde_json::{Value,json};

pub(super) fn run(args:&[OsString])->Result<Value,Box<dyn std::error::Error>> {
    if args.len()!=6 {return Err("Usage: --pronunciation-cache VOICE CLF_DATA PROJECT NOTE_ID NEW_LYRIC".into())}
    let inventory=VoiceInventory::load(&PathBuf::from(&args[1]))?;
    let voice=inventory.prepare()?;let tree=voice.model("duration")?;
    let config=inventory.configuration.as_ref().ok_or("Missing voice configuration")?;
    let frontend=LyricFrontend::for_voice(PathBuf::from(&args[2]),config)?;
    let reference=frontend.without_incremental_cache()?;
    let project=rynthv::storage::load(&PathBuf::from(&args[3]))?;
    let mut planner=Planner::default();let plan=planner.update(&project,0,|_|Some(inventory.identity().to_owned()));
    let original=plan.requests.values().map(|request|(*request.input).clone()).collect::<Vec<_>>();
    let note_id=args[4].to_string_lossy().parse::<u64>()?;
    let lyric=args[5].to_string_lossy().into_owned();
    let mut edited=original.clone();let mut count=0;
    for input in &mut edited {for note in &mut input.notes {if note.id==note_id {note.lyrics=lyric.clone();count+=1;}}}
    if count==0 {return Err("Note ID is absent from the synthesis plan".into())}
    let mut pitched=edited.clone();for input in &mut pitched {for note in &mut input.notes {if note.id==note_id {note.pitch+=1;}}}
    let mut phases=Vec::new();
    for (name,inputs) in [("cold",&original),("unchanged",&original),("lyric-edit",&edited),("pitch-only",&pitched),("restore",&original)] {
        let before=frontend.pronunciation_cache_metrics()?;let start=Instant::now();
        let mut notes=0;let mut errors=0;let mut cached_seconds=0.0;let mut reference_seconds=0.0;
        for input in inputs {
            let clock=Instant::now();
            let cached=ExplicitScore::from_owner(input,&frontend,tree,voice.family)?;
            cached_seconds+=clock.elapsed().as_secs_f64();let clock=Instant::now();
            let full=ExplicitScore::from_owner(input,&reference,tree,voice.family)?;
            reference_seconds+=clock.elapsed().as_secs_f64();
            let cached=serde_json::to_value(cached)?;let full=serde_json::to_value(full)?;
            if cached!=full {return Err(format!("Incremental pronunciation differs from full context: {name}").into())}
            notes+=input.notes.len();
            errors+=cached["notes"].as_array().map_or(0,|rows|rows.iter().filter(|row|row.get("note_error").is_some_and(|v|!v.is_null())).count());
        }
        let after=frontend.pronunciation_cache_metrics()?;
        let delta=|key:&str|after[key].as_u64().unwrap()-before[key].as_u64().unwrap();
        let row=json!({"phase":name,"notes":notes,"seconds":start.elapsed().as_secs_f64(),"full_context_equal":true,
            "cached_seconds":cached_seconds,"reference_seconds":reference_seconds,
            "notes_derived":delta("notes_derived"),"notes_reused":delta("notes_reused"),
            "lexical_derived":delta("lexical_derived"),"lexical_reused":delta("lexical_reused"),"note_errors":errors});
        if matches!(name,"unchanged"|"pitch-only") && delta("notes_derived")!=0 {return Err(format!("Unexpected phoneme rederivation for {name}").into())}
        phases.push(row);
    }
    Ok(json!({"project":args[3].to_string_lossy(),"note_id":note_id,"changed_references":count,
        "phases":phases,"neural_execution":false,"project_files_unchanged":true}))
}
