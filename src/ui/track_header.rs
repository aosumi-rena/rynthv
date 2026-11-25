mod source;
mod menu;
pub use menu::show as context_menu;

use super::{icons::Icon, theme::*};
use crate::{app::App, model::Track};
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect, track: &Track) {
    let response = ui.interact(rect, Id::new(("track", track.id)), Sense::click());
    if response.clicked() {
        app.commit_lyric();
        let modifiers=ui.input(|i|i.modifiers);
        app.editor.select_track_header(track.id,modifiers.command||modifiers.ctrl,modifiers.shift);
    }
    let name_rect = Rect::from_min_max(
        rect.min + vec2(16.0, 5.0),
        pos2(rect.right() - 95.0, rect.top() + 29.0),
    );
    if response.double_clicked()
        && response.interact_pointer_pos().is_some_and(|p| name_rect.contains(p))
    {
        app.rename = Some(super::rename::Rename::track(track.id, &track.name));
    }
    response.context_menu(|ui| context_menu(app, ui, track));
    let text_rect = Rect::from_min_max(
        rect.min + vec2(16.0, 0.0),
        pos2(rect.right() - 95.0, rect.bottom()),
    );
    let painter = ui
        .painter()
        .with_clip_rect(ui.clip_rect().intersect(text_rect));
    if !super::rename::track_field(app, ui, name_rect, track.id) {
        painter.text(
            rect.min + vec2(18.0, 17.0),
            Align2::LEFT_CENTER,
            &track.name,
            FontId::proportional(12.0),
            TEXT,
        );
    }
    source::show(app, ui, rect, text_rect, track);
    let morphing = app.synthesis.morph_pending(track.id);
    if morphing || app.synthesis.morph_error(track.id).is_some() {
        let at = pos2(text_rect.right() - 3.0, rect.top() + 17.0);
        let (label, color) = if morphing { (crate::i18n::t("ROCOFLEX_MORPHING"), MUTED) }
        else { (crate::i18n::t("ROCOFLEX_RENDER_ERROR"), Color32::LIGHT_RED) };
        let galley = painter.layout_no_wrap(label.to_owned(), FontId::proportional(10.0), color);
        let badge = Rect::from_min_size(at - vec2(galley.size().x + 4.0, galley.size().y * 0.5), galley.size()).expand2(vec2(4.0, 1.0));
        if badge.left() > text_rect.left() + 52.0 {
            painter.rect_filled(badge, 3, PANEL);
            painter.galley(badge.min + vec2(4.0, 1.0), galley, color);
            if let Some(error) = app.synthesis.morph_error(track.id) {
                ui.interact(badge, response.id.with("morph-error"), Sense::hover()).on_hover_text(crate::i18n::message(error));
            }
        }
    }
    if rect.width() < 210.0 {
        return;
    }
    for (field, label, center, min, max, reset) in [
        (
            "gainDecibel",
            crate::i18n::t("音量"),
            pos2(rect.right() - 79.0, rect.top() + 23.0),
            -48.0,
            12.0,
            0.0,
        ),
        (
            "pan",
            crate::i18n::t("声像"),
            pos2(rect.right() - 49.0, rect.top() + 23.0),
            -1.0,
            1.0,
            0.0,
        ),
    ] {
        let old = track
            .mixer
            .get(field)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(reset);
        let (value, response) = knob(ui, (track.id, field), center, old, min, max, reset);
        let response = response.on_hover_text(format!("{label}  {old:.2}"));
        if response.drag_started() || response.double_clicked() {
            app.editor.begin(crate::i18n::t("调整混音"));
        }
        if value != old {
            app.editor
                .project
                .tracks
                .iter_mut()
                .find(|t| t.id == track.id)
                .unwrap()
                .mixer
                .insert(field.into(), value.into());
        }
        if response.drag_stopped() || response.double_clicked() {
            app.editor.finish();
        }
        if field == "pan" {
            Icon::Pan.paint(
                ui,
                Rect::from_center_size(pos2(center.x, rect.top() + 43.0), vec2(13.0, 13.0)),
                Color32::from_rgb(120, 117, 124),
            );
        } else {
            let x = center.x - 6.0;
            for i in 0..6 {
                ui.painter().vline(
                    x + i as f32 * 2.0,
                    rect.top() + 45.0 - (1.0 + i as f32)..=rect.top() + 45.0,
                    Stroke::new(1.0, MUTED.gamma_multiply(0.55)),
                );
            }
        }
    }
    for (field, text, y) in [("mute", "M", 10.0), ("solo", "S", 30.0)] {
        let value = track
            .mixer
            .get(field)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let r = Rect::from_min_size(pos2(rect.right() - 27.0, rect.top() + y), vec2(16.0, 16.0));
        let response = ui.interact(r, Id::new((track.id, field)), Sense::click());
        ui.painter().rect_filled(
            r,
            2,
            if value {
                crate::ui::track_color::TrackColors::new(track).accent
            } else if response.hovered() {
                BUTTON_HOVER
            } else {
                BUTTON
            },
        );
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(9.0),
            if value { CANVAS } else { TEXT },
        );
        if response
            .on_hover_text(if field == "mute" {
                crate::i18n::t("音轨静音")
            } else {
                crate::i18n::t("音轨独奏")
            })
            .clicked()
        {
            app.editor.begin(crate::i18n::t("音轨混音"));
            app.editor
                .project
                .tracks
                .iter_mut()
                .find(|t| t.id == track.id)
                .unwrap()
                .mixer
                .insert(field.into(), (!value).into());
            app.editor.finish();
        }
    }
}
fn knob(
    ui: &mut Ui,
    id: (u64, &str),
    center: Pos2,
    value: f64,
    min: f64,
    max: f64,
    reset: f64,
) -> (f64, Response) {
    let rect = Rect::from_center_size(center, vec2(21.0, 25.0));
    let response = ui.interact(rect, Id::new(id), Sense::click_and_drag());
    ui.painter().circle_filled(
        center,
        9.5,
        if response.hovered() {
            BUTTON_HOVER
        } else {
            Color32::from_rgb(77, 75, 81)
        },
    );
    let normalized = if value <= reset {
        ((value - reset) / (reset - min).max(1e-6)) * 0.5 + 0.5
    } else {
        ((value - reset) / (max - reset).max(1e-6)) * 0.5 + 0.5
    };
    let angle = (-225.0 + normalized as f32 * 270.0).to_radians();
    let end = center + vec2(angle.cos(), angle.sin()) * 8.0;
    ui.painter()
        .line_segment([center, end], Stroke::new(1.0, MUTED));
    let next = if response.double_clicked() {
        reset
    } else if response.dragged() {
        (value - response.drag_delta().y as f64 * (max - min) / 160.0).clamp(min, max)
    } else {
        value
    };
    (next, response)
}
