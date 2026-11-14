use crate::{
    model::{GroupRef, Note, beats},
    ui::{state::Viewport, timeline::x_at},
};
use eframe::egui::*;

pub fn group_rect(reference: &GroupRef, canvas: Rect, view: &Viewport) -> Rect {
    let left = x_at(canvas, view, beats(reference.blick_absolute_begin));
    let right = if reference.blick_absolute_end < 0 {
        canvas.right()
    } else {
        x_at(canvas, view, beats(reference.blick_absolute_end))
    };
    Rect::from_min_max(pos2(left, canvas.top()), pos2(right, canvas.bottom()))
}
pub fn clip_rect(reference: &GroupRef, canvas: Rect, view: &Viewport) -> Rect {
    group_rect(reference, canvas, view).intersect(canvas)
}
pub fn visible_note_rect(
    note: &Note,
    reference: &GroupRef,
    canvas: Rect,
    view: &Viewport,
    top_pitch: f32,
    row_height: f32,
) -> Option<Rect> {
    let (start, end) = reference.note_range(note)?;
    let y = canvas.top() + (top_pitch - (note.pitch + reference.pitch_offset) as f32) * row_height;
    let rect = Rect::from_min_max(
        pos2(x_at(canvas, view, beats(start)), y + 1.0),
        pos2(x_at(canvas, view, beats(end)), y + row_height - 1.0),
    )
    .intersect(canvas);
    (rect.is_positive()).then_some(rect)
}

pub fn label_rect(group: Rect, canvas: Rect, width: f32) -> Option<Rect> {
    let left = group.left().max(canvas.left());
    let right = group.right().min(canvas.right());
    (right > left).then(|| {
        Rect::from_min_size(
            pos2(left, canvas.top() - 20.0),
            vec2(width.min(right - left), 20.0),
        )
    })
}
