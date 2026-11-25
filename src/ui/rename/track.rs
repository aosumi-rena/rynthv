use super::{Rename, Target};
use crate::{app::App, model::EntityId};
use eframe::egui::{*, text::{CCursor, CCursorRange}};

pub fn field(app: &mut App, ui: &mut Ui, rect: Rect, id: EntityId) -> bool {
    if !app.rename.as_ref().is_some_and(|r| matches!(r.target, Target::Track(target) if target == id))
        || !ui.is_rect_visible(rect)
    {
        return false;
    }
    let mut rename = app.rename.take().unwrap();
    let focusing = rename.focus;
    let cancel = ui.input(|i| i.key_pressed(Key::Escape));
    let enter = ui.input(|i| i.key_pressed(Key::Enter));
    let edit_id = Id::new(("track-name", id));
    let response = ui.put(rect, TextEdit::singleline(&mut rename.text)
        .id(edit_id).font(FontId::proportional(12.0))
        .margin(vec2(3.0, 2.0)).desired_width(rect.width()));
    if focusing {
        response.request_focus();
        if let Some(mut state) = TextEdit::load_state(ui.ctx(), edit_id) {
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(0), CCursor::new(rename.text.chars().count()),
            )));
            state.store(ui.ctx(), edit_id);
        }
        rename.focus = false;
        ui.ctx().request_repaint();
    }
    rename.shown_frame = Some(ui.ctx().cumulative_frame_nr());
    if !cancel && (enter || (!focusing && response.lost_focus())) {
        response.surrender_focus();
        finish(app, rename);
        ui.ctx().request_repaint();
    } else if cancel {
        response.surrender_focus();
        ui.ctx().request_repaint();
    } else if !cancel {
        app.rename = Some(rename);
    }
    true
}

pub(super) fn end_frame(app: &mut App, ctx: &Context) {
    let Some(rename) = app.rename.as_mut() else { return };
    let Target::Track(id) = rename.target else { return };
    let Some(index) = app.editor.project.tracks.iter().position(|t| t.id == id) else {
        app.rename = None;
        return;
    };
    let frame = ctx.cumulative_frame_nr();
    if rename.shown_frame == Some(frame) {
        return;
    }
    if rename.focus && rename.shown_frame.is_none() {
        app.view.arrangement_collapsed = false;
        ctx.data_mut(|d| d.insert_temp(Id::new("track-scroll"), index as f32 * 56.0));
        rename.shown_frame = Some(frame);
        ctx.request_repaint();
    } else if let Some(rename) = app.rename.take() {
        finish(app, rename);
    }
}

fn finish(app: &mut App, rename: Rename) {
    if let Target::Track(id) = rename.target
        && !rename.text.trim().is_empty()
        && app.editor.project.tracks.iter().any(|t| t.id == id)
    {
        app.editor.rename_track(id, rename.text.trim().into());
    }
}
