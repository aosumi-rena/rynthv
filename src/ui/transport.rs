mod toggle;

use super::{
    child,
    icons::{Icon, IconButton},
    theme::*,
};
use crate::app::App;
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui) {
    let rect = ui.max_rect();
    let controls = Rect::from_center_size(
        pos2(rect.left()+((rect.width()-519.0)*0.5).trunc()+259.5, rect.center().y + 1.0),
        vec2(519.0, 44.0),
    );
    let mut bar = child(ui, controls, "transport-buttons");
    bar.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let button = |icon, label| {
            IconButton::new(icon, label).transport()
                .size(vec2(24.0, 28.0))
                .icon_size(21.0)
        };
        let transport = &mut app.editor.transport;
        if ui
            .add(button(
                if transport.playing {
                    Icon::Pause
                } else {
                    Icon::Play
                },
                crate::i18n::t("播放 / 暂停（Space）"),
            ))
            .clicked()
        {
            transport.toggle();
        }
        if ui
            .add(button(Icon::Stop, crate::i18n::t("停止（Ctrl+Space）")))
            .on_hover_text(crate::ui::chrome::shortcut_text("Ctrl+Space"))
            .clicked()
        {
            transport.stop();
        }
        ui.add_enabled(false, button(Icon::Record, crate::i18n::t("录制（尚未接入）")));
        if ui.add(button(Icon::Rewind, crate::i18n::t("跳到开头"))).clicked() {
            transport.position = 0;
        }
        if ui.add(button(Icon::Forward, crate::i18n::t("跳到结尾"))).clicked() {
            transport.position = app.editor.project.end();
        }
        if ui
            .add(
                button(Icon::Loop, crate::i18n::t("显示循环标记 / 循环播放"))
                    .selected(transport.looping)
                    .background(background_color()),
            )
            .clicked()
        {
            transport.looping = !transport.looping;
            app.preferences.looping=transport.looping;
        }
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(16.0);
        let position = transport.position;
        let time = &app.editor.project.time;
        let meter = time.meter_at(time.bar_at(position).0);
        let meter_index = meter.index;
        if display(
            ui,
            &format!("{}/{}", meter.numerator, meter.denominator),
            48.0,
        )
        .on_hover_text(crate::i18n::t("编辑当前拍号"))
        .clicked()
        {
            super::time_edit::open(app, super::time_edit::Target::Meter(meter_index));
        }
        ui.add_space(0.0);
        if app.bpm_edit.is_some() {
            super::tempo_input::show(app, ui);
        } else if display(
            ui,
            &format!("{:.2}", app.editor.project.time.bpm_at(position)),
            super::tempo_input::WIDTH,
        )
        .on_hover_text(crate::i18n::t("编辑当前曲速"))
        .clicked()
        {
            super::tempo_input::open(app, position, ui.ctx());
        }
        ui.add_space(0.0);
        let metronome = app.synthesis.metronome_enabled();
        if toggle::show(
            ui,
            Icon::Metronome,
            crate::i18n::t("节拍器"),
            metronome,
            41.0,
        )
        .clicked()
        {
            app.synthesis.set_metronome(!metronome);
            app.preferences.metronome = !metronome;
        }
        ui.add_space(16.0);
        let seconds = app.editor.project.time.seconds_at(position).max(0.0);
        display(
            ui,
            &format!(
                "{:02}:{:02}:{:02}",
                (seconds / 60.0) as u64,
                seconds as u64 % 60,
                (seconds.fract() * 100.0) as u64
            ),
            92.0,
        );
        ui.add_space(0.0);
        if toggle::show(
            ui,
            Icon::Return,
            crate::i18n::t("暂停播放时返回起播位置"),
            app.editor.transport.return_on_pause,
            41.0,
        )
        .clicked()
        {
            app.editor.transport.return_on_pause = !app.editor.transport.return_on_pause;
            app.preferences.return_on_pause = app.editor.transport.return_on_pause;
        }
    });
    Icon::Brand.paint(
        ui,
        Rect::from_center_size(pos2(rect.right() - 23.0, rect.center().y), vec2(27.0, 27.0)),
        Color32::WHITE,
    );
}
fn display(ui: &mut Ui, text: &str, width: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click());
    ui.painter().rect_filled(rect, 3, panel_color());
    let color = Color32::from_rgb(155, 152, 163);
    let mut job = eframe::egui::text::LayoutJob::simple_singleline(text.into(), native_font(24.5), color);
    job.sections[0].format.extra_letter_spacing = 0.245;
    let mut galley = (*ui.painter().layout_job(job)).clone();
    for placed in &mut galley.rows {
        placed.pos.x *= 1.13;
        let row = std::sync::Arc::make_mut(&mut placed.row);
        row.size.x *= 1.13;
        for glyph in &mut row.glyphs { glyph.pos.x *= 1.13; }
        for vertex in &mut row.visuals.mesh.vertices { vertex.pos.x *= 1.13; }
        row.visuals.mesh_bounds.min.x *= 1.13;
        row.visuals.mesh_bounds.max.x *= 1.13;
    }
    galley.rect.min.x *= 1.13;
    galley.rect.max.x *= 1.13;
    galley.mesh_bounds.min.x *= 1.13;
    galley.mesh_bounds.max.x *= 1.13;
    ui.painter().galley(rect.center()-galley.size()*0.5, std::sync::Arc::new(galley), color);
    response
}

pub fn classic(app: &mut App, ui: &mut Ui) {
    ScrollArea::horizontal()
        .id_salt("classic-transport-scroll")
        .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let toggle_button = |icon, label, selected, index| IconButton::new(icon, label).tool()
                    .size(vec2(24.0, 16.0)).group_edges(index==0,index==2).selected(selected);
                if ui.add(toggle_button(Icon::Return, crate::i18n::t("暂停播放时返回起播位置"), app.editor.transport.return_on_pause,0)).clicked() {
                    app.editor.transport.return_on_pause = !app.editor.transport.return_on_pause;
                    app.preferences.return_on_pause = app.editor.transport.return_on_pause;
                }
                if ui.add(toggle_button(Icon::Loop, crate::i18n::t("显示循环标记 / 循环播放"), app.editor.transport.looping,1)).clicked() {
                    app.editor.transport.looping = !app.editor.transport.looping;
                    app.preferences.looping = app.editor.transport.looping;
                }
                let metronome = app.synthesis.metronome_enabled();
                if ui.add(toggle_button(Icon::Metronome, crate::i18n::t("节拍器"), metronome,2)).clicked() {
                    app.synthesis.set_metronome(!metronome);
                    app.preferences.metronome = !metronome;
                }
                ui.add_space(4.0);
                let button = |icon, label| IconButton::new(icon,label).transport().size(vec2(20.0,24.0)).icon_size(16.0);
                if ui.add(button(if app.editor.transport.playing {Icon::Pause} else {Icon::Play},crate::i18n::t("播放 / 暂停（Space）"))).clicked() {app.editor.transport.toggle();}
                if ui.add(button(Icon::Stop,crate::i18n::t("停止（Ctrl+Space）"))).clicked() {app.editor.transport.stop();}
                ui.add_enabled(false,button(Icon::Record,crate::i18n::t("录制（尚未接入）")));
                if ui.add(button(Icon::Rewind,crate::i18n::t("跳到开头"))).clicked() {app.editor.transport.position=0;}
                if ui.add(button(Icon::Forward,crate::i18n::t("跳到结尾"))).clicked() {app.editor.transport.position=app.editor.project.end();}
                ui.add_space(8.0);
                let position=app.editor.transport.position;
                let meter=app.editor.project.time.meter_at(app.editor.project.time.bar_at(position).0);
                let meter_index=meter.index;
                if ui.add(Button::new(format!("{}/{}",meter.numerator,meter.denominator)).min_size(vec2(40.0,20.0))).on_hover_text(crate::i18n::t("编辑当前拍号")).clicked() {
                    super::time_edit::open(app,super::time_edit::Target::Meter(meter_index));
                }
                if app.bpm_edit.is_some() {super::tempo_input::show(app,ui);}
                else if ui.add(Button::new(format!("{:.2}",app.editor.project.time.bpm_at(position))).min_size(vec2(60.0,20.0))).on_hover_text(crate::i18n::t("编辑当前曲速")).clicked() {super::tempo_input::open(app,position,ui.ctx());}
                let seconds=app.editor.project.time.seconds_at(position).max(0.0);
                ui.label(format!("{:02}:{:02}:{:02}",(seconds/60.0) as u64,seconds as u64%60,(seconds.fract()*100.0) as u64));
            });
        });
}
