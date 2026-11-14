use super::{Scale, Tonic};
use crate::{
    app::App,
    ui::theme::{BORDER, MUTED},
};
use eframe::egui::*;
use serde_json::{Value, json};

pub fn toolbar(app: &mut App, ui: &mut Ui) {
    ui.add_space(15.0);
    ui.label(RichText::new(crate::i18n::t("调式音阶")).color(MUTED));
    ui.add_space(8.0);
    ui.push_id("scale-toolbar", |ui| controls(app, ui, false));
}

pub fn panel(app: &mut App, ui: &mut Ui) {
    ui.push_id("voice-scale", |ui| {
        ui.spacing_mut().interact_size.y = 28.0;
        ui.visuals_mut().widgets.inactive.bg_fill = BORDER;
        ui.visuals_mut().widgets.inactive.corner_radius = CornerRadius::same(4);
        controls(app, ui, true);
    });
}

fn controls(app: &mut App, ui: &mut Ui, vertical: bool) {
    let current = selection(app);
    let mut tonic = None;
    let mut scale = None;
    ui.add_enabled_ui(current.editable, |ui| {
        let width = ui.available_width();
        tonic = tonic_selector(ui, &current, if vertical { width } else { 52.0 });
        if !vertical {
            ui.add_space(3.0);
        }
        scale = scale_selector(ui, &current, if vertical { width } else { 146.0 });
    });
    if let Some(tonic) = tonic {
        update(app, "root", tonic.native_name());
    }
    if let Some(scale) = scale {
        update(app, "type", scale.native_name());
    }
}

fn tonic_selector(ui: &mut Ui, current: &Selection, width: f32) -> Option<Tonic> {
    let mut result = None;
    ComboBox::from_id_salt("scale-tonic")
        .width(width)
        .selected_text(current.tonic_label.as_str())
        .show_ui(ui, |ui| {
            for value in Tonic::ALL {
                if ui
                    .selectable_label(current.tonic == Some(value), value.label())
                    .clicked()
                {
                    result = Some(value);
                }
            }
        });
    result
}

fn scale_selector(ui: &mut Ui, current: &Selection, width: f32) -> Option<Scale> {
    let mut result = None;
    ComboBox::from_id_salt("scale-mode")
        .width(width)
        .selected_text(current.scale_label.as_str())
        .show_ui(ui, |ui| {
            for value in Scale::ALL {
                if ui
                    .selectable_label(current.scale == Some(value), value.label())
                    .clicked()
                {
                    result = Some(value);
                }
            }
        });
    result
}

struct Selection {
    tonic: Option<Tonic>,
    scale: Option<Scale>,
    tonic_label: String,
    scale_label: String,
    editable: bool,
}

fn selection(app: &App) -> Selection {
    let raw = app
        .editor
        .group()
        .and_then(|group| group.extra.get("musicalScale"));
    let root = raw.and_then(|value| value.get("root"));
    let kind = raw.and_then(|value| value.get("type"));
    let tonic = root.and_then(Value::as_str).and_then(Tonic::from_native);
    let scale = kind.and_then(Value::as_str).and_then(Scale::from_native);
    Selection {
        tonic,
        scale,
        tonic_label: tonic.map_or_else(|| raw_label(root), |value| value.short_label().into()),
        scale_label: scale.map_or_else(|| raw_label(kind), |value| value.label().into()),
        editable: !app.editor.is_editing()
            && app.gesture.is_none()
            && app.group_gesture.is_none()
            && app.audio_drag.is_none()
            && !app.view_pan.is_active()
            && app.editor.group().is_some()
            && raw.is_none_or(Value::is_object),
    }
}

fn raw_label(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(value) => value.to_string(),
        None => "—".into(),
    }
}

fn update(app: &mut App, field: &str, value: &str) {
    if app.editor.is_editing() {
        return;
    }
    let Some(group) = app.editor.group() else {
        return;
    };
    if let Some(raw) = group.extra.get("musicalScale")
        && (!raw.is_object() || raw.get(field).and_then(Value::as_str) == Some(value))
    {
        return;
    }
    app.commit_lyric();
    app.editor.begin(crate::i18n::t("修改调式音阶"));
    if let Some(group) = app.editor.group_mut() {
        let raw = group
            .extra
            .entry("musicalScale".into())
            .or_insert_with(|| json!({"type": "Major", "root": "C"}));
        if let Some(fields) = raw.as_object_mut() {
            fields.insert(field.into(), Value::String(value.into()));
        }
    }
    app.editor.finish();
}
