mod experiments;
pub(crate) mod artwork;
mod context;
mod language;
mod management;
mod panel;
mod parameters;
mod unison;
mod tuning;
mod paths;
mod duration_key;
mod selector;
mod widgets;
mod vocal_modes;
mod style_presets;
pub use style_presets::PresetLibrary;
mod mode_controls;
mod mode_knob;
mod mode_value;

use super::theme::*;
use crate::{
    app::App,
    synthesis::voice::{VoiceCatalogEntry, VoiceFamily},
};
use eframe::egui::*;

pub use management::dialogs;
pub use panel::show;
pub use paths::settings;
pub use selector::popup;

fn selected_entry(app: &App) -> Option<&VoiceCatalogEntry> {
    app.synthesis.library.entry(&app.voice_selection()?)
}

fn family_name(entry: &VoiceCatalogEntry) -> &'static str {
    match entry.family {
        Some(VoiceFamily::Sv1) => crate::i18n::t("SV1 原生"),
        Some(VoiceFamily::Sv2Compatible) => crate::i18n::t("SV2 · 版本 1 兼容"),
        Some(VoiceFamily::Sv2) => "SV2",
        None => crate::i18n::t("未知格式"),
    }
}

fn text(ui: &mut Ui, value: impl Into<WidgetText>) {
    ui.add(Label::new(value).selectable(false).wrap());
}

fn weak(ui: &mut Ui, value: impl Into<String>) {
    text(ui, RichText::new(value).color(MUTED));
}

fn error_label(ui: &mut Ui, value: impl Into<String>) {
    let value=crate::i18n::message(&value.into());
    text(
        ui,
        RichText::new(value).color(Color32::from_rgb(218, 142, 126)),
    );
}

fn language_name(language: &str) -> &str {
    match language {
        "mandarin" | "mandarin-xsampa" | "zh" | "ZH" => crate::i18n::t("普通话"),
        "english" | "english-arpabet" | "en" | "EN" => crate::i18n::t("英语"),
        "japanese" | "japanese-romaji" | "ja" | "JA" => crate::i18n::t("日语"),
        "spanish" | "spanish-xsampa" | "es" | "ES" => crate::i18n::t("西班牙语"),
        "cantonese" | "cantonese-xsampa" | "yue" | "YUE" => crate::i18n::t("粤语"),
        "korean" | "korean-xsampa" | "ko" | "KO" => crate::i18n::t("韩语"),
        "french" | "french-xsampa" => crate::i18n::t("法语"),
        "german" | "german-xsampa" => crate::i18n::t("德语"),
        "portuguese" | "portuguese-xsampa" => crate::i18n::t("葡萄牙语"),
        other => other,
    }
}
