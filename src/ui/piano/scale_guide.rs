use crate::{
    app::App,
    ui::{
        icons::{Icon, IconButton},
        scale::{Scale, ScaleGuide, Tonic},
        track_color::TrackColors,
    },
};
use eframe::egui::*;
use serde_json::Value;

pub(super) fn toggle(app: &mut App, ui: &mut Ui) {
    if ui
        .add(
            IconButton::new(Icon::Scale, crate::i18n::t("显示调式音阶"))
                .framed()
                .size(vec2(32.0, 24.0))
                .selected(app.view.scale.enabled),
        )
        .clicked()
    {
        app.view.scale.enabled = !app.view.scale.enabled;
    }
}

pub(super) fn effective(app: &App) -> ScaleGuide {
    let raw = app
        .editor
        .group()
        .and_then(|group| group.extra.get("musicalScale"));
    let tonic = raw
        .and_then(|value| value.get("root"))
        .and_then(Value::as_str)
        .and_then(Tonic::from_native);
    let scale = raw
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        .and_then(Scale::from_native);
    match (tonic, scale) {
        (Some(tonic), Some(scale)) => ScaleGuide {
            enabled: app.view.scale.enabled,
            tonic,
            scale,
        },
        _ => ScaleGuide::default(),
    }
}

pub(super) fn draw_tonic_lines(app: &App, ui: &Ui, canvas: Rect) {
    let scale = effective(app);
    if !scale.enabled {
        return;
    }
    let accent = TrackColors::new(app.editor.current_track()).accent;
    let row_height = app.view.row_height;
    let top_pitch = app.view.top_pitch;
    let painter = ui
        .painter()
        .with_clip_rect(canvas.intersect(ui.clip_rect()));
    let count = (canvas.height() / row_height).ceil() as i32 + 1;
    for row in -1..count {
        let pitch = top_pitch.floor() as i32 - row;
        if !scale.is_tonic(pitch) {
            continue;
        }
        let y = canvas.top() + (top_pitch - pitch as f32) * row_height;
        let bounds =
            Rect::from_min_max(pos2(canvas.left(), y), pos2(canvas.right(), y + row_height));
        painter.hline(
            bounds.x_range(),
            bounds.bottom(),
            Stroke::new(1.0, accent.gamma_multiply(0.35)),
        );
    }
}
