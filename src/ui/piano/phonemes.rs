use super::scene::SceneGroup;
use crate::{app::App,model::{GroupRef,Note},synthesis::plan::OwnerId,ui::{group_geometry,theme::*}};
use eframe::egui::*;
use std::collections::BTreeMap;

fn readback(app:&App,reference:&GroupRef,note:&Note)->(String,Option<String>) {
    let mut phones=BTreeMap::new();let mut error=None;
    for snapshot in app.synthesis.phonemes(OwnerId::new(reference.id)) {
        let Some(source_note)=snapshot.input.notes.iter().find(|n|crate::ui::phoneme_alphabet::matches(note,n)) else {continue;};
        for phone in snapshot.phones.0.iter() {
            if let Some(source)=phone.source.filter(|s|s.note_id==note.id) {
                // Published labels already use the job's selected alphabet.
                // Only convert an older snapshot while a toggle is rerendering.
                let token=match crate::ui::phoneme_alphabet::label(app,reference,note,source_note,snapshot.input.voice.dictionary.as_ref(),&phone.token) {
                    Ok(value)=>value,Err(message)=>{error=Some(message);phone.token.to_string()}
                };
                phones.insert(source.phone_index,token);
                if let Some(message)=&phone.error {error=Some(message.to_string());}
            }
        }
    }
    let text=phones.into_values().collect::<Vec<_>>().join(" ");
    (text,error)
}
pub(super) fn text(app:&App,reference:&GroupRef,note:&Note)->String {
    if !note.phonemes.is_empty() {return note.phonemes.clone();}
    readback(app,reference,note).0
}
fn label_rect(app:&App,canvas:Rect,reference:&GroupRef,note:&Note)->Option<Rect> {
    let r=group_geometry::visible_note_rect(note,reference,canvas,&app.view.piano,app.view.top_pitch,app.view.row_height)?;
    Some(Rect::from_min_max(pos2(r.left(),r.top()-18.0),pos2(r.right(),r.top())).intersect(canvas))
}
pub(super) fn hit(app:&App,groups:&[SceneGroup],canvas:Rect,pointer:Pos2)->Option<(GroupRef,Note)> {
    groups.iter().rev().find_map(|group|group.notes.iter().rev().find(|note|
        label_rect(app,canvas,&group.reference,note).is_some_and(|r|r.contains(pointer)))
        .map(|note|(group.reference.clone(),note.clone())))
}
pub(super) fn draw(app:&App,ui:&Ui,canvas:Rect,groups:&[SceneGroup]) {
    let painter=ui.painter().with_clip_rect(canvas.intersect(ui.clip_rect()));
    for group in groups {
        for note in &group.notes {
            let Some(label)=label_rect(app,canvas,&group.reference,note) else {continue;};
            let (computed,error)=readback(app,&group.reference,note);
            let value=if error.is_some() {"!"} else if !note.phonemes.is_empty() {note.phonemes.as_str()}else{&computed};
            painter.with_clip_rect(label).text(label.left_center()+vec2(4.0,0.0),Align2::LEFT_CENTER,
                value,FontId::proportional(12.0),if error.is_some(){Color32::LIGHT_RED}else{TEXT});
            if let Some(error)=error {
                let note_rect=super::canvas::note_rect(note,group.reference.blick_offset,group.reference.pitch_offset,canvas,app);
                ui.interact(label.union(note_rect).intersect(canvas),ui.id().with(("note-phoneme-error",group.reference.id,note.id)),Sense::hover())
                    .on_hover_text(crate::i18n::message(&error));
            }
        }
    }
}
