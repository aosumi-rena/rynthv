mod geometry;
mod interaction;
use crate::{
    app::App,
    model::{GroupRef, NotePerformance, beats},
    synthesis::plan::{NoteInput, OwnerId},
    ui::{theme::*, timeline::x_at},
};
use eframe::egui::*;
pub use interaction::{RapGesture, end_frame};
use std::collections::BTreeSet;

#[derive(Clone)]
struct Cell {
    reference: GroupRef,
    note: NoteInput,
    area: Rect,
    left: f32,
    right: f32,
    tone: f32,
    intonation: f32,
    editable: bool,
    explicit: bool,
    end: f32,
}
impl Cell {
    fn point(&self, t: f32) -> Pos2 {
        geometry::point(
            self.area,
            self.left,
            self.right,
            self.tone,
            self.intonation,
            t * self.end,
        )
    }
}
pub(super) fn show(app: &mut App, ui: &Ui, response: &Response, canvas: Rect, foreground: bool) {
    let area = Rect::from_min_max(canvas.min + vec2(0.0, 2.0), canvas.max);
    if area.height() <= 1.0 {
        return;
    }
    let mut cells = Vec::new();
    let mut seen = BTreeSet::new();
    for reference in app.editor.current_track().visible_references() {
        let Some(group) = app.editor.project.group(&reference.group_id) else {
            continue;
        };
        for piece in app.synthesis.visuals(OwnerId::new(reference.id)).flat_map(|v|v.pieces()) {
            for predicted in piece.snapshot.data.pitch.rap_controls.iter() {
                let Some(snapshot) = piece
                    .snapshot
                    .input
                    .notes
                    .iter()
                    .find(|n| n.id == predicted.note_id)
                else {
                    continue;
                };
                let onset = snapshot.onset.0 + piece.snapshot.input.content_offset.0;
                if onset + snapshot.duration <= piece.range.start
                    || onset >= piece.range.end
                    || !seen.insert((reference.id, predicted.note_id))
                {
                    continue;
                }
                let Some(note) = group.notes.iter().find(|n| n.id == predicted.note_id) else {
                    continue;
                };
                if note.musical_type != "rap" {
                    continue;
                }
                let Ok(performance) = NotePerformance::from_note(note) else {
                    continue;
                };
                let Some((start, end)) = reference.note_range(note) else {
                    continue;
                };
                let left = x_at(canvas, &app.view.piano, beats(start));
                let right = x_at(canvas, &app.view.piano, beats(end));
                if right < canvas.left() || left > canvas.right() || right <= left {
                    continue;
                }
                let tone = performance.r_tone.unwrap_or(predicted.tone);
                let intonation = performance.r_intonation.unwrap_or(predicted.intonation);
                cells.push(Cell {
                    reference: reference.clone(),
                    note: snapshot.clone(),
                    area,
                    left,
                    right,
                    tone,
                    intonation,
                    end: geometry::end(area, tone, intonation),
                    editable: foreground
                        && app.editor.reference == Some(reference.id)
                        && NoteInput::from(note) == *snapshot,
                    explicit: performance.r_tone.is_some() || performance.r_intonation.is_some(),
                });
            }
        }
    }
    let painter = ui
        .painter()
        .with_clip_rect(canvas.intersect(ui.clip_rect()));
    for cell in &cells {
        let color = if !foreground {
            MUTED.gamma_multiply(0.45)
        } else if cell.explicit {
            GREEN
        } else {
            MUTED
        };
        let points: Vec<_> = (0..=32).map(|i| cell.point(i as f32 / 32.0)).collect();
        if cell.explicit && foreground {

            for p in points.windows(2) {
                painter.add(Shape::convex_polygon(
                    vec![
                        p[0],
                        p[1],
                        pos2(p[1].x, area.bottom()),
                        pos2(p[0].x, area.bottom()),
                    ],
                    GREEN.gamma_multiply(0.10),
                    Stroke::NONE,
                ));
            }
        }
        painter.add(Shape::line(points, Stroke::new(1.0, color)));
        let tip = cell.point(1.0);
        let direction = (tip - cell.point(0.95)).normalized();
        let normal = vec2(-direction.y, direction.x);
        painter.add(Shape::convex_polygon(
            vec![
                tip,
                tip - direction * 5.0 + normal * 3.0,
                tip - direction * 5.0 - normal * 3.0,
            ],
            color,
            Stroke::NONE,
        ));
    }
    if foreground {
        interaction::show(app, ui, response, &cells);
    }
}
