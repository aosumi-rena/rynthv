use super::geometry;
use crate::{
    app::App,
    model::{Automation, EntityId, GroupRef, Parameter, QUARTER, ticks},
    ui::{
        piano::Gesture,
        state::{CurveTool, Lane},
        timeline::beat_at,
    },
};
use eframe::egui::*;
use std::collections::BTreeMap;

pub struct CurveGesture {
    pub lane: u64,
    reference: GroupRef,
    track: EntityId,
    parameter: Parameter,
    tool: CurveTool,
    original: Automation,
    points: BTreeMap<i64, f64>,
    first: (i64, f64),
    last: (i64, f64),
    guard: i64,
    frame: u64,
}
pub(super) fn interaction(
    app: &mut App,
    ui: &Ui,
    response: &Response,
    canvas: Rect,
    lane: &Lane,
    clip: Rect,
) {
    if lane.tool == CurveTool::Point {
        return;
    }
    if response.drag_started_by(PointerButton::Primary)
        && app.gesture.is_none()
        && !app.editor.is_editing()
        && let Some(p) = ui
            .input(|i| i.pointer.press_origin())
            .filter(|p| clip.contains(*p))
        && let Some(reference) = app.editor.active_ref().cloned()
    {
        let first = sample(app, canvas, lane, &reference, p);
        let original = app
            .editor
            .group()
            .unwrap()
            .automation(&lane.foreground)
            .cloned()
            .unwrap_or_default();
        app.editor.begin(crate::i18n::t("绘制参数曲线"));
        app.gesture = Some(Gesture::Curve(Box::new(CurveGesture {
            lane: lane.id,
            reference,
            track: app.editor.track,
            parameter: lane.foreground.clone(),
            tool: lane.tool,
            original,
            points: BTreeMap::from([first]),
            first,
            last: first,
            guard: if lane.tool == CurveTool::Line {
                QUARTER / 40
            } else {
                QUARTER / 64
            },
            frame: ui.ctx().cumulative_frame_nr(),
        })));
    }
    let active = matches!(&app.gesture,Some(Gesture::Curve(g)) if g.lane==lane.id);
    if !active {
        return;
    }
    if let Some(p) = response.interact_pointer_pos() {
        let Some(Gesture::Curve(mut g)) = app.gesture.take() else {
            unreachable!()
        };
        if g.track != app.editor.track || app.editor.active_ref() != Some(&g.reference) {
            app.editor.cancel();
            return;
        }
        let point = sample(app, canvas, lane, &g.reference, p);
        if g.tool == CurveTool::Line {
            g.points.clear();
            g.points.insert(g.first.0, g.first.1);
            g.points.insert(point.0, point.1);
        } else {
            let left = g.last.0.min(point.0);
            let right = g.last.0.max(point.0);
            g.points.retain(|t, _| *t < left || *t > right);
            g.points.insert(g.last.0, g.last.1);
            g.points.insert(point.0, point.1);
        }
        g.last = point;
        g.frame = ui.ctx().cumulative_frame_nr();
        apply(app, &g);
        app.gesture = Some(Gesture::Curve(g));
    }
    if response.drag_stopped_by(PointerButton::Primary) {
        app.editor.finish();
        app.gesture = None;
    }
}
fn sample(app: &App, canvas: Rect, lane: &Lane, reference: &GroupRef, p: Pos2) -> (i64, f64) {
    let time = ticks(beat_at(canvas, &app.view.piano, p.x)).max(0)
        .saturating_sub(reference.blick_offset);
    let (lo, hi) = geometry::lane_range(lane);
    let (allowed_lo, allowed_hi) = lane.foreground.range();
    let value = (lo + f64::from((canvas.bottom() - p.y) / canvas.height()) * (hi - lo))
        .clamp(allowed_lo, allowed_hi);
    (time, f64::from(value as f32))
}
fn apply(app: &mut App, g: &CurveGesture) {
    let left = *g.points.first_key_value().unwrap().0;
    let right = *g.points.last_key_value().unwrap().0;
    let begin = 0_i64.saturating_sub(g.reference.blick_offset);
    let end = i64::MAX.saturating_sub(g.reference.blick_offset);
    let low = left.saturating_sub(g.guard).max(begin);
    let high = right.saturating_add(g.guard).min(end);
    let mut curve = g.original.clone();
    curve.points.retain(|p| p.0 < low || p.0 > high);
    if low < left {
        curve.insert(
            low,
            f64::from(g.original.sample(low, g.parameter.default_value() as f32)),
        );
    }
    if high > right {
        curve.insert(
            high,
            f64::from(g.original.sample(high, g.parameter.default_value() as f32)),
        );
    }
    for (&time, &value) in &g.points {
        curve.insert(time, value);
    }
    *app.editor.group_mut().unwrap().automation_mut(&g.parameter) = curve;
}
pub fn end_frame(app: &mut App, ctx: &Context) {
    let Some(Gesture::Curve(g)) = &app.gesture else {
        return;
    };
    let cancel = g.frame != ctx.cumulative_frame_nr()
        || g.track != app.editor.track
        || app.editor.active_ref() != Some(&g.reference)
        || ctx.input(|i| !i.focused);
    if cancel || ctx.input(|i| !i.pointer.primary_down()) {
        if cancel {
            app.editor.cancel();
        } else {
            app.editor.finish();
        }
        app.gesture = None;
        ctx.stop_dragging();
    }
}
