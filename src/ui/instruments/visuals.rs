use crate::{app::App, model::beats, ui::timeline};
use eframe::egui::*;
fn x(app: &App, r: Rect, seconds: f64) -> f32 {
    timeline::x_at(
        r,
        &app.view.piano,
        beats(app.editor.project.time.position_at(seconds)),
    )
}
pub fn show(app: &App, ui: &Ui, r: Rect, pitch: bool) {
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    for (_id, output) in &app.synthesis.instruments.outputs {
        if output.track != app.editor.track {
            continue;
        }
        if pitch {
            let color = Color32::from_rgba_unmultiplied(215, 213, 220, 170);
            let seconds_at=|px|app.editor.project.time.seconds_at(crate::model::ticks(
                timeline::beat_at(r,&app.view.piano,px)));
            let first=((seconds_at(r.left())-output.origin)*200.).floor().max(0.) as usize;
            let last=((seconds_at(r.right())-output.origin)*200.).ceil().max(0.) as usize;
            let curves:Vec<&[f32]>=if output.member_pitch.len()>1 {
                output.member_pitch.iter().map(Vec::as_slice).collect()
            }else{vec![&output.pitch]};
            for curve in curves {
            let mut line = Vec::new();
            for i in first.saturating_sub(1)..last.saturating_add(2).min(curve.len()) {
                let hz=curve[i];
                let seconds = output.origin + i as f64 / 200.0;
                let px = x(app, r, seconds);
                if px < r.left() - 2.0 || px > r.right() + 2.0 {
                    continue;
                }
                let midi = 69.0 + 12.0 * (hz / 440.0).log2();
                if !midi.is_finite() {
                    if line.len()>1{p.add(Shape::line(std::mem::take(&mut line),Stroke::new(1.,color)));}
                    line.clear();
                    continue;
                }
                let y = r.top() + (app.view.top_pitch - midi + 0.5) * app.view.row_height;
                line.push(pos2(px, y));
            }
            if line.len() > 1 {
                p.add(Shape::line(line, Stroke::new(1.0, color)));
            }
            }
        }
    }
}

pub fn waveform_lane(app: &App, ui: &Ui) {
    let r = ui.max_rect();
    let canvas = Rect::from_min_max(r.min + vec2(crate::ui::timeline::GUTTER, 0.0), r.max);
    ui.painter().rect_filled(r, 0, crate::ui::theme::CANVAS);
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
    let color = crate::ui::track_color::TrackColors::new(app.editor.current_track())
        .accent
        .gamma_multiply(0.55);
    if let Some(output) = app.synthesis.instruments.mixer.tracks.get(&app.editor.track) {
        for audio in &output.parts {
        let samples = audio.samples();
        let channels = audio.layout().channels();
        let mut px = x(app, canvas, audio.start_seconds()).max(canvas.left());
        let end = x(app, canvas, audio.end_seconds()).min(canvas.right());
        while px < end {
            let seconds = |pos| {
                app.editor
                    .project
                    .time
                    .seconds_at(crate::model::ticks(timeline::beat_at(
                        canvas,
                        &app.view.piano,
                        pos,
                    )))
            };
            let a = ((seconds(px) - audio.start_seconds()) * 48000.0).max(0.0) as usize * channels;
            let b = (((seconds(px + 1.0) - audio.start_seconds()) * 48000.0)
                .ceil()
                .max(0.0) as usize
                * channels)
                .min(samples.len());
            if a < b {
                let peak = samples[a..b].iter().fold(0.0_f32, |p, s| p.max(s.abs()));
                let h = (peak * canvas.height() * 1.5).min(canvas.height() / 2.0);
                p.line_segment(
                    [
                        pos2(px, canvas.center().y - h),
                        pos2(px, canvas.center().y + h),
                    ],
                    Stroke::new(1.0, color),
                );
            }
            px += 1.0;
        }
        }
    }
}
