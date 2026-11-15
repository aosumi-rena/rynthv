use serde::{Deserialize,Serialize};
use serde_json::Value;
use std::{collections::BTreeMap,path::Path};

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Preset { pub signature:String, pub value:Value }
#[derive(Clone,Default,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Store { pub scopes:BTreeMap<String,BTreeMap<String,Preset>> }
impl Store {
    pub fn load(path:&Path)->Result<Self,String> {
        let file=match std::fs::File::open(path){Ok(v)=>v,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Self::default()),Err(e)=>return Err(e.to_string())};
        if file.metadata().map_err(|e|e.to_string())?.len()>2*1024*1024{return Err("Preset library exceeds 2 MiB".into());}
        serde_json::from_reader(file).map_err(|e|format!("Invalid preset library: {e}"))
    }
    pub fn save(&self,path:&Path)->Result<(),String> {
        let bytes=serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?;
        if bytes.len()>2*1024*1024{return Err("Preset library exceeds 2 MiB".into());}
        if let Some(parent)=path.parent(){std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
        crate::storage::atomic_write(path,&bytes)
    }
}
