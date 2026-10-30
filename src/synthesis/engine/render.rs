use super::{
    OwnerSynthesis, PcmComponents, RenderStage, StageTimings, models,
};
use crate::synthesis::{
    audio::RenderedAudio,
    rendered::{RenderVisuals, Waveform},
    score::PreparedPhrase,
    stages::vocoder::VocoderStage,
    voice::{PreparedVoice, Result},
};
use std::time::Instant;

pub struct RenderResult {
    pub audio: RenderedAudio,
    pub visuals: RenderVisuals,
    pub timings: StageTimings,
    pub state: OwnerSynthesis,
}

pub fn synthesize(
    voice: &PreparedVoice,
    duration_identity: Option<u64>,
    phrase: &PreparedPhrase,
    mut checkpoint: impl FnMut(RenderStage) -> Result<()>,
) -> Result<RenderResult> {
    if !voice.speakers.is_empty() || phrase.unison.count>1 {
        return super::ensemble::synthesize(voice, duration_identity, phrase, checkpoint);
    }
    synthesize_member(voice, duration_identity, phrase, &mut checkpoint)
}

pub(super) fn synthesize_member(
    voice: &PreparedVoice,
    duration_identity: Option<u64>,
    phrase: &PreparedPhrase,
    checkpoint: impl FnMut(RenderStage) -> Result<()>,
 ) -> Result<RenderResult> {
    synthesize_member_with_host(voice, duration_identity, phrase, checkpoint, None)
}

pub fn synthesize_host_pitch(
    voice: &PreparedVoice,
    phrase: &PreparedPhrase,
    checkpoint: impl FnMut(RenderStage) -> Result<()>,
    mut sampler: impl FnMut(super::DdsPitchClock) -> Result<Vec<Option<f32>>>,
) -> Result<RenderResult> {
    if voice.family != crate::synthesis::voice::VoiceFamily::Sv1
        || !voice.speakers.is_empty() || phrase.unison.count>1 || phrase.pitch_controls.is_some()
    {
        return Err(crate::synthesis::voice::VoiceError::new("Host pitch mode requires single Sv1 with no competing curve source"));
    }
    synthesize_member_with_host(voice, None, phrase, checkpoint, Some(&mut sampler))
}

fn synthesize_member_with_host(
    voice: &PreparedVoice,
    duration_identity: Option<u64>,
    phrase: &PreparedPhrase,
    mut checkpoint: impl FnMut(RenderStage) -> Result<()>,
    host_pitch: Option<&mut super::pitch::HostPitchSampler<'_>>,
) -> Result<RenderResult> {
    let acoustic = super::acoustics::synthesize_acoustics_with_host(voice, duration_identity, phrase, &mut checkpoint, host_pitch)?;
    publish_acoustic(voice,phrase,acoustic,checkpoint)
}

pub(super) fn publish_acoustic(voice:&PreparedVoice,phrase:&PreparedPhrase,acoustic:super::AcousticRender,
    mut checkpoint:impl FnMut(RenderStage)->Result<()>)->Result<RenderResult> {
    checkpoint(RenderStage::Vocoder)?;
    let started = Instant::now();
    #[cfg(feature = "synth-profile")]
    let profile = crate::synthesis::profiling::span("stage.vocoder");
    let mut vocoder = VocoderStage::prepare(voice.model("vocoder")?)?;
    let origin = phrase.start_seconds
        + acoustic.timbre.first_frame as f64 * crate::synthesis::score::FRAME_SECONDS;
    if voice.family==crate::synthesis::voice::VoiceFamily::Sv2 {
        vocoder.set_editor_controls(&acoustic.prepared.phrase,origin);
        let seeds=crate::synthesis::stages::vocoder::noise_seeds::for_phrase(&acoustic.prepared.phrase,acoustic.timbre.first_frame)?;
        vocoder.set_editor_seed_source(seeds.into());
    }
    if voice.family==crate::synthesis::voice::VoiceFamily::Sv2Compatible {vocoder.set_compatible_dispatch();}
    if acoustic.prepared.phrase.experimental_dds_expression {
        let p=acoustic.prepared.phrase.voice_parameters;
        let curves=acoustic.prepared.phrase.automation.as_ref();
        if p.loudness!=0.0 || p.tension!=0.0 || p.breathiness!=0.0 || p.gender!=0.0
            || curves.is_some_and(|a|["loudness","tension","breathiness","gender","voicing"].iter().any(|k|a.has(k))) {
            vocoder.set_editor_controls(&acoustic.prepared.phrase,origin);
        }
    }
    if voice.family==crate::synthesis::voice::VoiceFamily::Sv1 {vocoder.set_native_dds();}
    let output = vocoder.render_components(
        &acoustic.timbre.acoustics,
        acoustic.timbre.controls.as_ref(),
    )?;
    let mut timings = acoustic.timings;
    timings.vocoder_ms = started.elapsed().as_secs_f64() * 1000.0;
    #[cfg(feature = "synth-profile")]
    drop(profile);
    #[cfg(feature = "synth-profile")]
    let profile = crate::synthesis::profiling::span("stage.pcm_publication");
    let components = PcmComponents::new(origin, output)?;
    let audio = components.publish(phrase.visible_begin_seconds, phrase.visible_end_seconds)?;
    #[cfg(feature = "synth-profile")]
    drop(profile);
    let started = Instant::now();
    #[cfg(feature = "synth-profile")]
    let _profile = crate::synthesis::profiling::span("stage.visuals");
    let pitch = models::pitch_track(voice, &acoustic.prepared.phrase, acoustic.pitch.clone())?;
    let visuals = RenderVisuals {
        phonemes:crate::synthesis::rendered::PhonemeTrack::new(&acoustic.prepared.phrase,&acoustic.duration),
        pitch,
        waveform: Waveform::new(&audio),
    };
    timings.visuals_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok(RenderResult {
        audio,
        visuals,
        timings,
        state: OwnerSynthesis {
            prepared: acoustic.prepared,
            duration: acoustic.duration,
            pitch: acoustic.pitch,
            components,
        },
    })
}
