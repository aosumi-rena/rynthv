//! Headless measurement of the production progressive path, including PCM delivery.
use std::{ffi::OsString,fs,path::PathBuf,time::Instant};
use rynthv::synthesis::{audio,engine,plan::Planner,score::{self,LyricFrontend},
    settings::SynthesisSettings,voice::VoiceInventory};
use serde_json::json;

pub(super) fn run(args:&[OsString])->Result<(),Box<dyn std::error::Error>> {
    if !(4..=5).contains(&args.len()) {return Err("Usage: --stream-project VOICE PROJECT NEW_REPORT IDENTITY [--complete|--features-off]".into());}
    let output=PathBuf::from(&args[2]);let wav=output.with_extension("wav");
    if output.exists() || wav.exists() {return Err("Streaming measurement refuses to replace evidence".into());}
    let identity=args[3].to_str().ok_or("Invalid duration operand")?;
    let identity=if let Some(hex)=identity.strip_prefix("0x") {u64::from_str_radix(hex,16)?}else{identity.parse::<u64>()?};
    let complete=args.get(4).is_some_and(|v|v=="--complete");
    let features_off=args.get(4).is_some_and(|v|v=="--features-off");
    if args.len()==5 && !complete && !features_off {return Err("Unknown streaming measurement mode".into());}
    let inventory=VoiceInventory::load(&PathBuf::from(&args[0]))?;let voice=inventory.prepare()?;
    let config=inventory.configuration.as_ref().ok_or("Missing voice metadata")?;
    let settings=SynthesisSettings::load().map_err(std::io::Error::other)?;
    let version=rynthv::synthesis::settings::PhonemeVersion::configuration(config,inventory.database_id.starts_with("flat:"))?;
    let lexicon=LyricFrontend::for_voice(settings.require_phoneme_directory(version)?,config)?;
    let project=rynthv::storage::load(&PathBuf::from(&args[1])).map_err(std::io::Error::other)?;
    let mut planner=Planner::default();let plan=planner.update(&project,0,|_|Some(inventory.identity().to_owned()));
    if plan.requests.len()!=1 {return Err("Choose exactly one sounding section for a bounded streaming measurement".into());}
    let mut input=(*plan.requests.values().next().unwrap().input).clone();
    input.experiments.settings=if features_off {Default::default()}else{settings.experiments};
    #[cfg(feature="synth-profile")]
    rynthv::synthesis::profiling::start();
    let start=Instant::now();let mut chunks=0;let mut samples=0;let mut first_pcm_ms=None;
    let mut first_pcm_at=None;let mut last_pcm_end=None;
    let prepared_ms;
    let result=if complete {
        let phrase=score::prepare_owner(&input,&lexicon,voice.model("duration")?,voice.family)?;
        let result=engine::synthesize(&voice,Some(identity),&phrase,|_|Ok(()))?;
        prepared_ms=start.elapsed().as_secs_f64()*1000.;
        result
    }else{
        let network=engine::progressive::Network::prepare(&voice)?;
        let mut prepared=engine::progressive::prepare(&voice,Some(identity),&input,&lexicon,None,|_|Ok(()))?;
        prepared_ms=start.elapsed().as_secs_f64()*1000.;
        let mut record=|audio:&audio::RenderedAudio| {
            first_pcm_ms.get_or_insert_with(||start.elapsed().as_secs_f64()*1000.);
            first_pcm_at.get_or_insert(audio.start_seconds());
            last_pcm_end=Some(audio.start_seconds()+audio.duration_seconds());
            chunks+=1;samples+=audio.frames();
        };
        for audio in prepared.ready_audio()? {record(&audio);}
        while let Some(mut cursor)=prepared.start(&network,&voice,0.0)? {
            while !cursor.is_finished() {
                for audio in prepared.advance(&mut cursor,128,|_|Ok(()))? {record(&audio);}
            }
        }
        prepared.finish()?
    };
    let elapsed=start.elapsed().as_secs_f64()*1000.;
    #[cfg(feature="synth-profile")]
    let profile=rynthv::synthesis::profiling::finish();
    audio::save_wav(&wav,&result.audio)?;
    let mut report=json!({"scope":"Actual original production engine, explicit invocation duration operand, no GUI/model/session writes",
        "complete":complete,"experimental_capabilities":input.experiments.settings,"prepare_ms":prepared_ms,
        "first_pcm_ms":first_pcm_ms,"first_pcm_at_seconds":first_pcm_at,"last_pcm_end_seconds":last_pcm_end,
        "streamed_chunks":chunks,"streamed_frames":samples,"render_ms":elapsed,
        "audio_frames":result.audio.frames(),"stage_timings":result.timings,
        "note_errors":result.state.prepared.phrase.note_errors,"duration_frames":result.state.duration.durations,
        "pitch_midi":result.state.pitch.pitch});
    #[cfg(feature="synth-profile")]
    {report["profile"]=serde_json::to_value(profile)?;}
    fs::write(output,serde_json::to_vec(&report)?)?;
    Ok(())
}
