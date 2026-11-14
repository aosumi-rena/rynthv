use crate::{app::App, synthesis::voice::{session_identity, VoiceCatalogEntry}};
use eframe::egui::*;

pub(super) fn settings(app: &mut App, ui: &mut Ui) {
    let id = ui.make_persistent_id("duration-key-override-draft");
    let settings = &app.synthesis.library.settings;
    let mut draft = ui.ctx().data_mut(|data| data.get_temp::<(bool, String)>(id))
        .unwrap_or((settings.duration_key_override_enabled, settings.duration_key_override_value.to_string()));
    ui.add_space(12.0);
    ui.label(RichText::new(crate::i18n::t("DURATION_MODEL_KEY_R3")).strong());
    ui.checkbox(&mut draft.0, crate::i18n::t("覆盖 DUR key"));
    ui.add_enabled_ui(draft.0, |ui| {
        ui.label(crate::i18n::t("DUR key（无符号 64 位整数）"));
        ui.add(TextEdit::singleline(&mut draft.1).desired_width(ui.available_width()));
    });
    let parsed = session_identity::parse_identity(&draft.1);
    if draft.0 && parsed.is_err() {
        ui.colored_label(Color32::from_rgb(218, 142, 126), crate::i18n::t("请输入 0 到 18446744073709551615 之间的整数。"));
    }
    if ui.add_enabled(!draft.0 || parsed.is_ok(), Button::new(crate::i18n::t("应用 DUR key 设置"))).clicked() {
        let value = parsed.unwrap_or(app.synthesis.library.settings.duration_key_override_value);
        app.synthesis.set_duration_key_override(draft.0, value);
    }
    ui.ctx().data_mut(|data| data.insert_temp(id, draft));
}

pub(super) fn voice_settings(app: &mut App, ui: &mut Ui, entry: &VoiceCatalogEntry) {
    let selection = entry.selection();
    let id = ui.make_persistent_id(("voice-duration-key-override-draft", &selection.database_id, selection.revision.to_string()));
    let saved = app.synthesis.library.settings.voice_duration_key_override(&selection);
    let initial = saved.map(|entry| (entry.enabled, entry.value))
        .unwrap_or((false, 0));
    let mut draft = ui.ctx().data_mut(|data| data.get_temp::<(bool, String)>(id))
        .unwrap_or((initial.0, initial.1.to_string()));
    ui.separator();
    ui.label(RichText::new(crate::lformat!("时长模型键值 · {}", entry.revision)).strong());
    ui.checkbox(&mut draft.0, crate::i18n::t("覆盖此版本 DUR key"));
    ui.add_enabled_ui(draft.0, |ui| {
        ui.label(crate::i18n::t("DUR key（无符号 64 位整数）"));
        ui.add(TextEdit::singleline(&mut draft.1).desired_width(ui.available_width()));
    });
    let parsed = session_identity::parse_identity(&draft.1);
    if draft.0 && parsed.is_err() {
        ui.colored_label(Color32::from_rgb(218, 142, 126), crate::i18n::t("请输入 0 到 18446744073709551615 之间的整数。"));
    }
    if ui.add_enabled(!draft.0 || parsed.is_ok(), Button::new(crate::i18n::t("应用此版本 DUR key 设置"))).clicked() {
        app.synthesis.set_voice_duration_key_override(selection, draft.0, parsed.unwrap_or(initial.1));
    }
    ui.ctx().data_mut(|data| data.insert_temp(id, draft));
}
