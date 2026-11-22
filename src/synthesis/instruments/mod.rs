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

pub fn validate_waveform(path: &std::path::Path, accept_any_v4_signature: bool) -> Result<(), String> {
    runtime::WaveFile::read(path.to_path_buf())?;
    native::open_waveform(path, accept_any_v4_signature).map(|_| ())
}

pub fn scan_directory(root: PathBuf, accept_any_v4_signature: bool) -> Result<Catalog, String> {
    let settings = InstrumentSettings {sources:vec![root],accept_any_v4_signature,..Default::default()};
    native::scan(&settings,std::sync::Arc::default())
}

pub fn preview(
    entry: &Entry, notes: Vec<serde_json::Value>, accept_any_v4_signature: bool,
) -> Result<Vec<f32>,String> {
    let settings = InstrumentSettings {accept_any_v4_signature,..Default::default()};
    let cancel: std::sync::Arc<std::sync::atomic::AtomicBool> = std::sync::Arc::default();
    let mut source = native::prepare(&settings,entry,notes,0,1,cancel.clone(),None)?;
    let microphones = microphones::Configuration::read(None,&entry.microphones,entry.room.as_deref())?
        .mix(&entry.microphones);
    let mut microphones = native::Microphones::from_path(&entry.main,&microphones,accept_any_v4_signature)?;
    let gain = 10f32.powf(0.45000001788139343);
    let mut pcm = Vec::new();
    let mut publish = |_:usize,block:Vec<f32>| {
        pcm.extend(block);
        Ok(())
    };
    loop {
        let complete = source.advance(cancel.clone(),&mut |_,block| {
            let mixed:Vec<[f32;4]> = block.chunks_exact(8).map(|v| std::array::from_fn(
                |c|(v[c]+v[4+c])*gain
            )).collect();
            microphones.feed_mixed(&mixed,false,&cancel,&mut publish)
        })?;
        if complete {break;}
    }
    microphones.feed_mixed(&[],true,&cancel,&mut publish)?;
    Ok(pcm)
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
    pub artwork: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnavailableRevision {
    pub selection: InstrumentSelection,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
    pub unavailable: Vec<UnavailableRevision>,
}
pub mod articulation;
mod vibrato;
