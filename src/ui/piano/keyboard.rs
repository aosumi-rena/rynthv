mod decoration;
mod layout;

use crate::{app::App, ui::track_color::TrackColors};
use eframe::egui::*;
use layout::KeyboardLayout;
pub(super) use layout::white_index;

#[derive(Default)]
pub struct KeyboardAudition {
    captured: bool,
    note: Option<u8>,
    visible: bool,
}

pub fn begin_keyboard_frame(app: &mut App, ctx: &Context) {
    app.piano_audition.visible = false;
    if blocked(app, ctx) || ctx.input(|i| !i.focused) {
        cancel_audition(app);
    }
}

pub fn end_keyboard_frame(app: &mut App, ctx: &Context) {
    if !app.piano_audition.visible
        || blocked(app, ctx)
        || ctx.input(|i| {
            !i.focused || !i.pointer.has_pointer() || !i.pointer.button_down(PointerButton::Primary)
        })
    {
        let was_active = app.piano_audition.note.is_some();
        cancel_audition(app);
        if was_active {
            ctx.request_repaint();
        }
    }
}

pub fn cancel_audition(app: &mut App) {
    app.piano_audition.captured = false;
    set_note(app, None);
}

fn set_note(app: &mut App, note: Option<u8>) {
    if app.piano_audition.note != note {
        app.piano_audition.note = note;
        app.synthesis.audition_key(note);
    }
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

pub(super) fn show(app: &mut App, ui: &mut Ui, all: Rect, canvas: Rect) {
    app.piano_audition.visible = true;
    let layout = KeyboardLayout::new(all, canvas, app.view.top_pitch, app.view.row_height);
    let visible = layout.rect.intersect(ui.clip_rect());
    let response = ui.interact(visible, Id::new("piano-audition-keys"), Sense::drag());
    let (pressed, held, pointer, focused) = ui.input(|i| {
        (
            i.pointer.button_pressed(PointerButton::Primary),
            i.pointer.button_down(PointerButton::Primary),
            i.pointer.interact_pos(),
            i.focused,
        )
    });
    let enabled = ui.is_enabled() && focused && !blocked(app, ui.ctx());
    let note = pointer
        .filter(|&point| visible.contains(point))
        .and_then(|point| layout.hit(point));
    if enabled && pressed && response.contains_pointer() && note.is_some() {
        app.piano_audition.captured = true;

        set_note(app, note);
    }
    if app.piano_audition.captured {
        if !enabled || !held {
            cancel_audition(app);
        } else {

            set_note(app, note);
        }
    }
    layout.draw(
        ui,
        app.piano_audition.note,
        super::scale_guide::effective(app),
        TrackColors::new(app.editor.current_track()).accent,
    );
    if enabled && note.is_some() {
        response
            .on_hover_cursor(CursorIcon::PointingHand);
    }
}
