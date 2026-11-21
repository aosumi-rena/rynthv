mod backend;
mod native;
mod runtime;
mod mixing;
mod performance;
pub use performance::transpose as tuning_transposition;
pub fn players(reference:&crate::model::GroupRef)->Result<usize,String> {
    reference.extra.get("voice").and_then(|v|v.get("choirNumStems"))
        .map_or(Ok(1),|v|v.as_u64().filter(|n|(1..=128).contains(n))
            .map(|n|n as usize).ok_or_else(||"Invalid instrument player count".into()))
}
pub mod microphones;
pub mod direction;
pub mod rewrite;
mod settings;
use crate::model::InstrumentSelection;
pub use runtime::Instruments;
use serde::{Deserialize, Serialize};
pub use settings::*;
use std::path::PathBuf;

pub fn validate_waveform(path: &std::path::Path) -> Result<(), String> {
    runtime::WaveFile::read(path.to_path_buf())?;
    native::open_waveform(path).map(|_| ())
}

#[derive(Clone, Debug, Serialize)]
pub struct EffectiveNote {
    pub id: crate::model::EntityId,
    pub dynamic: f32,
    pub articulations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub selection: InstrumentSelection,
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    pub main: PathBuf,
    pub auxiliary: Option<PathBuf>,
    pub pitch_range: Option<[i32; 2]>,
    pub microphones: Vec<String>,
    pub identity: String,
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub artwork: Option<Vec<u8>>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}
pub mod articulation;
mod vibrato;
