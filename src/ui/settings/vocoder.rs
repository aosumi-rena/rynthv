use super::widgets::{row, section, toggle};
use crate::{app::App, synthesis::{experiments::ExperimentalSettings, voice::R2VocoderSource}};
use eframe::egui::{self, Ui};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Validation {
    source: R2VocoderSource,
    result: Arc<Mutex<Option<Result<(), String>>>>,
}

pub(super) fn show(app: &mut App, ui: &mut Ui, settings: &mut ExperimentalSettings) {
    section(ui, crate::i18n::t("R2_VOCODER"));
    toggle(ui, crate::i18n::t("R2_VOCODER_ENABLE"), &mut settings.r2_vocoder);
    if !settings.r2_vocoder { return; }
    let id = ui.id().with("r2-vocoder-validation");
    let mode_id = id.with("voice-source");
    let mut voice_source = app.synthesis.library.settings.r2_vocoder_voice.is_some()
        || ui.ctx().data(|data| data.get_temp::<bool>(mode_id)).unwrap_or(false);
    let before = voice_source;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut voice_source, false, crate::i18n::t("R2_VOCODER_FILE"));
        ui.selectable_value(&mut voice_source, true, crate::i18n::t("R2_VOCODER_VOICE"));
    });
    if voice_source != before {
        app.synthesis.set_r2_vocoder(None);
        ui.ctx().data_mut(|data| data.remove::<Validation>(id));
    }
    ui.ctx().data_mut(|data| data.insert_temp(mode_id, voice_source));
    let changed = if voice_source { voice_field(app, ui) } else { file_field(app, ui) };
    if changed { ui.ctx().data_mut(|data| data.remove::<Validation>(id)); }
    let source = match app.synthesis.library.r2_vocoder_source() {
        Ok(Some(source)) => source,
        Ok(None) => {
            ui.colored_label(egui::Color32::LIGHT_RED, crate::i18n::t("R2_VOCODER_REQUIRED"));
            return;
        }
        Err(error) => { ui.colored_label(egui::Color32::LIGHT_RED, crate::i18n::message(&error)); return; }
    };
    let existing = ui.ctx().data(|data| data.get_temp::<Validation>(id));
    let validation = match existing {
        Some(value) if value.source == source => value,
        _ => {
            let value = Validation { source: source.clone(), result: Arc::new(Mutex::new(None)) };
            let result = Arc::clone(&value.result);
            let repaint = ui.ctx().clone();
            if let Err(error) = std::thread::Builder::new().name("vocoder-validation".into()).spawn(move || {
                let checked = crate::synthesis::voice::validate_r2_vocoder(&source).map_err(|error| error.to_string());
                if let Ok(mut result) = result.lock() { *result = Some(checked); }
                repaint.request_repaint();
            }) {
                if let Ok(mut result) = value.result.lock() { *result = Some(Err(error.to_string())); }
            }
            ui.ctx().data_mut(|data| data.insert_temp(id, value.clone()));
            value
        }
    };
    if let Ok(result) = validation.result.lock() {
        match result.as_ref() {
            None => { ui.label(crate::i18n::t("R2_VOCODER_CHECKING")); }
            Some(Ok(())) => { ui.label(crate::i18n::t("R2_VOCODER_VALID")); }
            Some(Err(error)) => {
                ui.colored_label(egui::Color32::LIGHT_RED, crate::i18n::t("R2_VOCODER_INVALID"))
                    .on_hover_text(crate::i18n::message(error));
            }
        }
    }
}

fn file_field(app: &mut App, ui: &mut Ui) -> bool {
    let mut path = app.synthesis.library.settings.r2_vocoder.clone();
    let before = path.clone();
    let mut picked = false;
    row(ui, "DNNI", |ui| {
        if ui.button(crate::i18n::t("ROCOFLEX_BROWSE")).clicked() {
            let mut dialog = rfd::FileDialog::new().add_filter("DNNI", &["dnni"]);
            if let Some(parent) = path.as_deref().and_then(std::path::Path::parent) { dialog = dialog.set_directory(parent); }
            if let Some(selected) = dialog.pick_file() { path = Some(selected); picked = true; }
        }
        if let Some(file) = path.clone() {
            if clear_button(ui).clicked() { path = None; }
            ui.add(egui::Label::new(file.file_name().unwrap_or(file.as_os_str()).to_string_lossy()).truncate())
                .on_hover_text(file.display().to_string());
        } else { ui.label(crate::i18n::t("尚未指定")); }
    });
    if path != before || picked { app.synthesis.set_r2_vocoder(path); return true; }
    false
}

fn voice_field(app: &mut App, ui: &mut Ui) -> bool {
    let mut voice = app.synthesis.library.settings.r2_vocoder_voice.clone();
    let before = voice.clone();
    ui.horizontal(|ui| {
        if voice.is_some() && clear_button(ui).clicked() { voice = None; }
        crate::ui::voice_library::vocoder_picker(ui, app.synthesis.library.catalog(), &mut voice, app.synthesis.library.is_scanning());
    });
    if voice != before { app.synthesis.set_r2_vocoder_voice(voice); return true; }
    false
}

fn clear_button(ui: &mut Ui) -> egui::Response {
    ui.add(crate::ui::icons::IconButton::new(crate::ui::icons::Icon::Close, crate::i18n::t("VOICE_LAB_CLEAR_DONOR")))
}
