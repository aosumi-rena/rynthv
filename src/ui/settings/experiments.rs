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
        ui.add_space(12.0);
        ui.weak(crate::i18n::t("启用的控件显示在歌声面板和参数编辑器中。"));
    }
    if settings!=before {app.synthesis.set_experiments(settings);}
    if let Some(error)=app.synthesis.library.settings_error() {ui.colored_label(eframe::egui::Color32::LIGHT_RED,crate::i18n::message(error));}
}
