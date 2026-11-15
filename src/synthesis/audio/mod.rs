pub(crate) mod conversion;
mod export;
pub mod live;
pub mod metronome;
mod piano;
mod playback;
mod source;
mod wav;

pub use live::LiveAudio;
pub use export::AudioFileFormat;
pub use piano::PianoControl;
pub(crate) use playback::LoopControl;
pub use playback::{OutputFormat, PlaybackControl, Player};
pub use source::{AudioLayer, PlaybackSource};
pub(crate) use source::MorphLayer;
pub use wav::{WaveFormat, save_wav, save_wav_as};

use std::{fmt, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelLayout {
    Mono,
    Stereo,
}

impl ChannelLayout {
    pub fn channels(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderedAudio {
    sample_rate: u32,
    start_seconds: f64,
    layout: ChannelLayout,
    samples: Arc<[f32]>,
    components: Option<ComponentSamples>,
}

#[derive(Clone, Debug)]
struct ComponentSamples { primary: Arc<[f32]>, aspiration: Arc<[f32]>, offset: usize }

impl RenderedAudio {
    pub fn new(
        sample_rate: u32,
        start_seconds: f64,
        layout: ChannelLayout,
        samples: Vec<f32>,
    ) -> Result<Self, AudioError> {
        if !(1..=384_000).contains(&sample_rate) {
            return Err(AudioError::InvalidPcm(
                "Sample rate must be 1..=384000 Hz".into(),
            ));
        }
        if !start_seconds.is_finite() || start_seconds < 0.0 {
            return Err(AudioError::InvalidPcm(
                "PCM start must be finite and nonnegative".into(),
            ));
        }
        if samples.is_empty() || !samples.len().is_multiple_of(layout.channels()) {
            return Err(AudioError::InvalidPcm(
                "PCM must contain complete, nonempty interleaved frames".into(),
            ));
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(AudioError::InvalidPcm(
                "Rendered PCM contains a nonfinite sample".into(),
            ));
        }
        let frames = samples.len() / layout.channels();
        if !(start_seconds + frames as f64 / f64::from(sample_rate)).is_finite() {
            return Err(AudioError::InvalidPcm("PCM end time is not finite".into()));
        }
        Ok(Self {
            sample_rate,
            start_seconds,
            layout,
            samples: samples.into(),
            components: None,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn start_seconds(&self) -> f64 {
        self.start_seconds
    }
    pub fn end_seconds(&self) -> f64 {
        self.start_seconds + self.duration_seconds()
    }
    pub fn layout(&self) -> ChannelLayout {
        self.layout
    }
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
    pub fn frames(&self) -> usize {
        self.samples.len() / self.layout.channels()
    }
    pub fn duration_seconds(&self) -> f64 {
        self.frames() as f64 / f64::from(self.sample_rate)
    }

    pub(crate) fn with_components(mut self, primary:Arc<[f32]>, aspiration:Arc<[f32]>, offset:usize) -> Result<Self,AudioError> {
        if self.layout!=ChannelLayout::Mono || primary.len()!=aspiration.len()
            || offset.checked_add(self.frames()).is_none_or(|end|end>primary.len()) {
            return Err(AudioError::InvalidPcm("Invalid primary/aspiration slice".into()));
        }
        self.components=Some(ComponentSamples {primary,aspiration,offset});Ok(self)
    }

    pub(crate) fn voice_sample(&self, frame:usize)->[f32;2] {
        if let Some(c)=&self.components {[c.primary[c.offset+frame],c.aspiration[c.offset+frame]]}
        else {[self.samples[frame*self.layout.channels()],0.0]}
    }
}

#[derive(Debug)]
pub enum AudioError {
    InvalidPcm(String),
    InvalidControl(String),
    Wave(String),
    Device(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, detail) = match self {
            Self::InvalidPcm(detail) => ("Invalid PCM", detail),
            Self::InvalidControl(detail) => ("Invalid playback control", detail),
            Self::Wave(detail) => ("WAV export", detail),
            Self::Device(detail) => ("Audio output device", detail),
        };
        write!(f, "{kind}: {detail}")
    }
}

impl std::error::Error for AudioError {}
