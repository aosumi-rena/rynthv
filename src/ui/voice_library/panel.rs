use super::{language, selected_entry, selector, vocal_modes, weak, widgets};
use crate::{
    app::App,
    synthesis::{runtime::VoiceState, settings::VoiceSelection},
    ui::theme::*,
};
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui) {
    ui.spacing_mut().item_spacing.y = 0.0;
    Frame::new().inner_margin(16).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 8.0;
        selector::selected_card(app, ui);
        weak(ui, crate::i18n::t("版本"));
        version(app, ui);
        weak(ui, crate::i18n::t("预设"));
        super::style_presets::show(app, ui);
        status(app, ui);
    });
    section(ui, crate::i18n::t("语言"), |ui| language::show(app, ui));
    section(ui, crate::i18n::t("齐唱"), |ui| super::unison::show(app, ui));
    if app.synthesis.library.settings.experiments.enabled || !app.voice_inventory().and_then(|v|v.configuration.as_ref()).is_some_and(|c|c.family==crate::synthesis::voice::VoiceFamily::Sv1) {
        section(ui, crate::i18n::t("唱法"), |ui| vocal_modes::show(app, ui));
    }
    section(ui, crate::i18n::t("参数"), |ui| super::parameters::show(app, ui));
    section(ui, crate::i18n::t("调音"), |ui| super::tuning::show(app,ui));
    if super::experiments::available(app) {
        section(ui,crate::i18n::t("实验功能"),|ui|super::experiments::show(app,ui));
    }
    section(ui, crate::i18n::t("调式音阶"), |ui| {
        crate::ui::scale::controls::panel(app, ui);
    });
}

fn version(app: &mut App, ui: &mut Ui) {
    let track=app.editor.current_track();
    let reference=app.editor.active_ref().unwrap_or(&track.main_ref);
    let requested=crate::model::effective_singer_database(&track.main_ref.database,&reference.database)
        .get("version").and_then(serde_json::Value::as_str).unwrap_or("").to_owned();
    let Some(selected) = app.voice_selection() else {
        widgets::field(ui, "", true, if app.synthesis.voice_not_installed(track,reference) {
            crate::i18n::t("未安装")
        }else{crate::i18n::t("先选择歌声")});
        return;
    };
    let explicit=!requested.is_empty() && requested!="-2";
    let missing=explicit && app.synthesis.library.catalog().is_some_and(|catalog|
        !catalog.entries.iter().any(|entry|entry.database_id==selected.database_id && entry.revision.to_string()==requested));
    let label=if missing {crate::lformat!("{} (未安装)",requested)}else{selected.revision.to_string()};
    let mut revision = None;
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 28.0;
        ui.visuals_mut().widgets.inactive.bg_fill = BORDER;
        ui.visuals_mut().widgets.inactive.corner_radius = CornerRadius::same(4);
        ComboBox::from_id_salt("voice-revision")
            .width(ui.available_width())
            .selected_text(label)
            .show_ui(ui, |ui| {
                if let Some(catalog) = app.synthesis.library.catalog() {
                    for entry in catalog
                        .entries
                        .iter()
                        .filter(|entry| entry.database_id == selected.database_id)
                    {
                        ui.add_enabled_ui(entry.availability.is_loadable(), |ui| {
                            if ui.selectable_label(!missing && entry.revision==selected.revision,
                                entry.revision.to_string()).clicked() {revision=Some(entry.revision);ui.close();}
                        })
                        .response
                        .on_disabled_hover_text(crate::i18n::t("此版本尚未支持合成"));
                    }
                }
            });
    });
    if let Some(revision)=revision {
        app.select_voice(VoiceSelection {
            database_id: selected.database_id,
            revision,
        });
    }
}

fn status(app: &mut App, ui: &mut Ui) {
    let failed = matches!(app.voice_state(), VoiceState::Failed(_))
        || app.synthesis.library.settings_error().is_some()
        || app.voice_error().is_some();
    if matches!(app.voice_state(), VoiceState::Loading) || app.synthesis.library.is_scanning() {
        ui.horizontal(|ui| {
            ui.spinner();
            weak(ui, crate::i18n::t("正在加载歌声…"));
        });
    } else if failed {
        if ui
            .button(RichText::new(crate::i18n::t("加载异常 · 管理歌声…")).color(Color32::from_rgb(218, 142, 126)))
            .clicked()
        {
            app.voice_manager = true;
        }
    } else if app.synthesis.library.catalog_error().is_some() {
        if ui.button(crate::i18n::t("目录读取异常 · 管理歌声…")).clicked() {
            app.voice_manager = true;
        }
    } else if selected_entry(app).is_none() && ui.button(crate::i18n::t("管理歌声…")).clicked() {
        app.voice_manager = true;
    }
}

fn section(ui: &mut Ui, title: &str, content: impl FnOnce(&mut Ui)) {
    let y = ui.next_widget_position().y;
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        y,
        Stroke::new(1.0, EDGE),
    );
    Frame::new()
        .inner_margin(Margin {
            left: 16,
            right: 16,
            top: 12,
            bottom: 16,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 12.0;
            if title == crate::i18n::t("唱法") {
                let mut toggle = false;
                let mut header = collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),ui.make_persistent_id(("voice-section",title)),true)
                    .show_header(ui,|ui| {
                        toggle = ui.add(Label::new(RichText::new(title).font(section_title_font()))
                            .sense(Sense::click())).clicked();
                        super::mode_controls::snap_button(ui);
                    });
                if toggle {header.toggle();}
                header.body_unindented(|ui| {ui.set_width(ui.available_width());content(ui);});
            } else {
                CollapsingHeader::new(RichText::new(title).font(section_title_font()))
                .id_salt(("voice-section", title))
                .default_open(true)
                .show_unindented(ui, |ui| {
                    ui.set_width(ui.available_width());
                    content(ui);
                });
            }
        });
}
