use super::{
    override_controls as controls,
    widgets::{section, toggle},
};
use crate::{
    app::App,
    i18n::t,
    synthesis::{experiments::ExperimentalSettings, voice::VocoderSource},
};
use eframe::egui::Ui;

pub(super) fn show(app: &mut App, ui: &mut Ui, settings: &mut ExperimentalSettings) {
    section(ui, t("VOCODER_OVERRIDE"));
    toggle(ui, t("VOCODER_OVERRIDE_ENABLE"), &mut settings.r2_vocoder);
    if !settings.r2_vocoder {
        return;
    }
    let id = ui.id().with("vocoder-validation");
    let mode_id = id.with("voice-source");
    let voice_source = app.synthesis.library.settings.r2_vocoder_voice.is_some()
        || ui
            .ctx()
            .data(|data| data.get_temp::<bool>(mode_id))
            .unwrap_or(false);
    let mut choice =
        usize::from(app.synthesis.library.settings.vocoder_r3) * 2 + usize::from(voice_source);
    if controls::source_choices(
        ui,
        &mut choice,
        &[
            "VOCODER_OVERRIDE_FILE",
            "VOCODER_OVERRIDE_VOICE",
            "VOCODER_OVERRIDE_R3_FILE",
            "VOCODER_OVERRIDE_R3_VOICE",
        ],
    ) {
        app.synthesis.set_vocoder_generation(choice >= 2);
        controls::clear_validation::<VocoderSource>(ui, id);
    }
    let voice_source = choice % 2 == 1;
    ui.ctx()
        .data_mut(|data| data.insert_temp(mode_id, voice_source));
    if voice_source {
        let library = &app.synthesis.library;
        let mut voice = library.settings.r2_vocoder_voice.clone();
        let before = voice.clone();
        controls::donor_field(ui, &mut voice, |ui, voice| {
            crate::ui::voice_library::vocoder_picker(
                ui,
                library.catalog(),
                voice,
                library.is_scanning(),
                library.settings.vocoder_r3,
            );
        });
        if voice != before {
            app.synthesis.set_r2_vocoder_voice(voice);
            controls::clear_validation::<VocoderSource>(ui, id);
        }
    } else {
        let mut path = app.synthesis.library.settings.r2_vocoder.clone();
        if controls::file_field(ui, &mut path) {
            app.synthesis.set_r2_vocoder(path);
            controls::clear_validation::<VocoderSource>(ui, id);
        }
    }
    controls::validation(
        ui,
        id,
        app.synthesis.library.r2_vocoder_source(),
        [
            "VOCODER_OVERRIDE_REQUIRED",
            "VOCODER_OVERRIDE_CHECKING",
            "VOCODER_OVERRIDE_VALID",
            "VOCODER_OVERRIDE_INVALID",
        ],
        |source| {
            crate::synthesis::voice::validate_vocoder(&source).map_err(|error| error.to_string())
        },
    );
}
