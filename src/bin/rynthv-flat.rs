#[path = "rynthv_flat/host_pitch.rs"]
mod host_pitch;
use rynthv::{storage, synthesis::{audio::save_wav, engine, score::ExplicitScore,
    voice::VoiceInventory}};
use serde_json::json;
use std::{fs::File, io::Read, path::PathBuf, process::ExitCode, time::Instant};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let prepare = args.first().is_some_and(|arg| arg == "prepare");
    let render = args.first().is_some_and(|arg| arg == "render");
    let host_pitch_mode = args.first().is_some_and(|arg| arg == "render-host-pitch");
    if !(prepare && args.len() == 3 || (render || host_pitch_mode) && args.len() == 5) {
        return Err("Usage: rynthv-flat prepare MANIFEST REPORT | render|render-host-pitch MANIFEST SCORE WAV REPORT".into());
    }
    let manifest = PathBuf::from(&args[1]);
    let report_path = PathBuf::from(args.last().unwrap());
    if report_path.exists() { return Err("Report output already exists".into()); }
    let started = Instant::now();
    let inventory=VoiceInventory::load(&manifest).map_err(|error|error.to_string())?;
    let voice=inventory.prepare().map_err(|error|error.to_string())?;
    let preparation_ms = started.elapsed().as_secs_f64() * 1000.0;
    if prepare {
        return write_report(&report_path, &json!({"schema":1,"preparation_ms":preparation_ms,
            "preparation":voice.summary(),"audio_generated":false}));
    }
    let input_path = PathBuf::from(&args[2]);
    let wav_path = PathBuf::from(&args[3]);
    if wav_path.exists() || wav_path == report_path { return Err("WAV output exists or aliases report".into()); }
    let file = File::open(input_path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > 16 * 1024 * 1024 {
        return Err("Explicit score JSON exceeds 16 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 { return Err("Explicit score grew past 16 MiB".into()); }
    let score: ExplicitScore = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;

    let phrase = score.prepare(voice.model("duration").map_err(|error| error.to_string())?, voice.family, "")
        .map_err(|error| error.to_string())?;
    let started = Instant::now();

    let checkpoint = |stage: engine::RenderStage| {
        eprintln!("stage: {}", stage.label()); Ok(())
    };
    let output = if host_pitch_mode {
        engine::synthesize_host_pitch(&voice, &phrase, checkpoint, host_pitch::exchange)
    } else {
        engine::synthesize(&voice, None, &phrase, checkpoint)
    }.map_err(|error| error.to_string())?;
    let render_ms = started.elapsed().as_secs_f64() * 1000.0;
    save_wav(&wav_path, &output.audio).map_err(|error| error.to_string())?;
    let pitch = &output.visuals.pitch;
    let prepared = &output.state.prepared;
    let notes: Vec<_> = prepared.phrase.notes.iter().enumerate().map(|(index,note)| json!({
        "record_index":index,"source_note_id":prepared.phrase.source_note_ids[index],
        "duration_seconds":note.duration_seconds,"phones":note.phones,"kind":note.kind,
        "parts":note.parts,"part_index":note.part_index
    })).collect();
    let phones: Vec<_> = prepared.phones.iter().map(|phone| json!({
        "note_index":phone.note_index,"language":phone.language,"token":phone.token,"frames":phone.frames
    })).collect();
    write_report(&report_path, &json!({
        "schema":1,"audio_generated":true,"wav":wav_path,"preparation_ms":preparation_ms,
        "render_ms":render_ms,"stage_timings":output.timings,
        "sample_rate":output.audio.sample_rate(),"channels":output.audio.layout().channels(),
        "frames":output.audio.frames(),"start_seconds":output.audio.start_seconds(),
        "duration_seconds":output.audio.duration_seconds(),
        "pitch":{"start_seconds":pitch.start_seconds,"frame_seconds":pitch.frame_seconds,
            "unit":"absolute_midi","values":pitch.values.as_ref(),
            "segments":pitch.segments.iter().map(|span| [span.start,span.end]).collect::<Vec<_>>()},
        "prepared_phrase_start_seconds":prepared.phrase.start_seconds,"prepared_records":notes,
        "phone_frames":phones,"duration_stage":output.state.duration,
        "original_engine_bit_exact_verified":false
    }))
}

fn write_report(path: &std::path::Path, value: &serde_json::Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n'); storage::atomic_write(path, &bytes)
}
