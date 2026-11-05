mod draw;
mod interaction;
mod preview;
use crate::{
    app::App,
    model::beats,
    synthesis::{
        plan::{OwnerId, OwnerInput},
        rendered::RenderedPhone,
    },
    ui::{icons::Icon, theme::*, timeline::*},
};
use eframe::egui::*;
pub use interaction::{PhonemeGesture, end_frame};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone)]
struct Cell {
    phone: RenderedPhone,
    input: Arc<OwnerInput>,
    rect: Rect,
    editable: bool,
}
pub fn show(app: &mut App, ui: &mut Ui) {
    let rect = ui.max_rect();
    let canvas = Rect::from_min_max(rect.min + vec2(GUTTER, 0.0), rect.max);
    ui.painter().rect_filled(rect, 0, PANEL);
    Icon::Phonemes.paint(
        ui,
        Rect::from_center_size(
            pos2(rect.left() + 22.0, rect.top() + 16.0),
            vec2(18.0, 18.0),
        ),
        TEXT,
    );
    let response = ui.interact(
        canvas,
        ui.id().with("phoneme-canvas"),
        Sense::click_and_drag(),
    );
    if response.clicked() || response.drag_started() {
        app.editor.track_focus=false;
        crate::ui::lanes::parameter_deactivate(app);
        app.phoneme_focus=Some((app.editor.track,app.editor.reference));
    }
    scroll(ui, &response, canvas, &mut app.view.piano,10.0..=500.0);
    let pan=app.view_pan.drag(ui,crate::ui::view_pan::Target::Phonemes,canvas,
        app.gesture.is_none() && app.group_gesture.is_none());
    app.view.piano.start=(app.view.piano.start-f64::from(pan.x)/f64::from(app.view.piano.beat_width)).max(0.0);
    grid(
        ui,
        canvas,
        &app.view.piano,
        &app.editor.project.time,
        app.view.step(),
    );
    let labels = Rect::from_min_size(canvas.min, vec2(canvas.width(), 32.0));
    ui.painter()
        .hline(canvas.x_range(), labels.bottom(), Stroke::new(0.7, EDGE));
    let mut cells = Vec::new();
    let mut seen = BTreeSet::new();
    let color = crate::ui::track_color::TrackColors::new(app.editor.current_track()).accent;
    for reference in app.editor.current_track().visible_references() {
        let active = app.editor.reference == Some(reference.id);
        let standard=app.synthesis.resolve_voice(app.editor.current_track(),reference).ok().flatten()
            .and_then(|key|app.synthesis.library.entry(&key)).and_then(|entry|entry.family)
            .is_some_and(|family|family.is_standard());
        for snapshot in app.synthesis.phonemes(OwnerId::new(reference.id)) {
            let input = &snapshot.input;
            let (begin,limit)=input.publication_bounds();
            let Some(group)=app.editor.project.group(&reference.group_id) else {continue;};
            let mut phones=preview::phones(group,input,&snapshot.phones);
            phones.retain_mut(|phone| {
                let Some(source)=phone.source else {return false;};
                let Some(note)=group.notes.iter().find(|n|n.id==source.note_id) else {return false;};
                let Some(original)=input.notes.iter().find(|n|crate::ui::phoneme_alphabet::matches(note,n)) else {return false;};
                match crate::ui::phoneme_alphabet::label(app,reference,note,original,input.voice.dictionary.as_ref(),&phone.token) {
                    Ok(value)=>phone.token=value,
                    Err(error)=>phone.error=Some(Arc::from(error)),
                }
                true
            });
            for phone in &phones {
                let start = input.tempo.position_at(phone.start_seconds).0;
                let end = input.tempo.position_at(phone.end_seconds).0;

                if end <= begin.0 || limit.is_some_and(|limit|start>=limit.0) || phone.source.is_none() {
                    continue;
                }
                let source = phone.source.unwrap();
                if !seen.insert((reference.id, source.note_id, source.phone_index)) {
                    continue;
                }
                let r = Rect::from_min_max(
                    pos2(x_at(canvas, &app.view.piano, beats(start)), canvas.top()),
                    pos2(x_at(canvas, &app.view.piano, beats(end)), canvas.bottom()),
                );
                if !r.intersects(canvas) {
                    continue;
                }
                let painter = ui
                    .painter()
                    .with_clip_rect(canvas.intersect(ui.clip_rect()));
                painter.rect_filled(r.shrink2(vec2(0.5,0.0)),CornerRadius{nw:4,ne:4,sw:0,se:0},BUTTON.gamma_multiply(0.45));
                painter.vline(r.left(), canvas.y_range(), Stroke::new(0.7, GRID));
                if active && ui.input(|i|i.pointer.hover_pos().is_some_and(|p|r.contains(p))) {
                    painter.rect_filled(r,0,Color32::WHITE.gamma_multiply(0.04));
                    painter.vline(r.left(),canvas.y_range(),Stroke::new(1.0,TEXT));
                    painter.vline(r.right(),canvas.y_range(),Stroke::new(1.0,TEXT));
                }
                let label = r.intersect(labels);
                painter
                    .with_clip_rect(label.intersect(ui.clip_rect()))
                    .text(
                        label.center(),
                        Align2::CENTER_CENTER,
                        &phone.token,
                        FontId::proportional(12.0),
                        if active { TEXT } else { MUTED },
                    );
                let cell = Cell {
                    phone: phone.clone(),
                    input: input.clone(),
                    rect: r,
                    editable: active && !standard && phone.error.is_none(),
                };
                draw::body(
                    &painter,
                    &cell,
                    Rect::from_min_max(pos2(canvas.left(), labels.bottom()), canvas.max),
                    if active { color } else { MUTED },
                );
                cells.push(cell);
            }
        }
    }
    interaction::show(app, ui, &response, canvas, &cells);
    playhead_line(ui, canvas, &app.view.piano, app.editor.transport.position);
}

pub fn phoneme_active(app:&App)->bool {
    app.view.phonemes && app.phoneme_focus==Some((app.editor.track,app.editor.reference))
}
pub fn phoneme_deactivate(app:&mut App) { app.phoneme_focus=None; }
