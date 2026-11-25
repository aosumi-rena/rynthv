use crate::model::InstrumentSelection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstrumentSettings {
    pub sources: Vec<PathBuf>,
    pub accept_any_v4_signature: bool,
    pub key_override_enabled: bool,
    pub key_override_value: u64,
    pub overrides: Vec<InstrumentKeyOverride>,
    pub waveform_override_enabled: bool,
    pub waveform_override: Option<WaveformSource>,
    pub trill_vibrato: bool,
    pub room_pack: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveformSource {
    File(PathBuf),
    Instrument(InstrumentSelection),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentKeyOverride {
    pub instrument: InstrumentSelection,
    pub enabled: bool,
    pub value: u64,
}
impl InstrumentSettings {
    pub fn room_pack(&self) -> Option<PathBuf> {
        self.room_pack.clone().or_else(super::rooms::installed_path)
    }
    pub fn key(&self, selection: &InstrumentSelection) -> u64 {
        self.overrides
            .iter()
            .find(|v| v.instrument == *selection && v.enabled)
            .map(|v| v.value)
            .or_else(|| self.key_override_enabled.then_some(self.key_override_value))
            .unwrap_or(0)
    }
    pub fn roots(&self) -> Vec<PathBuf> {
        if !self.sources.is_empty() {
            return self.sources.clone();
        }
        std::env::var_os("APPDATA")
            .map(|p| vec![PathBuf::from(p).join("Dreamtonics/Instrument X/databases")])
            .unwrap_or_default()
    }
}
