use super::state::Viewport;
use crate::model::{TimeAxis, beats, ticks};
use eframe::egui::*;

mod drawing;
pub mod loop_region;
pub use drawing::{draw_ruler, grid, playhead, playhead_line, hover_playhead};

pub const GUTTER: f32 = 48.0;
pub const ARRANGEMENT_GRID: f64 = 1.0;

fn grid_step(view: &Viewport, step: f64) -> f64 {
    step.max(5.0 / view.beat_width as f64)
}

pub fn snap_arrangement_position(view: &Viewport, time: &TimeAxis, beat: f64) -> i64 {
    let beat = beat.max(0.0);
    let step = grid_step(view, ARRANGEMENT_GRID);
    let position = ticks(beat);
    let (bar, start) = time.bar_at(position);

    [
        ticks((beat / step).round() * step),
        start,
        time.bar_position(bar.saturating_add(1)),
    ]
    .into_iter()
    .min_by_key(|candidate| candidate.abs_diff(position))
    .unwrap()
}
pub fn x_at(rect: Rect, view: &Viewport, beat: f64) -> f32 {
    rect.left() + ((beat - view.start) * view.beat_width as f64) as f32
}
pub fn beat_at(rect: Rect, view: &Viewport, x: f32) -> f64 {
    view.start + (x - rect.left()) as f64 / view.beat_width as f64
}

pub fn reveal(view:&mut Viewport,width:f32,position:i64) {
    let visible=f64::from(width.max(1.0))/f64::from(view.beat_width);
    let beat=beats(position).max(0.0);

    if beat<view.start {view.start=beat;}
    else if beat>view.start+visible {view.start=(beat-visible).max(0.0);}
}

pub fn follow_playhead(ui: &Ui, rect: Rect, view: &mut Viewport, position: i64, playing: bool) {
    if !playing || rect.width() <= 12.0 {
        return;
    }

    let interacting = ui.input(|input| {
        input.pointer.any_down()
            || (input.pointer.hover_pos().is_some_and(|p| rect.contains(p))
                && input
                    .raw
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::MouseWheel { .. })))
    });
    if interacting {
        return;
    }
    let visible = f64::from(rect.width() - 8.0) / f64::from(view.beat_width);
    let position = beats(position).max(0.0);
    let relative = position - view.start;
    if relative < 0.0 || relative >= visible {

        view.start = (view.start + (relative / visible).floor() * visible).max(0.0);
    }
}

pub fn scroll(ui: &Ui, response: &Response, rect: Rect, view: &mut Viewport, zoom:std::ops::RangeInclusive<f32>) -> f32 {
    if !response.contains_pointer() {
        return 0.0;
    }
    let delta = wheel_delta(ui, rect.height());
    let (modifiers, pointer) = ui.input(|i| (i.modifiers, i.pointer.hover_pos()));
    if modifiers.ctrl && delta.y != 0.0 {
        let x = pointer.map_or(rect.center().x, |p| p.x);
        let anchor = beat_at(rect, view, x);
        view.beat_width = (view.beat_width * (delta.y * 0.002).exp()).clamp(*zoom.start(), *zoom.end());
        view.start = (anchor - (x - rect.left()) as f64 / view.beat_width as f64).max(0.0);
    } else if modifiers.shift || delta.x != 0.0 {
        view.start = (view.start - (delta.x + delta.y) as f64 / view.beat_width as f64).max(0.0);
    } else {
        return delta.y;
    }
    0.0
}

pub fn wheel_delta(ui: &Ui, page_height: f32) -> Vec2 {
    ui.input(|i| i
            .raw
            .events
            .iter()
            .fold(Vec2::ZERO, |sum, event| match event {
                Event::MouseWheel { unit, delta, .. } => {
                    sum + *delta
                        * match unit {
                            MouseWheelUnit::Point => 1.0,
                            MouseWheelUnit::Line => 24.0,
                            MouseWheelUnit::Page => page_height,
                        }
                }
                _ => sum,
            }))
}
