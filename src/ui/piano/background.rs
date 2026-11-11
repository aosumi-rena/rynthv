use crate::{
    app::App,
    ui::{theme::*, track_color::TrackColors},
};
use eframe::egui::*;

pub fn draw(app: &App, ui: &Ui, canvas: Rect) {
    let top_pitch = app.view.top_pitch;
    let row_height = app.view.row_height;
    let scale = super::scale_guide::effective(app);
    let accent = TrackColors::new(app.editor.current_track()).accent;
    let scale_color =
        black_row_color().lerp_to_gamma(Color32::from_rgb(accent.r(), accent.g(), accent.b()), 0.09);
    let painter = ui
        .painter()
        .with_clip_rect(canvas.intersect(ui.clip_rect()));
    let count = (canvas.height() / row_height).ceil() as i32 + 2;
    for row in -2..count {
        let pitch = top_pitch.floor() as i32 - row;
        let y = canvas.top() + (top_pitch - pitch as f32) * row_height;
        let r = Rect::from_min_max(pos2(canvas.left(), y), pos2(canvas.right(), y + row_height));
        let fill = if scale.enabled {
            if scale.contains(pitch) {
                scale_color
            } else {
                black_row_color()
            }
        } else if super::keyboard::white_index(pitch).is_some() {
            row_color()
        } else {
            black_row_color()
        };
        painter.rect_filled(r, 0, fill);
        painter.hline(
            r.x_range(),
            y,
            Stroke::new(0.5, if pitch.rem_euclid(12) == 0 { palette().octave_line } else { palette().row_line }),
        );
    }
}
