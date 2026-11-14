use super::{
    geometry::{self, Handle},
    gesture, *,
};
use crate::{
    model::{PitchPoint, ticks},
    ui::{group_geometry::clip_rect, timeline::beat_at},
};

pub(in crate::ui::piano) fn interaction(
    app: &mut App,
    ui: &mut Ui,
    response: &Response,
    canvas: Rect,
) {
    reconcile(app);
    if !ui.is_enabled() || !ui.input(|input| input.focused) || blocked(app, ui.ctx()) {
        context_menu(app, response);
        return;
    }
    let Some(reference) = app.editor.active_ref().cloned() else {
        if response.double_clicked() {
            app.status = crate::i18n::t("请先点击音符组标题，再添加音高控制点").into();
        }
        return;
    };
    let clip = clip_rect(&reference, canvas, &app.view.piano);
    if let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
        && clip.contains(pointer)
    {
        let handle = geometry::selected_rect(app, canvas, &reference).and_then(|rect| {
            geometry::handles(rect)
                .into_iter()
                .find(|(_, r)| r.expand(4.0).contains(pointer))
        });
        let cursor = match handle.map(|(handle, _)| handle) {
            Some(Handle::Left | Handle::Right) => CursorIcon::ResizeHorizontal,
            Some(Handle::Top | Handle::Bottom) => CursorIcon::ResizeVertical,
            None if geometry::hit(app, canvas, &reference, pointer).is_some()
                || geometry::selected_rect(app, canvas, &reference)
                    .is_some_and(|r| r.contains(pointer)) =>
            {
                CursorIcon::Move
            }
            None => CursorIcon::Crosshair,
        };
        ui.ctx().set_cursor_icon(cursor);
    }
    if response.double_clicked()
        && app.gesture.is_none()
        && let Some(pointer) = response.interact_pointer_pos()
        && clip.contains(pointer)
        && geometry::hit(app, canvas, &reference, pointer).is_none()
        && reference.contains(ticks(beat_at(canvas, &app.view.piano, pointer.x)))
        && can_add_points(app)
    {
        app.commit_lyric();
        let (time, pitch) = geometry::sample(app, canvas, &reference, pointer);
        let point = PitchPoint::new(time, pitch);
        anchor_clear(app);
        app.editor.begin(crate::i18n::t("创建音高控制点"));
        if let Some(group) = app.editor.group_mut() {
            group.pitch_controls.push(PitchControl::Point(point));
        }
        app.editor.finish();
        app.show_pitch = true;
        app.status.clear();
    } else if response.clicked()
        && app.gesture.is_none()
        && let Some(pointer) = response.interact_pointer_pos()
        && clip.contains(pointer)
    {
        let additive = ui.input(|input| input.modifiers.ctrl || input.modifiers.shift);
        let hit = geometry::hit(app, canvas, &reference, pointer);
        if !additive {
            app.piano_anchor.ids.clear();
        }
        if hit.is_none() && !additive {
            app.piano_anchor.candidates.clear();
        }
        if let Some(id) = hit
            && !app.piano_anchor.ids.remove(&id)
        {
            app.piano_anchor.ids.insert(id);
        }
        app.piano_anchor
            .candidates
            .retain(|point| app.piano_anchor.ids.contains(&point.id));
    }
    if response.secondary_clicked()
        && app.gesture.is_none()
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(hit) = geometry::hit(app, canvas, &reference, pointer)
        && !app.piano_anchor.ids.contains(&hit)
    {
        app.piano_anchor.ids = BTreeSet::from([hit]);
    }
    context_menu(app, response);
    if response.drag_started_by(PointerButton::Primary) {
        gesture::start(app, ui, canvas);
    }
    if matches!(app.gesture, Some(Gesture::Anchor(_))) {
        gesture::update(app, ui, canvas);
    }
    if response.drag_stopped_by(PointerButton::Primary) {
        gesture::extract_selection(app, ui, canvas);
        gesture::finish(app, ui.ctx(), false);
    }
}

fn context_menu(app: &mut App, response: &Response) {
    response.context_menu(|ui| {
        if ui
            .add_enabled(
                !app.piano_anchor.ids.is_empty(),
                Button::new(crate::i18n::t("删除所选音高控制点")).shortcut_text("Delete"),
            )
            .clicked()
        {
            anchor_delete(app);
            ui.close();
        }
        if ui.button(crate::i18n::t("全选音高控制点")).clicked() {
            anchor_select_all(app);
            ui.close();
        }
    });
}
