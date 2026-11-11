mod source;

use super::{icons::Icon, theme::*};
use crate::{app::App, model::Track};
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect, track: &Track) {
    let response = ui.interact(rect, Id::new(("track", track.id)), Sense::click());
    if response.clicked() {
        app.commit_lyric();
        let modifiers=ui.input(|i|i.modifiers);
        app.editor.select_track_header(track.id,modifiers.command||modifiers.ctrl,modifiers.shift);
    }
    if response.double_clicked() {
        app.rename = Some(super::rename::Rename::track(track.id, &track.name));
    }
    crate::ui::theme::menu::context_menu(&response,|ui| {
        if !app.editor.selected_tracks.contains(&track.id){app.editor.select_track_header(track.id,false,false);}
        app.editor.track_focus=true;
        if ui.button(crate::i18n::t("Duplicate Track")).clicked(){app.commit_lyric();app.editor.duplicate_tracks(false);ui.close();}
        if ui.button(crate::i18n::t("Duplicate Track (Unlinked)")).clicked(){app.commit_lyric();app.editor.duplicate_tracks(true);ui.close();}
        ui.separator();
        if ui.button(crate::i18n::t("重命名音轨…")).clicked() {
            app.rename = Some(super::rename::Rename::track(track.id, &track.name));
            ui.close();
        }
        if ui.button(crate::i18n::t("新增音轨")).clicked() {
            app.editor.add_track();
            ui.close();
        }
        if ui.button(crate::i18n::t("新增伴奏轨…")).clicked() {
            let id = app.editor.add_instrumental_track();
            ui.close();
            app.import_audio(Some(id), ui.ctx());
        }
        let index = app
            .editor
            .project
            .tracks
            .iter()
            .position(|t| t.id == track.id)
            .unwrap();
        if ui.add_enabled(index > 0, Button::new(crate::i18n::t("上移音轨"))).clicked() {
            app.editor.reorder_track(track.id, -1);
            ui.close();
        }
        if ui
            .add_enabled(
                index + 1 < app.editor.project.tracks.len(),
                Button::new(crate::i18n::t("下移音轨")),
            )
            .clicked()
        {
            app.editor.reorder_track(track.id, 1);
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(!app.editor.selected_tracks.is_empty(), Button::new(crate::i18n::t("删除音轨")))
            .clicked()
        {
            app.commit_lyric();
            app.editor.delete_tracks();
            ui.close();
        }
        ui.separator();
        if let Some(color) = super::track_color::picker(ui, track.disp_color) {
            app.commit_lyric();
            app.editor.set_track_color(track.id, color);
            ui.close();
        }
    });
    let text_rect = Rect::from_min_max(
        rect.min + vec2(if current() == InterfaceTheme::Classic {32.0} else {16.0}, 0.0),
        pos2(rect.right() - 95.0, rect.bottom()),
    );
    let painter = ui
        .painter()
        .with_clip_rect(ui.clip_rect().intersect(text_rect));
    painter.text(
        rect.min + if current() == InterfaceTheme::Classic {vec2(32.0,15.5)} else {vec2(16.0,16.5)},
        Align2::LEFT_CENTER,
        &track.name,
        if current()==InterfaceTheme::Classic {native_font(17.0)}else{FontId::proportional(12.0)},
        text_color(),
    );
    source::show(app, ui, rect, text_rect, track);
    if rect.width() < 210.0 {
        return;
    }
    for (field, label, center, min, max, reset) in [
        (
            "gainDecibel",
            crate::i18n::t("音量"),
            pos2(rect.right() - if current() == InterfaceTheme::Classic {74.5} else {79.5}, rect.top() + if current() == InterfaceTheme::Classic {20.5} else {24.5}),
            -48.0,
            12.0,
            0.0,
        ),
        (
            "pan",
            crate::i18n::t("声像"),
            pos2(rect.right() - if current() == InterfaceTheme::Classic {47.5} else {46.5}, rect.top() + if current() == InterfaceTheme::Classic {20.5} else {24.5}),
            -1.0,
            1.0,
            0.0,
        ),
    ] {
        let old = track
            .mixer
            .get(field)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(reset);
        let (value, response) = knob(ui, (track.id, field), center, old, min, max, reset);
        let response = response.on_hover_text(crate::lformat!("{label}  {old:.2}\n拖动调整，双击复位"));
        if response.drag_started() || response.double_clicked() {
            app.editor.begin(crate::i18n::t("调整混音"));
        }
        if value != old {
            app.editor
                .project
                .tracks
                .iter_mut()
                .find(|t| t.id == track.id)
                .unwrap()
                .mixer
                .insert(field.into(), value.into());
        }
        if response.drag_stopped() || response.double_clicked() {
            app.editor.finish();
        }
        if current()==InterfaceTheme::Classic {
            let heights=if field=="pan" {[6.0,4.0,2.0,2.0,4.0,6.0]}else{[1.0,2.0,3.0,4.0,5.0,6.0]};
            let offsets=if field=="pan" {[1.0,2.0,3.0,3.0,2.0,1.0]}else{[1.0;6]};
            for i in 0..6 {
                let bottom=rect.bottom()-9.0-offsets[i];
                let bar=Rect::from_min_size(pos2(center.x-5.5+i as f32*2.0,bottom-heights[i]),vec2(1.0,heights[i]));
                ui.painter().rect_filled(bar,0,Color32::from_gray(148));
            }
        } else {
            let label_rect=Rect::from_min_size(pos2(center.x-5.5,center.y+14.5),vec2(11.0,6.0));
            let label_color=Color32::from_rgb(108,107,110);
            if field=="pan" {Icon::Pan.paint(ui,label_rect,label_color);}
            else {for i in 0..6 {
                let height=(i+1) as f32;
                ui.painter().rect_filled(Rect::from_min_size(label_rect.min+vec2(i as f32*2.0,6.0-height),vec2(1.0,height)),0,label_color);
            }}
        }
    }
    for (field, text, y) in [("mute", "M", if current() == InterfaceTheme::Classic {9.0} else {11.0}), ("solo", "S", if current() == InterfaceTheme::Classic {27.0} else {32.0})] {
        let value = track
            .mixer
            .get(field)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let r = Rect::from_min_size(pos2(rect.right() - 24.0, rect.top() + y), Vec2::splat(if current() == InterfaceTheme::Classic {14.0} else {16.0}));
        let response = ui.interact(r, Id::new((track.id, field)), Sense::click());
        if current() == InterfaceTheme::Classic {
            let fill = if value {
                let [red, green, blue, alpha] = track.disp_color.rgba();
                let brighten = |channel: u8| {
                    (255.0 - (255.0 - channel as f32) * 0.9090908765792847_f32) as u8
                };
                Color32::from_rgba_unmultiplied(brighten(red), brighten(green), brighten(blue), alpha)
            } else { Color32::from_rgba_unmultiplied(26, 26, 26, 160) };
            ui.painter().rect_filled(r, 0, fill);
            ui.painter().text(
                r.center() - vec2(0.0, 1.0),
                Align2::CENTER_CENTER,
                text,
                native_font(14.0),
                if value { Color32::from_rgb(37, 38, 38) } else { Color32::from_gray(184) },
            );
        } else {
            ui.painter().rect_filled(
                r,
                2,
                if value {
                    accent_color()
                } else if response.hovered() {
                    button_hover_color()
                } else {
                    button_color()
                },
            );
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                text,
                FontId::proportional(9.0),
                if value { canvas_color() } else { text_color() },
            );
        }
        if response
            .on_hover_text(if field == "mute" {
                crate::i18n::t("音轨静音")
            } else {
                crate::i18n::t("音轨独奏")
            })
            .clicked()
        {
            app.editor.begin(crate::i18n::t("音轨混音"));
            app.editor
                .project
                .tracks
                .iter_mut()
                .find(|t| t.id == track.id)
                .unwrap()
                .mixer
                .insert(field.into(), (!value).into());
            app.editor.finish();
        }
    }
}
fn knob(
    ui: &mut Ui,
    id: (u64, &str),
    center: Pos2,
    value: f64,
    min: f64,
    max: f64,
    reset: f64,
) -> (f64, Response) {
    let size=if current()==InterfaceTheme::Classic {vec2(19.0,19.0)}else{vec2(21.0,25.0)};
    let rect=Rect::from_center_size(center,size);
    let response = ui.interact(rect, Id::new(id), Sense::click_and_drag());
    ui.painter().circle_filled(
        center,
        if current() == InterfaceTheme::Classic {9.5} else {10.5},
        if response.hovered() {
            button_hover_color()
        } else {
            Color32::from_rgb(77, 75, 81)
        },
    );
    let normalized = if value <= reset {
        ((value - reset) / (reset - min).max(1e-6)) * 0.5 + 0.5
    } else {
        ((value - reset) / (max - reset).max(1e-6)) * 0.5 + 0.5
    };
    let angle = (-225.0 + normalized as f32 * 270.0).to_radians();
    let end = center + vec2(angle.cos(), angle.sin()) * 8.0;
    ui.painter()
        .line_segment([center, end], Stroke::new(1.0, muted_color()));
    let next = if response.double_clicked() {
        reset
    } else if response.dragged() {
        (value - response.drag_delta().y as f64 * (max - min) / 160.0).clamp(min, max)
    } else {
        value
    };
    (next, response)
}
