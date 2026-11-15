use crate::{app::App, model::Note};
use eframe::egui::*;

pub(super) fn direction(app:&mut App,ui:&mut Ui) {
    use crate::synthesis::instruments::direction::ITEMS;
    let reference=app.editor.active_ref().unwrap_or(&app.editor.current_track().main_ref);
    let id=reference.id;
    let mut selected=reference.extra.get("voice").and_then(|v|v.get("performanceDirection")).and_then(serde_json::Value::as_str).unwrap_or("").to_owned();
    let before=selected.clone();
    ComboBox::from_id_salt("instrument-direction").width(ui.available_width())
        .selected_text(crate::i18n::t(ITEMS.iter().find(|v|v.0==selected).map_or(selected.as_str(),|v|v.1)))
        .show_ui(ui,|ui|{for (key,label) in ITEMS {ui.selectable_value(&mut selected,key.to_owned(),crate::i18n::t(label));}});
    if selected!=before && !app.editor.is_editing() {
        app.editor.begin("Performance Direction");
        let track=app.editor.project.tracks.iter_mut().find(|t|t.id==app.editor.track).unwrap();
        if let Some(reference)=std::iter::once(&mut track.main_ref).chain(&mut track.groups).find(|r|r.id==id) {
            let voice=reference.extra.entry("voice".into()).or_insert_with(||serde_json::json!({}));
            if let Some(voice)=voice.as_object_mut(){
                if selected.is_empty(){voice.remove("performanceDirection");}else{voice.insert("performanceDirection".into(),serde_json::json!(selected));}
            }
        }
        app.editor.finish();
    }
}

pub(super) fn tuning(app:&mut App,ui:&mut Ui) {
    use crate::synthesis::parameters::ParameterSpec;
    ui.columns(2,|columns| {
        for (column,spec) in columns.iter_mut().zip([
            ParameterSpec {key:"transposeCents",label:"Pitch Shift",limit:50.,step:1.,unit:"cents"},
            ParameterSpec {key:"transposeSemitones",label:"Key Shift",limit:12.,step:1.,unit:"smt."},
        ]) {
            column.label(crate::i18n::t(spec.label));
            if let Ok((reference,mut value))=app.editor.voice_parameter(spec) {
                let response=column.add_sized(vec2(column.available_width(),28.),
                    DragValue::new(&mut value).speed(spec.step).range(-spec.limit..=spec.limit).max_decimals(0).suffix(format!(" {}",spec.unit)));
                if response.changed() && let Err(error)=app.editor.set_voice_parameter(reference,spec,value) {app.error=Some(error);}
            }
        }
    });
}

pub(super) fn edit_notes(app: &mut App, label: &str, mut change: impl FnMut(&mut Note)) {
    if app.editor.is_editing() || app.editor.selected.is_empty() { return; }
    app.editor.begin(label);
    let selected = app.editor.selected.clone();
    if let Some(group) = app.editor.group_mut() {
        for note in &mut group.notes {
            if selected.contains(&note.id) { change(note); }
        }
    }
    app.editor.finish();
}

pub(super) fn vibrato(app: &mut App, ui: &mut Ui) {
    let selected = app.editor.notes().iter().find(|n| app.editor.selected.contains(&n.id));
    let enabled = selected.is_some() && !app.editor.is_editing();
    let values: [f64; 2] = std::array::from_fn(|i| selected
        .and_then(|n| n.attributes.get(["dF0VbrMod", "fF0VbrMod"][i]))
        .and_then(serde_json::Value::as_f64).unwrap_or([1.0, 0.0][i]));
    ui.add_enabled_ui(enabled, |ui| {
        for (i, label, key, min, max) in [(0, "Depth", "dF0VbrMod", 0.0, 2.0),
            (1, "Frequency", "fF0VbrMod", -1.0, 1.0)] {
            let id = ui.make_persistent_id(("instrument-vibrato", app.editor.reference,
                app.editor.revision, key));
            if let Some(value) = crate::ui::voice_library::parameter_slider(ui, id,
                crate::i18n::t(label), values[i], min, max, 0.01) {
                edit_notes(app, crate::i18n::t("Vibrato"), |n| {
                    n.attributes.insert(key.into(), serde_json::json!(value));
                });
            }
        }
    });
}
