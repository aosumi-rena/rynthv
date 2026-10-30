use std::collections::VecDeque;

use super::{
    output::FrameMetadata, render::VocoderOutput, stage::VocoderStage,
    stream_parameters::ParameterStream, stream_signals::SignalStream,
};
use crate::synthesis::{
    inference::Sequence,
    voice::{Result, VoiceError},
};

pub struct VocoderStream<'a> {
    neural:NeuralStream<'a>,
    compatible:Option<super::compatible_dispatch::CompatibleDispatch>,
    sv2:Option<super::sv2_packets::Sv2Packets>,
    dds_smoothing: Option<crate::synthesis::stages::sv1_smoothing::Smoother>,
}

pub(super) struct NeuralStream<'a> {
    parameters: ParameterStream<'a>,
    signals: SignalStream,
    metadata: VecDeque<FrameMetadata>,
    input_frames: usize,
    output_frames: usize,
    finished: bool,
}

impl<'a> VocoderStage<'a> {
    pub fn into_stream(self) -> Result<VocoderStream<'a>> {
        if self.native_dds && (self.compatible_dispatch || self.editor_seed_source.is_some()) {
            return Err(VoiceError::new("Conflicting DDS vocoder programs"));
        }
        if self.compatible_dispatch&&self.editor_seed_source.is_some() {return Err(VoiceError::new("Conflicting vocoder packet programs"));}
        let signals = SignalStream::new(&self)?;
        let compatible=(self.compatible_dispatch || self.native_dds).then(super::compatible_dispatch::CompatibleDispatch::new);
        let sv2=self.editor_seed_source.clone().map(super::sv2_packets::Sv2Packets::new);
        let dds_smoothing = self.native_dds.then(crate::synthesis::stages::sv1_smoothing::Smoother::new);
        Ok(VocoderStream {compatible,sv2,dds_smoothing,neural:NeuralStream {
            parameters: ParameterStream::new(self)?,
            signals,
            metadata: VecDeque::new(),
            input_frames: 0,
            output_frames: 0,
            finished: false,
        }})
    }
}

impl VocoderStream<'_> {
    pub fn process(&mut self,physical:&Sequence,controls:Option<&Sequence>,final_batch:bool)->Result<VocoderOutput> {
        if let Some(smoother) = &mut self.dds_smoothing {
            if controls.is_some() { return Err(VoiceError::new("DDS smoothing requires paired native controls")); }
            let smoothed = smoother.process(physical, final_batch)?;
            return self.compatible.as_mut().expect("DDS dispatch prepared")
                .process(&mut self.neural, &smoothed, None, final_batch);
        }
        if let Some(packets)=&mut self.sv2 {return packets.process(&mut self.neural,physical,controls,final_batch);}
        match &mut self.compatible {
            Some(adapter)=>adapter.process(&mut self.neural,physical,controls,final_batch),
            None=>self.neural.process(physical,controls,final_batch),
        }
    }
}

impl NeuralStream<'_> {
    pub(crate) fn append_frame_noise_seeds(&mut self,seeds:&[u64])->Result<()> {self.signals.append_frame_noise_seeds(seeds)}

    pub fn process(
        &mut self,
        physical: &Sequence,
        controls: Option<&Sequence>,
        final_batch: bool,
    ) -> Result<VocoderOutput> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("vocoder.stream.process");
        if self.finished {
            return Err(VoiceError::new("Vocoder stream has already finished"));
        }
        let parameters = self.parameters.process(physical, controls, final_batch)?;
        self.input_frames = self
            .input_frames
            .checked_add(physical.frames)
            .ok_or_else(|| VoiceError::new("Vocoder input frame count overflow"))?;

        if self.input_frames == 0 {
            self.finished = final_batch;
            return Ok(VocoderOutput {
                primary: Sequence::zeros(0, self.parameters.stage.condition.frame_samples)?,
                secondary: Sequence::zeros(0, self.parameters.stage.condition.frame_samples)?,
                metadata: Vec::new(),
            });
        }
        self.metadata.extend(parameters.metadata.iter().copied());
        let (primary, secondary) =
            self.signals
                .process(&mut self.parameters.runtime, parameters, final_batch)?;
        if primary.frames != secondary.frames || primary.frames > self.metadata.len() {
            return Err(VoiceError::new(
                "Vocoder PCM and metadata frames do not align",
            ));
        }
        self.output_frames = self
            .output_frames
            .checked_add(primary.frames)
            .ok_or_else(|| VoiceError::new("Vocoder output frame count overflow"))?;
        if final_batch
            && (self.output_frames != self.input_frames || primary.frames != self.metadata.len())
        {
            return Err(VoiceError::new(
                "Vocoder ended with pending acoustic frames",
            ));
        }
        let metadata = self.metadata.drain(..primary.frames).collect();
        self.finished = final_batch;
        Ok(VocoderOutput {
            primary,
            secondary,
            metadata,
        })
    }
}
