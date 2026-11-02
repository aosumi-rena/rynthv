use super::{
    editable, id, reconcile,
    transform::{self, Transform},
};
use crate::{
    app::App,
    model::{GroupRef, PitchControl, QUARTER},
};
use std::collections::BTreeSet;

fn ready(app: &mut App) -> bool {
    reconcile(app);
    app.gesture.is_none() && !app.editor.is_editing()
}

fn selected(app: &App) -> Vec<PitchControl> {
    let Some(reference) = app.editor.active_ref() else {
        return Vec::new();
    };
    super::controls(app)
        .into_iter()
        .filter(|control| {
            editable(control, reference)
                && id(control).is_some_and(|id| app.piano_anchor.ids.contains(id))
        })
        .collect()
}

pub fn anchor_copy(app: &mut App,ctx:&eframe::egui::Context)->bool {
    if !ready(app) {
        return false;
    }
    let controls = selected(app);
    if controls.is_empty() {
        return false;
    }
    let offset=app.editor.active_ref().unwrap().blick_offset;
    let mut controls=controls;
    for c in &mut controls {match c {PitchControl::Point(p)=>p.pos=p.pos.saturating_add(offset),PitchControl::Curve(p)=>p.pos=p.pos.saturating_add(offset),_=>{}}}
    let data=rynthv::clipboard::Content{pitch_controls:controls,position_set:true,..Default::default()};
    match rynthv::clipboard::write(&data){Ok(text)=>{ctx.copy_text(text);true},Err(error)=>{app.error=Some(error);false}}
}

pub fn anchor_cut(app: &mut App,ctx:&eframe::egui::Context) {
    if !ready(app) || app.piano_anchor.ids.is_empty() {
        return;
    }
    if !anchor_copy(app,ctx){return;}
    let ids = app.piano_anchor.ids.clone();
    app.editor.begin(crate::i18n::t("剪切音高控制点"));
    if let Some(group) = app.editor.group_mut() {
        group
            .pitch_controls
            .retain(|control| !id(control).is_some_and(|id| ids.contains(id)));
    }
    app.editor.finish();
    app.piano_anchor
        .candidates
        .retain(|point| !ids.contains(&point.id));
    app.piano_anchor.ids.clear();
}

pub fn anchor_paste_text(app: &mut App,text:&str) {
    if !ready(app) {
        return;
    }
    let at = app.editor.transport.position;
    let before:BTreeSet<_>=app.editor.group().into_iter().flat_map(|g|&g.pitch_controls).filter_map(id).map(str::to_owned).collect();
    match app.editor.paste_text(text,at){
        Ok(_)=>app.piano_anchor.ids=app.editor.group().into_iter().flat_map(|g|&g.pitch_controls).filter_map(id).filter(|s|!before.contains(*s)).map(str::to_owned).collect(),
        Err(error)=>app.error=Some(error),
    }
}

pub fn anchor_duplicate(app: &mut App) {
    if !ready(app) {
        return;
    }
    let controls = selected(app);
    let Some(area) = transform::bounds(&controls, &app.piano_anchor.ids) else {
        return;
    };
    let Some(reference) = app.editor.active_ref() else {
        return;
    };

    let left = if area.left == area.right {
        area.right.saturating_add(QUARTER)
    } else {
        area.right
    };
    let latest = reference
        .visible_end()
        .saturating_sub(reference.blick_offset)
        .saturating_sub(area.right.saturating_sub(area.left));
    if left.min(latest) <= area.left {
        return;
    }
    let pitch_offset = reference.pitch_offset;
    let at = left.saturating_add(reference.blick_offset);
    insert(app, controls, pitch_offset, at, crate::i18n::t("复制音高控制点"));
}

pub fn anchor_shift_octave(app: &mut App, up: bool) {
    if !ready(app) {
        return;
    }
    let transient: BTreeSet<_> = app
        .piano_anchor
        .candidates
        .iter()
        .map(|point| point.id.clone())
        .collect();
    if transient.iter().any(|id| app.piano_anchor.ids.contains(id)) && !super::can_add_points(app) {
        return;
    }
    let Some(reference) = app.editor.active_ref() else {
        return;
    };
    let original: Vec<_> = super::controls(app)
        .into_iter()
        .filter(|control| {
            !id(control)
                .is_some_and(|id| transient.contains(id) && !app.piano_anchor.ids.contains(id))
        })
        .collect();
    let controls = transform::apply(
        &original,
        &app.piano_anchor.ids,
        reference,
        Transform::Move {
            time: 0,
            pitch: if up { 12.0 } else { -12.0 },
        },
    );
    if controls == original {
        return;
    }
    app.editor.begin(if up {
        crate::i18n::t("音高控制点升八度")
    } else {
        crate::i18n::t("音高控制点降八度")
    });
    app.editor.group_mut().unwrap().pitch_controls = controls;
    app.editor.finish();
    app.piano_anchor
        .candidates
        .retain(|point| !app.piano_anchor.ids.contains(&point.id));
}

fn insert(
    app: &mut App,
    source: Vec<PitchControl>,
    source_pitch_offset: i32,
    at: i64,
    label: &str,
) {
    if source
        .iter()
        .any(|control| matches!(control, PitchControl::Point(_)))
        && !super::can_add_points(app)
    {
        return;
    }
    let Some(reference) = app.editor.active_ref() else {
        return;
    };
    let Some(controls) = prepare(source, source_pitch_offset, reference) else {
        return;
    };
    let ids: BTreeSet<_> = controls.iter().filter_map(id).map(str::to_owned).collect();
    let Some(area) = transform::bounds(&controls, &ids) else {
        return;
    };
    let local = at.saturating_sub(reference.blick_offset);
    let controls = transform::apply(
        &controls,
        &ids,
        reference,
        Transform::Move {
            time: local.saturating_sub(area.left),
            pitch: 0.0,
        },
    );
    app.editor.begin(label);
    app.editor
        .group_mut()
        .unwrap()
        .pitch_controls
        .extend(controls);
    app.editor.finish();
    app.piano_anchor.ids = ids;
}

fn prepare(
    mut controls: Vec<PitchControl>,
    source_pitch_offset: i32,
    reference: &GroupRef,
) -> Option<Vec<PitchControl>> {
    let ids = controls.iter().filter_map(id).map(str::to_owned).collect();
    let area = transform::bounds(&controls, &ids)?;
    let begin = reference
        .blick_absolute_begin
        .saturating_sub(reference.blick_offset);
    let end = reference
        .visible_end()
        .saturating_sub(reference.blick_offset);
    if i128::from(area.right) - i128::from(area.left) > i128::from(end) - i128::from(begin)
        || area.top - area.bottom > 127.0
    {
        return None;
    }
    let low = -f64::from(reference.pitch_offset) - area.bottom;
    let high = 127.0 - f64::from(reference.pitch_offset) - area.top;
    let pitch =
        (f64::from(source_pitch_offset) - f64::from(reference.pitch_offset)).clamp(low, high);
    let time = i128::from(begin) - i128::from(area.left);
    for control in &mut controls {
        let fresh_id = (uuid::Uuid::new_v4().as_u128() as u64).to_string();
        match control {
            PitchControl::Point(point) => {
                point.id = fresh_id;
                point.pos = (i128::from(point.pos) + time).try_into().ok()?;
                point.pitch += pitch;
            }
            PitchControl::Curve(curve) => {
                curve.id = fresh_id;
                curve.pos = (i128::from(curve.pos) + time).try_into().ok()?;
                curve.pitch += pitch;
            }
            PitchControl::Unsupported(_) => return None,
        }
        if !editable(control, reference) {
            return None;
        }
    }
    Some(controls)
}
