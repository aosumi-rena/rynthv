use std::path::Path;

use super::{error_label, text, weak};
use crate::app::App;
use eframe::egui::*;

pub fn settings(app: &mut App, ui: &mut Ui) {
    text(ui, RichText::new(crate::i18n::t("歌声资源")).strong());
    source_list(app,ui);
    ui.add_space(8.0);
    text(ui, crate::i18n::t("发音词典目录（clf-data）"));
    path_label(ui, app.synthesis.dictionaries());
    if ui.button(crate::i18n::t("选择词典目录…")).clicked() {
        choose_dictionaries(app);
    }
    super::duration_key::settings(app, ui);
    if let Some(error) = app.synthesis.library.settings_error() {
        error_label(ui, error);
    }
    weak(
        ui,
        crate::i18n::t("请手动选择资源目录。路径和所选歌声会自动保存，下次启动恢复。"),
    );
}

pub(super) fn directory_menu(app:&mut App,ui:&mut Ui) {
    if ui.button(crate::i18n::t("VOICE_SOURCE_ADD")).clicked() {
        ui.close();add_source(app);
    }
}

fn add_source(app:&mut App) {
    if let Some(path)=folder_dialog(None).pick_folder() {
        let mut paths:Vec<_>=app.synthesis.library.settings.database_roots().map(|(_,p)|p.to_owned()).collect();
        if !paths.iter().any(|p|std::fs::canonicalize(p).unwrap_or_else(|_|p.clone())==std::fs::canonicalize(&path).unwrap_or_else(|_|path.clone())) {
            paths.push(path);app.synthesis.set_voice_sources(paths);
        }
    }
}
fn source_list(app:&mut App,ui:&mut Ui) {
    text(ui,RichText::new(crate::i18n::t("VOICE_SOURCES")).strong());
    let mut paths:Vec<_>=app.synthesis.library.settings.database_roots().map(|(_,p)|p.to_owned()).collect();
    let mut remove=None;let mut replace=None;
    for (index,path) in paths.iter().enumerate() {
        ui.push_id(index,|ui| {
            ui.horizontal(|ui| {
                if ui.small_button(crate::i18n::t("VOICE_SOURCE_CHANGE")).clicked() {
                    replace=folder_dialog(Some(path)).pick_folder().map(|p|(index,p));
                }
                if ui.small_button("×").clicked() {remove=Some(index);}
                ui.add(Label::new(RichText::new(path.display().to_string()).small().weak()).wrap());
            });
        });
    }
    if let Some((index,path))=replace {paths[index]=path;app.synthesis.set_voice_sources(paths.clone());}
    if let Some(index)=remove {paths.remove(index);app.synthesis.set_voice_sources(paths);}
    ui.horizontal(|ui| {
        if ui.button(crate::i18n::t("VOICE_SOURCE_ADD")).clicked() {add_source(app);}
        if ui.add_enabled(!app.synthesis.library.is_scanning() && app.synthesis.library.settings.has_databases(),Button::new(crate::i18n::t("VOICE_RESCAN"))).clicked() {app.synthesis.rescan_voices();}
    });
}

pub(super) fn choose_dictionaries(app: &mut App) {
    if let Some(path) = folder_dialog(app.synthesis.dictionaries()).pick_folder() {
        app.synthesis.set_dictionaries(path);
    }
}

fn path_label(ui: &mut Ui, path: Option<&Path>) {
    match path {
        Some(path) => weak(ui, path.display().to_string()),
        None => weak(ui, crate::i18n::t("尚未指定")),
    }
}

fn folder_dialog(path: Option<&Path>) -> rfd::FileDialog {
    let dialog = rfd::FileDialog::new();
    match path {
        Some(path) => dialog.set_directory(path),
        None => dialog,
    }
}
