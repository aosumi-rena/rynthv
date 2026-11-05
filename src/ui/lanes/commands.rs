use rynthv::clipboard::{self,Content};
use super::{active_lane, editable_bounds, reconcile};
use crate::{app::App, model::QUARTER};
use eframe::egui::{Context, ViewportCommand};
use std::collections::{BTreeMap, BTreeSet};

pub fn parameter_keyboard(app: &mut App, ctx: &Context) -> bool {
    if parameter_active(app) && !capability_allowed(app) {
        use eframe::egui::{Key,Modifiers};
        for key in [Key::ArrowLeft,Key::ArrowRight,Key::ArrowDown,Key::ArrowUp] {
            ctx.input_mut(|i|i.consume_key(Modifiers::NONE,key));
        }
        return true;
    }
    if !parameter_has_selection(app) || app.gesture.is_some() || app.editor.is_editing() {
        return false;
    }
    use eframe::egui::{Key, Modifiers};
    let target = &app.parameter_lane.owner.as_ref().unwrap().target;
    let (low, high) = super::accepted_parameter_range(app,target);
    for (key, time, value) in [
        (Key::ArrowLeft, -crate::model::ticks(app.view.step()), 0.0),
        (Key::ArrowRight, crate::model::ticks(app.view.step()), 0.0),
        (Key::ArrowDown, 0, -(high - low) * 0.01),
        (Key::ArrowUp, 0, (high - low) * 0.01),
    ] {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
            shift_selected(app, time, value);
            return true;
        }
    }
    false
}

pub fn parameter_active(app: &App) -> bool {
    let Some(owner) = &app.parameter_lane.owner else {
        return false;
    };
    !app.piano_collapsed
        && app.parameter_lane.focused
        && app.editor.track == owner.track
        && app.editor.reference == Some(owner.reference)
        && app
            .editor
            .active_ref()
            .is_some_and(|reference| reference.group_id == owner.group)
        && active_lane(app).is_some_and(|lane| lane.visible && lane.foreground == owner.target)
}

pub fn parameter_has_selection(app: &App) -> bool {
    parameter_active(app) && capability_allowed(app) && !selected(app).is_empty()
}

pub fn parameter_can_paste(app: &App) -> bool {
    clipboard::can_paste() && parameter_active(app)
        && editable(app)
        && app.editor.group().is_some()
        && !app.editor.is_editing()
        && app.gesture.is_none()
}

pub fn parameter_deactivate(app: &mut App) {
    if matches!(
        app.gesture,
        Some(crate::ui::piano::Gesture::Parameter { .. })
    ) {
        super::selection::finish_drag(app, false);
    }
    app.parameter_lane.focused = false;
}

pub fn parameter_clear(app: &mut App) {
    app.parameter_lane.selected.clear();
    app.parameter_lane.anchor = None;
}

pub fn parameter_select_all(app: &mut App) {
    if !ready(app) {
        return;
    }
    let Some((begin, end)) = editable_bounds(app) else {
        return;
    };
    let target = &app.parameter_lane.owner.as_ref().unwrap().target;
    app.parameter_lane.selected = app
        .editor
        .automation(target)
        .into_iter()
        .flat_map(|curve| &curve.points)
        .filter_map(|&(time, _)| (begin..=end).contains(&time).then_some(time))
        .collect();
    app.parameter_lane.anchor = app.parameter_lane.selected.first().copied();
}

pub fn parameter_copy(app: &mut App, ctx: &Context) {
    if !ready(app) {
        return;
    }
    match clipboard_text(app) {
        Ok(Some((text, count))) => {
            ctx.copy_text(text);
            app.status = format!("已复制 {count} 个参数控制点");
        }
        Ok(None) => {}
        Err(error) => app.error = Some(error),
    }
}

pub fn parameter_cut(app: &mut App, ctx: &Context) {
    if !ready(app) {
        return;
    }
    let points = selected(app);
    if points.is_empty() {
        return;
    }
    match clipboard_text(app) {
        Ok(Some((text, count))) => {
            ctx.copy_text(text);
            if remove(app, &points, "剪切参数控制点") {
                parameter_clear(app);
                app.status = format!("已剪切 {count} 个参数控制点");
            }
        }
        Ok(None) => {}
        Err(error) => app.error = Some(error),
    }
}

pub fn parameter_delete(app: &mut App) {
    if !ready(app) {
        return;
    }
    let points = selected(app);
    if !points.is_empty() && remove(app, &points, "删除参数控制点") {
        parameter_clear(app);
    }
}

pub fn parameter_paste(app: &mut App, ctx: &Context) {
    if ready(app) && parameter_can_paste(app) {
        ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
    }
}

pub fn parameter_paste_text(app: &mut App, text: &str) {
    if !ready(app) || !parameter_can_paste(app) {
        return;
    }
    let mut data = match clipboard::read(text) {
        Ok(data) => data,
        Err(error) => {
            app.error = Some(error);
            return;
        }
    };
    // Notes/groups retain their destination voice and complete control payload.
    // Lane-specific editing bounds apply only to a curve-only paste.
    if !data.notes.is_empty()||!data.groups.is_empty() {
        let at=app.editor.transport.position;
        if let Err(error)=app.paste_content(data,at){app.error=Some(error);}
        return;
    }
    for parameter in crate::model::Parameter::ALL {
        if data.parameters.contains_key(parameter.key())
            && let Some(reason)=super::parameter_unavailable(app,&parameter) {
            app.status=crate::i18n::t(reason).into();
            return;
        }
        if super::standard_expression_limit(app,&parameter).is_some()
            && let Some(curve)=data.parameters.get_mut(parameter.key()) {
            let (low,high)=super::accepted_parameter_range(app,&parameter);
            for (_,value) in &mut curve.points {*value=(*value).clamp(low,high);}
        }
    }
    let at=app.editor.transport.position;
    if let Err(error)=app.paste_content(data,at){app.error=Some(error);}
}

pub fn parameter_duplicate(app: &mut App) {
    if !ready(app) || !editable(app) {
        return;
    }
    let points = selected(app);
    let Some(&(last, _)) = points.last() else {
        return;
    };
    let Some(at) = last.checked_add(QUARTER) else {
        app.status = "复制位置超出参数控制点的可用时间范围。".into();
        return;
    };
    insert(app, points, at, "重复参数控制点");
}

pub(super) fn shift_selected(app: &mut App, delta_time: i64, delta_value: f64) {
    if !ready(app) || !editable(app) || !delta_value.is_finite() {
        return;
    }
    let points = selected(app);
    let (Some(&(first, _)), Some(&(last, _)), Some((begin, end))) =
        (points.first(), points.last(), editable_bounds(app))
    else {
        return;
    };
    let target = &app.parameter_lane.owner.as_ref().unwrap().target;
    let (low, high) = super::accepted_parameter_range(app,target);
    let bounded=super::standard_expression_limit(app,target).is_some();
    let lowest = points
        .iter()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let highest = points
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let time = i128::from(delta_time).clamp(
        i128::from(begin) - i128::from(first),
        i128::from(end) - i128::from(last),
    );
    let value = if delta_value == 0.0 && !bounded {
        0.0
    } else {
        let minimum = low - lowest;
        let maximum = high - highest;
        if minimum > maximum {
            app.status = "所选参数控制点的取值跨度超出可编辑范围。".into();
            return;
        }
        delta_value.clamp(minimum, maximum)
    };
    if time == 0 && value == 0.0 {
        return;
    }
    let moved: Vec<_> = points
        .iter()
        .map(|&(position, original)| ((i128::from(position) + time) as i64,
            if bounded{(original+value).clamp(low,high)}else{original+value}))
        .collect();
    let original = app.editor.automation(target).unwrap().points.clone();
    let selected_times: BTreeSet<_> = points.iter().map(|point| point.0).collect();
    let moved_times: BTreeSet<_> = moved.iter().map(|point| point.0).collect();
    if original
        .iter()
        .any(|point| !selected_times.contains(&point.0) && moved_times.contains(&point.0))
    {
        app.status = "移动后会与未选中的参数控制点重合。".into();
        return;
    }
    let mut replacement: BTreeMap<_, _> = original
        .into_iter()
        .filter(|point| !selected_times.contains(&point.0))
        .collect();
    replacement.extend(moved);
    if replace(app, replacement.into_iter().collect(), "移动参数控制点") {
        app.parameter_lane.selected = moved_times;
        app.parameter_lane.anchor = app.parameter_lane.selected.first().copied();
    }
}

fn ready(app: &mut App) -> bool {
    reconcile(app);
    parameter_active(app) && capability_allowed(app) && app.gesture.is_none() && !app.editor.is_editing()
}

fn capability_allowed(app: &App) -> bool {
    app.parameter_lane.owner.as_ref()
        .is_none_or(|owner|super::parameter_unavailable(app,&owner.target).is_none())
}

fn editable(app: &App) -> bool {
    app.parameter_lane
        .owner
        .as_ref()
        .is_some_and(|owner| super::editable(app, &owner.target)
            && super::parameter_unavailable(app,&owner.target).is_none())
}

fn clipboard_text(app: &App) -> Result<Option<(String, usize)>, String> {
    let target=&app.parameter_lane.owner.as_ref().unwrap().target;
    let points=selected(app);if points.is_empty(){return Ok(None);}
    let count=points.len();let mut curve=app.editor.automation(target).cloned().unwrap_or_default();curve.points=points;
    let offset=app.editor.active_ref().ok_or("请先选择音符组。")?.blick_offset;
    for (position,_) in &mut curve.points{*position=position.checked_add(offset).ok_or("粘贴位置超出可用范围。")?;}
    let key=if matches!(target,crate::model::Parameter::VocalMode(_)){format!("vocalMode_{}",target.label())}else{target.key().to_owned()};
    let data=Content{parameters:BTreeMap::from([(key,curve)]),position_set:true,..Default::default()};
    clipboard::write(&data).map(|text|Some((text,count)))
}

fn selected(app: &App) -> Vec<(i64, f64)> {
    let (Some(owner), Some((begin, end))) = (&app.parameter_lane.owner, editable_bounds(app))
    else {
        return Vec::new();
    };
    app.editor
        .automation(&owner.target)
        .into_iter()
        .flat_map(|curve| &curve.points)
        .filter(|point| {
            (begin..=end).contains(&point.0) && app.parameter_lane.selected.contains(&point.0)
        })
        .copied()
        .collect()
}

fn remove(app: &mut App, points: &[(i64, f64)], label: &str) -> bool {
    let target = &app.parameter_lane.owner.as_ref().unwrap().target;
    let times: BTreeSet<_> = points.iter().map(|point| point.0).collect();
    let replacement = app
        .editor
        .automation(target)
        .into_iter()
        .flat_map(|curve| &curve.points)
        .filter(|point| !times.contains(&point.0))
        .copied()
        .collect();
    replace(app, replacement, label)
}

fn insert(app: &mut App, source: Vec<(i64, f64)>, at: i64, label: &str) {
    let Some(&(first, _)) = source.first() else {
        return;
    };
    let Some((begin, end)) = editable_bounds(app) else {
        return;
    };
    let target = &app.parameter_lane.owner.as_ref().unwrap().target;
    let bounded=super::standard_expression_limit(app,target).is_some();
    let (low,high)=super::accepted_parameter_range(app,target);
    let mut inserted = Vec::with_capacity(source.len());
    for (time, value) in source {
        let shifted = i128::from(at) + i128::from(time) - i128::from(first);
        if shifted < i128::from(begin) || shifted > i128::from(end) {
            app.status = "粘贴的参数控制点超出当前音符组的可编辑范围。".into();
            return;
        }
        if !value.is_finite() {
            app.error = Some("参数控制点剪贴板包含无效的数值。".into());
            return;
        }
        inserted.push((shifted as i64, if bounded{value.clamp(low,high)}else{value}));
    }
    let times = inserted.iter().map(|point| point.0).collect();
    let count = inserted.len();
    let mut replacement: BTreeMap<_, _> = app
        .editor
        .automation(target)
        .into_iter()
        .flat_map(|curve| curve.points.iter().copied())
        .collect();
    replacement.extend(inserted);
    if replace(app, replacement.into_iter().collect(), label) {
        app.parameter_lane.selected = times;
        app.parameter_lane.anchor = app.parameter_lane.selected.first().copied();
        app.status = format!("已粘贴 {count} 个参数控制点");
    }
}

fn replace(app: &mut App, points: Vec<(i64, f64)>, label: &str) -> bool {
    if !parameter_active(app) || !capability_allowed(app) {return false;}
    let target = app.parameter_lane.owner.as_ref().unwrap().target.clone();
    app.editor.begin(label);
    match app.editor.replace_automation_points(&target, points) {
        Ok(_) => {
            app.editor.finish();
            true
        }
        Err(error) => {
            app.editor.cancel();
            app.error = Some(error);
            false
        }
    }
}
