use crate::app::App;
use eframe::egui::*;

pub fn status(app: &App) -> &'static str {
    if !app.editor.current_track().is_instrument() {
        return app.synthesis.status();
    }
    let library = &app.synthesis.instruments;
    let track = app.editor.current_track();
    if library.is_scanning() {
        "正在加载…"
    } else if track
        .references()
        .any(|r| library.errors.contains_key(&r.id))
    {
        "Instrument rendering failed"
    } else if track.references().any(|r| library.reference_busy(r.id)) {
        "正在合成歌声"
    } else if library.for_track(track).is_none() {
        "Select Instrument"
    } else if library.outputs.values().any(|o| o.track == track.id) {
        "音频已就绪"
    } else {
        "等待音符"
    }
}

pub fn render_status(app: &mut App, ui: &mut Ui) {
    if app.editor.current_track().is_instrument() {
        ui.label(RichText::new(crate::i18n::t("歌声合成")).strong());
        ui.separator();
        ui.label(crate::i18n::message(status(app)));
        let mut failed = false;
        for reference in app.editor.current_track().references() {
            if let Some(error) = app.synthesis.instruments.errors.get(&reference.id) {
                ui.colored_label(Color32::LIGHT_RED, crate::i18n::message(error));
                failed = true;
            }
        }
        if app.editor.current_track().references().any(|r|app.synthesis.instruments.reference_busy(r.id)){ui.spinner();}
        if failed && ui.button(crate::i18n::t("重试合成")).clicked() {
            app.synthesis.retry_instruments();
        }
        return;
    }
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
        stats.ready,
        stats.queued,
        stats.failed
    ));
    if let Some(error) = &app.synthesis.error {
        ui.colored_label(
            Color32::from_rgb(218, 142, 126),
            crate::i18n::message(error),
        );
    }
    if !app.synthesis.failures.is_empty() {
        ui.collapsing(crate::i18n::t("失败详情"), |ui| {
            for (owner, error) in &app.synthesis.failures {
                ui.label(crate::lformat!("音符组 {}：{error}", owner.0));
            }
        });
    }
    if (stats.failed > 0 || app.synthesis.error.is_some())
        && ui.button(crate::i18n::t("重试合成")).clicked()
    {
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
        ui.label(crate::lformat!(
            "当前混音 {:.2} 秒 · 48 kHz · 立体声",
            duration
        ));
    }
    ui.collapsing(crate::i18n::t("合成计划"), |ui| {
        let summary = app.synthesis.planner.summary();
        ui.label(crate::lformat!(
            "{} 个音符组纳入当前计划",
            summary.pending_owners
        ));
        ui.label(crate::lformat!(
            "本次变化 {} 组 · 保留 {} 组请求",
            summary.changed_owners,
            summary.unchanged_owners
        ));
    });
}
