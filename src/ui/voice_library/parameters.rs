use crate::synthesis::parameters::{ParameterSpec, VOICE_CONTROLS};
use crate::ui::slider::{Appearance, Slider};
use crate::{app::App, synthesis::voice::VoiceFamily};
use eframe::egui::*;

pub(super) fn show(app: &mut App, ui: &mut Ui) {
    ui.spacing_mut().item_spacing.y = 0.0;
    for spec in VOICE_CONTROLS {
        control(app, ui, spec);
    }
}
pub(crate) fn control(app: &mut App, ui: &mut Ui, mut spec: ParameterSpec) {
    let instrument = app.editor.current_track().is_instrument();
    if !instrument
        && crate::synthesis::parameters::VOICE_CONTROLS
            .iter()
            .any(|s| s.key == spec.key)
    {
        spec.limit *= app
            .synthesis
            .library
            .settings
            .experiments
            .expression_factor();
    }
    let parameter = match spec.key {
        "paramTension" => Some(crate::model::Parameter::Tension),
        "paramBreathiness" => Some(crate::model::Parameter::Breathiness),
        "paramGender" => Some(crate::model::Parameter::Gender),
        _ => None,
    };
    let bounded = !instrument
        && parameter
            .as_ref()
            .is_some_and(|p| crate::ui::lanes::standard_expression_limit(app, p).is_some());
    if bounded {
        let (_, high) =
            crate::ui::lanes::accepted_parameter_range(app, parameter.as_ref().unwrap());
        spec.limit = spec.limit.min(high as f32);
    }
    let additional = VOICE_CONTROLS
        .iter()
        .chain(crate::synthesis::parameters::TUNING_CONTROLS.iter())
        .any(|s| s.key == spec.key);
    let supported = (instrument && matches!(spec.key, "paramLoudness" | "paramBreathiness"))
        || (!instrument
            && app
                .voice_inventory()
                .and_then(|v| v.configuration.as_ref())
                .is_some_and(|v| {
                    v.family == VoiceFamily::Sv2
                        || (v.family.is_standard() && additional && spec.key != "paramToneShift")
                        || (!v.family.is_standard()
                            && additional
                            && app
                                .synthesis
                                .library
                                .settings
                                .experiments
                                .active(app.synthesis.library.settings.experiments.dds_controls))
                }));
    let appearance = if instrument {
        Appearance::Instrument(
            crate::ui::track_color::TrackColors::new(app.editor.current_track()).accent,
        )
    } else {
        Appearance::Voice
    };
    match app.editor.voice_parameter(spec) {
        Ok((reference, value)) if supported => {
            let id = ui.make_persistent_id((spec.key, reference, app.editor.revision));
            let output = Slider::new(
                id,
                crate::i18n::t(spec.label),
                f64::from(value),
                -f64::from(spec.limit),
                f64::from(spec.limit),
                0.,
            )
            .step(f64::from(spec.step))
            .suffix(crate::i18n::t(spec.unit))
            .appearance(appearance)
            .bipolar()
            .show(ui);
            if bounded
                && !{
                    let (low, high) = crate::ui::lanes::accepted_parameter_range(
                        app,
                        parameter.as_ref().unwrap(),
                    );
                    (low..=high).contains(&f64::from(value))
                }
            {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    crate::lformat!(
                        "Current value {value:.3} is outside this voice's supported range"
                    ),
                );
            }
            if let Some(change) = output.change
                && let Err(error) =
                    app.editor
                        .set_voice_parameter(reference, spec, change.value as f32)
            {
                app.error = Some(error);
            }
        }
        _ => {
            ui.add_enabled_ui(false, |ui| {
                Slider::new(
                    ui.id().with(spec.key),
                    crate::i18n::t(spec.label),
                    0.,
                    -f64::from(spec.limit),
                    f64::from(spec.limit),
                    0.,
                )
                .appearance(appearance)
                .bipolar()
                .show(ui);
            });
        }
    }
}
