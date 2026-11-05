use std::{ffi::OsString,fs,path::PathBuf,time::Instant};
use serde_json::json;
use rynthv::synthesis::{audio,engine,plan::Planner,score::{self,LyricFrontend},voice::{VoiceInventory,VoiceFamily}};

pub(super) fn run(args:&[OsString])->Result<(),Box<dyn std::error::Error>> {
    if args.len()!=5 {return Err("Usage: --ensemble VOICE PROJECT NEW_REPORT OBSERVED_IDENTITY_U64 PHONEME_DATA".into())}
    let path=PathBuf::from(&args[2]);let wav=path.with_extension("wav");
    if path.exists()||wav.exists(){return Err("Ensemble diagnostic preserves existing evidence".into())}
    let identity=args[3].to_str().ok_or("Identity must be UTF-8")?.parse::<u64>()?;
    let inventory=VoiceInventory::load(&PathBuf::from(&args[0]))?;let voice=inventory.prepare()?;
    if !matches!(voice.family,VoiceFamily::Sv2|VoiceFamily::Sv2Compatible) {return Err("Ensemble diagnostic requires an R3 voice".into())}
    let config=inventory.configuration.as_ref().ok_or("Voice metadata absent")?;
    let lexicon=LyricFrontend::for_voice(PathBuf::from(&args[4]),config)?;
    let project=rynthv::storage::load(&PathBuf::from(&args[1]))?;
    let mut planner=Planner::default();let plan=planner.update(&project,0,|_|Some(inventory.identity().to_owned()));
    if plan.requests.len()!=1 {return Err("Choose exactly one sounding owner section".into())}
    let input=&plan.requests.values().next().unwrap().input;
    if input.notes.len()>128 {return Err("Ensemble diagnostic exceeds 128 source notes".into())}
    let phrase=score::prepare_owner(input,&lexicon,voice.model("duration")?,voice.family)?;
    if phrase.unison.count>4 {return Err("Ensemble audio diagnostic is bounded to four members".into())}
    let seating=phrase.unison.seating(&voice.speakers)?;
    #[cfg(feature="acoustic-diagnostic")]
    let prepared_members=rynthv::synthesis::unison::diagnostic_members(&voice,&phrase,&seating)?;
    let conditions=(0..seating.count).map(|index| {
        let member=phrase.unison.member_phrase(&phrase,index)?;
        let mut condition=json!({"local_index":index,"seat_index":seating.first+index,
            "speaker":seating.speakers[seating.first+index],"note_seeds":member.retake_seeds});
        #[cfg(feature="acoustic-diagnostic")]
        {condition["prepared_records"]=json!(prepared_members[index].notes.iter().map(|note|
            json!({"duration_seconds":note.duration_seconds,"pitch":note.pitch})).collect::<Vec<_>>());}
        Ok(condition)
    }).collect::<Result<Vec<_>,rynthv::synthesis::voice::VoiceError>>()?;
    #[cfg(feature="acoustic-diagnostic")]
    let trace=rynthv::synthesis::stages::duration::diagnostic::TraceGuard::begin_members(seating.count)?;
    let started=Instant::now();
    let result=engine::synthesize_owner(&voice,Some(identity),input,&lexicon,None,|_|Ok(()))?;
    let runtime_ms=started.elapsed().as_secs_f64()*1000.0;
    audio::save_wav(&wav,&result.audio)?;
    let samples=result.audio.samples();
    let rms=(samples.iter().map(|&v|f64::from(v).powi(2)).sum::<f64>()/samples.len() as f64).sqrt();
    let features=if voice.family==VoiceFamily::Sv2 {Some(result.state.prepared.sv2_features()?)} else {None};
    let pitch=&result.visuals.pitch;
    let mut report=json!({"scope":"Production owner ensemble, original weights; native audio equality is not assumed", "core_reference":"2.3.0tp1",
        "member_vocal_modes":voice.member_vocal_modes.as_ref(),
        "phonemes":result.visuals.phonemes.0.iter().map(|p|json!({"token":p.token,"start_seconds":p.start_seconds,
            "end_seconds":p.end_seconds,"source_note_id":p.source.map(|s|s.note_id)})).collect::<Vec<_>>(),
        "project":args[1].to_string_lossy(),"identity":identity,"members":conditions,
        "seating":{"part":seating.part,"rows":seating.rows,"first":seating.first,"speakers":seating.speakers},
        "runtime_ms":runtime_ms,"stage_timings":result.timings,
        "wav":{"path":wav,"start_seconds":result.audio.start_seconds(),"frames":result.audio.frames(),"rms":rms},
        "pitch":{"start_seconds":pitch.start_seconds,"frame_seconds":pitch.frame_seconds,"values":pitch.values.as_ref(),
            "segments":pitch.segments.iter().map(|v|[v.start,v.end]).collect::<Vec<_>>(),
            "range":pitch.unison_range.as_ref().map(|v|[v[0].as_ref(),v[1].as_ref()])},
        "lead":{"duration":result.state.duration,"f0_encoded_notes":features.map(|(f0,_)|f0.encoding.notes.as_ref()),
            "timbre_encoded_notes":features.map(|(_,timbre)|timbre.notes.as_ref())}});
    #[cfg(feature="acoustic-diagnostic")]
    {report["duration_trace"]=serde_json::to_value(trace.finish()?)?;}
    use std::io::Write;
    fs::OpenOptions::new().write(true).create_new(true).open(path)?.write_all(&serde_json::to_vec(&report)?)?;
    Ok(())
}
