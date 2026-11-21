use super::args::RenderArgs;
use rynthv::synthesis::{
    audio::save_wav,
    engine,
    score::ExplicitScore,
    settings::{SynthesisSettings, VoiceSelection},
    voice::{PreparedVoice, VoiceInventory},
};
use serde_json::{Value, json};
use std::{fs, time::Instant};

pub fn run(
    args: &RenderArgs,
    inventory: &VoiceInventory,
    voice: &PreparedVoice,
) -> Result<Value, String> {
    let settings = SynthesisSettings::load()?;
    rynthv::synthesis::execution::scope_gru_weights(settings.experiments.zero_gru_weights, || {
        run_with_settings(args, inventory, voice, &settings)
    })
}

fn run_with_settings(
    args: &RenderArgs,
    inventory: &VoiceInventory,
    voice: &PreparedVoice,
    settings: &SynthesisSettings,
) -> Result<Value, String> {
    if fs::metadata(&args.input)
        .map_err(|error| error.to_string())?
        .len()
        > 16 * 1024 * 1024
    {
        return Err("Explicit score JSON exceeds 16 MiB".into());
    }
    let score: ExplicitScore =
        serde_json::from_slice(&fs::read(&args.input).map_err(|error| error.to_string())?)
            .map_err(|error| format!("Invalid explicit score: {error}"))?;
    let phrase = score
        .prepare(
            voice.model("duration").map_err(|error| error.to_string())?,
            voice.family,
            &inventory.configuration.as_ref().ok_or("Voice metadata is unavailable")?.default_language,
        )
        .map_err(|error| error.to_string())?;
    let config = inventory
        .configuration
        .as_ref()
        .ok_or("Voice metadata is unavailable")?;
    let mut identity_source = "not required";
    let identity = if voice.family.requires_identity() {
        let selection = VoiceSelection { database_id: inventory.database_id.clone(), revision: config.version.parse()? };
        let per_version = settings.voice_duration_key_override(&selection).filter(|entry| entry.enabled);

        let selected = if let Some(entry) = per_version {
            identity_source = "voice revision override";
            entry.value
        } else if let Some(value) = args.duration_identity {
            identity_source = "explicit invocation override";
            value
        } else if let Some(value) = settings.duration_key_override_for(&selection) {
            identity_source = "global settings override";
            value
        } else {
            identity_source = "default zero";
            0
        };
        Some(selected)
    } else if args.duration_identity.is_some() {
        return Err("原生 SV1 时长模型不接受 --duration-identity 条件。".into());
    } else {
        None
    };
    #[cfg(feature = "synth-profile")]
    if args.profile.is_some() {
        rynthv::synthesis::profiling::start();
    }
    let started = Instant::now();
    let output = engine::synthesize(voice, identity, &phrase, |stage| {
        eprintln!("合成：{}", stage.label());
        Ok(())
    })
    .map_err(|error| error.to_string());
    let render_ms = started.elapsed().as_secs_f64() * 1000.0;
    #[cfg(feature = "synth-profile")]
    if let Some(path) = &args.profile {
        let profile = rynthv::synthesis::profiling::finish();
        let bytes = serde_json::to_vec_pretty(&profile).map_err(|error| error.to_string())?;
        rynthv::storage::atomic_write(path, &bytes)?;
    }
    let output = output?;
    let started = Instant::now();
    save_wav(&args.wav, &output.audio).map_err(|error| error.to_string())?;
    let wav_write_ms = started.elapsed().as_secs_f64() * 1000.0;
    let peak = output
        .audio
        .samples()
        .iter()
        .fold(0.0_f32, |peak, value| peak.max(value.abs()));
    let rms = (output
        .audio
        .samples()
        .iter()
        .map(|&value| f64::from(value).powi(2))
        .sum::<f64>()
        / output.audio.samples().len() as f64)
        .sqrt();
    Ok(json!({
        "wav": args.wav,
        "sample_rate": output.audio.sample_rate(),
        "channels": output.audio.layout().channels(),
        "frames": output.audio.frames(),
        "start_seconds": output.audio.start_seconds(),
        "duration_seconds": output.audio.duration_seconds(),
        "peak": peak,
        "rms": rms,
        "render_ms": render_ms,
        "wav_write_ms": wav_write_ms,
        "stage_timings": output.timings,
        "identity_source": identity_source,
        "duration_identity": identity.map(|value| value.to_string()),
        "duration_identity_hex": identity.map(|value| format!("0x{value:016x}")),
        "original_engine_bit_exact_verified": false,
    }))
}
