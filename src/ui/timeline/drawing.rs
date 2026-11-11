use super::{Viewport, grid_step, x_at};
use crate::{
    model::{TimeAxis, beats},
    ui::theme::{edge_color, text_color, palette, current, InterfaceTheme},
};
use eframe::egui::*;

fn visible_end(rect: Rect, view: &Viewport) -> f64 {
    view.start + rect.width() as f64 / view.beat_width as f64
}

fn visible_steps(rect: Rect, view: &Viewport, step: f64) -> impl Iterator<Item = f64> {
    let step = grid_step(view, step);
    let start = (view.start / step).floor() as i64;
    let count = ((visible_end(rect, view) - view.start) / step).ceil() as i64 + 2;
    (start..start + count).map(move |i| i as f64 * step)
}

pub fn grid(ui: &Ui, rect: Rect, view: &Viewport, time: &TimeAxis, step: f64) {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    for beat in visible_steps(rect, view, step) {
        let whole = (beat - beat.round()).abs() < 1e-5;
        painter.vline(
            x_at(rect, view, beat),
            rect.y_range(),
            Stroke::new(
                if whole { 1.0 } else { 0.75 },
                if whole { palette().beat_line } else { palette().subdivision_line },
            ),
        );
    }
    for (beat, _, _, _) in time.bars(visible_end(rect, view) + 1.0) {
        painter.vline(
            x_at(rect, view, beat),
            rect.y_range(),
            Stroke::new(1.0, palette().bar_line),
        );
    }
}

pub fn draw_ruler(ui: &Ui, rect: Rect, view: &Viewport, time: &TimeAxis, step: f64) {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));

    let baseline = rect.bottom();
    for beat in visible_steps(rect, view, step) {
        painter.vline(
            x_at(rect, view, beat),
            baseline - 16.0..=baseline,
            Stroke::new(1.0, palette().beat_line),
        );
    }
    for (beat, index, _, _) in time.bars(visible_end(rect, view) + 1.0) {
        let x = x_at(rect, view, beat);
        painter.vline(x, (baseline - 32.0).max(rect.top())..=baseline, Stroke::new(1.0, palette().bar_line));
        let text =
            painter.layout_no_wrap((index + 1).to_string(), FontId::proportional(12.0), text_color());
        let pos = pos2(x + 7.0, baseline - if current() == InterfaceTheme::Classic { 7.5 } else { 25.0 } - text.size().y / 2.0);
        if Rect::from_min_size(pos, text.size()).intersects(painter.clip_rect()) {
            painter.galley(pos, text, text_color());
        }
    }
    painter.hline(rect.x_range(), baseline, Stroke::new(1.0, edge_color()));
}

pub fn playhead_line(ui: &Ui, rect: Rect, view: &Viewport, position: i64) {
    let x = x_at(rect, view, beats(position));
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .vline(x, rect.y_range(), Stroke::new(1.0, palette().playhead));
}

pub fn playhead(ui: &Ui, rect: Rect, baseline: f32, view: &Viewport, position: i64) {
    playhead_color(ui,rect,baseline,view,position,palette().playhead);
}

pub fn hover_playhead(ui: &Ui, rect: Rect, baseline: f32, view: &Viewport, position: i64) {
    playhead_color(ui,rect,baseline,view,position,
        Color32::from_rgba_unmultiplied(palette().playhead.r(),palette().playhead.g(),palette().playhead.b(),128));
}

fn playhead_color(ui: &Ui, rect: Rect, baseline: f32, view: &Viewport, position: i64, color: Color32) {
    let x = x_at(rect, view, beats(position));
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.vline(x,baseline..=rect.bottom(),Stroke::new(1.0,color));
    let top = baseline - 14.0;
    painter.rect_filled(
        Rect::from_min_size(pos2(x - 4.0, top), vec2(8.0, 9.0)),
        CornerRadius {
            nw: 1,
            ne: 1,
            sw: 0,
            se: 0,
        },
        color,
    );
    painter.add(Shape::convex_polygon(
        vec![
            pos2(x - 4.0, top + 8.0),
            pos2(x + 4.0, top + 8.0),
            pos2(x + 4.0, top + 9.0),
            pos2(x, baseline),
            pos2(x - 4.0, top + 9.0),
        ],
        color,
        Stroke::NONE,
    ));
}
