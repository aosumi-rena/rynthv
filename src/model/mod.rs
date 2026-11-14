mod automation;
mod dictionary;
mod instrumental;
mod language;
mod phoneme_catalog;
mod performance;
pub mod pitch;
mod project;
mod retakes;
mod time;
mod track_color;
mod vocal_modes;

pub use automation::*;
pub use dictionary::*;
pub use instrumental::*;
pub use language::*;
pub use phoneme_catalog::*;
pub use performance::*;
pub use pitch::{PitchControl, PitchCurve, PitchPoint};
pub use project::*;
pub use retakes::*;
pub use time::*;
pub use track_color::*;
pub use vocal_modes::*;

use std::sync::atomic::{AtomicU64, Ordering};
pub type EntityId = u64;
pub const SVP_VERSION: i32 = 203;
pub const RVP_VERSION: i32 = 1;
pub fn new_id() -> EntityId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
pub type Extra = std::collections::BTreeMap<String, serde_json::Value>;

pub(crate) mod shared_json {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::sync::Arc;

    pub fn serialize<S: serde::Serializer>(value: &Arc<Value>, writer: S) -> Result<S::Ok, S::Error> {
        value.as_ref().serialize(writer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(reader: D) -> Result<Arc<Value>, D::Error> {
        Value::deserialize(reader).map(Arc::new)
    }
}
mod singer;
pub use singer::{singer_database,effective_singer_database};
