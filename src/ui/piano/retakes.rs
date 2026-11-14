use super::scene::SceneGroup;
use crate::{
    app::App,
    editor::RetakeSelection,
    model::TakeSet,
    ui::{
        group_geometry::visible_note_rect,
        icons::Icon,
        retakes::{self, Target},
        theme::CANVAS,
        track_color::TrackColors,
    },
};
use eframe::egui::*;

pub(super) fn show(app: &mut App, ui: &mut Ui, canvas: Rect, groups: &[SceneGroup]) {
    if !app.show_retakes || app.gesture.is_some() || app.group_gesture.is_some() {
        return;
    }
    let colors = TrackColors::new(app.editor.current_track());
    let idle_color = colors.accent.lerp_to_gamma(CANVAS, 0.30);
    for group in groups {
        for note in &group.notes {
            let Ok(takes) = TakeSet::from_extra(&note.extra) else {
                continue;
            };
            if takes.takes.len() < 2 {
                continue;
            }
            let Some(note_rect) = visible_note_rect(
                note,
                &group.reference,
                canvas,
                &app.view.piano,
                app.view.top_pitch,
                app.view.row_height,
            ) else {
                continue;
            };
            let text = if takes.active_take_id == 0 {
                String::new()
            } else {
                retakes::label(takes.active_take_id)
            };
            let galley =
                ui.painter()
                    .layout_no_wrap(text, FontId::proportional(10.5), Color32::PLACEHOLDER);
            let width = if galley.is_empty() {
                22.0
            } else {
                galley.size().x + 27.0
            };
            let rect =
                Rect::from_min_size(note_rect.left_bottom() + vec2(0.0, 4.0), vec2(width, 20.0));
            let clipped = rect.intersect(canvas).intersect(ui.clip_rect());
            if clipped.width() < 16.0 || clipped.height() < 16.0 {
                continue;
            }
            let response = ui.interact(
                clipped,
                Id::new(("note-retake", group.reference.id, note.id)),
                Sense::click_and_drag(),
            );
            let color = if response.hovered() || response.is_pointer_button_down_on() {
                colors.accent
            } else {
                idle_color
            };
            let painter = ui.painter().with_clip_rect(clipped);
            let mut child = crate::ui::child(
                ui,
                clipped,
                ("note-retake-icon", group.reference.id, note.id),
            );
            child.set_clip_rect(clipped);
            Icon::Retake.paint(
                &child,
                Rect::from_center_size(rect.left_center() + vec2(10.0, 0.0), Vec2::splat(14.0)),
                color,
            );
            painter.galley(
                rect.left_center() + vec2(22.0, -galley.size().y * 0.5),
                galley,
                color,
            );
            let selection = RetakeSelection::from_set(&takes);
            Popup::menu(&response).width(220.0).show(|ui| {
                retakes::choices_at(
                    app,
                    ui,
                    &selection,
                    Target::Note {
                        reference: &group.reference,
                        id: note.id,
                    },
                );
            });
            response
                .on_hover_cursor(CursorIcon::Default)
                .on_hover_text(crate::lformat!(
                    "{} · 点击切换此音符的采样",
                    retakes::label(takes.active_take_id)
                ));
        }
    }
}
