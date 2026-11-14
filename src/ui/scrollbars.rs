use crate::ui::{state::Viewport, theme::*};
use eframe::egui::*;

pub fn horizontal(
    ui: &mut Ui,
    id: &'static str,
    rect: Rect,
    view: &mut Viewport,
    project_end: f64,
) {
    let visible = rect.width() as f64 / view.beat_width as f64;
    let total = project_end.max(128.0).max(view.start + visible);
    let width = (rect.width() * (visible / total) as f32).clamp(16.0, rect.width());
    let span = (rect.width() - width).max(1.0);
    let max_start = (total - visible).max(0.0);
    let x = rect.left()
        + if max_start > 0.0 {
            (view.start / max_start) as f32 * span
        } else {
            0.0
        };
    let handle = Rect::from_min_size(pos2(x, rect.center().y - 1.5), vec2(width, 3.0));
    let response = ui
        .interact(rect, Id::new(id), Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    ui.painter().rect_filled(
        handle,
        1,
        if response.hovered() || response.dragged_by(PointerButton::Primary) {
            TEXT
        } else {
            MUTED
        },
    );
    if response.dragged_by(PointerButton::Primary) {
        view.start = (view.start + response.drag_delta().x as f64 / span as f64 * max_start)
            .clamp(0.0, max_start);
    } else if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        view.start = ((p.x - rect.left() - width / 2.0) as f64 / span as f64 * max_start)
            .clamp(0.0, max_start);
    }
}

pub fn vertical(ui: &mut Ui, rect: Rect, top_pitch: &mut f32, row_height: f32) {
    let visible = rect.height() / row_height;
    let height = (rect.height() * visible / 128.0).clamp(18.0, rect.height());
    let span = (rect.height() - height).max(1.0);
    let y = rect.top() + (127.0 - *top_pitch) / 115.0 * span;
    let handle = Rect::from_min_size(pos2(rect.center().x - 1.5, y), vec2(3.0, height));
    let response = ui
        .interact(rect, Id::new("pitch-scrollbar"), Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::ResizeVertical);
    ui.painter().rect_filled(
        handle,
        1,
        if response.hovered() || response.dragged_by(PointerButton::Primary) {
            TEXT
        } else {
            MUTED
        },
    );
    if response.dragged_by(PointerButton::Primary) {
        *top_pitch = (*top_pitch - response.drag_delta().y / span * 115.0).clamp(12.0, 127.0);
    } else if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        *top_pitch = (127.0 - (p.y - rect.top() - height / 2.0) / span * 115.0).clamp(12.0, 127.0);
    }
}
