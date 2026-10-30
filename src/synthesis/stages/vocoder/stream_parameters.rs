use std::collections::VecDeque;

use crate::synthesis::{
    inference::{ModelRuntime, Sequence, recurrent::GruState},
    voice::{Result, VoiceError},
};

use super::{
    condition::CHANNELS,
    excitation::ExcitationFrame,
    metadata,
    output::FrameMetadata,
    source,
    spectral_heads::SpectralHeads,
    stage::{VocoderParameters, VocoderStage},
    stream_queue::FrameQueue,
};

pub(crate) struct ParameterStream<'a> {
    pub(crate) stage: VocoderStage<'a>,
    pub(crate) runtime: ModelRuntime<'a>,
    positional: Option<GruState>,
    heads: SpectralHeads,
    input_frames: usize,
    output_frames: usize,
    finished: bool,
    physical: FrameQueue,
    logits: FrameQueue,
    normalized: FrameQueue,
    spectra: [FrameQueue; 3],
    harmonic: FrameQueue,
    excitation: VecDeque<ExcitationFrame>,
    reference_shapes:VecDeque<f32>,
    metadata: VecDeque<FrameMetadata>,
}

impl<'a> ParameterStream<'a> {
    pub(crate) fn new(stage: VocoderStage<'a>) -> Result<Self> {
        let heads = SpectralHeads::prepare(stage.tree, stage.modules.spectral_heads)?;
        if heads.head_count() != 3 || heads.channels() != 512 {
            return Err(VoiceError::new(
                "Vocoder requires three 512-channel spectral heads",
            ));
        }
        let positional = stage.positional.as_ref().map(|model| model.new_state());
        let runtime = ModelRuntime::new(stage.tree);
        Ok(Self {
            stage,
            runtime,
            positional,
            heads,
            input_frames: 0,
            output_frames: 0,
            finished: false,
            physical: FrameQueue::new(CHANNELS),
            logits: FrameQueue::new(1),
            normalized: FrameQueue::new(CHANNELS),
            spectra: std::array::from_fn(|_| FrameQueue::new(512)),
            harmonic: FrameQueue::new(10),
            excitation: VecDeque::new(),
            reference_shapes:VecDeque::new(),
            metadata: VecDeque::new(),
        })
    }

    pub(crate) fn process(
        &mut self,
        physical: &Sequence,
        controls: Option<&Sequence>,
        final_batch: bool,
    ) -> Result<VocoderParameters> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("vocoder.prepare_parameters");
        if self.finished {
            return Err(VoiceError::new(
                "Vocoder parameter stream has already finished",
            ));
        }
        physical.validate()?;
        let metadata = metadata::from_timbre(controls, physical.frames)?;
        let normalized = if self.stage.native_dds {
            self.stage.condition.normalize_native(physical)?
        } else {
            self.stage.condition.normalize(physical)?
        };
        let input_frames = self
            .input_frames
            .checked_add(physical.frames)
            .ok_or_else(|| VoiceError::new("Vocoder frame count overflows"))?;

        if physical.frames == 0 && (!final_batch || self.input_frames == 0) {
            self.finished = final_batch;
            return self.take_ready(final_batch);
        }
        self.metadata.extend(metadata);

        self.reference_shapes.extend(physical.values.chunks_exact(CHANNELS).map(|frame|frame[2]));
        self.physical.push(physical.clone())?;
        self.normalized.push(normalized.clone())?;
        let features = {
            #[cfg(feature = "synth-profile")]
            let _profile = crate::synthesis::profiling::span("vocoder.conditioned_network");
            if let Some(model) = &self.stage.positional {
                let positions = (self.input_frames..input_frames)
                    .map(|position| {
                        u32::try_from(position)
                            .map_err(|_| VoiceError::new("Vocoder frame position exceeds u32"))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let recurrent = model.execute(
                    &normalized,
                    &positions,
                    self.positional.as_mut().expect("prepared positional state"),
                )?;
                self.runtime.execute_conditioned(
                    self.stage.modules.conditioned_network,
                    &normalized,
                    &recurrent,
                    final_batch,
                )?
            } else {

                self.runtime.execute(
                    self.stage.modules.conditioned_network,
                    &normalized,
                    final_batch,
                )?
            }
        };
        self.input_frames = input_frames;
        let spectra = self
            .heads
            .execute(&mut self.runtime, &features, final_batch)?;
        for (queue, spectrum) in self.spectra.iter_mut().zip(spectra) {
            queue.push(spectrum)?;
        }
        let harmonic = {
            #[cfg(feature = "synth-profile")]
            let _profile = crate::synthesis::profiling::span("vocoder.gain_network");
            self.runtime
                .execute(self.stage.modules.gain_network, &features, final_batch)?
        };
        self.harmonic.push(harmonic)?;
        let logits = {
            #[cfg(feature = "synth-profile")]
            let _profile = crate::synthesis::profiling::span("vocoder.input_projection");
            self.runtime.execute(
                self.stage
                    .modules
                    .input_projection
                    .expect("prepared shape source"),
                &normalized,
                final_batch,
            )?
        };
        self.logits.push(logits)?;
        let source_frames = self.physical.frames().min(self.logits.frames());
        let physical = self.physical.take(source_frames)?;
        let logits = self.logits.take(source_frames)?;
        self.excitation.extend(source::excitation_frames(
            &physical,
            &logits,
            &vec![0.0; source_frames],
        )?);
        let output = self.take_ready(final_batch)?;
        self.finished = final_batch;
        Ok(output)
    }

    fn take_ready(&mut self, final_batch: bool) -> Result<VocoderParameters> {
        if self.stage.independent_branches {return self.take_independent(final_batch);}
        let frames = self
            .spectra
            .iter()
            .map(FrameQueue::frames)
            .chain([
                self.normalized.frames(),
                self.harmonic.frames(),
                self.excitation.len(),
                self.reference_shapes.len(),
                self.metadata.len(),
            ])
            .min()
            .expect("vocoder has output branches");
        let output_frames = self
            .output_frames
            .checked_add(frames)
            .ok_or_else(|| VoiceError::new("Vocoder output frame count overflows"))?;
        if output_frames > self.input_frames {
            return Err(VoiceError::new(
                "Vocoder parameter branches produced excess frames",
            ));
        }
        if final_batch
            && (output_frames != self.input_frames
                || self.physical.frames() != 0
                || self.logits.frames() != 0
                || self.spectra.iter().any(|queue| queue.frames() != frames)
                || self.normalized.frames() != frames
                || self.harmonic.frames() != frames
                || self.excitation.len() != frames
                || self.reference_shapes.len()!=frames
                || self.metadata.len() != frames)
        {
            return Err(VoiceError::new(
                "Vocoder parameter branches ended with unaligned frame counts",
            ));
        }
        let normalized = self.normalized.take(frames)?;
        let spectra = [
            self.spectra[0].take(frames)?,
            self.spectra[1].take(frames)?,
            self.spectra[2].take(frames)?,
        ];
        let harmonic_parameters = self.harmonic.take(frames)?;
        let excitation:Vec<_> = self.excitation.drain(..frames).collect();
        let reference_shapes:Vec<_>=self.reference_shapes.drain(..frames).collect();
        let effects=excitation.iter().enumerate().map(|(i,&e)|self.stage.effects.as_ref()
            .map(|timeline|timeline.frame(self.output_frames+i,e,reference_shapes[i]))).collect();
        let metadata = self.metadata.drain(..frames).collect();
        self.output_frames = output_frames;
        Ok(VocoderParameters {
            normalized,
            spectra,
            harmonic_parameters,
            excitation,
            metadata,
            effects,
        })
    }

    fn take_independent(&mut self,end:bool)->Result<VocoderParameters> {
        let frames=self.excitation.len();
        if self.normalized.frames()<frames||self.reference_shapes.len()<frames||self.metadata.len()<frames {
            return Err(VoiceError::new("Compatible vocoder source branches are not aligned"));
        }
        let output_frames=self.output_frames.checked_add(frames).ok_or_else(||VoiceError::new("Vocoder source count overflows"))?;
        if output_frames>self.input_frames||(end&&(output_frames!=self.input_frames||self.physical.frames()!=0||self.logits.frames()!=0
            ||self.normalized.frames()!=frames||self.reference_shapes.len()!=frames||self.metadata.len()!=frames)) {
            return Err(VoiceError::new("Compatible vocoder source did not cover its input timeline"));
        }
        let normalized=self.normalized.take(frames)?;
        let excitation:Vec<_>=self.excitation.drain(..frames).collect();
        let shapes:Vec<_>=self.reference_shapes.drain(..frames).collect();
        let effects=excitation.iter().enumerate().map(|(i,&e)|self.stage.effects.as_ref()
            .map(|timeline|timeline.frame(self.output_frames+i,e,shapes[i]))).collect();
        let metadata=self.metadata.drain(..frames).collect();
        let counts=self.spectra.each_ref().map(FrameQueue::frames);
        let spectra=[self.spectra[0].take(counts[0])?,self.spectra[1].take(counts[1])?,self.spectra[2].take(counts[2])?];
        let count=self.harmonic.frames();let harmonic_parameters=self.harmonic.take(count)?;
        self.output_frames=output_frames;
        Ok(VocoderParameters{normalized,spectra,harmonic_parameters,excitation,metadata,effects})
    }
}
