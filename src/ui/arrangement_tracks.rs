use super::{group_edit, theme::*, timeline::*, track_color::TrackColors};
use crate::{app::App, editor::GroupEdit, model::*};
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui, timeline_left: f32, scroll: f32) {
    let rect = ui.max_rect();
    let tracks = app.editor.project.tracks.clone();
    let mut track_rects = Vec::new();
    for (index, track) in tracks.iter().enumerate() {
        let colors = TrackColors::new(track);
        let y = rect.top() + index as f32 * metrics().track_height - scroll;
        let head = Rect::from_min_size(
            pos2(rect.left(), y),
            vec2(timeline_left - rect.left(), metrics().track_height),
        );
        let body = Rect::from_min_max(pos2(timeline_left, y), pos2(rect.right(), y + metrics().track_height));
        track_rects.push((track.id, body));
        if !head.intersects(rect) {
            continue;
        }
        ui.painter().rect_filled(
            head,
            0,
            if app.editor.selected_tracks.contains(&track.id) {
                Color32::from_rgb(57, 55, 60)
            } else {
                panel_color()
            },
        );
        ui.painter().rect_filled(
            Rect::from_min_size(head.min, vec2(5.0, head.height())),
            0,
            colors.accent,
        );
        super::track_header::show(app, ui, head, track);
        if track.is_instrumental() {
            super::instrumental::show(app, ui, body, track);
            continue;
        }
        let response = ui.interact(body, Id::new(("track-body", track.id)), Sense::click());
        let pointer = response.interact_pointer_pos();
        let at = pointer.map(|p| {
            ticks(
                app.view
                    .snap_beat(beat_at(body, &app.view.arrangement, p.x)),
            )
            .max(0)
        });

        let context_id = Id::new(("insert-group-position", track.id));
        if response.secondary_clicked()
            && let Some(at) = at
        {
            ui.ctx().data_mut(|d| d.insert_temp(context_id, at));
        }
        if response.double_clicked()
            && let Some(at) = at
        {
            app.commit_lyric();
            app.editor.add_group_at(track.id, at);
        } else if response.clicked() {
            app.commit_lyric();
            app.editor.select_track(track.id);
            app.editor.group_focus=true;
            app.editor.selected_groups.clear();
        }
        crate::ui::theme::menu::context_menu(&response,|ui| {
            if ui.button(crate::i18n::t("插入新的音符组")).clicked() {
                let at = ui
                    .ctx()
                    .data(|d| d.get_temp::<i64>(context_id))
                    .unwrap_or(0);
                app.commit_lyric();
                app.editor.add_group_at(track.id, at);
                ui.close();
            }
            ui.separator();
            let enabled=rynthv::clipboard::can_paste_groups() && !app.editor.is_editing();
            for (label,linked) in [("粘贴",false),("作为链接音符组粘贴",true)] {
                if ui.add_enabled(enabled,Button::new(crate::i18n::t(label))).clicked(){
                    let at=ui.ctx().data(|d|d.get_temp::<i64>(context_id)).unwrap_or(0);
                    super::chrome::edit::request_group_paste(app,ui.ctx(),track.id,at,linked);ui.close();
                }
            }
        });
        let mut references: Vec<_> = track.visible_references().collect();
        references.sort_by_key(|r| app.editor.reference == Some(r.id));
        for reference in references {
            group(app, ui, body, track, reference);
        }
    }
    if app.group_gesture.as_ref().is_some_and(|g| g.arrangement) {
        let target = ui.input(|i| i.pointer.interact_pos()).and_then(|p| {
            track_rects
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(id, _)| *id)
        });
        group_edit::update(app, ui.ctx(), target);
    }
}
fn group(app: &mut App, ui: &mut Ui, body: Rect, track: &Track, reference: &GroupRef) {
    let colors = TrackColors::new(track);
    let Some(group) = app.editor.project.group(&reference.group_id) else {
        return;
    };
    let end = if reference.blick_absolute_end < 0 {
        group
            .notes
            .iter()
            .filter_map(|n| reference.note_range(n).map(|r| r.1))
            .max()
            .unwrap_or(0)
    } else {
        reference.blick_absolute_end
    };
    let region = Rect::from_min_max(
        pos2(
            x_at(
                body,
                &app.view.arrangement,
                beats(reference.blick_absolute_begin),
            ),
            body.top() + 1.0,
        ),
        pos2(
            x_at(body, &app.view.arrangement, beats(end)),
            body.bottom() - 1.0,
        ),
    );
    let clip = region.intersect(body).intersect(ui.clip_rect());
    if !clip.is_positive() {
        return;
    }
    let selected = if app.editor.group_focus {app.editor.selected_groups.contains(&(track.id,reference.id))}else{app.editor.reference==Some(reference.id)};
    let missing=app.synthesis.voice_not_installed(track,reference);
    let painter = ui.painter().with_clip_rect(body.intersect(ui.clip_rect()));
    let fill = if selected {
        colors.accent.lerp_to_gamma(canvas_color(), 0.15)
    } else {
        colors.note
    };
    painter.rect_filled(region, 3, fill);
    let content = painter.with_clip_rect(clip);
    content.text(
        pos2(region.left() + 6.0, region.top() + 5.0),
        Align2::LEFT_TOP,
        &group.name,
        FontId::proportional(11.0),
        Color32::WHITE,
    );
    for note in &group.notes {
        if let Some((start, end)) = reference.note_range(note) {
            let y = (body.top() + if missing {29.0}else{40.0} - (note.pitch + reference.pitch_offset - 60) as f32 * 0.6)
                .clamp(body.top() + 22.0, body.bottom() - if missing {20.0}else{5.0});
            content.line_segment(
                [
                    pos2(x_at(body, &app.view.arrangement, beats(start)), y),
                    pos2(x_at(body, &app.view.arrangement, beats(end)), y),
                ],
                Stroke::new(1.5, colors.highlight),
            );
        }
    }
    if missing {
        let center=pos2(clip.left()+11.0,body.bottom()-10.0);
        super::icons::Icon::Warning.paint(ui,Rect::from_center_size(center,Vec2::splat(13.0)),text_color());
        let db=crate::model::effective_singer_database(&track.main_ref.database,&reference.database);
        let name=db.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
        content.text(center+vec2(9.0,0.0),Align2::LEFT_CENTER,name,FontId::proportional(11.0),text_color());
    }
    let editable = reference.blick_absolute_end >= 0;
    let response = ui.interact(
        clip,
        Id::new(("arrangement-group", reference.id)),
        if editable {
            Sense::click_and_drag()
        } else {
            Sense::click()
        },
    );
    if missing {response.clone().on_hover_text(crate::i18n::t("未安装"));}
    if response.clicked() || response.secondary_clicked() {
        app.commit_lyric();
        let multiple=response.clicked() && ui.input(|i|i.modifiers.command);
        if response.secondary_clicked() && app.editor.selected_groups.contains(&(track.id,reference.id)) {
            app.editor.group_focus=true;
        }else{
            app.editor.select_group_clipboard(track.id,reference.id,multiple);
        }
    }
    let kind_at = |p: Pos2| {
        if (p.x - region.left()).abs() < 6.0 {
            GroupEdit::Left
        } else if (p.x - region.right()).abs() < 6.0 {
            GroupEdit::Right
        } else {
            GroupEdit::Move
        }
    };
    if editable
        && let Some(p) = ui.input(|i| i.pointer.hover_pos())
        && clip.contains(p)
    {
        response.clone().on_hover_cursor(match kind_at(p) {
            GroupEdit::Move => CursorIcon::Grab,
            _ => CursorIcon::ResizeHorizontal,
        });
    }
    if response.drag_started_by(PointerButton::Primary)
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        group_edit::start(
            app,
            reference,
            track.id,
            origin,
            kind_at(origin),
            app.view.arrangement.beat_width,
            true,
        );
    }
    crate::ui::theme::menu::context_menu(&response,|ui| group_edit::context_menu(app, ui, reference));
}
