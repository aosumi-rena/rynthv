mod pitch;
mod pitch_band;
mod waveform;
mod waveform_flash;

use std::{
    collections::HashMap,
    ops::Range,
    time::{Duration, Instant},
};

use super::scene::SceneGroup;
use crate::{
    app::App,
    model::{beats, ticks},
    synthesis::plan::{GlobalBlick, OwnerId, TempoMapping},
    ui::timeline,
};
use eframe::egui::*;

pub fn waveforms(app: &App, ui: &Ui, canvas: Rect, groups: &[SceneGroup]) {
    if !app.show_waveform {
        return;
    }
    let now = Instant::now();
    let mut animating = false;
    let mut notes = HashMap::new();
    for group in groups {
        for visuals in app.synthesis.visuals(OwnerId::new(group.reference.id)) {
            for piece in visuals.pieces() {
                let painter = painter(ui, canvas, app, &piece.range);
                if painter.clip_rect().is_positive() {
                    let input = std::sync::Arc::as_ptr(&piece.snapshot.input);
                    let notes = notes.entry(input).or_insert_with(|| waveform::active_notes(&piece.snapshot.input));
                    animating |= waveform::draw(app, &painter, canvas, group, &piece.snapshot, notes, visuals, now);
                }
            }
        }
    }
    if animating {
        ui.ctx().request_repaint_after(Duration::from_millis(33));
    }
}

pub fn pitch(app: &App, ui: &Ui, canvas: Rect, groups: &[SceneGroup]) {
    if !app.show_pitch {
        return;
    }
    for group in groups {
        for piece in app.synthesis.visuals(OwnerId::new(group.reference.id)).flat_map(|v|v.pieces()) {
            let painter = painter(ui, canvas, app, &piece.range);
            if painter.clip_rect().is_positive() {
                pitch::draw(app, &painter, canvas, group, &piece.snapshot);
            }
        }
    }
}

fn painter(ui: &Ui, canvas: Rect, app: &App, range: &Range<i64>) -> Painter {
    // VisualTimeline clips every installed piece to its publication bounds.
    let left = timeline::x_at(
        canvas,
        &app.view.piano,
        beats(range.start),
    );
    let right = timeline::x_at(canvas, &app.view.piano, beats(range.end)).min(canvas.right());
    let bounds = Rect::from_min_max(pos2(left, canvas.top()), pos2(right, canvas.bottom()));
    ui.painter()
        .with_clip_rect(bounds.intersect(canvas).intersect(ui.clip_rect()))
}

fn position_at_x(app: &App, canvas: Rect, x: f32) -> i64 {
    ticks(timeline::beat_at(canvas, &app.view.piano, x))
}

fn seconds_at_x(app: &App, tempo: &TempoMapping, canvas: Rect, x: f32) -> f64 {
    tempo.seconds_at(GlobalBlick(ticks(timeline::beat_at(
        canvas,
        &app.view.piano,
        x,
    ))))
}

fn x_at_seconds(app: &App, tempo: &TempoMapping, canvas: Rect, seconds: f64) -> f32 {
    timeline::x_at(canvas, &app.view.piano, beats(tempo.position_at(seconds).0))
}

fn pitch_y(app: &App, canvas: Rect, midi: f32) -> f32 {
    canvas.top() + (app.view.top_pitch - midi + 0.5) * app.view.row_height
}
