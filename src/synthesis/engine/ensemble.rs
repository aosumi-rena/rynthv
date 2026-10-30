use std::sync::Arc;

use super::{PcmComponents, RenderResult, RenderStage, StageTimings};
use crate::synthesis::{
    rendered::Waveform,
    score::{PreparedPhrase, SAMPLE_RATE},
    voice::{PreparedVoice, Result, VoiceError},
};

pub(super) fn synthesize(
    voice: &PreparedVoice,
    identity: Option<u64>,
    phrase: &PreparedPhrase,
    checkpoint: impl FnMut(RenderStage) -> Result<()>,
) -> Result<RenderResult> {
    synthesize_with(voice,identity,phrase,checkpoint,|v,i,p,c|super::render::synthesize_member(v,i,p,c))
}

pub(super) fn synthesize_with(
    voice:&PreparedVoice,identity:Option<u64>,phrase:&PreparedPhrase,
    mut checkpoint:impl FnMut(RenderStage)->Result<()>,
    mut render:impl FnMut(&PreparedVoice,Option<u64>,&PreparedPhrase,&mut dyn FnMut(RenderStage)->Result<()>)->Result<RenderResult>,
)->Result<RenderResult> {
    let seating=phrase.unison.seating(&voice.speakers)?;
    let mut mixed = Mix::default();
    let mut lead = None;
    let mut timings = StageTimings::default();
    let mut pitch=super::ensemble_pitch::EnsemblePitch::default();

    let normalization = (1.0 / seating.count as f32).sqrt();
    let member_phrases = crate::synthesis::unison::prepare_members(voice, phrase, &seating)?;
    for index in 0..seating.count {
        crate::synthesis::execution::checkpoint()?;
        let speaker=seating.speakers[seating.first+index].map(|id|voice.speakers.get(id)
            .ok_or_else(||VoiceError::new("Seating references an undeclared choir member"))).transpose()?;
        let selected=speaker.map(|speaker|voice.member(speaker)).transpose()?;
        #[cfg(feature="acoustic-diagnostic")]
        let _member_trace=crate::synthesis::stages::duration::diagnostic::member_scope(index,speaker.map(|s|s.id));
        let member=selected.as_ref().unwrap_or(voice);
        let member_phrase = &member_phrases[index];
        let result = render(member, identity, member_phrase, &mut checkpoint)?;
        if seating.count>1 {pitch.add(&result.visuals.pitch)?;}
        mixed.add(
            &result.state.components,
            normalization * 10.0_f32.powf(speaker.map_or(0.0,|s|s.gain_db) / 20.0),
        )?;
        timings.duration_ms += result.timings.duration_ms;
        timings.pitch_ms += result.timings.pitch_ms;
        timings.timbre_ms += result.timings.timbre_ms;
        timings.vocoder_ms += result.timings.vocoder_ms;
        timings.visuals_ms += result.timings.visuals_ms;

        if lead.is_none() {
            lead = Some(result);
        }
    }
    let mut result = lead.ok_or_else(|| VoiceError::new("Choir produced no members"))?;
    if seating.count>1 {pitch.finish(&mut result.visuals.pitch);}
    result.state.components = mixed.finish()?;
    result.audio = result
        .state
        .components
        .publish(phrase.visible_begin_seconds, phrase.visible_end_seconds)?;
    result.visuals.waveform = Waveform::new(&result.audio);
    result.timings = timings;
    Ok(result)
}

#[derive(Default)]
struct Mix {
    start: Option<i64>,
    primary: Vec<f32>,
    secondary: Vec<f32>,
}

impl Mix {
    fn add(&mut self, pcm: &PcmComponents, gain: f32) -> Result<()> {
        if !gain.is_finite() || pcm.primary.len() != pcm.secondary.len() {
            return Err(VoiceError::new("Invalid choir member PCM or gain"));
        }
        let start = (pcm.start_seconds * f64::from(SAMPLE_RATE)).round() as i64;
        let old_start = self.start.unwrap_or(start);
        let first = old_start.min(start);
        let end = start
            .checked_add(i64::try_from(pcm.primary.len()).map_err(|_| overflow())?)
            .ok_or_else(overflow)?;
        let old_end = old_start
            .checked_add(i64::try_from(self.primary.len()).map_err(|_| overflow())?)
            .ok_or_else(overflow)?;
        let count = usize::try_from(end.max(old_end).checked_sub(first).ok_or_else(overflow)?)
            .map_err(|_| overflow())?;
        let prefix = usize::try_from(old_start - first).map_err(|_| overflow())?;
        for destination in [&mut self.primary, &mut self.secondary] {
            let old_len = destination.len();
            destination
                .try_reserve(count.saturating_sub(old_len))
                .map_err(|_| overflow())?;
            destination.resize(count, 0.0);
            if prefix > 0 {
                destination.copy_within(..old_len, prefix);
                destination[..prefix].fill(0.0);
            }
        }
        let offset = usize::try_from(start - first).map_err(|_| overflow())?;
        for (destination, source) in [
            (&mut self.primary, &pcm.primary),
            (&mut self.secondary, &pcm.secondary),
        ] {
            for (chunk_index, chunk) in source.chunks(8192).enumerate() {
                crate::synthesis::execution::checkpoint()?;
                let begin = offset + chunk_index * 8192;
                for (output, input) in destination[begin..begin + chunk.len()]
                    .iter_mut()
                    .zip(chunk)
                {
                    *output += gain * input;
                    if !output.is_finite() {
                        return Err(VoiceError::new("Choir mix produced a nonfinite sample"));
                    }
                }
            }
        }
        self.start = Some(first);
        Ok(())
    }

    fn finish(self) -> Result<PcmComponents> {
        Ok(PcmComponents {
            start_seconds: self
                .start
                .ok_or_else(|| VoiceError::new("Empty choir mix"))?
                as f64
                / f64::from(SAMPLE_RATE),
            primary: Arc::from(self.primary),
            secondary: Arc::from(self.secondary),
        })
    }
}

fn overflow() -> VoiceError {
    VoiceError::new("Choir PCM timeline or allocation overflow")
}
