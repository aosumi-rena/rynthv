use super::{Network, PreparedRender, network::Stream, pcm::Segment, pipeline};
use crate::synthesis::{
    audio::RenderedAudio,
    engine::{PcmComponents, RenderStage},
    score::{FRAME_SECONDS, SAMPLE_RATE},
    stages::timbre::TimbreOutput,
    voice::{PreparedVoice, Result, VoiceError},
};
use std::time::Instant;

pub struct Cursor<'network, 'tree> {
    timbre: Stream<'network, 'tree>,
    vocoder: pipeline::Pipeline,
    segment: Segment,
    pcm_origin: f64,
    produced_samples: usize,
    setup_timbre_ms: f64,
    pending: Option<TimbreOutput>,
    pending_offset: usize,
}

impl Cursor<'_, '_> {
    pub fn is_finished(&self) -> bool {
        self.segment.is_finished()
    }

    pub fn covers_demand(&self, seconds: f64) -> bool {
        self.segment.covers_demand(seconds)
    }
}

impl PreparedRender {

    pub fn start<'network, 'tree>(
        &self,
        network: &'network Network<'tree>,
        voice: &'tree PreparedVoice,
        seconds: f64,
    ) -> Result<Option<Cursor<'network, 'tree>>> {
        let Some(plan) = self.pcm.start_plan(seconds) else {
            return Ok(None);
        };
        let started = Instant::now();
        let prepared = &self.acoustic.prepared;
        let phrase = &prepared.phrase;
        let requested_frame = if voice.family == crate::synthesis::voice::VoiceFamily::Sv1 {
            0
        } else {
            ((plan.requested_seconds - phrase.start_seconds) / FRAME_SECONDS)
                .floor()
                .max(0.0) as usize
        };
        if self.timbre_frames.get().is_none() {
            let frames = network.prepare_frames(&self.acoustic)?;
            let _ = self.timbre_frames.set(frames);
        }
        let timbre = network.start(
            &self.acoustic,
            self.timbre_frames
                .get()
                .expect("frame conditions prepared above"),
            requested_frame,
        )?;
        let pcm_origin = phrase.start_seconds + timbre.first_frame() as f64 * FRAME_SECONDS;
        let setup_timbre_ms = started.elapsed().as_secs_f64() * 1000.0;
        let effects=(voice.family==crate::synthesis::voice::VoiceFamily::Sv2)
            .then(||crate::synthesis::stages::vocoder::effects::EffectTimeline::new(phrase,pcm_origin));
        let seeds=if voice.family==crate::synthesis::voice::VoiceFamily::Sv2 {
            Some(crate::synthesis::stages::vocoder::noise_seeds::for_phrase(phrase,timbre.first_frame())?.into())
        }else{None};
        let vocoder = pipeline::Pipeline::new(voice.shared_model("vocoder")?,effects,
            voice.family==crate::synthesis::voice::VoiceFamily::Sv2Compatible,
            voice.family==crate::synthesis::voice::VoiceFamily::Sv1,seeds)?;
        Ok(Some(Cursor {
            timbre,
            vocoder,
            segment: self.pcm.segment(plan),
            pcm_origin,
            produced_samples: 0,
            setup_timbre_ms,
            pending: None,
            pending_offset: 0,
        }))
    }

    pub fn advance(
        &mut self,
        cursor: &mut Cursor<'_, '_>,
        max_frames: usize,
        mut checkpoint: impl FnMut(RenderStage) -> Result<()>,
    ) -> Result<Vec<RenderedAudio>> {
        if cursor.is_finished() {
            return Err(VoiceError::new("Progressive cursor has already finished"));
        }
        if max_frames == 0 {
            return Err(VoiceError::new("Progressive frame budget must be positive"));
        }
        self.acoustic.timings.timbre_ms += std::mem::take(&mut cursor.setup_timbre_ms);
        checkpoint(RenderStage::Vocoder)?;
        if let Some(output) = cursor.vocoder.poll()? {
            return self.publish_output(cursor, output);
        }
        if !cursor.vocoder.can_submit() {
            let output = cursor.vocoder.wait()?;
            checkpoint(RenderStage::Vocoder)?;
            return output.map_or_else(
                || Ok(Vec::new()),
                |output| self.publish_output(cursor, output),
            );
        }
        if cursor.pending.is_none() {
            checkpoint(RenderStage::Timbre)?;
            let started = Instant::now();
            cursor.pending = Some(cursor.timbre.next(max_frames)?);
            cursor.pending_offset = 0;
            self.acoustic.timings.timbre_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
        checkpoint(RenderStage::Vocoder)?;

        let pending = cursor
            .pending
            .as_ref()
            .expect("acoustic output prepared above");
        let end = (cursor.pending_offset + max_frames.min(128)).min(pending.acoustics.frames);
        let frames = cursor.pending_offset..end;
        let acoustics = pending.acoustics.slice_frames(frames.clone())?;
        let controls = pending
            .controls
            .as_ref()
            .map(|controls| controls.slice_frames(frames))
            .transpose()?;
        let drained = end == pending.acoustics.frames;
        let natural_final = drained && cursor.timbre.is_finished();
        cursor.vocoder.submit(acoustics, controls, natural_final)?;
        cursor.pending_offset = end;
        if drained {
            cursor.pending = None;
        }
        if let Some(output) = cursor.vocoder.poll()? {
            return self.publish_output(cursor, output);
        }
        Ok(Vec::new())
    }

    fn publish_output(
        &mut self,
        cursor: &mut Cursor<'_, '_>,
        output: pipeline::Output,
    ) -> Result<Vec<RenderedAudio>> {
        self.acoustic.timings.vocoder_ms += output.elapsed_ms;
        let origin = cursor.pcm_origin + cursor.produced_samples as f64 / f64::from(SAMPLE_RATE);
        cursor.produced_samples = cursor
            .produced_samples
            .checked_add(output.pcm.primary.values.len())
            .ok_or_else(|| VoiceError::new("Progressive PCM sample count overflow"))?;
        let fresh = PcmComponents::new(origin, output.pcm)?;
        let chunks = self
            .pcm
            .push(&mut cursor.segment, &fresh, output.natural_final)?;
        if cursor.segment.is_finished() {

            cursor.vocoder.stop();
        }
        Ok(chunks)
    }
}
