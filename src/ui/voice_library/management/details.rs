use super::{
    super::{family_name, language_name, text, weak},
    feedback,
};
use crate::{
    app::App,
    synthesis::voice::{VoiceAvailability, VoiceCatalogEntry},
};
use eframe::egui::*;

pub(super) fn show(app: &mut App, ui: &mut Ui, entry: &VoiceCatalogEntry) {
    ui.spacing_mut().item_spacing.y = 10.0;
    let selection = entry.selection();
    text(ui, crate::lformat!("声库类型：{}", family_name(entry)));
    if let Some(range) = &entry.pitch_range {
        weak(ui, crate::lformat!("推荐音域  {range}"));
    }
    if let Some(config) = app
        .synthesis
        .inventory(&selection)
        .and_then(|voice| voice.configuration.as_ref())
    {
        text(
            ui,
            crate::lformat!(
                "支持语言：{}",
                config
                    .supported_languages
                    .iter()
                    .map(|s| language_name(s))
                    .collect::<Vec<_>>()
                    .join("、")
            ),
        );
        if !config.styles.is_empty() {
            text(ui, crate::lformat!("唱法：{}", config.styles.join(" · ")));
        }
        if !config.speakers.is_empty() {
            text(
                ui,
                crate::lformat!("合唱：{} 位成员", config.speakers.len()),
            );
        }
    } else if !entry.languages.is_empty() {
        text(
            ui,
            crate::lformat!(
                "语言：{}",
                entry
                    .languages
                    .iter()
                    .map(|s| language_name(s))
                    .collect::<Vec<_>>()
                    .join("、")
            ),
        );
    }
    if let VoiceAvailability::Unsupported(reason) = &entry.availability {
        super::super::error_label(ui, reason);
    }
    if app.synthesis.inventory(&selection).is_some()
        || app.synthesis.voice_error(&selection).is_some()
    {
        feedback::status(app, ui, &selection);
    }
    feedback::diagnostics(app, ui, entry);
    if entry
        .family
        .is_some_and(|family| family.requires_identity())
    {
        super::super::duration_key::voice_settings(app, ui, entry);
    }
}
