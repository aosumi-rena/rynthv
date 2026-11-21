use crate::{
    i18n::{message, t},
    ui::icons::{Icon, IconButton},
};
use eframe::egui::{self, Ui};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use super::widgets::row;

pub(super) fn source_choices(ui: &mut Ui, choice: &mut usize, labels: &[&str]) -> bool {
    let before = *choice;
    ui.horizontal_wrapped(|ui| {
        for (index, label) in labels.iter().enumerate() {
            ui.selectable_value(choice, index, t(label));
        }
    });
    *choice != before
}

pub(super) fn file_field(ui: &mut Ui, path: &mut Option<PathBuf>) -> bool {
    let before = path.clone();
    let mut picked = false;
    row(ui, "DNNI", |ui| {
        if ui.button(t("ROCOFLEX_BROWSE")).clicked() {
            let mut dialog = rfd::FileDialog::new().add_filter("DNNI", &["dnni"]);
            if let Some(parent) = path.as_deref().and_then(Path::parent) {
                dialog = dialog.set_directory(parent);
            }
            if let Some(selected) = dialog.pick_file() {
                *path = Some(selected);
                picked = true;
            }
        }
        if let Some(file) = path.clone() {
            if clear_button(ui).clicked() {
                *path = None;
            }
            ui.add(
                egui::Label::new(
                    file.file_name()
                        .unwrap_or(file.as_os_str())
                        .to_string_lossy(),
                )
                .truncate(),
            )
            .on_hover_text(file.display().to_string());
        } else {
            ui.label(t("尚未指定"));
        }
    });
    *path != before || picked
}

pub(super) fn donor_field<T>(
    ui: &mut Ui,
    selection: &mut Option<T>,
    picker: impl FnOnce(&mut Ui, &mut Option<T>),
) {
    ui.horizontal(|ui| {
        if selection.is_some() && clear_button(ui).clicked() {
            *selection = None;
        }
        picker(ui, selection);
    });
}

fn clear_button(ui: &mut Ui) -> egui::Response {
    ui.add(IconButton::new(Icon::Close, t("VOICE_LAB_CLEAR_DONOR")))
}

#[derive(Clone)]
struct Validation<S> {
    source: S,
    result: Arc<Mutex<Option<Result<(), String>>>>,
}

pub(super) fn clear_validation<S: Clone + Send + Sync + 'static>(ui: &Ui, id: egui::Id) {
    ui.ctx().data_mut(|data| data.remove::<Validation<S>>(id));
}

pub(super) fn validation<S: Clone + Eq + Send + Sync + 'static>(
    ui: &mut Ui,
    id: egui::Id,
    source: Result<Option<S>, String>,
    [required, checking, valid, invalid]: [&str; 4],
    check: impl FnOnce(S) -> Result<(), String> + Send + 'static,
) {
    let source = match source {
        Ok(Some(source)) => source,
        Ok(None) => {
            ui.colored_label(egui::Color32::LIGHT_RED, t(required));
            return;
        }
        Err(error) => {
            ui.colored_label(egui::Color32::LIGHT_RED, message(&error));
            return;
        }
    };
    let existing = ui.ctx().data(|data| data.get_temp::<Validation<S>>(id));
    let validation = match existing {
        Some(value) if value.source == source => value,
        _ => {
            let value = Validation {
                source: source.clone(),
                result: Arc::new(Mutex::new(None)),
            };
            let result = Arc::clone(&value.result);
            let repaint = ui.ctx().clone();
            if let Err(error) = std::thread::Builder::new()
                .name("override-validation".into())
                .spawn(move || {
                    let checked = check(source);
                    if let Ok(mut result) = result.lock() {
                        *result = Some(checked);
                    }
                    repaint.request_repaint();
                })
            {
                if let Ok(mut result) = value.result.lock() {
                    *result = Some(Err(error.to_string()));
                }
            }
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, value.clone()));
            value
        }
    };
    if let Ok(result) = validation.result.lock() {
        match result.as_ref() {
            None => {
                ui.label(t(checking));
            }
            Some(Ok(())) => {
                ui.label(t(valid));
            }
            Some(Err(error)) => {
                ui.colored_label(egui::Color32::LIGHT_RED, t(invalid))
                    .on_hover_text(message(error));
            }
        }
    }
}
