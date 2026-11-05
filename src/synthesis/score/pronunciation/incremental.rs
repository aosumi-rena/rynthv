mod dependencies;
mod key;
use super::{ResolvedNote,LyricFrontend};
use crate::{model::Language,synthesis::{plan::{NoteInput,OwnerInput},voice::{PreparedTree,VoiceFamily,Result,VoiceError}}};
use std::collections::HashMap;

type Id=(usize,String,u64);
struct Entry {key:key::Key,rows:Vec<ResolvedNote>,used:u64}
#[derive(Default)]
pub(in crate::synthesis::score) struct Cache {
    entries:HashMap<Id,Entry>,
    generation:u64,
    pub derived:u64,
    pub reused:u64,
}

pub(super) fn resolve(notes:&[&NoteInput],inherited:Language,input:&OwnerInput,frontend:&LyricFrontend,
    tree:&PreparedTree,family:VoiceFamily)->Result<Vec<ResolvedNote>> {
    if notes.is_empty() {return Ok(Vec::new())}
    let settings=key::Settings::new(input,inherited,family);
    let languages=notes.iter().map(|note|frontend.note_language(note,inherited)).collect::<Vec<_>>();
    let local=languages.iter().map(|value|value.as_ref().copied().unwrap_or(inherited)).collect::<Vec<_>>();
    let joined=super::hyphens::join(notes,&local);
    let units=dependencies::units(notes,&joined,&local,frontend,tree,input,settings.european)?;
    let eligible=units.iter().map(|unit|dependencies::contextual(unit,&joined,&local,frontend,tree,settings.european)).collect::<Vec<_>>();
    let mut cache=frontend.pronunciation_cache.lock().map_err(|_|VoiceError::new("Pronunciation cache unavailable"))?;
    cache.generation=cache.generation.wrapping_add(1);
    let generation=cache.generation;
    let mut result=Vec::with_capacity(notes.len());
    for (index,unit) in units.iter().enumerate() {
        let window=dependencies::context(index,&units,notes,&local,&eligible,settings.relaxed);
        let key=key::Key::new(&notes[window.clone()],&joined[window.clone()],&languages[window.clone()],
            input,inherited,settings,unit.start-window.start,unit.len());
        let id=(tree as *const PreparedTree as usize,input.group_id.clone(),notes[unit.start].id);
        if let Some(entry)=cache.entries.get_mut(&id).filter(|entry|entry.key==key) {
            entry.used=generation;
            result.extend_from_slice(&entry.rows);
            cache.reused+=unit.len() as u64;
            continue;
        }
        let rows=super::resolve_uncached(&notes[window.clone()],inherited,input,frontend,tree,family)?;
        let rows=rows[unit.start-window.start..unit.end-window.start].to_vec();
        cache.derived+=rows.len() as u64;
        result.extend_from_slice(&rows);
        cache.entries.insert(id,Entry{key,rows,used:generation});
    }
    if cache.entries.len()>16384 {
        let mut ages=cache.entries.iter().map(|(id,entry)|(entry.used,id.clone())).collect::<Vec<_>>();
        ages.sort_unstable();
        for (_,id) in ages.into_iter().take(cache.entries.len()-16384) {cache.entries.remove(&id);}
    }
    Ok(result)
}
