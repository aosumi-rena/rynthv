use crate::app::App;
use eframe::egui::Ui;
use super::widgets::{row, toggle, section};

pub(super) fn show(app:&mut App,ui:&mut Ui) {
    let mut settings=app.synthesis.library.settings.experiments;
    let before=settings;
    toggle(ui,crate::i18n::t("启用实验功能"),&mut settings.enabled);
    if settings.enabled {
        section(ui,crate::i18n::t("唱法"));
        toggle(ui,crate::i18n::t("唱法范围扩展至 300%"),&mut settings.extended_styles);
        toggle(ui,crate::i18n::t("允许负唱法值"),&mut settings.negative_styles);
        toggle(ui,crate::i18n::t("随机唱法混合"),&mut settings.random_styles);
        toggle(ui,crate::i18n::t("独立音高、音色和发音曲线"),&mut settings.component_curves);
        section(ui,crate::i18n::t("声音"));
        toggle(ui,crate::i18n::t("双倍表现参数范围"),&mut settings.extended_expression);
        toggle(ui,crate::i18n::t("多歌声混合"),&mut settings.voice_blending);
        toggle(ui,crate::i18n::t("Random voice blend"),&mut settings.random_voices);
        toggle(ui,crate::i18n::t("嘶吼"),&mut settings.growl);
        toggle(ui,crate::i18n::t("R2_FLAT_CONTROLS"),&mut settings.dds_controls);
        toggle(ui,crate::i18n::t("谐波、气声和环形调制"),&mut settings.component_effects);
        section(ui,crate::i18n::t("PHONE_CONVERSION"));
        toggle(ui,crate::i18n::t("PHONE_CONVERSION_ENABLE"),&mut settings.phone_conversion);
        if settings.phone_conversion {
            let path = app.synthesis.library.settings.phone_conversion.clone();
            row(ui, "BIN", |ui| {
                if ui.button(crate::i18n::t("ROCOFLEX_BROWSE")).clicked() {
                    let mut dialog = rfd::FileDialog::new().add_filter("BIN", &["bin"]);
                    if let Some(parent) = path.as_deref().and_then(std::path::Path::parent) { dialog = dialog.set_directory(parent); }
                    if let Some(replacement) = dialog.pick_file() { app.synthesis.set_phone_conversion(Some(replacement)); }
                }
                if let Some(file) = &path {
                    if ui.add(crate::ui::icons::IconButton::new(crate::ui::icons::Icon::Close, crate::i18n::t("VOICE_LAB_CLEAR_DONOR"))).clicked() {
                        app.synthesis.set_phone_conversion(None);
                    }
                    ui.add(eframe::egui::Label::new(file.file_name().unwrap_or(file.as_os_str()).to_string_lossy()).truncate())
                        .on_hover_text(file.display().to_string());
                } else { ui.label(crate::i18n::t("尚未指定")); }
            });
        }
    }
    if settings.enabled || settings.zero_gru_weights {
        ui.push_id("gru-override", |ui| {
            section(ui, crate::i18n::t("GRU_OVERRIDE"));
            toggle(ui, crate::i18n::t("GRU_OVERRIDE_ENABLE"), &mut settings.zero_gru_weights);
        });
    }
    if settings.enabled || settings.r2_vocoder {
        ui.push_id("vocoder-override", |ui| super::vocoder::show(app, ui, &mut settings));
    }
    if settings.enabled || app.synthesis.library.settings.instruments.waveform_override_enabled {
        ui.push_id("waveform-override", |ui| super::waveform::show(app, ui));
    }
    if settings!=before {app.synthesis.set_experiments(settings);}
    if let Some(error)=app.synthesis.library.settings_error() {ui.colored_label(eframe::egui::Color32::LIGHT_RED,crate::i18n::message(error));}
}
