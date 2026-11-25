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
    let explicit = super::current(app).is_some_and(|entry| entry.explicit_vibrato);
    let allow_trill = app.synthesis.library.settings.instruments.trill_vibrato;
    let reference = app.editor.reference.unwrap_or(app.editor.current_track().main_ref.id);
    let selected = app.editor.notes().iter().find(|n| app.editor.selected.contains(&n.id));
    let has_trill = app.editor.notes().iter().filter(|n| app.editor.selected.contains(&n.id))
        .any(|n| super::smart::articulations(app, reference, n).iter().any(|a| a.starts_with("Trill ")));
    let enabled = selected.is_some() && !app.editor.is_editing() && (!has_trill || allow_trill);
    let prepared = selected.and_then(|n| super::smart::prepared(app, reference, n.id));
    let controls = if explicit {
        vec![("Depth", "dF0VbrMod", 0., 2., 0.01, 1., ""),
            ("Frequency", "fF0Vbr", 4., 8., 0.05, prepared.and_then(|n| n.vibrato_hz).map_or(5., f64::from), " Hz"),
            ("Phase", "pF0Vbr", -1., 1., 0.1, prepared.and_then(|n| n.vibrato_phase).map_or(0., f64::from), "")]
    } else {
        vec![("Depth", "dF0VbrMod", 0., 2., 0.01, 1., ""),
            ("Frequency", "fF0VbrMod", -1., 1., 0.01, 0., "")]
    };
    let controls: Vec<_> = controls.into_iter().map(|(label,key,min,max,step,default,suffix)| {
        let value = selected.and_then(|n| n.attributes.get(key)).and_then(serde_json::Value::as_f64).unwrap_or(default);
        (label,key,min,max,step,default,value,suffix)
    }).collect();
    ui.add_enabled_ui(enabled, |ui| {
        for (label, key, min, max, step, default, value, suffix) in controls {
            let id = ui.make_persistent_id(("instrument-vibrato", app.editor.reference,
                app.editor.revision, key));
            let appearance=crate::ui::slider::Appearance::Instrument(crate::ui::track_color::TrackColors::new(app.editor.current_track()).accent);
            if let Some(change) = crate::ui::slider::Slider::new(id,crate::i18n::t(label),value,min,max,default)
                .step(step).suffix(suffix).appearance(appearance).show(ui).change {
                edit_notes(app, crate::i18n::t("Vibrato"), |n| {
                    if change.reset {n.attributes.remove(key);}
                    else {n.attributes.insert(key.into(), serde_json::json!(change.value));}
                });
            }
        }
    });
}
