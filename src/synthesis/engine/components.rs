use crate::synthesis::{
    audio::{ChannelLayout, RenderedAudio},
    score::SAMPLE_RATE,
    stages::vocoder::VocoderOutput,
    voice::{Result, VoiceError},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct PcmComponents {
    pub start_seconds: f64,
    pub primary: Arc<[f32]>,
    pub secondary: Arc<[f32]>,
}

impl PcmComponents {
    pub fn new(start_seconds: f64, output: VocoderOutput) -> Result<Self> {
        if !start_seconds.is_finite()
            || output.primary.values.len() != output.secondary.values.len()
            || output
                .primary
                .values
                .iter()
                .chain(&output.secondary.values)
                .any(|v| !v.is_finite())
        {
            return Err(VoiceError::new("Invalid vocoder component timeline"));
        }
        Ok(Self {
            start_seconds,
            primary: output.primary.values.into(),
            secondary: output.secondary.values.into(),
        })
    }

    pub fn publish(&self, begin_seconds: f64, end_seconds: Option<f64>) -> Result<RenderedAudio> {
        let rate = f64::from(SAMPLE_RATE);
        let first = ((begin_seconds.max(0.0) - self.start_seconds) * rate)
            .ceil()
            .clamp(0.0, self.primary.len() as f64) as usize;
        let end = end_seconds.map_or(self.primary.len(), |end| {
            ((end - self.start_seconds) * rate)
                .floor()
                .clamp(0.0, self.primary.len() as f64) as usize
        });
        if first >= end || self.primary.len() != self.secondary.len() {
            return Err(VoiceError::new(
                "Rendered components have no visible samples",
            ));
        }
        let samples: Vec<_> = self.primary[first..end]
            .iter()
            .zip(&self.secondary[first..end])
            .map(|(&a, &b)| a + b)
            .collect();
        if samples.iter().any(|value| !value.is_finite()) {
            return Err(VoiceError::new("Vocoder produced non-finite mono samples"));
        }
        RenderedAudio::new(
            SAMPLE_RATE,
            self.start_seconds + first as f64 / rate,
            ChannelLayout::Mono,
            samples,
        )
        .and_then(|audio|audio.with_components(Arc::clone(&self.primary),Arc::clone(&self.secondary),first))
        .map_err(|error| VoiceError::new(error.to_string()))
    }

    pub fn memory_bytes(&self) -> usize {
        std::mem::size_of_val(self.primary.as_ref())
            .saturating_add(std::mem::size_of_val(self.secondary.as_ref()))
    }
}
