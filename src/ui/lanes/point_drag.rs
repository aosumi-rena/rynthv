use crate::{app::App, model::Automation, ui::piano::Gesture};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct PointDrag {
    original: Automation,
    points: Vec<(i64, f64)>,
    origin: (i64, f64),
}
pub(super) fn begin(app: &mut App, lane: u64, time: i64) {
    if app.gesture.is_some() || app.editor.is_editing() {
        return;
    }
    if !app.parameter_lane.selected.contains(&time) {
        super::selection::select(app, Some(time), false);
    }
    let Some(owner) = &app.parameter_lane.owner else {
        return;
    };
    let Some(original) = app.editor.automation(&owner.target).cloned() else {
        return;
    };
    let Some(origin) = original.points.iter().find(|p| p.0 == time).copied() else {
        return;
    };
    let points = original
        .points
        .iter()
        .filter(|p| app.parameter_lane.selected.contains(&p.0))
        .copied()
        .collect();
    app.editor.begin(crate::i18n::t("移动参数控制点"));
    app.parameter_lane.drag = Some(PointDrag {
        original,
        points,
        origin,
    });
    app.gesture = Some(Gesture::Parameter { lane, time });
}
pub(super) fn update(app: &mut App, lane: u64, point: (i64, f64)) {
    if !matches!(app.gesture,Some(Gesture::Parameter {lane:active,..}) if active==lane) {
        return;
    }
    let (Some(drag), Some(owner), Some((begin, end))) = (
        &app.parameter_lane.drag,
        &app.parameter_lane.owner,
        super::selection::editable_bounds(app),
    ) else {
        return;
    };
    let (Some(first), Some(last)) = (drag.points.first(), drag.points.last()) else {
        return;
    };
    let delta = (i128::from(point.0) - i128::from(drag.origin.0)).clamp(
        i128::from(begin) - i128::from(first.0),
        i128::from(end) - i128::from(last.0),
    );
    let (low, high) = super::accepted_parameter_range(app,&owner.target);
    let bounded=super::standard_expression_limit(app,&owner.target).is_some();
    let minimum = drag
        .points
        .iter()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min);
    let maximum = drag
        .points
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max);
    if low - minimum > high - maximum {
        return;
    }
    let value = (point.1 - drag.origin.1).clamp(low - minimum, high - maximum);
    let selected: BTreeSet<_> = drag.points.iter().map(|p| p.0).collect();
    let moved: Vec<_> = drag
        .points
        .iter()
        .map(|p| {
            (
                (i128::from(p.0) + delta) as i64,
                if bounded{f64::from((p.1+value)as f32).clamp(low,high)}else{f64::from((p.1+value)as f32)},
            )
        })
        .collect();
    let moved_times: BTreeSet<_> = moved.iter().map(|p| p.0).collect();
    if drag
        .original
        .points
        .iter()
        .any(|p| !selected.contains(&p.0) && moved_times.contains(&p.0))
    {
        return;
    }
    let mut points: BTreeMap<_, _> = drag
        .original
        .points
        .iter()
        .filter(|p| !selected.contains(&p.0))
        .copied()
        .collect();
    points.extend(moved);
    match app
        .editor
        .replace_automation_points(&owner.target, points.into_iter().collect())
    {
        Ok(_) => app.parameter_lane.selected = moved_times,
        Err(error) => {
            app.error = Some(error);
            super::selection::finish_drag(app, false);
        }
    }
}
