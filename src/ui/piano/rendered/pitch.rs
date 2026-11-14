use super::{pitch_y, seconds_at_x, x_at_seconds};
use crate::{
    app::App,
    synthesis::{
        plan::TempoMapping,
        rendered::{PitchTrack, VisualSnapshot},
    },
    ui::{
        group_geometry::visible_note_rect,
        piano::scene::SceneGroup,
        track_color::{TrackColors, foreground},
    },
};
use eframe::egui::*;

pub(super) fn draw(
    app: &App,
    painter: &Painter,
    canvas: Rect,
    group: &SceneGroup,
    snapshot: &VisualSnapshot,
) {
    let pitch = &snapshot.data.pitch;
    let tempo = &snapshot.input.tempo;
    let active = app.editor.reference == Some(group.reference.id);
    let color = Color32::from_rgba_unmultiplied(213, 210, 213, if active { 150 } else { 78 });
    super::pitch_band::draw(app,painter,canvas,pitch,tempo,active);
    curve(app, painter, canvas, pitch, tempo, Stroke::new(1.0, color));
    if active && !app.piano_tool.is_pitch() {
        let selected_ink = foreground(TrackColors::new(app.editor.current_track()).accent);

        for note in group
            .notes
            .iter()
            .filter(|note| !note.muted() && app.editor.selected.contains(&note.id))
        {
            let Some(rect) = visible_note_rect(
                note,
                &group.reference,
                canvas,
                &app.view.piano,
                app.view.top_pitch,
                app.view.row_height,
            ) else {
                continue;
            };
            let clipped = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
            if clipped.clip_rect().is_positive() {
                curve(
                    app,
                    &clipped,
                    canvas,
                    pitch,
                    tempo,
                    Stroke::new(1.0, selected_ink),
                );
            }
        }
    }
}

fn curve(
    app: &App,
    painter: &Painter,
    canvas: Rect,
    pitch: &PitchTrack,
    tempo: &TempoMapping,
    stroke: Stroke,
) {
    if pitch.values.is_empty() {
        return;
    }
    let clip = painter.clip_rect();
    let start = ((seconds_at_x(app, tempo, canvas, clip.left()) - pitch.start_seconds)
        / pitch.frame_seconds)
        .floor()
        .max(0.0) as usize;
    let end = ((seconds_at_x(app, tempo, canvas, clip.right()) - pitch.start_seconds)
        / pitch.frame_seconds)
        .ceil()
        .max(0.0) as usize;
    let first = start.saturating_sub(1);
    let last = end.saturating_add(2).min(pitch.values.len());
    let pixels_per_point = painter.ctx().pixels_per_point();
    for segment in &pitch.segments {
        let from = first.max(segment.start);
        let to = last.min(segment.end);
        if from >= to {
            continue;
        }
        let mut points = Vec::with_capacity((to - from).min((clip.width() as usize + 2) * 4));
        let mut bucket: Option<PixelBucket> = None;
        for index in from..to {
            let midi = pitch.values[index];
            if !midi.is_finite() {
                flush(&mut bucket, &mut points);
                paint(painter, &mut points, stroke);
                continue;
            }
            let seconds = pitch.start_seconds + index as f64 * pitch.frame_seconds;
            let point = pos2(
                x_at_seconds(app, tempo, canvas, seconds),
                pitch_y(app, canvas, midi),
            );
            let pixel = (point.x * pixels_per_point).floor() as i64;
            if bucket.as_ref().is_some_and(|bucket| bucket.pixel != pixel) {
                flush(&mut bucket, &mut points);
            }
            if let Some(bucket) = &mut bucket {
                bucket.push(index, point);
            } else {
                bucket = Some(PixelBucket::new(pixel, index, point));
            }
        }
        flush(&mut bucket, &mut points);
        paint(painter, &mut points, stroke);
    }
}

struct PixelBucket {
    pixel: i64,
    first: (usize, Pos2),
    minimum: (usize, Pos2),
    maximum: (usize, Pos2),
    last: (usize, Pos2),
}
impl PixelBucket {
    fn new(pixel: i64, index: usize, point: Pos2) -> Self {
        Self {
            pixel,
            first: (index, point),
            minimum: (index, point),
            maximum: (index, point),
            last: (index, point),
        }
    }
    fn push(&mut self, index: usize, point: Pos2) {
        if point.y < self.minimum.1.y {
            self.minimum = (index, point);
        }
        if point.y > self.maximum.1.y {
            self.maximum = (index, point);
        }
        self.last = (index, point);
    }
}
fn flush(bucket: &mut Option<PixelBucket>, points: &mut Vec<Pos2>) {
    if let Some(bucket) = bucket.take() {
        let mut ordered = [bucket.first, bucket.minimum, bucket.maximum, bucket.last];
        ordered.sort_by_key(|point| point.0);
        let mut previous = None;
        for (index, point) in ordered {
            if previous != Some(index) {
                points.push(point);
                previous = Some(index);
            }
        }
    }
}
fn paint(painter: &Painter, points: &mut Vec<Pos2>, stroke: Stroke) {
    if points.len() > 1 {
        painter.add(Shape::line(std::mem::take(points), stroke));
    } else {
        if let Some(&point) = points.first() {
            painter.circle_filled(point, stroke.width * 0.5, stroke.color);
        }
        points.clear();
    }
}
