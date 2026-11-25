use crate::{app::App,model::Parameter};
use super::selection::Owner;
use serde_json::{Value,json};
use std::collections::{BTreeMap,BTreeSet};
pub(super) fn key(parameter:&Parameter)->String{
    if let Parameter::VocalMode(name)=parameter{format!("vocalMode_{name}")}else{parameter.key().into()}
}
fn parameter(key:&str)->Option<Parameter>{
    Parameter::ALL.into_iter().find(|p|p.key()==key).or_else(||key.strip_prefix("vocalMode_").map(|n|Parameter::VocalMode(n.into())))
}
pub fn script_points(app:&App)->Value{
    let mut result=serde_json::Map::new();
    if let Some(owner)=&app.parameter_lane.owner&&owner.track==app.editor.track&&Some(owner.reference)==app.editor.reference{
        for(key,points)in &app.parameter_lane.others{result.insert(key.clone(),json!(points));}
        result.insert(key(&owner.target),json!(app.parameter_lane.selected));
    }Value::Object(result)
}
pub fn set_script_points(app:&mut App,value:&Value)->Result<(),String>{
    let value=value.as_object().ok_or("Invalid automation selection")?;let mut points=BTreeMap::new();
    for(key,times)in value{
        let parameter=parameter(key).ok_or("Unknown selection parameter")?;
        let times=times.as_array().ok_or("Invalid point positions")?.iter().map(|v|v.as_i64().ok_or("Invalid point position")).collect::<Result<BTreeSet<_>,_>>()?;
        let valid=app.editor.automation(&parameter).map(|c|c.points.iter().map(|p|p.0).collect::<BTreeSet<_>>()).unwrap_or_default();
        let selected=times.intersection(&valid).copied().collect::<BTreeSet<_>>();
        if !selected.is_empty(){points.insert(key.clone(),selected);}
    }
    if points.is_empty(){app.parameter_lane.selected.clear();app.parameter_lane.others.clear();return Ok(());}
    let old=app.parameter_lane.owner.as_ref().filter(|o|o.track==app.editor.track&&Some(o.reference)==app.editor.reference&&points.contains_key(&key(&o.target)));
    let chosen=old.map(|o|key(&o.target)).unwrap_or_else(||points.keys().next().unwrap().clone());
    let target=parameter(&chosen).unwrap();
    let lane=app.view.lanes.iter().chain(std::iter::once(&app.view.instrument_lanes.curve)).find(|l|l.visible&&l.foreground==target).map_or(0,|l|l.id);
    let Some(reference)=app.editor.active_ref()else{return Err("No active note group".into());};
    let owner=Owner{lane,track:app.editor.track,reference:reference.id,group:reference.group_id.clone(),target};
    app.parameter_lane.owner=Some(owner);app.parameter_lane.selected=points.remove(&chosen).unwrap();app.parameter_lane.others=points;
    app.parameter_lane.anchor=app.parameter_lane.selected.first().copied();Ok(())
}
