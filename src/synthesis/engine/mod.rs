mod acoustics;
mod components;
mod ensemble;
mod ensemble_pitch;
pub mod incremental;
mod models;
mod pitch;
mod prepared;
pub mod progressive;
mod render;
pub mod experimental;
mod acoustic_mix;

pub use acoustics::{
    AcousticRender, PreparedRenderAcoustics, StageTimings, prepare_acoustics, synthesize_acoustics,
};
pub use components::PcmComponents;
pub use incremental::synthesize_owner;
pub use prepared::{OwnerSynthesis, PreparedAcoustics, PreparedModelFeatures};
pub use render::{RenderResult, synthesize, synthesize_host_pitch};
pub use pitch::DdsPitchClock;

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum RenderStage {
    Duration,
    Pitch,
    Timbre,
    Vocoder,
}

impl RenderStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Duration => "音素时长",
            Self::Pitch => "音高",
            Self::Timbre => "音色",
            Self::Vocoder => "声码器",
        }
    }
}
