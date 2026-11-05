use std::{ffi::OsString,fs,path::PathBuf,io::Write};
use serde_json::json;
use rynthv::synthesis::voice::VoiceInventory;
pub(super) fn duration(args:&[OsString])->Result<(),Box<dyn std::error::Error>> {
    use rynthv::synthesis::{plan::Planner,score::{self,LyricFrontend},stages::duration::DurationStage};
    if args.len()!=5{return Err("Usage: --duration-project VOICE PROJECT NEW_REPORT IDENTITY PHONEME_DATA".into())}
    let path=PathBuf::from(&args[2]);if path.exists(){return Err("Duration evidence already exists".into())}
    let identity=args[3].to_str().ok_or("Invalid identity")?.parse::<u64>()?;
    let inventory=VoiceInventory::load(&PathBuf::from(&args[0]))?;
    let voice=inventory.prepare()?;
    if voice.family!=rynthv::synthesis::voice::VoiceFamily::Sv2 || !voice.speakers.is_empty(){return Err("Duration diagnostic requires single-speaker AI2".into())}
    let config=inventory.configuration.as_ref().ok_or("Missing voice configuration")?;
    let lexicon=LyricFrontend::for_voice(PathBuf::from(&args[4]),config)?;
    let project=rynthv::storage::load(&PathBuf::from(&args[1]))?;
    let mut planner=Planner::default();let plan=planner.update(&project,0,|_|Some(inventory.identity().to_owned()));
    let mut sections=Vec::new();
    for request in plan.requests.values() {
        let phrase=score::prepare_owner(&request.input,&lexicon,voice.model("duration")?,voice.family)?;
        #[cfg(feature="acoustic-diagnostic")]
        let trace=rynthv::synthesis::stages::duration::diagnostic::TraceGuard::begin()?;
        let started=std::time::Instant::now();
        let seeds=phrase.retake_seeds.iter().map(|s|s.unwrap_or_default().duration).collect::<Vec<_>>();
        let mut result=DurationStage::prepare(voice.model("duration")?,identity,None)?.predict_explicit_seeded(&phrase.records,&seeds)?;
        if phrase.voice_parameters.consonant_duration!=0.0 {
            result.durations=rynthv::synthesis::stages::duration::frontend::postprocess_controlled(voice.model("duration")?,&phrase.records,&result.prediction,phrase.voice_parameters.consonant_duration)?;
        }
        score::phonemes::apply(&phrase,&mut result)?;
        let phones=rynthv::synthesis::rendered::PhonemeTrack::new(&phrase,&result);
        let source=|s:Option<score::PhoneSource>|s.map(|s|json!({"note_id":s.note_id,"phone_index":s.phone_index}));
        let rows=phones.0.iter().map(|p|json!({"token":p.token,"start":p.start_seconds,"end":p.end_seconds,"source":source(p.source),"activity":p.boundary_strength,"position":p.boundary_position})).collect::<Vec<_>>();
        let sources=phrase.phone_sources.iter().map(|r|r.iter().map(|&s|source(s)).collect::<Vec<_>>()).collect::<Vec<_>>();
        let mut row=json!({"owner":request.owner,"note_count":request.input.notes.len(),"start_seconds":phrase.start_seconds,"records":phrase.records,"source_note_ids":phrase.source_note_ids,"phone_sources":sources,"phones":rows,"duration":result,"runtime_ms":started.elapsed().as_secs_f64()*1000.0});
        #[cfg(feature="acoustic-diagnostic")]
        {row["duration_trace"]=serde_json::to_value(trace.finish()?)?;}
        sections.push(row);
    }
    fs::OpenOptions::new().write(true).create_new(true).open(path)?.write_all(&serde_json::to_vec(&json!({"project":args[1].to_string_lossy(),"identity":identity,"sections":sections}))?)?;
    Ok(())
}
pub(super) fn lexical(args:&[OsString])->Result<(),Box<dyn std::error::Error>> {
    use rynthv::{model::Language,synthesis::{plan::Planner,score::LyricFrontend}};
    if args.len()!=4{return Err("Usage: --lexical PROJECT LANGUAGE NEW_REPORT PHONEME_DATA".into())}
    let output=PathBuf::from(&args[2]);if output.exists(){return Err("Lexical evidence already exists".into())}
    let language=args[1].to_str().and_then(Language::parse).ok_or("Unrecognized lexical language")?;
    let project=rynthv::storage::load(&PathBuf::from(&args[0]))?;
    let mut planner=Planner::default();
    let plan=planner.update(&project,0,|_|Some("linguistic-observation".into()));
    let lexicon=LyricFrontend::new(PathBuf::from(&args[3]),language.id(),&[language.id().to_owned()])?;
    let mut rows=Vec::new();
    for request in plan.requests.values() {
        let mut notes=request.input.notes.iter().collect::<Vec<_>>();notes.sort_by_key(|n|n.onset.0);
        let mut first=0;
        while first<notes.len() {
            let mut end=first+1;
            while end<notes.len()&&notes[end-1].onset.0.checked_add(notes[end-1].duration)==Some(notes[end].onset.0){end+=1}
            let lyrics=notes[first..end].iter().map(|n|n.lyrics.as_str()).collect::<Vec<_>>();
            let words=lexicon.lexical_group(language,&lyrics,false)?;
            rows.extend(notes[first..end].iter().zip(words).map(|(note,syllables)|json!({"note_id":note.id,"lyrics":note.lyrics,"syllables":syllables})));
            first=end;
        }
    }
    let report=json!({"project":args[0].to_string_lossy(),"language":language,"stage":"automatic words before model phone validation","rows":rows,"neural_execution":false});
    fs::OpenOptions::new().write(true).create_new(true).open(output)?.write_all(&serde_json::to_vec(&report)?)?;
    Ok(())
}
pub(super) fn run(args:&[OsString])->Result<(),Box<dyn std::error::Error>> {
    if args.len()!=4{return Err("Usage: --frontend VOICE PROJECT NEW_REPORT PHONEME_DATA".into())}
    let output=PathBuf::from(&args[2]);if output.exists(){return Err("Frontend evidence already exists".into())}
    let inventory=VoiceInventory::load(&PathBuf::from(&args[0]))?;let voice=inventory.prepare()?;
    let (score,phrase)=super::project::prepare(&PathBuf::from(&args[1]),&PathBuf::from(&args[3]),&inventory,&voice,None)?;
    let rows=phrase.notes.iter().map(|n|Ok(json!({"phones":n.phones,"kind":n.kind,"pitch":n.pitch,
        "duration_seconds":n.duration_seconds,"parts":n.parts,"part_index":n.part_index,
        "frames":rynthv::synthesis::stages::f0::frontend::frame_count(n)?})))
        .collect::<Result<Vec<_>,rynthv::synthesis::voice::VoiceError>>()?;
    let report=json!({"project":args[1].to_string_lossy(),"score":score,"prepared":rows,
        "source_note_ids":phrase.source_note_ids,"note_errors":phrase.note_errors,"neural_execution":false});
    fs::OpenOptions::new().write(true).create_new(true).open(output)?.write_all(&serde_json::to_vec(&report)?)?;
    Ok(())
}
