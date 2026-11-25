use crate::{app::App, ui::theme::*};
use eframe::egui::*;
pub fn show(app: &mut App, ui: &mut Ui) {
    super::section(ui, crate::i18n::t("Instrument"), |ui| {
        super::selector::card(app, ui);
        let current = super::current(app).cloned();
        ui.label(crate::i18n::t("Version"));
        let previous = current.as_ref().map(|e| e.selection.clone()).or_else(|| {
            app.synthesis.instruments.requested_selection(app.editor.current_track())
        });
        let mut selected = previous.clone();
        ComboBox::from_id_salt("instrument-version")
            .width(ui.available_width())
            .selected_text(
                selected
                    .as_ref()
                    .map(|s| s.revision.to_string())
                    .unwrap_or_else(|| "—".into()),
            )
            .show_ui(ui, |ui| {
                if let Some(previous) = &previous {
                    for entry in &app.synthesis.instruments.catalog.entries {
                        if entry.selection.database_id == previous.database_id {
                            ui.selectable_value(
                                &mut selected,
                                Some(entry.selection.clone()),
                                entry.selection.revision.to_string(),
                            );
                        }
                    }
                    super::unavailable_versions(ui, &app.synthesis.instruments, &previous.database_id);
                }
            });
        if selected != previous
            && let Some(selected) = selected
            && let Some(entry) = app.synthesis.instruments.entry(&selected).cloned()
        {
            app.editor.select_instrument(&entry);
        }
        ui.label(crate::i18n::t("Preset"));
        crate::ui::presets::show(app,ui,crate::ui::presets::Kind::Instrument);
        for reference in app.editor.current_track().references() {
            if let Some(error) = app.synthesis.instruments.errors.get(&reference.id) {
                ui.colored_label(Color32::LIGHT_RED, crate::i18n::message(error));
            }
        }
    });
    super::section(ui, crate::i18n::t("Group"), |ui| {
        if let Some(reference) = app.editor.active_ref()
            && let Some(group) = app.editor.project.group(&reference.group_id)
        {
            ui.label(RichText::new(&group.name).color(crate::ui::theme::accent(ui)));
        }
    });
    super::section(ui, crate::i18n::t("Ensemble"), |ui| {
        super::ensemble::players(app,ui);
    });
    super::section(ui, crate::i18n::t("Performance Direction"), |ui| {
        super::controls::direction(app,ui);
    });
    super::section(ui, crate::i18n::t("Parameters"), |ui| {
        use crate::synthesis::parameters::ParameterSpec;
        for spec in [
            ParameterSpec {key:"paramLoudness",label:"Gain",limit:12.,step:0.05,unit:"dB"},
            ParameterSpec {key:"paramBreathiness",label:"Roughness",limit:1.,step:0.005,unit:""},
        ] {crate::ui::voice_library::parameter_control(app,ui,spec);}
    });
    super::section(ui, crate::i18n::t("Tuning"), |ui| {
        let reference=app.editor.active_ref().unwrap_or(&app.editor.current_track().main_ref);
        let transpose=crate::synthesis::instruments::tuning_transposition(reference).unwrap_or(0.);
        let semitones=reference.extra.get("voice").and_then(|v|v.get("transposeSemitones")).and_then(serde_json::Value::as_f64).unwrap_or(0.).round() as i64;
        let tonic=crate::ui::scale::Tonic::ALL[semitones.rem_euclid(12) as usize].short_label();
        let frequency=440.*2f64.powf(transpose/12.);
        let text=format!("{}: {tonic}, A4 = {:.3} Hz",crate::i18n::t("Key"),frequency);
        Frame::new().fill(BORDER).corner_radius(4).inner_margin(8).show(ui,|ui|{ui.set_width(ui.available_width());ui.label(text);});
        super::controls::tuning(app,ui);
    });
    super::section(ui,crate::i18n::t("Scale"),|ui|crate::ui::scale::controls::instrument(app,ui));
}
