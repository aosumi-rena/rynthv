use super::{PcmStore, PreparedRender};
use crate::synthesis::{
    engine::{
        PreparedModelFeatures, PreparedRenderAcoustics, RenderStage,
        incremental::{IncrementalEdit, acoustics},
        models, prepare_acoustics,
    },
    plan::OwnerInput,
    render_cache::CompletedOwner,
    score::{self, HOP_SAMPLES, LyricFrontend, SAMPLE_RATE},
    voice::{PreparedVoice, Result, VoiceError, VoiceFamily},
};

pub fn prepare(
    voice: &PreparedVoice,
    identity: Option<u64>,
    input: &OwnerInput,
    lexicon: &LyricFrontend,
    base: Option<&CompletedOwner>,
    mut checkpoint: impl FnMut(RenderStage) -> Result<()>,
) -> Result<PreparedRender> {
    let mut phrase = score::prepare_owner(input, lexicon, voice.model("duration")?, voice.family)?;
    super::super::experimental::prepare_modes(voice,input,&mut phrase)?;
    let experimental=super::super::experimental::needs_complete(input,voice.family)?;
    if experimental || !voice.speakers.is_empty() || phrase.unison.count>1 {
        let result = if experimental {super::super::experimental::synthesize(voice,identity,input,&phrase,lexicon,checkpoint)?}
            else {super::super::ensemble::synthesize(voice, identity, &phrase, checkpoint)?};
        let pcm = PcmStore::complete(&result.state.components)?;
        return Ok(PreparedRender {
            acoustic: PreparedRenderAcoustics {
                family: voice.family,
                prepared: result.state.prepared,
                duration: result.state.duration,
                pitch: result.state.pitch,
                timings: result.timings,
            },
            pcm,
            pitch: result.visuals.pitch,
            timbre_frames: Default::default(),
        });
    }

    let base = base.filter(|base| {
        !input.experiments.settings.enabled && voice.family == VoiceFamily::Sv2
            && base.clip.state.prepared.phrase.unison.count==1
            && matches!(
                base.clip.state.prepared.features,
                PreparedModelFeatures::Sv2 { .. }
            )
    });
    let edit = base
        .map(|base| {
            IncrementalEdit::between(
                &base.input,
                input,
                &base.clip.state.prepared.phrase,
                &phrase,
            )
        })
        .transpose()?
        .flatten()
        .filter(|_| {
            let previous = &base.expect("an edit requires a baseline").clip.state;
            let pcm = &previous.components;
            previous.pitch.pitch.len().checked_mul(HOP_SAMPLES) == Some(pcm.primary.len())
                && pcm.primary.len() == pcm.secondary.len()
                && ((pcm.start_seconds - phrase.start_seconds) * f64::from(SAMPLE_RATE)).abs()
                    < 0.0001
        });
    let (acoustic, pcm) = if let Some((base, edit)) = base.zip(edit) {
        let updated = acoustics::recompute(
            voice,
            models::required_identity(identity)?,
            &phrase,
            &base.clip.state,
            &edit,
            &mut checkpoint,
        )?;
        let pcm = PcmStore::from_existing(
            &base.clip.state.components,
            edit.first_block..edit.stop_block,
        )?;
        (
            PreparedRenderAcoustics {
                family: voice.family,
                prepared: updated.state.prepared,
                duration: updated.state.duration,
                pitch: updated.state.pitch,
                timings: updated.timings,
            },
            pcm,
        )
    } else {
        let acoustic = prepare_acoustics(voice, identity, &phrase, checkpoint)?;
        let frames = match acoustic.prepared.features {
            PreparedModelFeatures::Sv1Local => acoustic
                .prepared
                .phones
                .iter()
                .try_fold(0_usize, |total, phone| total.checked_add(phone.frames)),
            PreparedModelFeatures::Sv2 { .. } => Some(acoustic.pitch.pitch.len()),
        }
        .ok_or_else(|| VoiceError::new("Progressive acoustic frame count overflow"))?;
        let frames = if voice.family == crate::synthesis::voice::VoiceFamily::Sv1 {
            frames.saturating_sub(crate::synthesis::stages::sv1_smoothing::LOOKAHEAD)
        } else { frames };
        let samples = frames
            .checked_mul(HOP_SAMPLES)
            .ok_or_else(|| VoiceError::new("Progressive PCM size overflow"))?;
        let pcm = PcmStore::new(phrase.start_seconds, samples)?;
        (acoustic, pcm)
    };
    let pitch = models::pitch_track(voice, &acoustic.prepared.phrase, acoustic.pitch.clone())?;
    Ok(PreparedRender {
        acoustic,
        pcm,
        pitch,
        timbre_frames: Default::default(),
    })
}
