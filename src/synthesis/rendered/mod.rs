pub mod patch;
mod pitch;
mod phonemes;
mod snapshot;
mod waveform;

pub use pitch::{PitchTrack, RenderedRapControl};
pub use phonemes::{PhonemeTrack,RenderedPhone,PhonemeSnapshot};
pub use snapshot::{VisualFlash, VisualPatch, VisualPiece, VisualSnapshot, VisualTimeline};
pub use waveform::Waveform;

use super::{audio::RenderedAudio, engine::OwnerSynthesis};
use std::sync::Arc;

#[derive(Debug)]
pub struct RenderedClip {
    pub audio: Arc<RenderedAudio>,
    pub visuals: Arc<RenderVisuals>,
    pub state: Option<Arc<OwnerSynthesis>>,
}

impl RenderedClip {

    pub fn memory_bytes(&self) -> usize {
        std::mem::size_of_val(self.audio.samples())
            .saturating_add(self.visuals.memory_bytes())
            .saturating_add(self.state.as_ref().map_or(0,|state|state.memory_bytes()))
    }
}

#[derive(Debug)]
pub struct RenderVisuals {
    pub pitch: PitchTrack,
    pub waveform: Waveform,
    pub phonemes:PhonemeTrack,
}

impl RenderVisuals {

    pub fn memory_bytes(&self) -> usize {
        self.pitch
            .memory_bytes()
            .saturating_add(self.waveform.memory_bytes())
            .saturating_add(self.phonemes.memory_bytes())
    }
}
