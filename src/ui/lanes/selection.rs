use crate::{
    app::App,
    model::{EntityId, Parameter},
    ui::{piano::Gesture, state::Lane},
};
use eframe::egui::*;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct ParameterSelection {
    pub(super) owner: Option<Owner>,
    pub(super) selected: BTreeSet<i64>,
    pub(super) anchor: Option<i64>,
    pub(super) focused: bool,
    pub(super) drag: Option<super::point_drag::PointDrag>,
}
pub(super) struct Owner {
    pub(super) lane: u64,
    pub(super) track: EntityId,
    pub(super) reference: EntityId,
    pub(super) group: String,
    pub(super) target: Parameter,
}
pub(super) fn active_lane(app: &App) -> Option<&Lane> {
    let owner = app.parameter_lane.owner.as_ref()?;
    app.view
        .lanes
        .iter()
        .find(|lane| lane.id == owner.lane && lane.visible && lane.foreground == owner.target)
}
pub(super) fn editable(app: &App, target: &Parameter) -> bool {
    !app.editor.current_track().is_instrumental() && *target != Parameter::Rap
}
pub(super) fn editable_bounds(app: &App) -> Option<(i64, i64)> {
    let reference = app.editor.active_ref()?;
    let begin = 0_i64.saturating_sub(reference.blick_offset);
    let end = i64::MAX.saturating_sub(reference.blick_offset);
    (begin <= end).then_some((begin, end))
}
pub(super) fn reconcile(app: &mut App) {
    let valid = app.parameter_lane.owner.as_ref().is_some_and(|owner| {
        app.editor.track == owner.track
            && app.editor.reference == Some(owner.reference)
            && app
                .editor
                .active_ref()
                .is_some_and(|reference| reference.group_id == owner.group)
            && active_lane(app).is_some()
    });
    if !valid {
        finish_drag(app, false);
        app.parameter_lane = Default::default();
        return;
    }
    let times: BTreeSet<_> = app
        .parameter_lane
        .owner
        .as_ref()
        .and_then(|owner| app.editor.automation(&owner.target))
        .into_iter()
        .flat_map(|curve| &curve.points)
        .map(|p| p.0)
        .collect();
    app.parameter_lane
        .selected
        .retain(|time| times.contains(time));
}
pub(super) fn activate(app: &mut App, lane: &Lane) {
    if !editable(app, &lane.foreground) {
        return;
    }
    if app.gesture.is_none() {
        app.commit_lyric();
    }
    let Some(reference) = app.editor.active_ref() else {
        return;
    };
    let owner = Owner {
        lane: lane.id,
        track: app.editor.track,
        reference: reference.id,
        group: reference.group_id.clone(),
        target: lane.foreground.clone(),
    };
    let same = app.parameter_lane.owner.as_ref().is_some_and(|old| {
        old.lane == owner.lane
            && old.reference == owner.reference
            && old.track == owner.track
            && old.group == owner.group
            && old.target == owner.target
    });
    if !same {
        finish_drag(app, false);
        app.parameter_lane = Default::default();
    }
    app.parameter_lane.owner = Some(owner);
    app.parameter_lane.focused = true;
    app.phoneme_focus = None;
}
pub(super) fn select(app: &mut App, time: Option<i64>, extend: bool) {
    if !extend {
        app.parameter_lane.selected.clear();
    }
    if let Some(time) = time {
        if extend && app.parameter_lane.selected.contains(&time) {
            app.parameter_lane.selected.remove(&time);
        } else {
            app.parameter_lane.selected.insert(time);
        }
    }
    app.parameter_lane.anchor = time;
}
pub(super) fn contains(app: &App, lane: u64, time: i64) -> bool {
    app.parameter_lane
        .owner
        .as_ref()
        .is_some_and(|owner| owner.lane == lane)
        && app.parameter_lane.selected.contains(&time)
}
pub(super) fn finish_drag(app: &mut App, commit: bool) {
    if app.parameter_lane.drag.take().is_some()
        && matches!(app.gesture, Some(Gesture::Parameter { .. }))
    {
        if commit {
            app.editor.finish();
        } else {
            app.editor.cancel();
        }
        app.gesture = None;
    }
}
pub(super) fn end_frame(app: &mut App, ctx: &Context) {
    if app.parameter_lane.drag.is_none() {
        return;
    }
    if !matches!(app.gesture, Some(Gesture::Parameter { .. })) {
        app.parameter_lane.drag = None;
        return;
    }
    if !ctx.input(|i| i.focused) {
        finish_drag(app, false);
        ctx.stop_dragging();
    } else if !ctx.input(|i| i.pointer.primary_down()) {
        finish_drag(app, true);
        ctx.stop_dragging();
    }
}
pub(super) fn menu(app: &mut App, ui: &mut Ui) {
    use super::commands::*;
    let selected = parameter_has_selection(app);
    let ctx = ui.ctx().clone();
    for (label, enabled, operation) in [
        ("全选", true, 0),
        ("复制", selected, 1),
        ("剪切", selected, 2),
        ("粘贴", parameter_can_paste(app), 3),
        ("删除", selected, 4),
        ("重复", selected, 5),
    ] {
        if ui
            .add_enabled(enabled, Button::new(crate::i18n::t(label)))
            .clicked()
        {
            match operation {
                0 => parameter_select_all(app),
                1 => parameter_copy(app, &ctx),
                2 => parameter_cut(app, &ctx),
                3 => parameter_paste(app, &ctx),
                4 => parameter_delete(app),
                _ => parameter_duplicate(app),
            }
            ui.close();
        }
    }
    ui.separator();
}
