pub(super) mod conditioning;
mod prepare;

pub use prepare::prepare_acoustics;

use super::{PreparedAcoustics, PreparedModelFeatures, RenderStage, models};
use crate::synthesis::{
    score::PreparedPhrase,
    stages::{
        duration::DurationResult,
        f0::PitchResult,
        timbre::{TimbreNetwork, TimbreOutput, TimbreRequest, records::RecordsConfig},
    },
    voice::{PreparedVoice, Result, VoiceError, VoiceFamily},
};
use serde::Serialize;
use std::time::Instant;

#[derive(Clone, Debug, Default, Serialize)]

pub struct StageTimings {
    pub duration_ms: f64,
    pub pitch_ms: f64,
    pub timbre_ms: f64,
    pub vocoder_ms: f64,
    pub visuals_ms: f64,
}

pub struct PreparedRenderAcoustics {
    pub(crate) family: VoiceFamily,
    pub prepared: PreparedAcoustics,
    pub duration: DurationResult,
    pub pitch: PitchResult,
    pub timings: StageTimings,
}

pub struct AcousticRender {
    pub duration: DurationResult,
    pub pitch: PitchResult,
    pub timbre: TimbreOutput,
    pub timings: StageTimings,
    pub prepared: PreparedAcoustics,
}

pub fn synthesize_acoustics(
    voice: &PreparedVoice,
    duration_identity: Option<u64>,
    phrase: &PreparedPhrase,
    checkpoint: impl FnMut(RenderStage) -> Result<()>,
) -> Result<AcousticRender> {
    synthesize_acoustics_with_host(voice, duration_identity, phrase, checkpoint, None)
}

pub(super) fn synthesize_acoustics_with_host(
    voice: &PreparedVoice,
    duration_identity: Option<u64>,
    phrase: &PreparedPhrase,
    checkpoint: impl FnMut(RenderStage) -> Result<()>,
    host_pitch: Option<&mut super::pitch::HostPitchSampler<'_>>,
) -> Result<AcousticRender> {
    let mut render = prepare::prepare_acoustics_with_host(voice, duration_identity, phrase, checkpoint, host_pitch)?;
    let started = Instant::now();
    #[cfg(feature = "synth-profile")]
    let _profile = crate::synthesis::profiling::span("stage.timbre");
    let modulation = render.modulation();
    let impulses = render.impulses();
    let frame_controls=render.frame_controls();
    let acoustic_pitch = render.acoustic_pitch()?;
    let request = TimbreRequest { dds:None,
        mode_coefficients: render.prepared.phrase.mode_coefficients.as_ref(),
        notes: &render.prepared.phrase.notes,
        phones: &render.prepared.phones,
        midi_pitch: &acoustic_pitch,
        note_modulation: &modulation,
        phone_impulses: (voice.family == VoiceFamily::Sv2).then_some(impulses.as_slice()),
        frame_controls: frame_controls.as_deref(),
        requested_frame: 0,
    };
    let timbre = match &render.prepared.features {
        PreparedModelFeatures::Sv1Local => {
            let stage = models::dds_timbre(voice)?;
            stage.render(&request, &render.pitch)?
        }
        PreparedModelFeatures::Sv2 { timbre, .. } => TimbreNetwork::prepare(
            voice.model("timbre")?,
            voice.speaker,
            voice.retained_acoustic_noise_only,
        )?
        .render_prepared(&request, &RecordsConfig::default(), timbre)?,
    };
    if timbre
        .acoustics
        .values
        .iter()
        .chain(timbre.controls.iter().flat_map(|controls| &controls.values))
        .any(|value| !value.is_finite())
    {
        return Err(VoiceError::new(
            "Acoustic synthesis returned a nonfinite value",
        ));
    }
    render.timings.timbre_ms += started.elapsed().as_secs_f64() * 1000.0;
    Ok(AcousticRender {
        duration: render.duration,
        pitch: render.pitch,
        timbre,
        timings: render.timings,
        prepared: render.prepared,
    })
}
