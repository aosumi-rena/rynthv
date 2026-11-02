mod fields;

use super::{error_label, text, weak};
use crate::{
    app::App,
    synthesis::{
        runtime::{ContextImportPhase, ContextImportSource},
        voice::session_identity,
    },
};
use eframe::egui::*;

pub(super) fn controls(
    app: &mut App,
    ui: &mut Ui,
    selection: &crate::synthesis::settings::VoiceSelection,
) {
    if !app
        .synthesis
        .library
        .entry(selection)
        .and_then(|entry| entry.family)
        .is_some_and(|family| family.requires_identity())
    {
        return;
    }
    if let Some(result) = &app.synthesis.context_import.feedback {
        match result {
            Ok(message) => text(ui, message),
            Err(error) => error_label(ui, error),
        }
    }
    let label = if app.synthesis.needs_context_import(selection) {
        crate::i18n::t("导入模型上下文…")
    } else {
        crate::i18n::t("重新导入上下文…")
    };
    if ui
        .add_enabled(
            app.synthesis.can_import_context(selection),
            Button::new(label),
        )
        .on_hover_text(
            crate::i18n::t("从本地会话文件、上下文文件、手动输入或原编辑器进程重新导入此歌声的模型上下文"),
        )
        .on_disabled_hover_text(crate::i18n::t("当前声库不可用，或正在扫描声库、导入上下文"))
        .clicked()
    {
        app.synthesis.open_context_import(selection.clone());
    }
}

pub(super) fn dialog(app: &mut App, ctx: &Context) {
    let Some(dialog) = &mut app.synthesis.context_import.dialog else {
        return;
    };
    let mut refresh = false;
    let mut import = false;
    let mut cancel = false;
    let mut edited = false;
    let mut source = dialog.source;
    let importing = dialog.phase == ContextImportPhase::Importing;
    let modal = Modal::new(Id::new("voice-model-context-import")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.spacing_mut().item_spacing.y = 8.0;
        text(ui, RichText::new(crate::i18n::t("导入模型上下文")).strong().size(16.0));
        text(ui, crate::lformat!("当前声库：{}", dialog.voice_name));
        weak(
            ui,
            if dialog.model_version == 1 {
                crate::i18n::t("版本 1 兼容歌声")
            } else {
                crate::i18n::t("版本 2 歌声")
            },
        );
        ui.add_enabled_ui(!importing, |ui| {
            ui.horizontal(|ui| {
                ui.add_enabled_ui(session_identity::supports_session_import(), |ui| {
                    ui.selectable_value(
                        &mut source,
                        ContextImportSource::SessionFile,
                        crate::i18n::t("本地会话文件"),
                    )
                    .on_disabled_hover_text(
                        crate::i18n::t("导入本地会话文件仅支持 Windows x64；当前平台可导入上下文文件或手动输入"),
                    );
                });
                ui.add_enabled_ui(session_identity::supports_online_import(), |ui| {
                    ui.selectable_value(&mut source, ContextImportSource::Online, crate::i18n::t("从原版获取"))
                        .on_disabled_hover_text(
                            crate::i18n::t("从原版获取仅支持 Windows x64；当前平台可导入上下文文件或手动输入"),
                        );
                });
            });
            ui.horizontal(|ui| {
                ui.selectable_value(&mut source, ContextImportSource::File, crate::i18n::t("上下文文件"));
                ui.selectable_value(&mut source, ContextImportSource::Manual, crate::i18n::t("手动输入"));
            });
        });
        ui.separator();
        ui.add_enabled_ui(!importing, |ui| {
            edited = fields::show(ui, dialog);
        });
        if edited {
            dialog.error = None;
        }
        if importing {
            ui.horizontal(|ui| {
                ui.spinner();
                text(ui, crate::i18n::t("导入"));
            });
        }
        if let Some(error) = &dialog.error {
            error_label(ui, error);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if dialog.source == ContextImportSource::Online {
                refresh = ui
                    .add_enabled(
                        dialog.phase == ContextImportPhase::Ready,
                        Button::new(crate::i18n::t("刷新进程")),
                    )
                    .clicked();
            }
            cancel = ui.add_enabled(!importing, Button::new(crate::i18n::t("取消"))).clicked();
            import = ui
                .add_enabled(dialog.can_import(), Button::new(crate::i18n::t("导入")))
                .clicked();
        });
    });
    cancel |= !importing
        && modal.is_top_modal
        && !modal.any_popup_open
        && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
    let source_changed = source != dialog.source;
    if edited {
        app.synthesis.clear_context_import_error();
    }
    if cancel {
        app.synthesis.cancel_context_import();
    } else if source_changed {
        app.synthesis.set_context_import_source(source);
    } else if import {
        app.synthesis.import_context();
    } else if refresh {
        app.synthesis.refresh_context_sources();
    }
}
