//! Note-local timeline admission shared by neural and standard synthesis.
use std::collections::BTreeMap;
use super::OwnerInput;

pub fn timeline(rows:impl IntoIterator<Item=(u64,i64,i64,i32,bool)>)->BTreeMap<u64,String> {
    let mut errors=BTreeMap::new();let mut active=Vec::new();
    let mut rows=rows.into_iter().filter(|r|!r.4).collect::<Vec<_>>();
    rows.sort_by_key(|r|(r.1,r.0));
    for (id,start,duration,pitch,_) in rows {
        if duration<=0 || start.checked_add(duration).is_none() {
            errors.insert(id,"音符时长无效".into());continue;
        }
        if !(0..=127).contains(&pitch) {errors.insert(id,"音符音高超出范围".into());continue;}
        active.retain(|&(_,end)|end>start);
        if !active.is_empty() {
            errors.insert(id,"音符重叠，请调整受影响的音符".into());
            for &(other,_) in &active {errors.insert(other,"音符重叠，请调整受影响的音符".into());}
        }
        active.push((id,start+duration));
    }
    errors
}

pub fn for_input(input:&OwnerInput)->BTreeMap<u64,String> {
    timeline(input.notes.iter().map(|n|(n.id,n.onset.0,n.duration,
        n.pitch.saturating_add(input.pitch_offset),
        n.attributes.get("muted").and_then(serde_json::Value::as_bool)==Some(true))))
}

pub fn muted(input:&OwnerInput,errors:&BTreeMap<u64,String>)->OwnerInput {
    let mut clean=input.clone();
    for note in &mut clean.notes {
        if errors.contains_key(&note.id) {note.attributes.insert("muted".into(),true.into());}
    }
    clean
}
