use super::super::{super::paths, State, catalog};
use crate::{
    app::App,
    synthesis::{runtime::VoiceState, voice::VoiceCatalogEntry},
};
use eframe::egui::*;

pub(super) fn primary(
    app: &mut App,
    ui: &mut Ui,
    entry: &VoiceCatalogEntry,
    state: &mut State,
    current: bool,
    stable: bool,
) {
    let key = entry.selection();
    let voice = app.synthesis.voice_state(&key);
    let loading = matches!(voice, VoiceState::Loading);
    let problem =
        matches!(voice, VoiceState::Loaded(_)) && app.synthesis.voice_error(&key).is_some();
    let loaded = matches!(voice, VoiceState::Loaded(_)) && !problem;
    let missing_dictionary = app
        .synthesis
        .library
        .settings
        .phoneme_directory(entry)
        .is_none();
    let label = if !entry.availability.is_loadable() {
        crate::i18n::t("不可用")
    } else if current && loading {
        crate::i18n::t("正在加载…")
    } else if problem {
        crate::i18n::t("处理问题")
    } else if current && loaded {
        crate::i18n::t("取消当前音轨歌声")
    } else if loaded {
        crate::i18n::t("用于当前音轨")
    } else if missing_dictionary {
        crate::i18n::t("设置词典目录")
    } else if current && matches!(voice, VoiceState::Failed(_)) {
        crate::i18n::t("重试加载")
    } else {
        crate::i18n::t("加载歌声")
    };
    let enabled = stable
        && entry.availability.is_loadable()
        && !(current && loading)
        && !app.editor.current_track().is_instrumental()
        && !app.editor.current_track().is_instrument()
        && !app.synthesis.library.is_scanning()
        && app.synthesis.library.settings_error().is_none();
    let outline = loaded || problem || missing_dictionary;
    let size = vec2(ui.available_width(), 36.0);
    if crate::ui::resource_manager::action_button(ui, label, size, enabled, outline).clicked() {
        if problem {
            state.expanded = Some(entry.database_id.clone());
        } else if current && loaded {
            app.unload_voice();
        } else if missing_dictionary {
            if let Some(version) = crate::synthesis::settings::PhonemeVersion::entry(entry) {
                paths::choose_phoneme_data(app, version);
            }
        } else if current {
            app.reload_voice();
        } else {
            app.select_voice(catalog::selection(entry));
        }
    }
}

pub(super) fn menu(
    app: &mut App,
    ui: &mut Ui,
    entry: &VoiceCatalogEntry,
    state: &mut State,
    current: bool,
) {
    crate::ui::resource_manager::more(ui, crate::i18n::t("更多声库操作"), |ui| {
        crate::ui::resource_manager::details_action(ui, &mut state.expanded, &entry.database_id);
        if !matches!(
            app.synthesis.voice_state(&entry.selection()),
            VoiceState::Unloaded
        ) {
            if ui
                .add_enabled(
                    !matches!(
                        app.synthesis.voice_state(&entry.selection()),
                        VoiceState::Loading
                    ),
                    Button::new(crate::i18n::t("重新加载")),
                )
                .clicked()
            {
                app.synthesis.reload_voice(&entry.selection());
                ui.close();
            }
            if current && ui.button(crate::i18n::t("取消当前音轨歌声")).clicked() {
                app.unload_voice();
                ui.close();
            }
        }
    });
}
