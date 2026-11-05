use super::LyricFrontend;
use crate::{model::Language,synthesis::voice::{Result,VoiceError}};
use std::collections::HashMap;

type Key=(Language,String,bool,bool);
#[derive(Default)]
pub(super) struct LexicalCache {
    entries:HashMap<Key,Result<Vec<Vec<String>>>>,
    pub hits:u64,
    pub derived:u64,
}

impl LyricFrontend {
    pub(super) fn cached_lexical(&self,language:Language,lyrics:&str,european:bool,coda_l:bool,
        derive:impl FnOnce()->Result<Vec<Vec<String>>>)->Result<Vec<Vec<String>>> {
        let key=(language,lyrics.to_owned(),european,coda_l);
        let mut cache=self.lexical_cache.lock().map_err(|_|VoiceError::new("Pronunciation cache unavailable"))?;
        if let Some(result)=cache.entries.get(&key).cloned() {cache.hits+=1;return result}
        let result=derive();
        cache.derived+=1;
        if cache.entries.len()>=16384 {cache.entries.clear();}
        cache.entries.insert(key,result.clone());
        result
    }

    pub fn pronunciation_cache_metrics(&self)->Result<serde_json::Value> {
        let (derived,hits)={let lexical=self.lexical_cache.lock().map_err(|_|VoiceError::new("Pronunciation cache unavailable"))?;
            (lexical.derived,lexical.hits)};
        let units=self.pronunciation_cache.lock().map_err(|_|VoiceError::new("Pronunciation cache unavailable"))?;
        Ok(serde_json::json!({"lexical_derived":derived,"lexical_reused":hits,
            "notes_derived":units.derived,"notes_reused":units.reused}))
    }
}
