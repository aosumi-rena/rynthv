use crate::app::App;
use eframe::egui::Ui;
use super::widgets::{toggle,section};

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
            let path=app.synthesis.library.settings.phone_conversion.as_deref();
            if let Some(path)=path {
                ui.label(path.file_name().unwrap_or(path.as_os_str()).to_string_lossy()).on_hover_text(path.display().to_string());
            } else {ui.label(crate::i18n::t("尚未指定"));}
            let mut replacement=None;
            ui.horizontal(|ui|{
                if ui.button(crate::i18n::t("VOICE_SOURCE_CHANGE")).clicked() {
                    let mut dialog=rfd::FileDialog::new().add_filter("BIN",&["bin"]);
                    if let Some(parent)=path.and_then(std::path::Path::parent) {dialog=dialog.set_directory(parent);}
                    replacement=dialog.pick_file().map(Some);
                }
                if path.is_some() && crate::ui::icons::close_button(ui).clicked() {replacement=Some(None);}
            });
            if let Some(path)=replacement {app.synthesis.set_phone_conversion(path);}
        }
    }
    if settings!=before {app.synthesis.set_experiments(settings);}
    if let Some(error)=app.synthesis.library.settings_error() {ui.colored_label(eframe::egui::Color32::LIGHT_RED,crate::i18n::message(error));}
}
