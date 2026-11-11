use crate::app::App;
use eframe::egui::*;

pub fn render_status(app: &mut App, ui: &mut Ui) {
    ui.label(RichText::new(crate::i18n::t("歌声合成")).strong());
    ui.separator();
    ui.label(crate::i18n::message(app.synthesis.status()));
    if let Some(progress) = &app.synthesis.progress {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(crate::i18n::message(progress));
        });
    }
    let stats = app.synthesis.stats;
    ui.label(crate::lformat!(
        "已完成 {} 组 · 等待 {} 组 · 失败 {} 组",
        stats.ready, stats.queued, stats.failed
    ));
    if let Some(error) = &app.synthesis.error {
        ui.colored_label(Color32::from_rgb(218, 142, 126), crate::i18n::message(error));
    }
    for (selection, name) in app.synthesis.context_import_targets() {
        if ui.button(crate::lformat!("为 {name} 导入模型上下文…")).clicked() {
            app.synthesis.open_context_import(selection);
        }
    }
    if !app.synthesis.failures.is_empty() {
        crate::ui::theme::native::collapsing(crate::i18n::t("失败详情")).show(ui, |ui| {
            for (owner, error) in &app.synthesis.failures {
                ui.label(crate::lformat!("音符组 {}：{error}", owner.0));
            }
        });
    }
    if (stats.failed > 0 || app.synthesis.error.is_some()) && ui.button(crate::i18n::t("重试合成")).clicked() {
        app.synthesis.retry();
    }
    if let Some(error) = app.synthesis.playback_error() {
        ui.colored_label(
            Color32::from_rgb(218, 142, 126),
            crate::lformat!("播放设备：{error}"),
        );
    }
    ui.add_space(10.0);
    if let Some(duration) = app.synthesis.audio_duration() {
        ui.label(crate::lformat!("当前混音 {:.2} 秒 · 48 kHz · 立体声", duration));
    }
    crate::ui::theme::native::collapsing(crate::i18n::t("合成计划")).show(ui, |ui| {
        let summary = app.synthesis.planner.summary();
        ui.label(crate::lformat!("{} 个音符组纳入当前计划", summary.pending_owners));
        ui.label(crate::lformat!(
            "本次变化 {} 组 · 保留 {} 组请求",
            summary.changed_owners, summary.unchanged_owners
        ));
    });
}
