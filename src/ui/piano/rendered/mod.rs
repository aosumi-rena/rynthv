mod pitch;
mod pitch_band;
mod waveform;
mod waveform_flash;

use std::{
    ops::Range,
    time::{Duration, Instant},
};

use super::scene::SceneGroup;
use crate::{
    app::App,
    model::{beats, ticks},
    synthesis::plan::{GlobalBlick, OwnerId, OwnerInput, TempoMapping},
    ui::timeline,
};
use eframe::egui::*;

pub fn waveforms(app: &App, ui: &Ui, canvas: Rect, groups: &[SceneGroup]) {
    if !app.show_waveform {
        return;
    }
    let now = Instant::now();
    let mut animating = false;
    for group in groups {
        for visuals in app.synthesis.visuals(OwnerId::new(group.reference.id)) {
        for piece in visuals.pieces() {
            let painter = painter(ui, canvas, app, &piece.snapshot.input, &piece.range);
            if painter.clip_rect().is_positive() {
                animating |=
                    waveform::draw(app, &painter, canvas, group, &piece.snapshot, visuals, now);
            }
        }
        }
    }
    if animating {
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
}

pub fn pitch(app: &App, ui: &Ui, canvas: Rect, groups: &[SceneGroup]) {
    let visible=if crate::ui::theme::current()==crate::ui::theme::InterfaceTheme::Classic {app.show_output_pitch}else{app.show_pitch};
    if !visible {return;}
    for group in groups {
        for piece in app.synthesis.visuals(OwnerId::new(group.reference.id)).flat_map(|v|v.pieces()) {
            let painter = painter(ui, canvas, app, &piece.snapshot.input, &piece.range);
            if painter.clip_rect().is_positive() {
                pitch::draw(app, &painter, canvas, group, &piece.snapshot);
            }
        }
    }
}

fn painter(ui: &Ui, canvas: Rect, app: &App, input: &OwnerInput, range: &Range<i64>) -> Painter {

    let (publication_begin,publication_end) = input.publication_bounds();
    let left = timeline::x_at(
        canvas,
        &app.view.piano,
        beats(publication_begin.0.max(range.start)),
    );
    let right = publication_end
        .map_or(canvas.right(), |end| {
            timeline::x_at(canvas, &app.view.piano, beats(end.0))
        })
        .min(timeline::x_at(canvas, &app.view.piano, beats(range.end)));
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
