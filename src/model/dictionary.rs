use super::{GroupRef, Language, Project, Track};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PronunciationDictionary {
    pub language: Language,
    pub entries: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub xsampa_entries: BTreeSet<String>,
}

pub fn dictionary_name<'a>(track: &'a Track, reference: &'a GroupRef) -> &'a str {
    reference
        .extra
        .get("dictionary")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .or_else(|| {
            track
                .main_ref
                .extra
                .get("dictionary")
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or("")
}

impl Project {
    pub fn dictionary_for(
        &self,
        track: &Track,
        reference: &GroupRef,
    ) -> Option<&PronunciationDictionary> {
        self.dictionaries.get(dictionary_name(track, reference))
    }
}
