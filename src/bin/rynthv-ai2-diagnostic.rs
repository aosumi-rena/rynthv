//! Bounded AI2 activation diagnostic
#[path="ai2_diagnostic/project.rs"]
mod project;
#[path="ai2_diagnostic/incremental.rs"]
mod incremental;
#[path="ai2_diagnostic/seating.rs"]
mod seating;
#[path="ai2_diagnostic/ensemble.rs"]
mod ensemble;
#[path="ai2_diagnostic/frontend.rs"]
mod frontend;
#[path="ai2_diagnostic/phonetic.rs"]
mod phonetic;
#[path="ai2_diagnostic/phonetic_context.rs"]
mod phonetic_context;
#[path="ai2_diagnostic/streaming.rs"]
mod streaming;
#[path="ai2_diagnostic/routing.rs"]
mod routing;
use rynthv::synthesis::{engine, score::ExplicitScore, voice::{VoiceInventory,VoiceFamily}};
use serde_json::json;
use std::{fs,path::PathBuf,process::ExitCode,time::Instant};
fn main()->ExitCode { match run(){Ok(())=>ExitCode::SUCCESS,Err(e)=>{eprintln!("{e}");ExitCode::FAILURE}} }
fn run()->Result<(),Box<dyn std::error::Error>> {
 let mut args:Vec<_>=std::env::args_os().skip(1).collect();
 if args.first().is_some_and(|arg|arg=="--phone-routing") {return routing::run(&args[1..]);}
 if args.first().is_some_and(|arg|arg=="--stream-project") {return streaming::run(&args[1..]);}
 #[cfg(feature="acoustic-diagnostic")]
 if args.first().is_some_and(|arg|arg=="--schedule") {
  if args.len()!=3 && !(args.len()==4 && args[3]=="--muted"){return Err("Usage: --schedule PROJECT DEMAND_SECONDS [--muted]".into())}
  let mut project=rynthv::storage::load(&PathBuf::from(&args[1]))?;
  if args.len()==4 {for track in &mut project.tracks {track.mixer.insert("mute".into(),true.into());}}
  let seconds=args[2].to_str().ok_or("Invalid demand")?.parse::<f64>()?;
  println!("{}",rynthv::synthesis::runtime::scheduling_diagnostic(&project,seconds));return Ok(())
 }
 if args.first().is_some_and(|arg|arg=="--duration-project") {return frontend::duration(&args[1..]);}
 if args.first().is_some_and(|arg|arg=="--phonetic") {return phonetic::run(&args[1..]);}
 if args.first().is_some_and(|arg|arg=="--lexical") {return frontend::lexical(&args[1..]);}
 if args.first().is_some_and(|arg|arg=="--frontend") {return frontend::run(&args[1..]);}
 if args.first().is_some_and(|arg|arg=="--ensemble") {return ensemble::run(&args[1..]);}
 if args.first().is_some_and(|arg|arg=="--seating") {return seating::run(&args[1..]);}
 let speaker=if args.first().is_some_and(|arg|arg=="--speaker") {
  let id=args.get(1).and_then(|s|s.to_str()).ok_or("--speaker requires an installed member index")?.parse::<usize>()?;
  args.drain(..2);Some(id)
 }else{None};
 if args.first().is_some_and(|arg|arg=="--compare-boundary") {return incremental::run(&args[1..]);}
 let section=if args.len()==9 && args[7]=="--section" {
  let index=args[8].to_str().ok_or("Section must be an integer")?.parse::<usize>()?;
  args.truncate(7);Some(index)
 }else{None};
 if !(3..=5).contains(&args.len()) && args.len()!=7 {return Err("Usage: rynthv-ai2-diagnostic VOICE SCORE NEW_REPORT [DURATION_IDENTITY_U64] [--trace16|--trace-duration] [--project CLF_DATA]; project mode also writes adjacent WAV".into());}
 let project_mode=args.len()==7;
 if project_mode && args[5]!="--project" {return Err("Unknown project diagnostic option".into());}
 let trace16=args.get(4).is_some_and(|flag|flag=="--trace16");
 let trace_acoustic=args.get(4).is_some_and(|flag|flag=="--trace-acoustic");
 let trace_duration=args.get(4).is_some();
 if trace_duration && !trace16 && !trace_acoustic && args[4]!="--trace-duration" {return Err("Unknown diagnostic option".into());}
 #[cfg(not(feature = "acoustic-diagnostic"))]
 if trace_duration {return Err("Intermediate traces require acoustic-diagnostic build feature".into());}
 let requested_identity=match args.get(3) {
  None=>None,
  Some(value)=>{let text=value.to_str().ok_or("Identity must be UTF-8")?;
   Some(if let Some(hex)=text.strip_prefix("0x"){u64::from_str_radix(hex,16)?}else{text.parse::<u64>()?})}
 };
 let path=PathBuf::from(&args[2]); if path.exists(){return Err("Report already exists".into());}
 let inventory=VoiceInventory::load(&PathBuf::from(&args[0]))?; let mut voice=inventory.prepare()?;
 if let Some(index)=speaker {
  let member=voice.speakers.get(index).ok_or("Speaker index is outside the bank's declared members")?.clone();
  voice=voice.member(&member)?.into();
 }
 if !matches!(voice.family,VoiceFamily::Sv2|VoiceFamily::Sv2Compatible) || !voice.speakers.is_empty(){return Err("Diagnostic requires one R3 speaker".into());}
 if trace16 && voice.family!=VoiceFamily::Sv2 {return Err("16-child trace requires an AI2 voice".into());}
 let identity=match requested_identity {
  Some(value)=>value,
  None=>rynthv::synthesis::voice::session_identity::saved_identity(
    &inventory.configuration.as_ref().ok_or("Voice metadata is unavailable")?.name,
    voice.family.model_version())?,
 };
 let audio_path=path.with_extension("wav");
 if project_mode && audio_path.exists() {return Err("Project diagnostic WAV already exists".into());}
 let (score,phrase)=if project_mode {
  project::prepare(&PathBuf::from(&args[1]),&PathBuf::from(&args[6]),&inventory,&voice,section)?
 }else{
  let bytes=fs::read(&args[1])?;if bytes.len()>1024*1024{return Err("Diagnostic score exceeds 1MiB".into());}
  let score:ExplicitScore=serde_json::from_slice(&bytes)?;
  let default_language=&inventory.configuration.as_ref().ok_or("Voice metadata is unavailable")?.default_language;
  let phrase=score.prepare(voice.model("duration")?,voice.family,default_language)?;
  (score,phrase)
 };
 if phrase.notes.len()>1024 {return Err("Diagnostic exceeds 1024 records".into());}
 let identity_bits:Vec<f32>=(0..64).map(|index|((identity>>(63-index))&1)as f32).collect();
 let duration_tree=voice.model("duration")?;
 let identity_node=*duration_tree.node(duration_tree.root_index())?.children.get(6).ok_or("Duration child6 absent")?;
 // Separately labelled single projection diagnostic; synthesize_acoustics owns its own preparation.
 let identity_projection=duration_tree.execute_node(identity_node,&identity_bits)?;
 #[cfg(feature = "acoustic-diagnostic")]
 let trace=if trace16 {Some(rynthv::synthesis::stages::timbre::diagnostic::TraceGuard::begin()?)}else{None};
 #[cfg(feature = "acoustic-diagnostic")]
 let duration_trace=if trace_duration {Some(rynthv::synthesis::stages::duration::diagnostic::TraceGuard::begin()?)}else{None};
 #[cfg(feature = "acoustic-diagnostic")]
 let acoustic_trace=if trace_acoustic {Some(rynthv::synthesis::stages::timbre::probe::TraceGuard::begin()?)}else{None};
 #[cfg(feature="acoustic-diagnostic")]
 let feature_trace=if trace_acoustic {Some(rynthv::synthesis::stages::f0::features::diagnostic::TraceGuard::begin_with_frames(&path)?)}else{None};
 #[cfg(feature="acoustic-diagnostic")]
 let sampler_trace=if trace_acoustic {Some(rynthv::synthesis::stages::f0::diagnostic::TraceGuard::begin()?)}else{None};
 let started=Instant::now(); let output=engine::synthesize_acoustics(&voice,Some(identity),&phrase,|_|Ok(()))?;
 #[cfg(feature="acoustic-diagnostic")]
 let vocoder_trace=if project_mode {Some(rynthv::synthesis::stages::vocoder::effects::diagnostic::TraceGuard::begin()?)}else{None};
 let wav_origin=if project_mode {Some(project::save_audio(&audio_path,&voice,&phrase,&output)?)}else{None};
 let source_noise_seeds=if trace_acoustic && voice.family==VoiceFamily::Sv2 {
  Some(rynthv::synthesis::stages::vocoder::noise_seeds::for_phrase(&output.prepared.phrase,output.timbre.first_frame)?)
 }else{None};
 let phones:Vec<_>=output.prepared.phones.iter().map(|p|json!({"note":p.note_index,"language":p.language,"token":p.token,"frames":p.frames})).collect();
 let notes:Vec<_>=output.prepared.phrase.notes.iter().map(|n|json!({"kind":n.kind,"flag":n.flag,"pitch":n.pitch,"duration_seconds":n.duration_seconds,"phones":n.phones})).collect();
  let mut frontends=serde_json::Map::new();
 for role in ["f0","timbre"].into_iter().filter(|_|voice.family==VoiceFamily::Sv2) {
  let tree=voice.model(role)?;let frontend=rynthv::synthesis::stages::f0::frontend::NoteFrontend::prepare(tree,tree.node(tree.root_index())?.children[0])?;
  let mut frontend_notes=output.prepared.phrase.notes.clone();
  if role=="f0" {
   let (features,_)=output.prepared.sv2_features()?;
   for (note,&transpose) in frontend_notes.iter_mut().zip(features.transpose.iter()) {note.pitch_offset+=transpose;}
  }
  let raw=frontend.encode(&frontend_notes)?;let mut normalized=vec![0.;raw.values.len()];
  tree.range_normalizer(frontend.normalizer_node)?.normalize(&raw.values,&mut normalized,true)?;
  frontends.insert(role.into(),json!({"width":raw.width,"raw":raw.values,"normalized":normalized,"frames":raw.frame_counts}));
 }
 let report=json!({"model_generation":voice.family.model_version(),"duration_identity":identity,"duration_identity_hex":format!("0x{identity:016x}"),"duration_identity_bits_msb_first":identity_bits,"identity_projection_diagnostic":identity_projection,"additional_identity_projection_calls":1,"note_frontends":frontends,"diagnostic":"R3 original-weight activations with explicit reported duration identity; no arithmetic overrides","runtime_ms":started.elapsed().as_secs_f64()*1000.,"phrase_start_seconds":output.prepared.phrase.start_seconds,"notes":notes,"phones":phones,"duration":output.duration,"pitch_midi":output.pitch.pitch,"predicted_rap_intonation":output.pitch.predicted_controls.slope,"predicted_rap_tone":output.pitch.predicted_controls.offset,"source_note_ids":output.prepared.phrase.source_note_ids,"acoustic_first_frame":output.timbre.first_frame,"acoustic_channels":output.timbre.acoustics.channels,"acoustic_values":output.timbre.acoustics.values,"control_values":output.timbre.controls.map(|s|s.values)});
 #[cfg(feature = "acoustic-diagnostic")]
 let report=if let Some(trace)=trace {let mut report=report;report["acoustic_trace16"]=serde_json::to_value(trace.finish()?)?;report}else{report};
 let mut report=report;
 if let Some(seeds)=source_noise_seeds {report["source_noise_seeds"]=json!(seeds);}
 report["duration_identity_source"]=json!(if requested_identity.is_some(){"explicit diagnostic"}else{"saved context/session"});
 #[cfg(feature="acoustic-diagnostic")]
 if let Some(trace)=vocoder_trace {report["vocoder_probe"]=serde_json::to_value(trace.finish()?)?;}
 #[cfg(feature = "acoustic-diagnostic")]
 if let Some(trace)=acoustic_trace {report["acoustic_probe"]=serde_json::to_value(trace.finish()?)?;}
 #[cfg(feature="acoustic-diagnostic")]
 if let Some(trace)=feature_trace {let trace=trace.finish()?;report["shared_feature_trace"]=serde_json::to_value(trace.boundaries)?;report["frame_feature_trace"]=serde_json::to_value(trace.frames)?;}
 #[cfg(feature="acoustic-diagnostic")]
 if let Some(trace)=sampler_trace {report["sampler_trace"]=serde_json::to_value(trace.finish()?)?;}
 report["prepared_from_project"]=json!(project_mode);
 report["selected_speaker"]=json!(speaker);
 report["project_section_index"]=json!(section);
 report["diagnostic_wav_start_seconds"]=json!(wav_origin);
 report["prepared_score"]=serde_json::to_value(score)?;
 report["diagnostic_wav"]=if project_mode {json!(audio_path)}else{serde_json::Value::Null};
 #[cfg(feature = "acoustic-diagnostic")]
 let report=if let Some(trace)=duration_trace {let mut report=report;report["duration_trace"]=serde_json::to_value(trace.finish()?)?;report}else{report};
 use std::io::Write; let mut file=fs::OpenOptions::new().write(true).create_new(true).open(path)?; file.write_all(&serde_json::to_vec(&report)?)?; Ok(())
}

