use crate::model::{Project,Extra,EntityId};
use serde_json::{Value,json};
const ID:&str="__rynthvId";
pub fn encode(project:&Project)->Result<Value,String> {
    let mut value=serde_json::to_value(project).map_err(|e|e.to_string())?;
    for (track,row) in project.tracks.iter().zip(value["tracks"].as_array_mut().unwrap()) {
        row[ID]=json!(track.id);row["mainRef"][ID]=json!(track.main_ref.id);
        row["__audioTrack"]=json!(track.is_instrumental());
        if track.is_instrumental(){row["__audioSourceId"]=json!(track.id);}
        if let Some((begin,end))=track.audio_window(){row["__audioRange"]=json!([project.time.position_at(begin),project.time.position_at(end)]);}
        for (reference,value) in track.groups.iter().zip(row["groups"].as_array_mut().unwrap()){value[ID]=json!(reference.id);}
        for (note,value) in track.main_group.notes.iter().zip(row["mainGroup"]["notes"].as_array_mut().unwrap()){value[ID]=json!(note.id);}
    }
    for (group,row) in project.library.iter().zip(value["library"].as_array_mut().unwrap()) {
        for (note,value) in group.notes.iter().zip(row["notes"].as_array_mut().unwrap()){value[ID]=json!(note.id);}
    }
    Ok(value)
}
fn restore(id:&mut EntityId,extra:&mut Extra,seen:&mut std::collections::BTreeSet<EntityId>)->Result<(),String> {
    if let Some(value)=extra.remove(ID){*id=value.as_u64().filter(|v|*v>0).ok_or("Invalid script object identity")?;}
    if !seen.insert(*id){return Err("Duplicate script object identity".into());}Ok(())
}
fn identity(value:&Value)->Option<String> {
    value.get(ID).and_then(Value::as_u64).map(|id|format!("id:{id}"))
        .or_else(||value.get("uuid").and_then(Value::as_str).map(|id|format!("uuid:{id}")))
}
fn preserve_numbers(value:&mut Value,original:&Value) {
    match (value,original) {
        (Value::Number(value),Value::Number(original)) if value.as_f64()==original.as_f64()=>*value=original.clone(),
        (Value::Object(value),Value::Object(original))=>{
            for (key,value) in value {if let Some(old)=original.get(key){preserve_numbers(value,old);}}
        },
        (Value::Array(value),Value::Array(original))=>{
            let identities:std::collections::BTreeMap<_,_>=original.iter().filter_map(|old|identity(old).map(|id|(id,old))).collect();
            for (index,value) in value.iter_mut().enumerate() {
                let old=match identity(value){Some(id)=>identities.get(&id).copied(),None=>original.get(index)};
                if let Some(old)=old {preserve_numbers(value,old);}
            }
        },_=>(),
    }
}
pub fn decode(mut value:Value,original:&Project)->Result<Project,String> {
    preserve_numbers(&mut value,&encode(original)?);
    let mut project:Project=serde_json::from_value(value).map_err(|e|e.to_string())?;
    if project.uuid!=original.uuid{return Err("Script changed the project identity".into());}
    let mut seen=std::collections::BTreeSet::new();
    for track in &mut project.tracks {
        restore(&mut track.id,&mut track.extra,&mut seen)?;
        track.extra.remove("__audioTrack");track.extra.remove("__audioRange");
        let source=track.extra.remove("__audioSourceId").and_then(|v|v.as_u64()).unwrap_or(track.id);
        track.instrumental=original.tracks.iter().find(|t|t.id==source).and_then(|t|t.instrumental.clone());
        for reference in std::iter::once(&mut track.main_ref).chain(&mut track.groups){restore(&mut reference.id,&mut reference.extra,&mut seen)?;}
    }
    for group in project.library.iter_mut().chain(project.tracks.iter_mut().map(|t|&mut t.main_group)) {
        for note in &mut group.notes{restore(&mut note.id,&mut note.extra,&mut seen)?;}
        group.notes.sort_by_key(|n|n.onset);
    }
    crate::storage::validate(&project)?;Ok(project)
}
