use crate::app::App;
use serde_json::{Value,json};
pub(super) fn selection(app:&App)->Value{
    json!({"selected":app.editor.selected.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "track":app.editor.track.to_string(),"reference":app.editor.reference.map(|v|v.to_string()),
        "groups":app.editor.selected_groups.iter().map(|(t,r)|[t.to_string(),r.to_string()]).collect::<Vec<_>>(),
        "pitchControls":app.piano_anchor.script_ids(),"points":crate::ui::lanes::script_points(app),
        "fileName":app.path.as_ref().map(|p|p.to_string_lossy()),
        "editing":app.editor.is_editing()||app.gesture.is_some()||app.group_gesture.is_some(),
        "revision":app.editor.revision.to_string(),"instrument":app.editor.current_track().is_instrument()})
}
pub(super) fn snapshot(app:&App)->Result<Value,String>{
    let mut value=selection(app);
    value["project"]=rynthv::scripting::snapshot::encode(&app.editor.project)?;
    value["language"]=json!(crate::i18n::language().key());
    value["os"]=json!(if cfg!(windows){"Windows"}else if cfg!(target_os="macos"){"macOS"}else{"Linux"});
    Ok(value)
}
pub(super) fn apply_selection(app:&mut App,value:&Value)->Result<(),String>{
    use rynthv::scripting::snapshot::read_id;
    if let Some(a)=value["notes"].as_array(){
        let ids=app.editor.notes().iter().map(|n|n.id).collect::<std::collections::BTreeSet<_>>();
        app.editor.selected=a.iter().filter_map(read_id).filter(|id|ids.contains(id)).collect();
    }
    if let Some(groups)=value["groups"].as_array(){
        app.editor.selected_groups=groups.iter().filter_map(|a|Some((read_id(a.get(0)?)?,read_id(a.get(1)?)?)))
            .filter(|(t,r)|app.editor.project.tracks.iter().any(|track|track.id==*t&&track.references().any(|reference|reference.id==*r))).collect();
    }
    if let Some(controls)=value["pitchControls"].as_array(){
        let mut valid=app.editor.group().map(|g|g.pitch_controls.iter().filter_map(|p|match p{crate::model::PitchControl::Point(p)=>Some(p.id.as_str()),crate::model::PitchControl::Curve(c)=>Some(c.id.as_str()),_=>None}).collect::<std::collections::BTreeSet<_>>()).unwrap_or_default();
        valid.extend(app.piano_anchor.script_candidate_ids());
        let ids=controls.iter().filter_map(Value::as_str).filter(|id|valid.contains(id)).map(str::to_owned).collect();app.piano_anchor.set_script_ids(ids);
    }
    if value["points"].is_object(){crate::ui::lanes::set_script_points(app,&value["points"])?;}
    Ok(())
}
