use super::super::{super::paths, manager_card_color, manager_surface_color, State, catalog};
use crate::{
    app::App,
    synthesis::{
        runtime::VoiceState,
        voice::{VoiceCatalogEntry, VoiceFamily},
    },
    ui::theme::*,
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
    let needs_context =
        entry.family != Some(VoiceFamily::Sv1) && app.synthesis.needs_context_import(&key);
    let problem =
        matches!(voice, VoiceState::Loaded(_)) && app.synthesis.voice_error(&key).is_some();
    let loaded = matches!(voice, VoiceState::Loaded(_)) && !problem;
    let missing_dictionary = app.synthesis.library.settings.phoneme_directory(entry).is_none();
    let label = if !entry.availability.is_loadable() {
        crate::i18n::t("不可用")
    } else if current && loading {
        crate::i18n::t("正在加载…")
    } else if needs_context {
        crate::i18n::t("导入模型上下文…")
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
        && !app.synthesis.library.is_scanning()
        && app.synthesis.library.settings_error().is_none();
    let outline = loaded || problem || missing_dictionary;
    let button = Button::new(RichText::new(label).size(13.0).color(if !enabled {
        muted_color()
    } else if outline {
        palette().manager_accent
    } else {
        manager_surface_color()
    }))
    .corner_radius(13)
    .fill(if !enabled || outline { manager_card_color() } else { palette().manager_accent })
    .stroke(Stroke::new(0.8, if enabled { palette().manager_accent } else { border_color() }));
    if ui
        .add_enabled(enabled, button.min_size(vec2(ui.available_width(), 36.0)))
        .clicked()
    {
        if needs_context {
            app.synthesis.open_context_import(key);
        } else if problem {
            state.expanded = Some(entry.database_id.clone());
        } else if current && loaded {
            app.unload_voice();
        } else if missing_dictionary {
            if let Some(version)=app.synthesis.library.settings.phoneme_version(entry){paths::choose_phoneme_data(app,version);}
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
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 13, border_color());
    }
    for offset in [-5.0, 0.0, 5.0] {
        ui.painter()
            .circle_filled(rect.center() + vec2(0.0, offset), 1.5, muted_color());
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, crate::i18n::t("更多声库操作")));
    crate::ui::theme::menu::popup(&response).show(|ui| {
        if ui
            .button(if state.expanded.as_ref() == Some(&entry.database_id) {
                crate::i18n::t("收起详情")
            } else {
                crate::i18n::t("详情与诊断")
            })
            .clicked()
        {
            if state.expanded.as_ref() == Some(&entry.database_id) {
                state.expanded = None;
            } else {
                state.expanded = Some(entry.database_id.clone());
            }
            ui.close();
        }
        if entry.family.is_some_and(VoiceFamily::requires_identity)
            && ui
                .add_enabled(
                    app.synthesis.can_import_context(&entry.selection()),
                    Button::new(crate::i18n::t("重新导入上下文…")),
                )
                .on_hover_text(crate::i18n::t("重新选择上下文来源，导入成功后重新加载使用此上下文的歌声"))
                .on_disabled_hover_text(crate::i18n::t("当前声库不可用，或正在扫描声库、导入上下文"))
                .clicked()
        {
            app.synthesis.open_context_import(entry.selection());
            ui.close();
        }
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
