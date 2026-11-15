use crate::{app::App, model::EntityId};
use eframe::egui::*;

#[derive(Clone)]
struct Edit {
    group: String,
    origin: f32,
    values: Vec<(EntityId, f32)>,
}

pub(super) fn finish_released(app: &mut App, ctx: &Context) {
    if ctx.input(|i| i.pointer.button_down(PointerButton::Primary)) { return; }
    let id = Id::new("instrument-dynamics-drag");
    if ctx.data_mut(|d| { let present = d.get_temp::<Edit>(id).is_some(); d.remove::<Edit>(id); present }) {
        app.editor.finish();
    }
}

pub(super) fn update(app: &mut App, ui: &Ui, response: &Response, canvas: Rect,
    hit: Option<(String, EntityId)>) {
    let id = Id::new("instrument-dynamics-drag");
    let mut edit = ui.ctx().data(|d| d.get_temp::<Edit>(id));
    if (response.drag_started_by(PointerButton::Primary) || response.clicked())
        && edit.is_none() && !app.editor.is_editing()
        && let Some((group, note)) = hit
        && let Some(pointer) = response.interact_pointer_pos()
    {
        if !app.editor.selected.contains(&note) {
            app.editor.selected.clear();
            app.editor.selected.insert(note);
        }
        let values = app.editor.project.group(&group).into_iter().flat_map(|g| &g.notes)
            .filter(|n| app.editor.selected.contains(&n.id)).map(|n| (n.id, super::value(app,app.editor.reference.unwrap_or(0),n))).collect();
        app.editor.begin(crate::i18n::t("Dynamics"));
        edit = Some(Edit { group, origin: pointer.y - response.drag_delta().y, values });
    }
    if let Some(e) = &edit {
        if response.dragged_by(PointerButton::Primary)
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(group) = app.editor.project.group_mut(&e.group)
        {
            let delta = (e.origin - pointer.y) / canvas.height().max(1.0);
            for (id, initial) in &e.values {
                if let Some(note) = group.notes.iter_mut().find(|n| n.id == *id) {
                    note.attributes.insert("dynamic".into(), serde_json::json!((initial + delta).clamp(0.0, 1.0)));
                }
            }
        }
        if !ui.input(|i| i.pointer.button_down(PointerButton::Primary)) {
            app.editor.finish();
            edit = None;
        }
    }
    ui.ctx().data_mut(|d| {
        if let Some(e) = edit { d.insert_temp(id, e); }
        else { d.remove::<Edit>(id); }
    });
}
