use crate::editor::GroupEdit;
use crate::{
    app::App,
    model::*,
    ui::{
        group_edit,
        group_geometry::*,
        theme::*,
        track_color::{TrackColors, foreground},
    },
};
use eframe::egui::*;

pub struct SceneGroup {
    pub reference: GroupRef,
    pub notes: Vec<Note>,
}
pub fn snapshot(app: &App) -> Vec<SceneGroup> {
    let mut groups: Vec<_> = app
        .editor
        .current_track()
        .visible_references()
        .filter_map(|r| {
            app.editor.project.group(&r.group_id).map(|g| SceneGroup {
                reference: r.clone(),
                notes: g.notes.clone(),
            })
        })
        .collect();
    groups.sort_by_key(|g| app.editor.reference == Some(g.reference.id));
    groups
}
pub fn hit_note(
    app: &App,
    groups: &[SceneGroup],
    canvas: Rect,
    pos: Pos2,
) -> Option<(GroupRef, Note)> {
    groups.iter().rev().find_map(|g| {
        g.notes
            .iter()
            .rev()
            .find(|n| {
                visible_note_rect(
                    n,
                    &g.reference,
                    canvas,
                    &app.view.piano,
                    app.view.top_pitch,
                    app.view.row_height,
                )
                .is_some_and(|r| r.contains(pos))
            })
            .map(|n| (g.reference.clone(), n.clone()))
    })
}
pub fn draw_notes(app: &App, ui: &Ui, canvas: Rect, groups: &[SceneGroup]) {
    let colors = TrackColors::new(app.editor.current_track());
    let drawing_pitch = app.piano_tool.is_pitch();
    for g in groups {
        let active = app.editor.reference == Some(g.reference.id);
        let issues=crate::synthesis::plan::note_issues::timeline(g.notes.iter().map(|n|
            (n.id,n.onset,n.duration,n.pitch.saturating_add(g.reference.pitch_offset),n.muted())));
        let painter = ui.painter().with_clip_rect(
            clip_rect(&g.reference, canvas, &app.view.piano).intersect(ui.clip_rect()),
        );
        for note in &g.notes {
            if visible_note_rect(
                note,
                &g.reference,
                canvas,
                &app.view.piano,
                app.view.top_pitch,
                app.view.row_height,
            )
            .is_none()
            {
                continue;
            }
            let r = super::canvas::note_rect(
                note,
                g.reference.blick_offset,
                g.reference.pitch_offset,
                canvas,
                app,
            );
            let selected = active && !drawing_pitch && app.editor.selected.contains(&note.id);
            let invalid=issues.contains_key(&note.id)||super::phonemes::error(app,&g.reference,note).is_some();
            let color = if note.muted() || invalid {
                MUTED.gamma_multiply(0.4)
            } else if selected {
                colors.accent
            } else if active {
                colors.note
            } else {
                colors.inactive
            };
            let color = if drawing_pitch {
                color.gamma_multiply(0.45)
            } else {
                color
            };
            let corner_radius = if note.musical_type == NoteMode::Rap.id() {
                r.height() / 2.0
            } else {
                2.0
            };
            painter.rect_filled(r, corner_radius, color);
            if !drawing_pitch {
                painter.rect_stroke(
                    r,
                    corner_radius,
                    Stroke::new(
                        0.7,
                        if selected {
                            colors.highlight
                        } else {
                            colors
                                .accent
                                .gamma_multiply(if active { 0.7 } else { 0.35 })
                        },
                    ),
                    StrokeKind::Inside,
                );
            }
            painter
                .with_clip_rect(r.intersect(painter.clip_rect()))
                .text(
                    r.left_center() + vec2(6.0, 0.0),
                    Align2::LEFT_CENTER,
                    &note.lyrics,
                    FontId::proportional(12.0),
                    if drawing_pitch || (note.muted() && active) {
                        TEXT
                    } else if active {
                        foreground(color)
                    } else {
                        MUTED
                    },
                );
        }
    }
}
pub fn headers(app: &mut App, ui: &mut Ui, canvas: Rect, groups: &[SceneGroup]) {
    let track = app.editor.track;
    let colors = TrackColors::new(app.editor.current_track());
    let drawing_pitch = app.piano_tool.is_pitch();
    for g in groups {
        let r = group_rect(&g.reference, canvas, &app.view.piano);
        let color = if app.editor.reference == Some(g.reference.id) {
            colors.accent
        } else {
            colors.note
        };
        let header_clip = Rect::from_min_max(pos2(canvas.left(), canvas.top() - 20.0), canvas.max)
            .intersect(ui.clip_rect());
        let painter = ui.painter().with_clip_rect(header_clip);
        if !r.intersects(canvas) {
            continue;
        }
        painter.rect_stroke(r, 2, Stroke::new(1.0, color), StrokeKind::Inside);
        let name = app
            .editor
            .project
            .group(&g.reference.group_id)
            .map(|g| g.name.as_str())
            .unwrap_or("");
        let name = if name == "main" { crate::i18n::t("主音符组") } else { name };
        let ink = foreground(color);
        let text = painter.layout_no_wrap(name.into(), FontId::proportional(12.0), ink);
        if let Some(tag) = label_rect(r, canvas, text.size().x + 12.0) {
            painter.rect_filled(
                tag,
                CornerRadius {
                    nw: 3,
                    ne: 3,
                    sw: 0,
                    se: 0,
                },
                color,
            );
            painter.with_clip_rect(tag.intersect(header_clip)).galley(
                pos2(tag.left() + 6.0, tag.center().y - text.size().y / 2.0),
                text,
                ink,
            );
            let response = ui.interact(
                tag,
                Id::new(("group-label", g.reference.id)),
                if drawing_pitch {
                    Sense::click()
                } else {
                    Sense::click_and_drag()
                },
            );
            if response.clicked() || response.secondary_clicked() {
                app.commit_lyric();
                app.editor.select_reference(track, g.reference.id);
            }
            if !drawing_pitch
                && response.drag_started_by(PointerButton::Primary)
                && g.reference.blick_absolute_end >= 0
                && let Some(origin) = ui.input(|i| i.pointer.press_origin())
            {
                group_edit::start(
                    app,
                    &g.reference,
                    track,
                    origin,
                    GroupEdit::Move,
                    app.view.piano.beat_width,
                    false,
                );
            }
            response.context_menu(|ui| group_edit::context_menu(app, ui, &g.reference));
        }
        if !drawing_pitch && g.reference.blick_absolute_end >= 0 {
            for (side, x, kind) in [
                ("left", r.left(), GroupEdit::Left),
                ("right", r.right(), GroupEdit::Right),
            ] {
                if x < canvas.left() || x > canvas.right() {
                    continue;
                }
                let edge =
                    Rect::from_min_max(pos2(x - 3.0, canvas.top()), pos2(x + 3.0, canvas.bottom()))
                        .intersect(canvas);
                let response = ui
                    .interact(
                        edge,
                        Id::new(("piano-group-edge", g.reference.id, side)),
                        Sense::drag(),
                    )
                    .on_hover_cursor(CursorIcon::ResizeHorizontal);
                if response.drag_started_by(PointerButton::Primary)
                    && let Some(origin) = ui.input(|i| i.pointer.press_origin())
                {
                    group_edit::start(
                        app,
                        &g.reference,
                        track,
                        origin,
                        kind,
                        app.view.piano.beat_width,
                        false,
                    );
                }
            }
        }
    }
    if app.group_gesture.as_ref().is_some_and(|g| !g.arrangement) {
        group_edit::update(app, ui.ctx(), None);
    }
}
