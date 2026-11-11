use std::path::Path;

use super::{error_label, text, weak};
use crate::app::App;
use eframe::egui::*;

pub fn settings(app: &mut App, ui: &mut Ui) {
    source_list(app,ui);
    ui.add_space(8.0);
    text(ui,RichText::new(crate::i18n::t("R2_FLAT_DIRECTORIES")).strong());
    for models in [false,true] {
        text(ui,crate::i18n::t(if models{"R2_FLAT_MODELS"}else{"R2_FLAT_DATABASES"}));
        let selected=if models{&app.synthesis.library.settings.r2_ai_flat_models}else{&app.synthesis.library.settings.r2_ai_flat_databases};
        path_label(ui,selected.as_deref());
        let mut replacement=None;
        ui.horizontal(|ui|{
            if ui.button(crate::i18n::t("VOICE_SOURCE_CHANGE")).clicked(){replacement=folder_dialog(selected.as_deref()).pick_folder().map(Some);}
            if crate::ui::icons::close_button(ui).clicked(){replacement=Some(None);}
        });
        if let Some(path)=replacement {
            let settings=&app.synthesis.library.settings;
            let (databases,model_root)=if models{(settings.r2_ai_flat_databases.clone(),path)}else{(path,settings.r2_ai_flat_models.clone())};
            app.synthesis.set_flat_directories(databases,model_root);
        }
    }
    ui.add_space(8.0);
    text(ui,RichText::new(crate::i18n::t("PHONEME_DATA_BY_VERSION")).strong());
    use crate::synthesis::settings::PhonemeVersion;
    for version in PhonemeVersion::ALL.into_iter().rev().filter(|version|!PhonemeVersion::BETA.contains(version)) {
        phoneme_path(app,ui,version);
    }
    crate::ui::theme::native::collapsing(crate::i18n::t("BETA_DICTIONARIES"))
        .id_salt("beta-phoneme-directories").default_open(false)
        .show(ui,|ui|{for version in PhonemeVersion::BETA {phoneme_path(app,ui,version);}});
    super::duration_key::settings(app, ui);
    if let Some(error) = app.synthesis.library.settings_error() {
        error_label(ui, error);
    }
}

fn phoneme_path(app:&mut App,ui:&mut Ui,version:crate::synthesis::settings::PhonemeVersion) {
    ui.push_id(version.label(),|ui|{
        text(ui,version.label());
        path_label(ui,app.synthesis.library.settings.phoneme_data.get(version));
        ui.horizontal(|ui|{
            if ui.button(crate::i18n::t("VOICE_SOURCE_CHANGE")).clicked(){choose_phoneme_data(app,version);}
            if crate::ui::icons::close_button(ui).clicked(){app.synthesis.set_phoneme_data(version,None);}
        });
    });
}

pub(super) fn directory_menu(app:&mut App,ui:&mut Ui) {
    if ui.button(crate::i18n::t("VOICE_SOURCE_ADD")).clicked() {
        ui.close();add_source(app);
    }
}

fn add_source(app:&mut App) {
    if let Some(path)=folder_dialog(None).pick_folder() {
        let mut paths:Vec<_>=app.synthesis.library.settings.database_roots().filter(|(label,_)|*label!="R2 AI Flat").map(|(_,p)|p.to_owned()).collect();
        if !paths.iter().any(|p|std::fs::canonicalize(p).unwrap_or_else(|_|p.clone())==std::fs::canonicalize(&path).unwrap_or_else(|_|path.clone())) {
            paths.push(path);app.synthesis.set_voice_sources(paths);
        }
    }
}
fn source_list(app:&mut App,ui:&mut Ui) {
    text(ui,RichText::new(crate::i18n::t("VOICE_SOURCES_OTHERS")).strong());
    let mut paths:Vec<_>=app.synthesis.library.settings.database_roots().filter(|(label,_)|*label!="R2 AI Flat").map(|(_,p)|p.to_owned()).collect();
    let mut remove=None;let mut replace=None;
    for (index,path) in paths.iter().enumerate() {
        ui.push_id(index,|ui| {
            ui.horizontal(|ui| {
                if ui.small_button(crate::i18n::t("VOICE_SOURCE_CHANGE")).clicked() {
                    replace=folder_dialog(Some(path)).pick_folder().map(|p|(index,p));
                }
                if crate::ui::icons::close_button(ui).clicked() {remove=Some(index);}
                path_label(ui,Some(path));
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

pub(super) fn choose_phoneme_data(app: &mut App,version:crate::synthesis::settings::PhonemeVersion) {
    if let Some(path) = folder_dialog(app.synthesis.library.settings.phoneme_data.get(version)).pick_folder() {
        app.synthesis.set_phoneme_data(version,Some(path));
    }
}

pub(super) fn path_label(ui: &mut Ui, path: Option<&Path>) {
    match path {
        Some(path) => {
            let name=path.file_name().unwrap_or(path.as_os_str()).to_string_lossy();
            ui.add(Label::new(RichText::new(name).small().weak()).wrap())
                .on_hover_text(path.display().to_string());
        },
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
