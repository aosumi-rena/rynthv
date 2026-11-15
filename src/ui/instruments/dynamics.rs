use crate::{
    app::App,
    model::beats,
    ui::{theme::*, timeline},
};
use eframe::egui::*;
mod edit;
fn value(app:&App,reference:crate::model::EntityId,note:&crate::model::Note)->f32 {
    super::smart::dynamic(app,reference,note)
}
pub(crate) fn finish_released(app: &mut App, ctx: &Context) {
    edit::finish_released(app, ctx);
}
pub fn show(app: &mut App, ui: &mut Ui) {
    let r = ui.max_rect();
    let (canvas, response) = crate::ui::graph_surface::canvas(ui, r, None, "dynamics-canvas");
    timeline::scroll(ui, &response, canvas, &mut app.view.piano, 10.0..=500.0);
    let pan=app.view_pan.drag(ui,crate::ui::view_pan::Target::InstrumentDynamics,canvas,app.gesture.is_none()&&app.group_gesture.is_none());
    app.view.piano.start=(app.view.piano.start-f64::from(pan.x)/f64::from(app.view.piano.beat_width)).max(0.0);
    for (label, fraction) in [("fff",0.12),("mf",0.5),("ppp",0.88)] {
        ui.painter().text(
            pos2(r.left() + 8.0, r.top() + r.height() * fraction),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0),
            MUTED,
        );
    }
    timeline::grid(
        ui,
        canvas,
        &app.view.piano,
        &app.editor.project.time,
        app.view.step(),
    );
    let p = ui
        .painter()
        .with_clip_rect(canvas.intersect(ui.clip_rect()));
    let values = canvas.shrink2(vec2(0.0, 8.0));
    let colors = crate::ui::track_color::TrackColors::new(app.editor.current_track());
    let pointer = ui.input(|i| i.pointer.press_origin().or(i.pointer.hover_pos()));
    let mut hit = None;
    for reference in app.editor.current_track().visible_references() {
        if let Some(g) = app.editor.project.group(&reference.group_id) {
            for note in &g.notes {
                if note.muted() {
                    continue;
                }
                let left = timeline::x_at(
                    canvas,
                    &app.view.piano,
                    beats(note.onset + reference.blick_offset),
                );
                let right = timeline::x_at(
                    canvas,
                    &app.view.piano,
                    beats(note.onset + reference.blick_offset + note.duration),
                );
                let y = values.bottom() - values.height() * value(app,reference.id,note).clamp(0.0,1.0);
                let active = app.editor.reference == Some(reference.id);
                let color = if active { colors.accent } else { colors.inactive };
                p.line_segment([pos2(left, y), pos2(right, y)], Stroke::new(1.0, color));
                p.circle_filled(pos2(left, y), 3.0, color);
                if active && pointer.is_some_and(|point| canvas.contains(point)
                    && point.x >= left - 7.0 && point.x <= right && (point.y-y).abs() <= 7.0) {
                    hit = Some((g.uuid.clone(), note.id));
                }
            }
        }
    }
    edit::update(app, ui, &response, values, hit);
    timeline::playhead_line(ui, canvas, &app.view.piano, app.editor.transport.position);
}
