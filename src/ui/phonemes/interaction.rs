use super::Cell;
use crate::{
    app::App,
    model::{EntityId, GroupRef},
    synthesis::{
        plan::{GlobalBlick, TempoMapping},
        score::PhoneSource,
    },
    ui::{
        piano::Gesture,
        theme::*,
        timeline::{beat_at, x_at},
    },
};
use eframe::egui::*;

pub struct PhonemeGesture {
    reference: GroupRef,
    track: EntityId,
    source: PhoneSource,
    note_origin: f64,
    tempo: TempoMapping,
    initial_onset: f64,
    pointer_origin: f64,
    current: f64,
    body: Option<BodyEdit>,
    frame: u64,
}
struct BodyEdit {
    cell: Cell,
    canvas: Rect,
    activity: bool,
}
fn boundary_hit(cells:&[Cell],canvas:Rect,p:Pos2)->Option<&Cell> {
    if !canvas.contains(p) {return None;}
    cells.iter().rev().filter(|cell|cell.editable && (cell.rect.left()-p.x).abs()<6.0)
        .min_by(|a,b|(a.rect.left()-p.x).abs().total_cmp(&(b.rect.left()-p.x).abs()))
}
pub(super) fn show(app: &mut App, ui: &Ui, response: &Response, canvas: Rect, cells: &[Cell]) {
    let pointer = ui.input(|i| i.pointer.hover_pos());

    let hit = |p:Pos2|boundary_hit(cells,canvas,p);
    if let Some(c) = pointer.and_then(hit) {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        ui.painter()
            .vline(c.rect.left(), canvas.y_range(), Stroke::new(1.0, TEXT));
    }
    if response.double_clicked()
        && let Some(c) = pointer.and_then(|p| {
            cells
                .iter()
                .rev()
                .find(|c| c.editable && c.rect.contains(p) && p.y >= canvas.top() + 32.0)
        })
    {
        if let Some(reference) = app.editor.active_ref().cloned()
            && !app.editor.is_editing()
            && app.gesture.is_none()
            && c.input
                .notes
                .iter()
                .find(|n| n.id == c.phone.source.unwrap().note_id)
                .zip(app.editor.phoneme_note(c.phone.source.unwrap().note_id))
                .is_some_and(|(snapshot, note)| {
                    snapshot.phonemes == note.phonemes && snapshot.lyrics == note.lyrics
                })
        {
            app.editor.begin(crate::i18n::t("重置音素力度"));
            let result = ["strength", "activity", "position"]
                .into_iter()
                .try_for_each(|key| {
                    app.editor.set_phoneme_attribute(
                        &reference.group_id,
                        c.phone.source.unwrap(),
                        key,
                            (key == "strength").then_some(0.0),
                    )
                });
            if let Err(error) = result {
                app.error = Some(error);
                app.editor.cancel();
            } else {
                app.editor.finish();
            }
        }
    }
    if response.drag_started_by(PointerButton::Primary)
        && app.gesture.is_none()
        && !app.editor.is_editing()
        && let Some(p) = ui
            .input(|i| i.pointer.press_origin())
            .filter(|p| canvas.contains(*p))
        && let Some(cell) = hit(p).or_else(|| {
            cells
                .iter()
                .rev()
                .find(|c| c.editable && c.rect.contains(p) && p.y >= canvas.top() + 32.0)
        })
        && let Some(reference) = app.editor.active_ref().cloned()
    {
        let source = cell.phone.source.unwrap();
        let Some(note) = app.editor.phoneme_note(source.note_id) else {
            return;
        };
        let Some(snapshot_note) = cell.input.notes.iter().find(|n| n.id == source.note_id) else {
            return;
        };

        let stable_attributes=|attributes:&crate::model::Extra|{
            let mut attributes=attributes.clone();attributes.remove("phonemes");attributes
        };
        if note.onset != snapshot_note.onset.0 || note.duration!=snapshot_note.duration
            || note.lyrics!=snapshot_note.lyrics || note.phonemes!=snapshot_note.phonemes
            || stable_attributes(&note.attributes)!=stable_attributes(&snapshot_note.attributes) {
            return;
        }
        let note_origin = cell.input.tempo.seconds_at(GlobalBlick(
            snapshot_note.onset.0 + cell.input.content_offset.0,
        ));
        let pointer_origin =
            cell.input
                .tempo
                .seconds_at(GlobalBlick(crate::model::ticks(beat_at(
                    canvas,
                    &app.view.piano,
                    p.x,
                ))));
        let body = hit(p).is_none().then(|| BodyEdit {
            cell: cell.clone(),
            canvas,
            activity: cell.phone.has_activity,
        });
        app.editor.begin(if body.is_some() {
            crate::i18n::t("调整音素力度")
        } else {
            crate::i18n::t("调整音素时值")
        });
        app.gesture = Some(Gesture::Phoneme(Box::new(PhonemeGesture {
            reference,
            track: app.editor.track,
            source,
            note_origin,
            tempo: cell.input.tempo.clone(),
            initial_onset: cell.phone.start_seconds,
            pointer_origin,
            current: cell.phone.start_seconds,
            body,
            frame: ui.ctx().cumulative_frame_nr(),
        })));
    }
    if !matches!(&app.gesture, Some(Gesture::Phoneme(_))) {
        return;
    }
    if let Some(Gesture::Phoneme(mut g)) = app.gesture.take() {
        if app.editor.active_ref() != Some(&g.reference) || app.editor.track != g.track {
            app.editor.cancel();
            return;
        }
        if let Some(p) = response.interact_pointer_pos() {
            if let Some(body) = &g.body {

                let ratio = ((canvas.bottom() - p.y) / (canvas.height() - 32.0)).clamp(0.0, 1.0);
                let mut cell = body.cell.clone();
                let result = if body.activity {
                    let position = ((p.x - cell.rect.left()) / cell.rect.width()).clamp(0.0, 1.0);
                    let offset = cell
                        .input
                        .voice
                        .instance_options
                        .get("voice")
                        .and_then(|v| v.get("consonantStrength"))
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0) as f32;
                    cell.phone.boundary_strength = f64::from(ratio.sqrt());
                    cell.phone.boundary_position = f64::from(position);
                    app.editor
                        .set_phoneme_attribute(
                            &g.reference.group_id,
                            g.source,
                            "position",
                            Some(position),
                        )
                        .and_then(|_| {
                            app.editor.set_phoneme_attribute(
                                &g.reference.group_id,
                                g.source,
                                "activity",
                                Some(ratio.sqrt() - offset),
                            )
                        })
                } else {
                    cell.phone.strength = ratio * 2.0 - 1.0;
                    app.editor.set_phoneme_attribute(
                        &g.reference.group_id,
                        g.source,
                        "strength",
                        Some(cell.phone.strength),
                    )
                };
                if let Err(error) = result {
                    app.error = Some(error);
                    app.editor.cancel();
                    return;
                }
                let area = Rect::from_min_max(
                    pos2(cell.rect.left(), body.canvas.top() + 32.0),
                    pos2(cell.rect.right(), body.canvas.bottom()),
                );
                let painter = ui.painter().with_clip_rect(canvas);
                painter.rect_filled(area, 0, PANEL);
                super::draw::body(
                    &painter,
                    &cell,
                    area,
                    crate::ui::track_color::TrackColors::new(app.editor.current_track()).accent,
                );
            } else {
                let now = g.tempo.seconds_at(GlobalBlick(crate::model::ticks(beat_at(
                    canvas,
                    &app.view.piano,
                    p.x,
                ))));
                g.current = g.initial_onset + (now - g.pointer_origin);
                let value = (g.current - g.note_origin) as f32;
                if let Err(error) = app.editor.set_phoneme_attribute(
                    &g.reference.group_id,
                    g.source,
                    "leftOffset",
                    Some(value),
                ) {
                    app.error = Some(error);
                    app.editor.cancel();
                    return;
                }
                g.frame = ui.ctx().cumulative_frame_nr();
                let x = x_at(
                    canvas,
                    &app.view.piano,
                    crate::model::beats(g.tempo.position_at(g.current).0),
                );
                ui.painter()
                    .vline(x, canvas.y_range(), Stroke::new(1.0, GREEN));
            }
            g.frame = ui.ctx().cumulative_frame_nr();
        }
        if response.drag_stopped_by(PointerButton::Primary) {
            app.editor.finish();
        } else {
            app.gesture = Some(Gesture::Phoneme(g));
        }
    }
}
pub fn end_frame(app: &mut App, ctx: &Context) {
    let Some(Gesture::Phoneme(g)) = &app.gesture else {
        return;
    };
    let cancel = g.frame != ctx.cumulative_frame_nr()
        || app.editor.active_ref() != Some(&g.reference)
        || app.editor.track != g.track
        || ctx.input(|i| !i.focused);
    if cancel || ctx.input(|i| !i.pointer.primary_down()) {
        if cancel {
            app.editor.cancel();
        } else {
            app.editor.finish();
        }
        app.gesture = None;
        ctx.stop_dragging();
    }
}
