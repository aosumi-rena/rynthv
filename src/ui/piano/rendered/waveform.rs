use super::{pitch_y, position_at_x, seconds_at_x, waveform_flash::Flash, x_at_seconds};
use crate::{
    app::App,
    model::beats,
    synthesis::{
        plan::{NoteInput, OwnerInput},
        rendered::{VisualSnapshot, VisualTimeline, Waveform},
    },
    ui::{piano::scene::SceneGroup, timeline::x_at},
};
use eframe::egui::*;
use std::time::Instant;

pub(super) fn draw(
    app: &App,
    painter: &Painter,
    canvas: Rect,
    group: &SceneGroup,
    snapshot: &VisualSnapshot,
    notes: &[&NoteInput],
    visuals: &VisualTimeline,
    now: Instant,
) -> bool {
    let track = app.editor.current_track();
    let inputs = Draw { app, painter, canvas, group, snapshot, notes, visuals, now };
    if app.synthesis.morph_enabled(track.id) {
        let left = seconds_at_x(app, &snapshot.input.tempo, canvas, painter.clip_rect().left());
        let right = seconds_at_x(app, &snapshot.input.tempo, canvas, painter.clip_rect().right());
        let mut animating = false;
        for waveform in app.synthesis.morph_waveforms_in(track.id, left..right) {
            animating |= draw_waveform(&inputs, waveform);
        }
        animating
    } else {
        draw_waveform(&inputs, &snapshot.data.waveform)
    }
}

pub(super) fn active_notes(input: &OwnerInput) -> Vec<&NoteInput> {
    input.notes.iter().filter(|note|
        note.attributes.get("muted").and_then(serde_json::Value::as_bool) != Some(true)).collect()
}

#[derive(Clone, Copy)]
struct Draw<'a> {
    app: &'a App,
    painter: &'a Painter,
    canvas: Rect,
    group: &'a SceneGroup,
    snapshot: &'a VisualSnapshot,
    notes: &'a [&'a NoteInput],
    visuals: &'a VisualTimeline,
    now: Instant,
}

fn draw_waveform(context: &Draw<'_>, waveform: &Waveform) -> bool {
    let Draw { app, painter, canvas, group, snapshot, notes, visuals, now } = *context;
    let input = &snapshot.input;
    let tempo = &input.tempo;
    let active = app.editor.reference == Some(group.reference.id);
    let color = Color32::from_rgba_unmultiplied(166, 164, 170, if active { 72 } else { 40 });
    let clip = painter.clip_rect();
    let flash_active = visuals.animating_in(
        position_at_x(app, canvas, clip.left())..position_at_x(app, canvas, clip.right()),
        now,
    );
    let mut animating = false;
    let pixel = painter.ctx().pixels_per_point().recip();
    let scale = app.view.row_height * 2.0;
    let audio_left = x_at_seconds(app, tempo, canvas, waveform.start_seconds());
    let audio_right = x_at_seconds(app, tempo, canvas, waveform.end_seconds());
    let visible_left = audio_left.max(clip.left());
    let visible_right = audio_right.min(clip.right());
    if visible_right <= visible_left { return false; }
    let onset_x = |note: &NoteInput| {
        x_at(
            canvas,
            &app.view.piano,
            beats(note.onset.0 + input.content_offset.0),
        )
    };
    let first = notes.partition_point(|note| onset_x(note) <= visible_left).saturating_sub(1);
    for (index, note) in notes.iter().enumerate().skip(first) {

        let left = if index == 0 {
            f32::NEG_INFINITY
        } else {
            onset_x(note)
        }
        .max(visible_left);
        if left >= visible_right { break; }
        let right = notes
            .get(index + 1)
            .map_or(audio_right, |next| onset_x(next))
            .min(visible_right);
        if right <= left {
            continue;
        }
        let center = pitch_y(app, canvas, (note.pitch + input.pitch_offset) as f32)
            + app.view.row_height * 2.5;
        if center + scale < clip.top() || center - scale > clip.bottom() {
            continue;
        }
        let mut mesh = Mesh::default();
        let [r,g,b]=app.preferences.voice_accent;
        let mut flash = Flash::new(clip, pixel, Color32::from_rgb(r,g,b));
        let columns = ((right - left) / pixel).ceil() as usize;
        mesh.reserve_vertices(columns * 4);
        mesh.reserve_triangles(columns * 2);
        for column in 0..columns {
            let x = left + column as f32 * pixel;
            let next = (x + pixel).min(right);
            let Some((minimum, maximum)) = waveform.peak_range(
                seconds_at_x(app, tempo, canvas, x),
                seconds_at_x(app, tempo, canvas, next),
            ) else {
                flash.gap(x);
                continue;
            };
            if minimum == 0.0 && maximum == 0.0 {
                flash.gap(x);
                continue;
            }

            let top = center - maximum.clamp(-1.0, 1.0) * scale;
            let bottom = (center - minimum.clamp(-1.0, 1.0) * scale).max(top + pixel * 0.25);
            mesh.add_colored_rect(Rect::from_min_max(pos2(x, top), pos2(next, bottom)), color);
            if flash_active {
                let midpoint = (x + next) * 0.5;
                let intensity = visuals.intensity(position_at_x(app, canvas, midpoint), now);
                if column == 0 {
                    flash.push(x, top, bottom, intensity);
                }
                flash.push(midpoint, top, bottom, intensity);
            }
        }
        flash.extend_to(right);
        painter.add(Shape::mesh(mesh));
        animating |= flash.paint(painter);
    }
    animating
}
