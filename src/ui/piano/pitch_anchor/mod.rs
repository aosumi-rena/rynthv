mod commands;
mod draw;
mod extract;
mod geometry;
mod gesture;
mod interaction;
mod transform;

pub use commands::{anchor_copy, anchor_cut, anchor_duplicate, anchor_paste, anchor_shift_octave};
pub(super) use draw::draw;
pub use gesture::AnchorGesture;
pub(super) use interaction::interaction;

use super::{Gesture, Tool};
use crate::{
    app::App,
    model::{EntityId, GroupRef, PitchControl, PitchPoint},
};
use eframe::egui::*;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct AnchorSelection {
    owner: Option<(EntityId, EntityId, String)>,
    ids: BTreeSet<String>,
    clipboard: Vec<PitchControl>,
    clipboard_pitch_offset: i32,
    candidates: Vec<PitchPoint>,
    revision: u64,
}

pub fn anchor_has_selection(app: &App) -> bool {
    !app.piano_anchor.ids.is_empty()
}
pub fn anchor_has_clipboard(app: &App) -> bool {
    !app.piano_anchor.clipboard.is_empty()
}
pub fn anchor_clear(app: &mut App) {
    app.piano_anchor.ids.clear();
    app.piano_anchor.candidates.clear();
}

fn controls(app: &App) -> Vec<PitchControl> {
    app.editor
        .group()
        .map_or_else(Vec::new, |group| group.pitch_controls.clone())
        .into_iter()
        .chain(
            app.piano_anchor
                .candidates
                .iter()
                .cloned()
                .map(PitchControl::Point),
        )
        .collect()
}

fn id(control: &PitchControl) -> Option<&str> {
    match control {
        PitchControl::Point(point) => Some(&point.id),
        PitchControl::Curve(curve) => Some(&curve.id),
        PitchControl::Unsupported(_) => None,
    }
}

fn editable(control: &PitchControl, reference: &GroupRef) -> bool {
    let begin = reference
        .blick_absolute_begin
        .saturating_sub(reference.blick_offset);
    let end = reference
        .visible_end()
        .saturating_sub(reference.blick_offset);
    match control {
        PitchControl::Point(point) => point.valid() && point.pos >= begin && point.pos <= end,

        PitchControl::Curve(curve) => {
            curve.valid()
                && curve
                    .range()
                    .is_some_and(|(left, right)| left >= begin && right <= end)
        }
        PitchControl::Unsupported(_) => false,
    }
}

fn reconcile(app: &mut App) {
    let owner = app
        .editor
        .active_ref()
        .map(|reference| (app.editor.track, reference.id, reference.group_id.clone()));
    if app.piano_anchor.owner != owner {
        app.piano_anchor.owner = owner;
        anchor_clear(app);
    }
    if app.piano_anchor.revision != app.editor.revision {
        app.piano_anchor.candidates.clear();
        app.piano_anchor.revision = app.editor.revision;
    }
    let controls = controls(app);
    let valid: BTreeSet<_> = app
        .editor
        .active_ref()
        .into_iter()
        .flat_map(|reference| {
            controls
                .iter()
                .filter(move |control| editable(control, reference))
                .filter_map(id)
        })
        .map(str::to_owned)
        .collect();
    app.piano_anchor.ids.retain(|id| valid.contains(id));
}

fn blocked(app: &App, ctx: &Context) -> bool {
    app.piano_collapsed
        || app.pending.is_some()
        || app.error.is_some()
        || app.settings
        || app.voice_manager
        || app.about
        || app.rename.is_some()
        || app.time_edit.is_some()
        || app.fill_lyrics.is_some()
        || app.midi_import.is_some()
        || app.synthesis.context_import.dialog.is_some()
        || Popup::is_any_open(ctx)
        || app.view_pan.is_active()
}

fn can_add_points(app: &mut App) -> bool {
    let family = app.voice_selection().and_then(|key| {
        app.synthesis
            .library
            .entry(&key)
            .and_then(|entry| entry.family)
    });
    if family.is_some_and(|family| family != crate::synthesis::voice::VoiceFamily::Sv2) {
        app.status = crate::i18n::t("当前声库暂不支持音高控制点，请使用手绘音高模式").into();
        false
    } else {
        true
    }
}

pub fn anchor_select_all(app: &mut App) {
    reconcile(app);
    let controls = controls(app);
    app.piano_anchor.ids = app
        .editor
        .active_ref()
        .into_iter()
        .flat_map(|reference| {
            controls
                .iter()
                .filter(move |control| editable(control, reference))
                .filter_map(id)
        })
        .map(str::to_owned)
        .collect();
}

pub fn anchor_delete(app: &mut App) {
    reconcile(app);
    if app.piano_anchor.ids.is_empty() || app.gesture.is_some() || app.editor.is_editing() {
        return;
    }
    let selected = app.piano_anchor.ids.clone();
    app.editor.begin(crate::i18n::t("删除音高控制点"));
    if let Some(group) = app.editor.group_mut() {
        group
            .pitch_controls
            .retain(|control| !id(control).is_some_and(|id| selected.contains(id)));
    }
    app.editor.finish();
    app.piano_anchor
        .candidates
        .retain(|point| !selected.contains(&point.id));
    app.piano_anchor.ids.clear();
}

pub fn anchor_keyboard(app: &mut App, ctx: &Context) -> bool {
    if app.piano_tool != Tool::PitchAnchor || ctx.egui_wants_keyboard_input() {
        return false;
    }
    reconcile(app);
    if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
        if matches!(app.gesture, Some(Gesture::Anchor(_))) {
            gesture::finish(app, ctx, true);
        } else {
            anchor_clear(app);
        }
        return true;
    }
    if app.gesture.is_some() || blocked(app, ctx) {
        return false;
    }
    if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::A)) {
        anchor_select_all(app);
        return true;
    }
    for (key, command) in [
        (Key::C, anchor_copy as fn(&mut App)),
        (Key::X, anchor_cut),
        (Key::V, anchor_paste),
        (Key::D, anchor_duplicate),
    ] {
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, key)) {
            command(app);
            return true;
        }
    }
    for (key, up) in [(Key::ArrowUp, true), (Key::ArrowDown, false)] {
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, key)) {
            anchor_shift_octave(app, up);
            return true;
        }
    }
    if ctx.input_mut(|input| {
        input.consume_key(Modifiers::NONE, Key::Delete)
            || input.consume_key(Modifiers::NONE, Key::Backspace)
    }) {
        anchor_delete(app);
        return true;
    }
    false
}

pub fn end_anchor_frame(app: &mut App, ctx: &Context) {
    reconcile(app);
    let Some(Gesture::Anchor(gesture)) = &app.gesture else {
        if app.piano_tool != Tool::PitchAnchor {
            anchor_clear(app);
        }
        return;
    };
    let cancel = !gesture.owns(app)
        || app.piano_tool != Tool::PitchAnchor
        || gesture.frame != ctx.cumulative_frame_nr()
        || blocked(app, ctx)
        || ctx.input(|input| !input.focused || !input.pointer.has_pointer());
    if cancel || ctx.input(|input| !input.pointer.primary_down()) {
        gesture::finish(app, ctx, cancel);
    }
}
