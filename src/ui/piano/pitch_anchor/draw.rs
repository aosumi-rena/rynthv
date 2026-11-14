use super::{geometry, *};
use crate::ui::{group_geometry::clip_rect, theme::GREEN};

pub(in crate::ui::piano) fn draw(app: &App, ui: &Ui, canvas: Rect) {
    if app.piano_tool != Tool::PitchAnchor {
        return;
    }
    let Some(reference) = app.editor.active_ref() else {
        return;
    };
    if app.editor.group().is_none() {
        return;
    }
    let clip = clip_rect(reference, canvas, &app.view.piano).intersect(ui.clip_rect());
    if !clip.is_positive() {
        return;
    }
    let painter = ui.painter().with_clip_rect(clip);
    if let Some(rect) = geometry::selected_rect(app, canvas, reference) {
        if rect.width() > 14.01 || rect.height() > 14.01 {
            painter.rect_filled(rect, 3, Color32::from_white_alpha(24));
            painter.rect_stroke(
                rect,
                3,
                Stroke::new(1.0, Color32::from_white_alpha(105)),
                StrokeKind::Inside,
            );
        }
        for (_, handle) in geometry::handles(rect) {
            painter.rect_filled(handle, 1, Color32::WHITE);
        }
    }
    for control in &controls(app) {
        match control {
            PitchControl::Point(point) => {
                let position = geometry::screen(app, canvas, reference, point.pos, point.pitch);
                let selected = app.piano_anchor.ids.contains(&point.id);
                let color = if selected { Color32::WHITE } else { GREEN };
                painter.circle_filled(position, 3.0, color);
                painter.circle_stroke(
                    position,
                    3.0,
                    Stroke::new(0.7, Color32::from_black_alpha(100)),
                );
            }
            PitchControl::Curve(curve) if app.piano_anchor.ids.contains(&curve.id) => {
                let points = curve
                    .absolute_points()
                    .map(|(time, pitch)| geometry::screen(app, canvas, reference, time, pitch))
                    .collect::<Vec<_>>();
                if points.len() >= 2 {
                    painter.add(Shape::line(points, Stroke::new(1.5, Color32::WHITE)));
                }
            }
            _ => {}
        }
    }
    if let Some(Gesture::Anchor(gesture)) = &app.gesture
        && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
        && let Some(rect) = gesture.selection_rect(pointer)
    {
        painter.rect_filled(rect, 0, GREEN.gamma_multiply(0.12));
        painter.rect_stroke(rect, 0, Stroke::new(1.0, GREEN), StrokeKind::Inside);
    }
}
