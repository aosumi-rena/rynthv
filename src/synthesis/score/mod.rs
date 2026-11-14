mod context;
mod automation;
mod lyrics;
mod options;
mod owner;
mod model_phones;
mod performance;
mod pitch;
mod saved_pitch;
mod pronunciation;
mod records;
pub mod phonemes;
mod retakes;
mod types;
mod voice_binding;

pub use lyrics::LyricFrontend;
pub use owner::prepare_owner;
pub use performance::PreparedPerformanceControls;
pub use pitch::PreparedPitchControls;
pub use automation::PreparedAutomation;
pub use types::{ExplicitScore, PreparedPhrase, ScoreNote, ScorePhone,PhoneSource};
pub use voice_binding::validate_voice;

pub const SAMPLE_RATE: u32 = 48_000;
pub const HOP_SAMPLES: usize = 240;
pub const FRAME_SECONDS: f64 = HOP_SAMPLES as f64 / SAMPLE_RATE as f64;
