use super::{Entry,InstrumentSettings,native};
use crate::model::{EntityId,Project,GroupRef};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::{collections::BTreeSet,sync::{Arc,atomic::AtomicBool}};

#[derive(Clone,Copy,Debug,Deserialize,Serialize)]
pub enum Kind {Articulation,Dynamics,Both}
impl Kind {
    pub fn articulation(self)->bool{matches!(self,Self::Articulation|Self::Both)}
    pub fn dynamics(self)->bool{matches!(self,Self::Dynamics|Self::Both)}
}
#[derive(Deserialize,Serialize)]
pub struct Request {
    pub entry:Entry,pub notes:Vec<Value>,pub ids:Vec<EntityId>,pub selected:Vec<bool>,pub kind:Kind,
}
#[derive(Debug,Serialize)]
pub struct Edit {pub id:EntityId,pub articulations:Option<Vec<String>>,pub dynamic:Option<f32>}

pub fn prepare(project:&Project,reference:&GroupRef,entry:Entry,selected:&BTreeSet<EntityId>,kind:Kind)->Result<Request,String> {
    let group=project.group(&reference.group_id).ok_or("Instrument note group is absent")?;
    let transpose=super::performance::transpose(reference)?;
    let mut notes=Vec::new();
    for note in group.notes.iter().filter(|n|!n.muted()) {
        let onset=note.onset+reference.blick_offset;
        let end=onset+note.duration;
        if end<=reference.blick_absolute_begin||onset>=reference.visible_end(){continue;}
        let start=(project.time.seconds_at(onset)*200.0).round() as i64;
        let length=(project.time.seconds_at(end)*200.0).round() as i64-start;
        let chosen=selected.contains(&note.id);
        let seed=||uuid::Uuid::new_v4().as_u128() as u32;
        let input=json!({"start":start,"length":length.max(1),"linked_before":false,"linked_after":false,
            "pitch":(note.pitch+reference.pitch_offset) as f64+note.extra.get("detune").and_then(Value::as_f64).unwrap_or(0.)/100.+transpose,
            "direction":reference.extra.get("voice").and_then(|v|v.get("performanceDirection")).cloned().unwrap_or(json!("")),
            "articulations":note.attributes.get("articulations").cloned().unwrap_or(json!([])),
            "dynamic":note.attributes.get("dynamic").cloned().unwrap_or(json!(0.5)),
            "articulation_seed":if chosen&&kind.articulation(){seed()}else{0},
            "dynamics_seed":if chosen&&kind.dynamics(){seed()}else{0}});
        notes.push((note.id,chosen,input));
    }
    notes.sort_by_key(|n|n.2["start"].as_i64().unwrap());
    let first=notes.first().ok_or("No instrument notes to rewrite")?.2["start"].as_i64().unwrap();
    for (_,_,input) in &mut notes {input["start"]=json!(input["start"].as_i64().unwrap()-first+100);}
    Ok(Request {entry,ids:notes.iter().map(|n|n.0).collect(),selected:notes.iter().map(|n|n.1).collect(),
        notes:notes.into_iter().map(|n|n.2).collect(),kind})
}

pub fn execute(settings:&InstrumentSettings,request:Request,cancel:Arc<AtomicBool>)->Result<Vec<Edit>,String> {
    if request.ids.len()!=request.notes.len()||request.ids.iter().collect::<BTreeSet<_>>().len()!=request.ids.len(){return Err("Instrument rewrite note identities do not match".into());}
    let notes=request.notes.into_iter().map(serde_json::from_value).collect::<Result<Vec<_>,_>>()
        .map_err(|e|format!("Instrument rewrite score: {e}"))?;
    native::rewrite::predict(settings,&request.entry,notes,&request.selected,
        request.kind.articulation(),request.kind.dynamics(),cancel).map(|predictions|predictions.into_iter()
            .map(|p|Edit{id:request.ids[p.index],articulations:p.articulations,dynamic:p.dynamic}).collect())
}
