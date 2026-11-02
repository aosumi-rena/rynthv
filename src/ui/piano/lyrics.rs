use super::LyricEdit;
use crate::{app::App, model::*, ui::child};
use eframe::egui::{
    text::{CCursor, CCursorRange},
    *,
};
use std::collections::BTreeSet;
pub(super) fn lyric(app: &mut App, ui: &mut Ui, rect: Rect, reference: &GroupRef) {
    let Some(mut edit) = app.lyric.take() else {
        return;
    };
    let Some(note) = app.editor.notes().iter().find(|n| n.id == edit.id) else {
        return;
    };
    let Some(mut r) = crate::ui::group_geometry::visible_note_rect(
        note,
        reference,
        rect,
        &app.view.piano,
        app.view.top_pitch,
        app.view.row_height,
    ) else {
        app.lyric = Some(edit);
        return;
    };
    let clip = crate::ui::group_geometry::clip_rect(reference, rect, &app.view.piano);
    if edit.phonemes {
        r=Rect::from_min_max(pos2(r.left(),r.top()-app.view.row_height),pos2(r.right(),r.top()));
    }
    r.max.x = if edit.phonemes {
        let text=ui.painter().layout_no_wrap(edit.text.clone(),FontId::proportional(12.0),super::super::theme::TEXT);
        (r.min.x+(text.size().x+8.0).max(50.0)).min(clip.right())
    }else {r.max.x.max(r.min.x+130.0).min(clip.right())};
    let mut edit_ui = child(ui, r, ("lyric", edit.id));
    let output = TextEdit::singleline(&mut edit.text)
            .id(Id::new(("lyric-input", edit.id)))
            .margin(vec2(4.0, 1.0))
            .vertical_align(Align::Center).desired_width(r.width()).show(&mut edit_ui);
    let language=super::super::phoneme_candidates::language(app,note);
    let xsampa=note.attributes.get("phoneset").and_then(serde_json::Value::as_str)==Some("xsampa");
    let candidates=if edit.phonemes {super::super::phoneme_candidates::show(app,ui,&output,&mut edit.text,language,xsampa)} else {Default::default()};
    let response=(*output.response).clone();
    if edit.focus {
        response.request_focus();
        if let Some(mut state) = TextEdit::load_state(ui.ctx(), response.id) {
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(0),
                CCursor::new(edit.text.chars().count()),
            )));
            state.store(ui.ctx(), response.id);
        }
        edit.focus = false;
        ui.ctx().request_repaint();
    }
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    let tab = ui.input(|i| i.key_pressed(Key::Tab));
    let enter = ui.input(|i| i.key_pressed(Key::Enter));
    if candidates.handled {
        app.lyric=Some(edit);
    } else if escape {
        response.surrender_focus();
    } else if enter || tab || response.lost_focus() {
        if edit.text!=edit.original {
            if edit.phonemes {app.editor.set_phonemes(edit.id,edit.text);}
            else {app.editor.set_lyric(edit.id, edit.text);}
        }
        response.surrender_focus();
        if tab {
            let mut notes: Vec<_> = app
                .editor
                .notes()
                .iter()
                .filter(|n| reference.note_range(n).is_some())
                .collect();
            notes.sort_by_key(|n| n.onset);
            if let Some(next) = notes
                .iter()
                .position(|n| n.id == edit.id)
                .and_then(|i| notes.get(i + 1))
            {
                let text=if edit.phonemes {super::phonemes::text(app,reference,next)}else{next.lyrics.clone()};
                app.lyric = Some(LyricEdit {
                    id: next.id,
                    original:text.clone(),
                    text,
                    focus: true,
                    phonemes: edit.phonemes,
                });
                app.editor.selected = BTreeSet::from([next.id]);
            }
        }
    } else {
        app.lyric = Some(edit);
    }
}
