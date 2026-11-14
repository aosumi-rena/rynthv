use super::super::{context, error_label, text, weak};
use crate::{
    app::App,
    synthesis::{runtime::VoiceState, voice::VoiceCatalogEntry},
    ui::theme::*,
};
use eframe::egui::*;
pub(super) fn status(
    app: &mut App,
    ui: &mut Ui,
    selection: &crate::synthesis::settings::VoiceSelection,
) {
    match app.synthesis.voice_state(selection) {
        VoiceState::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                weak(ui, crate::i18n::t("正在准备歌声，请稍候…"));
            });
        }
        VoiceState::Loaded(_) if app.synthesis.voice_error(selection).is_none() => {
            text(ui, RichText::new(crate::i18n::t("已加载")).color(GREEN));
        }
        _ => {}
    }
    let error = app.synthesis.voice_error(selection).map(str::to_owned);
    if let Some(error) = error {
        error_label(
            ui,
            if matches!(app.synthesis.voice_state(selection), VoiceState::Failed(_)) {
                crate::i18n::t("歌声加载失败")
            } else {
                crate::i18n::t("合成环境准备失败")
            },
        );
        ui.collapsing(crate::i18n::t("查看错误详情"), |ui| error_label(ui, &error));
        if app
            .synthesis
            .inventory(selection)
            .is_some_and(|voice| voice.configuration.is_some())
        {
            context::controls(app, ui, selection);
        }
    }
}

pub(super) fn diagnostics(app: &App, ui: &mut Ui, entry: &VoiceCatalogEntry) {
    ui.label(crate::i18n::t("声库目录"))
        .on_hover_text(entry.directory.display().to_string());
    if let Some(voice) = app.synthesis.inventory(&entry.selection()) {
        weak(
            ui,
            crate::lformat!(
                "{} 个文件 · {} 个资源 · {} 棵模块树 · {} 个张量",
                voice.files.len(),
                voice.resource_count,
                voice.graph_count,
                voice.tensor_count
            ),
        );
        if let Some(prepared) = voice.preparation() {
            let mut affine = 0;
            let mut unary = 0;
            let mut other = 0;
            for tree in prepared.summary().files.iter().flat_map(|file| &file.trees) {
                affine += tree.modules.affine_nodes;
                unary += tree.modules.unary_nodes;
                other += tree.modules.unsupported_nodes;
            }
            weak(
                ui,
                crate::lformat!(
                    "已装配 {affine} 个线性层、{unary} 个逐元素模块；{other} 个模块由阶段执行器处理或尚未装配"
                ),
            );
        }
        if let Some(elapsed) = app.synthesis.load_elapsed(&entry.selection()) {
            weak(ui, crate::lformat!("加载耗时 {:.2} 秒", elapsed.as_secs_f64()));
        }
    }
    if let Some(catalog) = app.synthesis.library.catalog()
        && !catalog.warnings.is_empty()
    {
        ui.collapsing(
            crate::lformat!("扫描提示（{}）", catalog.warnings.len()),
            |ui| {
                for warning in &catalog.warnings {
                    weak(ui, warning);
                }
            },
        );
    }
}
