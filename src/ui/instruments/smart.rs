use crate::{app::App,model::{EntityId,Note}};

pub fn automatic(note:&Note)->bool {
    note.attributes.get("articulationsFixed").and_then(serde_json::Value::as_bool)==Some(false)
}
pub fn prepared(app:&App,reference:EntityId,id:EntityId)->Option<&crate::synthesis::instruments::EffectiveNote> {
    app.synthesis.instruments.outputs.get(&reference)?.controls.get(&id)
}
pub fn dynamic(app:&App,reference:EntityId,note:&Note)->f32 {
    note.attributes.get("dynamic").and_then(serde_json::Value::as_f64)
        .map(|v|v as f32).or_else(||prepared(app,reference,note.id).map(|v|v.dynamic))
        .unwrap_or(0.5).clamp(0.,1.)
}
pub fn articulations(app:&App,reference:EntityId,note:&Note)->Vec<String> {
    if automatic(note) && let Some(value)=prepared(app,reference,note.id){return value.articulations.clone();}
    note.attributes.get("articulations").and_then(serde_json::Value::as_array)
        .into_iter().flatten().filter_map(|v|v.as_str().map(str::to_owned)).collect()
}
pub fn chosen(app:&App)->bool {
    let mut notes=app.editor.notes().iter().filter(|n|app.editor.selected.contains(&n.id)).peekable();
    notes.peek().is_some()&&notes.all(automatic)
}
pub fn can_toggle(app:&App)->bool {
    !app.editor.is_editing()&&!app.editor.selected.is_empty()&&(!chosen(app)||
        app.editor.reference.is_some_and(|r|app.editor.selected.iter().all(|id|prepared(app,r,*id).is_some())))
}
pub fn toggle(app:&mut App) {
    if !can_toggle(app){return;}
    let disable=chosen(app);
    let cached=app.editor.reference.and_then(|r|app.synthesis.instruments.outputs.get(&r))
        .map(|o|o.controls.clone()).unwrap_or_default();
    super::controls::edit_notes(app,crate::i18n::t("Smart Articulations"),|note|{
        if disable {
            if let Some(value)=cached.get(&note.id) {
                note.attributes.insert("articulations".into(),serde_json::json!(value.articulations));
                note.attributes.entry("dynamic".into()).or_insert(serde_json::json!(value.dynamic));
            }
        } else {
            note.attributes.remove("articulations");note.attributes.remove("dynamic");
        }
        note.attributes.insert("articulationsFixed".into(),disable.into());
    });
}
