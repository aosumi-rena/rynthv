use std::{ffi::OsString,path::PathBuf,sync::Arc,time::Instant,fs};
use serde_json::{json,Value};
use rynthv::synthesis::{engine,plan::Planner,render_cache::CompletedOwner,
    rendered::RenderedClip,score::{self,LyricFrontend},voice::{VoiceInventory,VoiceFamily,session_identity},audio};

pub(super) fn run(args:&[OsString])->Result<(),Box<dyn std::error::Error>> {
    if args.len()!=7 {return Err("Usage: --compare-boundary VOICE PROJECT NEW_REPORT NOTE_INDEX PHONE_INDEX OFFSET_SECONDS PHONEME_DATA".into());}
    let path=PathBuf::from(&args[2]);
    let outputs=[path.clone(),path.with_extension("baseline.wav"),path.with_extension("whole.wav"),path.with_extension("local.wav")];
    if outputs.iter().any(|p|p.exists()) {return Err("Boundary diagnostic refuses to replace existing evidence".into());}
    let index=args[3].to_str().ok_or("Note index must be an integer")?.parse::<usize>()?;
    let phone=args[4].to_str().ok_or("Phone index must be an integer")?.parse::<usize>()?;
    let offset=args[5].to_str().ok_or("Offset must be a number")?.parse::<f64>()?;
    if phone>63 || !offset.is_finite() || offset.abs()>1.0 {return Err("Boundary diagnostic exceeds its bounded edit scope".into());}
    let inventory=VoiceInventory::load(&PathBuf::from(&args[0]))?;
    let voice=inventory.prepare()?;
    if voice.family!=VoiceFamily::Sv2 || !voice.speakers.is_empty() {return Err("Boundary diagnostic requires one AI2 speaker".into());}
    let config=inventory.configuration.as_ref().ok_or("Voice metadata missing")?;
    let resources=PathBuf::from(&args[6]);
    let lexicon=LyricFrontend::for_voice(resources,config)?;
    let project=rynthv::storage::load(&PathBuf::from(&args[1]))?;
    let mut planner=Planner::default();
    let plan=planner.update(&project,0,|_|Some(inventory.identity().to_owned()));
    if plan.requests.len()!=1 {return Err("Choose a project with exactly one sounding section".into());}
    let input=Arc::clone(&plan.requests.values().next().unwrap().input);
    if input.notes.len()>128 {return Err("Boundary comparison exceeds128 source notes".into());}
    let identity=session_identity::saved_identity(&config.name,voice.family.model_version())?;
    let start=Instant::now();
    let baseline=engine::synthesize_owner(&voice,Some(identity),&input,&lexicon,None,|_|Ok(()))?;
    let baseline_ms=start.elapsed().as_secs_f64()*1000.;
    audio::save_wav(&outputs[1],&baseline.audio)?;
    let base=CompletedOwner{input:Arc::clone(&input),clip:Arc::new(RenderedClip{
        audio:Arc::new(baseline.audio),visuals:Arc::new(baseline.visuals),state:Some(Arc::new(baseline.state))})};
    let baseline_state=base.clip.state.as_ref().ok_or("Neural diagnostic cache state missing")?;
    let mut changed=(*input).clone();
    let note=changed.notes.get_mut(index).ok_or("Note index outside the source section")?;
    let mut rows=note.attributes.get("phonemes").and_then(Value::as_array).cloned().unwrap_or_default();
    while rows.len()<=phone {rows.push(json!({"strength":0.0}));}
    rows[phone]["leftOffset"]=json!(offset);
    note.attributes.insert("phonemes".into(),json!(rows));
    let phrase=score::prepare_owner(&changed,&lexicon,voice.model("duration")?,voice.family)?;
    let edit=engine::incremental::IncrementalEdit::between(&input,&changed,&baseline_state.prepared.phrase,&phrase)?
        .ok_or("The requested boundary edit was not admitted for local rendering")?;
    let start=Instant::now();
    let whole=engine::synthesize_owner(&voice,Some(identity),&changed,&lexicon,None,|_|Ok(()))?;
    let whole_ms=start.elapsed().as_secs_f64()*1000.;
    audio::save_wav(&outputs[2],&whole.audio)?;
    let start=Instant::now();
    let local=engine::synthesize_owner(&voice,Some(identity),&changed,&lexicon,Some(&base),|_|Ok(()))?;
    let local_ms=start.elapsed().as_secs_f64()*1000.;
    audio::save_wav(&outputs[3],&local.audio)?;
    let a=&whole.state.duration;let b=&local.state.duration;
    let counts_equal=a.durations.iter().map(|p|p.frames).eq(b.durations.iter().map(|p|p.frames));
    let mut outside=0;let mut changed_outside=0;
    let mut outside_seams=0;let mut changed_outside_seams=0;
    let mut changed_begin=None;let mut changed_end=0;
    for (i,(&old,&new)) in base.clip.audio.samples().iter().zip(local.audio.samples()).enumerate() {
        let seconds=local.audio.start_seconds()+i as f64/48000.;
        if seconds<edit.first_block as f64*0.5 || seconds>=edit.stop_block as f64*0.5 {
            outside+=1;changed_outside+=usize::from(old.to_bits()!=new.to_bits());
        }
        if seconds<edit.first_block.saturating_sub(1) as f64*0.5 || seconds>=(edit.stop_block+4) as f64*0.5 {
            outside_seams+=1;changed_outside_seams+=usize::from(old.to_bits()!=new.to_bits());
        }
        if old.to_bits()!=new.to_bits() {changed_begin.get_or_insert(i);changed_end=i+1;}
    }
    let (whole_pitch,whole_timbre)=whole.state.prepared.sv2_features()?;
    let (local_pitch,local_timbre)=local.state.prepared.sv2_features()?;
    let report=json!({"project":args[1].to_string_lossy(),"source_note_index":index,"phone_index":phone,"offset_seconds":offset,
        "runtime_ms":{"baseline":baseline_ms,"whole":whole_ms,"local":local_ms},
        "admitted":true,"work_records":[edit.work_records.start,edit.work_records.end],"write_frames":[edit.write_frames.start,edit.write_frames.end],
        "pcm_blocks":[edit.first_block,edit.stop_block],"total_frames":whole.state.pitch.pitch.len(),
        "duration_frames_exact":counts_equal,"natural_frames_exact":a.natural_frames==b.natural_frames,
        "duration_scalars":error(&a.prediction.scalars,&b.prediction.scalars),
        "pitch_midi":error(&whole.state.pitch.pitch,&local.state.pitch.pitch),
        "pcm":error(whole.audio.samples(),local.audio.samples()),
        "outside_invalidated_blocks_before_phase_joins":{"samples":outside,"changed_samples":changed_outside},
        "phase_join_context_seconds":[edit.first_block.saturating_sub(1) as f64*0.5,(edit.stop_block+4) as f64*0.5],
        "outside_phase_join_context":{"samples":outside_seams,"changed_samples":changed_outside_seams},
        "changed_sample_range":changed_begin.map(|begin|[begin,changed_end]),
        "encoded_features":{"pitch_notes":error(&whole_pitch.encoding.notes,&local_pitch.encoding.notes),
            "pitch_phones":error(&whole_pitch.encoding.phones,&local_pitch.encoding.phones),
            "timbre_notes":error(&whole_timbre.notes,&local_timbre.notes),"timbre_phones":error(&whole_timbre.phones,&local_timbre.phones)},
        "pitch_baseline_midi":baseline_state.pitch.pitch,"pitch_whole_midi":whole.state.pitch.pitch,"pitch_local_midi":local.state.pitch.pitch,
        "audio_origin_seconds":local.audio.start_seconds(),"scope":"Real production completed-owner reuse; no gain/delay fitting, no parameter replacement"});
    use std::io::Write;
    let mut out=fs::OpenOptions::new().write(true).create_new(true).open(&outputs[0])?;
    out.write_all(&serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");Ok(())
}
fn error(a:&[f32],b:&[f32])->Value {
    if a.len()!=b.len(){return json!({"comparable":false,"whole_length":a.len(),"local_length":b.len()});}
    let mut maximum=0.0_f64;let mut squares=0.0_f64;
    for (&a,&b) in a.iter().zip(b){let d=f64::from(a)-f64::from(b);maximum=maximum.max(d.abs());squares+=d*d;}
    json!({"count":a.len(),"max_abs":maximum,"rmse":(squares/a.len().max(1) as f64).sqrt()})
}
