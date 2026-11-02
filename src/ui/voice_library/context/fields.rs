use super::{text, weak};
use crate::synthesis::runtime::{ContextImportDialog, ContextImportPhase, ContextImportSource};
use eframe::egui::*;
use zeroize::Zeroize;

pub(super) fn show(ui: &mut Ui, dialog: &mut ContextImportDialog) -> bool {
    match dialog.source {
        ContextImportSource::SessionFile => session_file(ui, dialog),
        ContextImportSource::File => file(ui, dialog),
        ContextImportSource::Manual => manual(ui, dialog),
        ContextImportSource::Online => online(ui, dialog),
    }
}

fn session_file(ui: &mut Ui, dialog: &mut ContextImportDialog) -> bool {
    text(
        ui,
        crate::i18n::t("加载 session。"),
    );
    if let Some(path) = &dialog.session_path {
        super::super::paths::path_label(ui,Some(path));
    } else {
        weak(ui, crate::i18n::t("尚未选择 session"));
    }
    let mut changed = false;
    if ui.button(crate::i18n::t("加载 session…")).clicked() {
        let mut picker = rfd::FileDialog::new().set_title(crate::i18n::t("加载 session"));
        if let Some(parent) = dialog.session_path.as_ref().and_then(|path| path.parent()) {
            picker = picker.set_directory(parent);
        }
        if let Some(path) = picker.pick_file() {
            dialog.session_path = Some(path);
            dialog.session_key.zeroize();
            dialog.session_key_required = false;
            changed = true;
        }
    }
    if dialog.session_key_required {
        text(ui, crate::i18n::t("session 自动解密失败。请输入 session 解密密钥后重试。"));
        changed |= ui.add(TextEdit::singleline(&mut dialog.session_key)
            .password(true).hint_text(crate::i18n::t("十进制整数或 0x 十六进制"))
            .desired_width(f32::INFINITY)).changed();
    }
    changed
}

fn file(ui: &mut Ui, dialog: &mut ContextImportDialog) -> bool {
    if let Some(path) = &dialog.file_path {
        super::super::paths::path_label(ui,Some(path));
    } else {
        weak(ui, crate::i18n::t("尚未选择文件"));
    }
    if !ui.button(crate::i18n::t("选择上下文文件…")).clicked() {
        return false;
    }
    let mut picker = rfd::FileDialog::new()
        .set_title(crate::i18n::t("选择模型上下文文件"))
        .add_filter(crate::i18n::t("模型上下文 JSON"), &["json"]);
    if let Some(parent) = dialog.file_path.as_ref().and_then(|path| path.parent()) {
        picker = picker.set_directory(parent);
    }
    if let Some(path) = picker.pick_file() {
        dialog.file_path = Some(path);
        return true;
    }
    false
}

fn manual(ui: &mut Ui, dialog: &mut ContextImportDialog) -> bool {
    ui.add(
        TextEdit::singleline(&mut dialog.manual_identity)
            .desired_width(f32::INFINITY)
            .hint_text(crate::i18n::t("十进制整数或 0x 十六进制"))
            .password(true),
    )
    .changed()
}

fn online(ui: &mut Ui, dialog: &mut ContextImportDialog) -> bool {
    if dialog.phase == ContextImportPhase::Discovering {
        ui.horizontal(|ui| {
            ui.spinner();
            text(ui, crate::i18n::t("正在查找已打开的编辑器…"));
        });
        return false;
    }
    if dialog.sources.is_empty() {
        weak(ui, crate::i18n::t("未找到可用的编辑器进程。打开原编辑器后点击刷新。"));
        return false;
    }
    let mut changed = false;
    ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
        for source in &dialog.sources {
            changed |= ui
                .radio_value(
                    &mut dialog.selected_process,
                    Some(source.process_id),
                    format!("{} · PID {}", source.label, source.process_id),
                )
                .changed();
        }
    });
    changed
}
