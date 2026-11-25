use crate::{
    app::App,
    synthesis::{
        instruments::{InstrumentKeyOverride, InstrumentSettings},
        voice::session_identity::parse_identity,
    },
};
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui) {
    app.synthesis.instruments.ensure_scan(ui.ctx());
    let mut settings = app.synthesis.library.settings.instruments.clone();
    let before = settings.clone();
    ui.label(RichText::new(crate::i18n::t("Instrument Resources")).strong());
    let mut remove = None;
    for (i, path) in settings.roots().iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.small_button("×").clicked() {
                remove = Some(i);
            }
            ui.label(path.file_name().unwrap_or_default().to_string_lossy())
                .on_hover_text(path.display().to_string());
        });
    }
    if let Some(i) = remove {
        let mut roots = settings.roots();
        roots.remove(i);
        settings.sources = roots;
    }
    ui.horizontal(|ui| {
        if ui.button(crate::i18n::t("Add Folder...")).clicked()
            && let Some(path) = rfd::FileDialog::new().pick_folder()
        {
            let mut roots = settings.roots();
            if !roots.contains(&path) {
                roots.push(path);
            }
            settings.sources = roots;
        }
        if ui.button(crate::i18n::t("Rescan")).clicked() {
            app.synthesis.rescan_instruments();
        }
    });
    ui.separator();
    ui.label(crate::i18n::t("Room Resources"));
    ui.horizontal(|ui| {
        if let Some(path)=settings.room_pack() {
            ui.label(path.file_name().unwrap_or_default().to_string_lossy()).on_hover_text(path.display().to_string());
        }
        if ui.button(crate::i18n::t("Browse...")).clicked()
            && let Some(path)=rfd::FileDialog::new().add_filter("Instrument X rooms", &["drirpack"]).pick_file() {
            match crate::synthesis::instruments::rooms::load(&path) {
                Ok(_)=>settings.room_pack=Some(path),Err(e)=>app.error=Some(e),
            }
        }
    });
    ui.separator();
    key(
        ui,
        "global-instrument-key",
        "Override auxiliary key",
        &mut settings.key_override_enabled,
        &mut settings.key_override_value,
    );
    if settings != before {
        app.synthesis.set_instrument_settings(settings);
    }
    if let Some(error) = &app.synthesis.instruments.scan_error {
        ui.colored_label(Color32::LIGHT_RED, crate::i18n::message(error));
    }
    if app.synthesis.instruments.is_scanning() {
        ui.spinner();
    }
}
fn key(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    enabled: &mut bool,
    value: &mut u64,
) {
    ui.push_id(id, |ui| {
        ui.checkbox(enabled, crate::i18n::t(label));
        let id = ui.id().with("draft");
        let mut draft = ui
            .ctx()
            .data(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| value.to_string());
        ui.add_enabled_ui(*enabled, |ui| {
            ui.text_edit_singleline(&mut draft);
            match parse_identity(&draft) {
                Ok(parsed) => {
                    if ui.button(crate::i18n::t("Apply")).clicked() {
                        *value = parsed;
                    }
                }
                Err(error) => {
                    ui.colored_label(Color32::LIGHT_RED, error.to_string());
                }
            }
        });
        ui.ctx().data_mut(|d| d.insert_temp(id, draft));
    });
}
pub(super) fn version(app: &mut App, ui: &mut Ui, entry: &crate::synthesis::instruments::Entry) {
    let mut settings: InstrumentSettings = app.synthesis.library.settings.instruments.clone();
    let index = settings
        .overrides
        .iter()
        .position(|o| o.instrument == entry.selection);
    let mut data = index
        .map(|i| settings.overrides[i].clone())
        .unwrap_or(InstrumentKeyOverride {
            instrument: entry.selection.clone(),
            enabled: false,
            value: 0,
        });
    let before = data.clone();
    key(
        ui,
        (&entry.selection.database_id, &entry.selection.revision),
        "Override this version's auxiliary key",
        &mut data.enabled,
        &mut data.value,
    );
    if data != before {
        if let Some(i) = index {
            settings.overrides[i] = data;
        } else {
            settings.overrides.push(data);
        }
        app.synthesis.set_instrument_settings(settings);
    }
}
