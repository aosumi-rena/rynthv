use crate::{app::App, model::Note, ui::{icons::{Icon, IconButton}, theme::*}};
use eframe::egui::*;

const ITEMS: [(&str, Icon, &str); 9] = [
    ("Staccato", Icon::Staccato, "Short, detached"),
    ("Tenuto", Icon::Tenuto, "Held full length"),
    ("Accent", Icon::Accent, "Strong attack"),
    ("Slur", Icon::Slur, "Smooth, connected"),
    ("Tremolo", Icon::Tremolo, "Rapid repeats"),
    ("Breath", Icon::Breath, "Long rest before note"),
    ("Harmonics", Icon::Harmonics, "Harmonic overtone"),
    ("Fall", Icon::Fall, ""),
    ("Trill", Icon::Trill, ""),
];
mod dock;
pub use dock::toolbar;
fn label(name: &str) -> &'static str {
    super::native_label(match name {
        "Staccato" => "Articulation_Staccato", "Tenuto" => "Articulation_Tenuto",
        "Accent" => "Articulation_Accent", "Slur" => "Articulation_Slur",
        "Tremolo" => "Articulation_Tremolo", "Breath" => "Articulation_Breath",
        "Harmonics" => "Articulation_Harmonics", "Fall" => "Articulation_Fall",
        _ => "Articulation_Trill",
    })
}

fn names(note: &Note) -> Vec<String> {
    note.attributes.get("articulations").and_then(serde_json::Value::as_array)
        .into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect()
}
fn contains(names: &[String], name: &str) -> bool {
    names.iter().any(|v| v == name || name == "Trill" && v.starts_with("Trill "))
}
fn chosen(app: &App, name: &str) -> bool {
    let mut notes = app.editor.notes().iter().filter(|n| app.editor.selected.contains(&n.id)).peekable();
    notes.peek().is_some() && notes.all(|n| contains(&super::smart::articulations(app,app.editor.reference.unwrap_or(0),n), name))
}
fn enabled(app: &App, name: &str) -> bool {
    if app.editor.is_editing() || app.editor.selected.is_empty() { return false; }
    let notes = app.editor.notes();
    notes.iter().enumerate().any(|(i, note)| {
        if !app.editor.selected.contains(&note.id) { return false; }
        let names = super::smart::articulations(app,app.editor.reference.unwrap_or(0),note);
        if name == "Slur" && !notes.get(i+1).is_some_and(|n| note.onset+note.duration == n.onset) {
            return false;
        }
        if matches!(name, "Staccato" | "Tremolo") && contains(&names, "Trill") { return false; }
        true
    })
}
fn toggle(app: &mut App, name: &str) {
    let remove = chosen(app, name);
    let current:std::collections::BTreeMap<_,_>=app.editor.notes().iter().map(|n|
        (n.id,super::smart::articulations(app,app.editor.reference.unwrap_or(0),n))).collect();
    let connected: Vec<_> = app.editor.notes().windows(2)
        .filter(|n| n[0].onset+n[0].duration == n[1].onset).map(|n| n[0].id).collect();
    super::controls::edit_notes(app, crate::i18n::t("Articulation"), |n| {
        if name == "Slur" && !connected.contains(&n.id) { return; }
        let mut next = current.get(&n.id).cloned().unwrap_or_else(||names(n));
        if matches!(name, "Staccato" | "Tremolo") && contains(&next, "Trill") { return; }
        next.retain(|v| !(v == name || name == "Trill" && v.starts_with("Trill ")));
        if !remove {
            if name == "Trill" { next.retain(|v| !matches!(v.as_str(), "Staccato" | "Tremolo")); }
            if name == "Fall" && !contains(&next, "Accent") { next.push("Accent".into()); }
            next.push(if name == "Trill" { "Trill Minor" } else { name }.into());
        }
        n.attributes.insert("articulations".into(), serde_json::json!(next));
        n.attributes.insert("articulationsFixed".into(), true.into());
    });
}

fn interval(app: &mut App, ui: &mut Ui) {
    let mut major = chosen(app, "Trill Major");
    let initial = major;
    ui.add_enabled_ui(chosen(app,"Trill") && !app.editor.is_editing(), |ui| {
        ComboBox::from_id_salt("instrument-trill-interval")
            .width(72.0)
            .selected_text(super::native_label(if major {"Articulation_major"} else {"Articulation_minor"}))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut major, false, super::native_label("Articulation_minor"));
                ui.selectable_value(&mut major, true, super::native_label("Articulation_major"));
            });
    });
    if initial != major {
        let current:std::collections::BTreeMap<_,_>=app.editor.notes().iter().map(|n|
            (n.id,super::smart::articulations(app,app.editor.reference.unwrap_or(0),n))).collect();
        super::controls::edit_notes(app, crate::i18n::t("Articulation"), |n| {
            let mut values = current.get(&n.id).cloned().unwrap_or_else(||names(n));
            for name in &mut values {
                if name.starts_with("Trill ") { *name = if major {"Trill Major"} else {"Trill Minor"}.into(); }
            }
            n.attributes.insert("articulations".into(), serde_json::json!(values));
            n.attributes.insert("articulationsFixed".into(), true.into());
        });
    }
}

pub(super) fn panel(app: &mut App, ui: &mut Ui) {
    for (name, icon, description) in ITEMS {
        let is_enabled = enabled(app,name);
        let is_chosen = chosen(app,name);
        let response = ui.add_enabled_ui(is_enabled, |ui| {
            let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 54.0), Sense::click());
            if response.hovered() || is_chosen { ui.painter().rect_filled(rect, 3, BUTTON); }
            let color = crate::ui::track_color::TrackColors::new(app.editor.current_track()).accent
                .gamma_multiply(if is_enabled {1.0} else {0.35});
            icon.paint(ui, Rect::from_center_size(rect.left_center()+vec2(16.0,0.0),vec2(24.0,24.0)),color);
            ui.painter().text(rect.min+vec2(45.0,8.0),Align2::LEFT_TOP,
                label(name),FontId::proportional(13.0),
                if is_enabled {TEXT} else {MUTED});
            if !description.is_empty() {
                ui.painter().text(rect.min+vec2(45.0,29.0),Align2::LEFT_TOP,
                    crate::i18n::t(description),FontId::proportional(12.0),MUTED);
            }
            response
        }).inner;
        if response.clicked() { toggle(app,name); }
    }
    interval(app,ui);
}

pub fn note_symbols(app:&App, reference:u64, ui: &Ui, note: &Note, rect: Rect, clip: Rect, color: Color32) {
    let names = super::smart::articulations(app,reference,note);
    let mut x=rect.left()+3.0;
    for (name,icon,_) in ITEMS {
        if contains(&names,name) && x+14.0 < rect.right() {
            let icon_rect=Rect::from_center_size(pos2(x+7.0,rect.center().y),vec2(14.0,14.0));
            if clip.contains_rect(icon_rect) { icon.paint(ui,icon_rect,color); }
            x+=16.0;
        }
    }
}
