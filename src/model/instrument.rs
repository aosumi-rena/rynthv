use crate::synthesis::voice::VoiceRevision;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentTrack {
    #[serde(default)]
    pub selection: Option<InstrumentSelection>,
    #[serde(default)]
    pub ambient_seed: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentSelection {
    pub database_id: String,
    pub revision: VoiceRevision,
}
