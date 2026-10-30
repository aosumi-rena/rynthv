pub(crate) mod edit;
use super::state::Column;
use crate::{
    app::{App, FileAction, MidiImportMode},
    model::*,
};
use eframe::egui::*;

fn item(ui: &mut Ui, text: &str, shortcut: &str, enabled: bool) -> bool {
    let response = ui.add_enabled(
        enabled,
        Button::new(text)
            .shortcut_text(shortcut_text(shortcut))
            .min_size(vec2(180.0, 22.0)),
    );
    if response.clicked() {
        ui.close();
        true
    } else {
        false
    }
}
/// Menu labels name the platform's primary modifier. Shortcuts are registered
/// with [`Modifiers::COMMAND`], which egui maps to ⌘ on macOS and Ctrl elsewhere,
/// so the hint text has to follow the same mapping.
pub(crate) fn shortcut_text(shortcut: &str) -> String {
    if cfg!(target_os = "macos") {
        shortcut
            .replace("Ctrl+", "⌘")
            .replace("Shift+", "⇧")
            .replace("Alt+", "⌥")
    } else {
        shortcut.to_owned()
    }
}

pub fn menus(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    MenuBar::new().ui(ui, |ui| {
        ui.spacing_mut().button_padding = vec2(11.0, 4.0);
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.menu_button(crate::i18n::t("文件"), |ui| {
            if item(ui, crate::i18n::t("新建"), "Ctrl+N", true) {
                app.request_file(FileAction::New, &ctx);
            }
            if item(ui, crate::i18n::t("打开…"), "Ctrl+O", true) {
                app.request_file(FileAction::Open, &ctx);
            }
            ui.separator();
            if item(ui, crate::i18n::t("保存"), "Ctrl+S", true) {
                app.save_project(false);
            }
            if item(ui, crate::i18n::t("另存为…"), "Ctrl+Shift+S", true) {
                app.save_project(true);
            }
            ui.separator();
            if item(ui, crate::i18n::t("导入…"), "Ctrl+Shift+I", true) {
                app.request_file(FileAction::ImportMidi, &ctx);
            }
            if item(ui, crate::i18n::t("作为音轨导入…"), "", true) {
                app.begin_midi_import(MidiImportMode::Tracks, &ctx);
            }
            if item(ui, crate::i18n::t("导入音频…"), "", true) {
                app.import_audio(None, &ctx);
            }
            item(ui, crate::i18n::t("导出音频"), "", false);
        });
        ui.menu_button(crate::i18n::t("编辑"), |ui| {
            if item(
                ui,
                &crate::lformat!("撤销 {}", crate::i18n::message(app.editor.undo_label())),
                "Ctrl+Z",
                app.editor.can_undo(),
            ) {
                app.editor.undo();
            }
            if item(ui, crate::i18n::t("重做"), "Ctrl+Y", app.editor.can_redo()) {
                app.editor.redo();
            }
            ui.separator();
            if item(ui, crate::i18n::t("全选"), "Ctrl+A", true) {
                edit::select_all(app);
            }
            if item(ui, crate::i18n::t("取消选择"), "Esc", true) {
                edit::clear(app);
            }
            let selected = edit::selected(app);
            if item(ui, crate::i18n::t("删除所选内容"), "Delete", selected) {
                edit::delete(app);
            }
            ui.separator();
            if item(ui, crate::i18n::t("复制"), "Ctrl+C", selected) {
                edit::copy(app, &ctx);
            }
            if item(ui, crate::i18n::t("剪切"), "Ctrl+X", selected) {
                edit::cut(app, &ctx);
            }
            if item(ui, crate::i18n::t("粘贴"), "Ctrl+V", edit::can_paste(app)) {
                edit::paste(app, &ctx);
            }
        });
        ui.menu_button(crate::i18n::t("查看"), |ui| {
            ui.checkbox(&mut app.show_time_tracks, crate::i18n::t("显示曲速和拍号轨"));
            if ui
                .checkbox(&mut app.view.arrangement_collapsed, crate::i18n::t("折叠编排"))
                .clicked()
            {
                ui.close();
            }
            ui.checkbox(&mut app.view.arrangement_first, crate::i18n::t("编排位于上方"));
            ui.separator();
            for column in Column::PANELS {
                let mut active = app.view.columns.contains(&column);
                if ui.checkbox(&mut active, column.label()).clicked() {
                    app.view.toggle(column);
                }
            }
            ui.separator();
            if item(ui, crate::i18n::t("设置"), "", true) {
                app.settings = true;
            }
        });
        ui.menu_button(crate::i18n::t("修改"), |ui| {
            let selected = app.piano_tool != super::piano::Tool::PitchAnchor
                && !super::lanes::parameter_active(app) && !super::phonemes::phoneme_active(app)
                && !app.editor.selected.is_empty();
            if item(ui, crate::i18n::t("填入歌词…"), "Ctrl+L", selected) {
                super::fill_lyrics::open(app);
            }
            if item(ui, crate::i18n::t("静音音符"), "Ctrl+M", selected) {
                app.editor.mute_selected();
            }
            ui.separator();
            if item(ui, crate::i18n::t("升高八度"), "Ctrl+↑", edit::can_transpose(app)) {
                edit::transpose(app, true);
            }
            if item(ui, crate::i18n::t("降低八度"), "Ctrl+↓", edit::can_transpose(app)) {
                edit::transpose(app, false);
            }
            item(ui, crate::i18n::t("智能量化"), "", false);
            item(ui, crate::i18n::t("解除组链接"), "", false);
        });
        ui.menu_button(crate::i18n::t("自动处理"), |ui| {
            item(ui, crate::i18n::t("装饰所选音符"), "", false);
            super::retakes::commands(app, ui);
            item(ui, crate::i18n::t("从音频提取音符"), "", false);
            item(ui, crate::i18n::t("检测曲速"), "", false);
        });
        ui.menu_button(crate::i18n::t("项目"), |ui| {
            if item(ui, crate::i18n::t("创建音轨"), "", true) {
                app.editor.add_track();
            }
            if item(
                ui,
                crate::i18n::t("在走带处插入新组"),
                "",
                !app.editor.current_track().is_instrumental(),
            ) {
                app.editor.add_group();
            }
            if item(ui, crate::i18n::t("创建伴奏轨"), "", true) {
                let id = app.editor.add_instrumental_track();
                app.import_audio(Some(id), &ctx);
            }
            item(ui, crate::i18n::t("设置歌声"), "", false);
        });
        ui.menu_button(crate::i18n::t("走带控制"), |ui| {
            if item(ui, crate::i18n::t("播放 / 暂停"), "Space", true) {
                app.editor.transport.toggle();
            }
            if item(ui, crate::i18n::t("停止"), "Ctrl+Space", true) {
                app.editor.transport.stop();
            }
            ui.checkbox(&mut app.editor.transport.looping, crate::i18n::t("循环播放"));
            if item(ui, crate::i18n::t("跳到开头"), "", true) {
                app.editor.transport.position = 0;
            }
            if item(ui, crate::i18n::t("跳到结尾"), "Ctrl+End", true) {
                app.editor.transport.position = app.editor.project.end();
            }
        });
        ui.menu_button(crate::i18n::t("脚本"), |ui| {
            item(ui, crate::i18n::t("重新扫描"), "", false);
            item(ui, crate::i18n::t("打开脚本文件夹"), "", false);
        });
        ui.menu_button(crate::i18n::t("其他"), |ui| {
            if item(ui, crate::i18n::t("关于 RynthV"), "", true) {
                app.about = true;
            }
        });
    });
}
pub fn shortcuts(app: &mut App, ctx: &Context) {
    if app.editor.has_performance_edit() {
        return;
    }
    if app.audio_drag.is_some() {
        return;
    }
    if app.view_pan.is_active() {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            app.view_pan.cancel(ctx);
        }
        return;
    }
    if !ctx.text_edit_focused()
        && app.lyric.is_none()
        && app.gesture.is_none()
        && app.group_gesture.is_none()
        && edit::clipboard_events(app, ctx)
    {
        return;
    }
    if ctx.egui_wants_keyboard_input() || app.lyric.is_some() {
        return;
    }
    let command = |key, modifiers| ctx.input_mut(|i| i.consume_key(modifiers, key));
    // COMMAND is ⌘ on macOS and Ctrl elsewhere; matching the platform's
    // primary modifier keeps the registered shortcuts consistent with the
    // hints shown in the menus.
    let ctrl = Modifiers::COMMAND;
    let shift = Modifiers::COMMAND | Modifiers::SHIFT;
    if matches!(app.gesture, Some(super::piano::Gesture::Anchor(_)))
        && super::piano::anchor_keyboard(app, ctx)
    {
        return;
    }
    if app.gesture.is_some() || app.group_gesture.is_some() {
        if command(Key::Escape, Modifiers::NONE) {
            app.editor.cancel();
            app.gesture = None;
            app.group_gesture = None;
        }
        return;
    }
    if command(Key::S, shift) {
        app.save_project(true);
    } else if command(Key::S, ctrl) {
        app.save_project(false);
    }
    if command(Key::I, shift) {
        app.request_file(FileAction::ImportMidi, ctx);
        return;
    }
    if command(Key::N, ctrl) {
        app.request_file(FileAction::New, ctx);
    }
    if command(Key::O, ctrl) {
        app.request_file(FileAction::Open, ctx);
    }
    if command(Key::Z, shift) || command(Key::Y, ctrl) {
        app.editor.redo();
    } else if command(Key::Z, ctrl) {
        app.editor.undo();
    }
    if !super::lanes::parameter_active(app) && !super::phonemes::phoneme_active(app)
        && super::piano::anchor_keyboard(app, ctx) {
        return;
    }
    if command(Key::T, Modifiers::ALT | Modifiers::SHIFT) {
        super::retakes::prune(app);
        return;
    } else if command(Key::T, Modifiers::ALT) {
        super::retakes::create(app, RetakeComponent::All);
        return;
    }
    if command(Key::A, ctrl) {
        edit::select_all(app);
    }
    if command(Key::D, ctrl) {
        edit::duplicate(app);
    }
    if !super::lanes::parameter_active(app) && !super::phonemes::phoneme_active(app)
        && app.piano_tool != super::piano::Tool::PitchAnchor && command(Key::L, ctrl) {
        super::fill_lyrics::open(app);
        return;
    }
    if !super::lanes::parameter_active(app) && !super::phonemes::phoneme_active(app)
        && app.piano_tool != super::piano::Tool::PitchAnchor && command(Key::M, ctrl) {
        app.editor.mute_selected();
    }
    if command(Key::B, ctrl) {
        app.view.toggle(Column::Note);
    }
    if command(Key::ArrowUp, ctrl) {
        edit::transpose(app,true);
    }
    if command(Key::ArrowDown, ctrl) {
        edit::transpose(app,false);
    }
    if command(Key::Delete, Modifiers::NONE) {
        edit::delete(app);
    }
    if command(Key::Escape, Modifiers::NONE) {
        edit::clear(app);
    }
    if command(Key::Num1, Modifiers::ALT) {
        app.piano_tool = super::piano::Tool::NoteSelect;
        app.status.clear();
    }
    if command(Key::Num2, Modifiers::ALT) {
        app.piano_tool = super::piano::Tool::NoteDraw;
        app.status.clear();
    }
    if command(Key::Num3, Modifiers::ALT) {
        app.commit_lyric();
        app.editor.selected.clear();
        app.piano_tool = super::piano::Tool::PitchAnchor;
        app.show_pitch = true;
        app.status.clear();
    }
    if command(Key::Num4, Modifiers::ALT) {
        app.commit_lyric();
        app.piano_tool = super::piano::Tool::PitchDraw;
        app.show_pitch = true;
        app.status.clear();
    }
    if command(Key::Space, ctrl) {
        app.editor.transport.stop();
    } else if command(Key::Space, Modifiers::NONE) {
        app.editor.transport.toggle();
    }
    if command(Key::End, ctrl) {
        app.editor.transport.position = app.editor.project.end();
    }
    if super::lanes::parameter_keyboard(app,ctx) {return;}
    for (key, direction) in [(Key::ArrowLeft, -1), (Key::ArrowRight, 1)] {
        if command(key, Modifiers::NONE) {
            app.editor.transport.position =
                (app.editor.transport.position + direction * ticks(app.view.step())).max(0);
        }
    }
}
