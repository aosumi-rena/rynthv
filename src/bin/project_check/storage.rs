use rynthv::{model::Project,storage};
use serde_json::{Value,json};
use std::path::Path;

pub(super) fn tracks(project:&Project)->Value {
    json!(project.tracks.iter().map(|track|json!({
        "name":track.name,"instrumental":track.is_instrumental(),"instrument":track.instrument,
        "groups":track.references().filter_map(|r|project.group(&r.group_id).map(|g|json!({"name":g.name,"notes":g.notes.len(),"offset":r.blick_offset,"begin":r.blick_absolute_begin,"end":r.blick_absolute_end,"database":r.database,"pitch_curves":g.pitch_controls.len(),"parameters":g.parameters}))).collect::<Vec<_>>(),
        "clip":track.instrumental.as_ref().and_then(|a|a.clip.as_ref()).map(|clip|json!({
            "path":clip.path,"onset_seconds":clip.onset_seconds,"duration_seconds":clip.duration_seconds,
            "window":clip.window(),"trim_begin_seconds":clip.trim_begin_seconds,"trim_end_seconds":clip.trim_end_seconds,
        }))
    })).collect::<Vec<_>>())
}
pub(super) fn roundtrip(source:&Path,target:&Path)->Result<Value,Box<dyn std::error::Error>> {
    if target.exists() {return Err("Storage diagnostic refuses to replace an existing file".into());}
    let before=storage::load(source)?;
    storage::save(target,&before)?;
    let after=storage::load(target)?;
    Ok(json!({"source":source,"saved":target,"before":tracks(&before),"after":tracks(&after),
        "source_file_unchanged":true,"source_group_count":before.library.len(),"saved_group_count":after.library.len(),
        "source_render_config":before.render_config,"saved_render_config":after.render_config}))
}

pub(super) fn bindings(databases:&Path,source:&Path)->Result<Value,Box<dyn std::error::Error>> {
    let project=storage::load(source)?;
    let catalog=rynthv::synthesis::voice::VoiceCatalog::scan(databases)?;
    let mut bindings=Vec::new();
    for track in project.tracks.iter().filter(|t|!t.is_instrumental()) {
        for reference in track.visible_references() {
            let selector=rynthv::model::effective_singer_database(&track.main_ref.database,&reference.database);
            let selected=match catalog.resolve(&track.main_ref.database,&reference.database) {
                Ok(Some(selection))=>json!({"database_id":selection.database_id,"revision":selection.revision.to_string()}),
                Ok(None)=>Value::Null,
                Err(error)=>json!({"error":error}),
            };
            bindings.push(json!({"track":track.name,"group":reference.group_id,"selector":selector,"selected":selected}));
        }
    }
    Ok(json!({"source":source,"render_config":project.render_config,"voice_bindings":bindings,"tracks":tracks(&project)}))
}
