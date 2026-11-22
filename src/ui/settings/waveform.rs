use super::{
    override_controls as controls,
    widgets::{section, toggle},
};
use crate::{
    app::App,
    i18n::t,
    synthesis::instruments::{WaveformSource, validate_waveform},
    ui::instruments::waveform_picker,
};
use eframe::egui::Ui;
use std::path::PathBuf;

pub(super) fn show(app: &mut App, ui: &mut Ui) {
    let mut settings = app.synthesis.library.settings.instruments.clone();
    let before = settings.clone();
    section(ui, t("Waveform program override"));
    toggle(
        ui,
        t("Override instrument waveform programs"),
        &mut settings.waveform_override_enabled,
    );
    if settings.waveform_override_enabled {
        let id = ui.id().with("waveform-validation");
        let mode_id = id.with("instrument-source");
        let accept_any_v4_signature = settings.accept_any_v4_signature;
        let id = id.with(accept_any_v4_signature);
        let mut choice = match &settings.waveform_override {
            Some(WaveformSource::File(_)) => 0,
            Some(WaveformSource::Instrument(_)) => 1,
            None => usize::from(
                ui.ctx()
                    .data(|data| data.get_temp::<bool>(mode_id))
                    .unwrap_or(false),
            ),
        };
        if controls::source_choices(
            ui,
            &mut choice,
            &["Waveform model file", "Instrument model"],
        ) {
            settings.waveform_override = None;
            controls::clear_validation::<PathBuf>(ui, id);
        }
        let instrument_source = choice == 1;
        ui.ctx()
            .data_mut(|data| data.insert_temp(mode_id, instrument_source));
        if instrument_source {
            let mut selection = match &settings.waveform_override {
                Some(WaveformSource::Instrument(selection)) => Some(selection.clone()),
                _ => None,
            };
            let previous = selection.clone();
            controls::donor_field(ui, &mut selection, |ui, selection| {
                waveform_picker(app, ui, selection)
            });
            if selection != previous {
                settings.waveform_override = selection.map(WaveformSource::Instrument);
                controls::clear_validation::<PathBuf>(ui, id);
            }
        } else {
            let mut path = match &settings.waveform_override {
                Some(WaveformSource::File(path)) => Some(path.clone()),
                _ => None,
            };
            if controls::file_field(ui, &mut path) {
                settings.waveform_override = path.map(WaveformSource::File);
                controls::clear_validation::<PathBuf>(ui, id);
            }
        }
        controls::validation(
            ui,
            id,
            app.synthesis
                .instruments
                .waveform_override_path(settings.waveform_override.as_ref()),
            [
                "Select a waveform override source",
                "Checking waveform program",
                "Valid waveform program",
                "Invalid waveform program",
            ],
            move |path| validate_waveform(&path, accept_any_v4_signature),
        );
    }
    if settings != before {
        app.synthesis.set_instrument_settings(settings);
    }
}
