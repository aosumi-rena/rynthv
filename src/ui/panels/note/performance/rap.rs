use super::{interaction, widgets};
use crate::{
    app::App,
    editor::{PerformanceParameter, PerformanceSelection},
    model::{Language, NotePerformance},
};
use eframe::egui::*;

pub(super) fn show(app: &mut App, ui: &mut Ui, selection: &PerformanceSelection, enabled: bool) {
    let accent_enabled = selected_mandarin(app);
    ui.add_enabled_ui(enabled, |ui| {
        ui.weak(crate::i18n::t("说唱声调"));
        let label = if accent_enabled {
            match selection.accent.as_deref() {
                None => crate::i18n::t("多个值"),
                Some("") => crate::i18n::t("自动"),
                Some(value) => value,
            }
        } else {
            "N/A"
        };
        let mut clicked = None;
        ui.add_enabled_ui(accent_enabled, |ui| {
            ComboBox::from_id_salt("note-rap-accent")
                .width(ui.available_width())
                .selected_text(label)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(selection.accent.as_deref() == Some(""), crate::i18n::t("自动"))
                        .clicked()
                    {
                        clicked = Some(None);
                    }
                    for value in 1_u8..=5 {
                        let label = value.to_string();
                        if ui
                            .selectable_label(
                                selection.accent.as_deref() == Some(label.as_str()),
                                label,
                            )
                            .clicked()
                        {
                            clicked = Some(Some(value));
                        }
                    }
                })
                .response
                .on_disabled_hover_text(
                    crate::i18n::t("说唱声调仅用于汉语普通话，请选择语言均为汉语普通话的音符。"),
                );
        });
        if let Some(value) = clicked {
            app.commit_lyric();
            app.editor.set_rap_accent(value);
        }
        for (parameter, label, current, minimum, maximum) in [
            (
                PerformanceParameter::RapIntonation,
                crate::i18n::t("语调"),
                selection.r_intonation,
                NotePerformance::RAP_INTONATION_MIN,
                NotePerformance::RAP_INTONATION_MAX,
            ),
            (
                PerformanceParameter::RapTone,
                crate::i18n::t("音调"),
                selection.r_tone,
                NotePerformance::RAP_TONE_MIN,
                NotePerformance::RAP_TONE_MAX,
            ),
        ] {
            ui.add_space(8.0);
            let minimum = f64::from(minimum);
            let span = f64::from(maximum) - minimum;
            let mut value = (f64::from(current.flatten().unwrap_or(0.0)) - minimum) / span;
            let text = match current {
                None => crate::i18n::t("多个值").into(),
                Some(None) => crate::i18n::t("自动").into(),
                Some(Some(value)) => format!("{value:+.2}"),
            };
            let response = widgets::labeled_slider(
                ui,
                label,
                text,
                &mut value,
                current.flatten().is_none(),
                -minimum / span,
            );
            let reset = response.double_clicked()
                || response.has_focus() && ui.input(|input| input.key_pressed(Key::Home));
            let value = (!reset).then_some((minimum + value * span) as f32);
            interaction::apply(
                app,
                ui.ctx(),
                &response,
                parameter,
                |editor| match parameter {
                    PerformanceParameter::RapTone => {
                        editor.update_performance_rap_tone(value);
                    }
                    PerformanceParameter::RapIntonation => {
                        editor.update_performance_rap_intonation(value);
                    }
                    _ => unreachable!(),
                },
            );
        }
    });
}

fn selected_mandarin(app: &App) -> bool {
    let Some(reference) = app.editor.active_ref() else {
        return false;
    };
    let Ok(inherited) = inherited_language(app) else {
        return false;
    };
    let mut notes = app
        .editor
        .notes()
        .iter()
        .filter(|note| {
            app.editor.selected.contains(&note.id) && reference.note_range(note).is_some()
        })
        .peekable();
    notes.peek().is_some()
        && notes.all(|note| {
            let language = match note.attributes.get("languageOverride") {
                None => inherited,
                Some(serde_json::Value::String(value)) if value.is_empty() => inherited,
                Some(serde_json::Value::String(value)) => Language::parse(value),
                Some(_) => None,
            };
            language == Some(Language::Mandarin)
        })
}

fn inherited_language(app: &App) -> Result<Option<Language>, ()> {
    let mut language = app
        .voice_inventory()
        .and_then(|voice| voice.configuration.as_ref())
        .and_then(|configuration| Language::parse(&configuration.default_language));
    let track = &app.editor.current_track().main_ref;
    for database in [
        &track.database,
        &app.editor.active_ref().unwrap_or(track).database,
    ] {
        if !database.is_object() {
            return Err(());
        }
        for field in ["language", "languageOverride"] {
            if let Some(value) = database.get(field) {
                let value = value.as_str().ok_or(())?;
                if !value.is_empty() {
                    language = Some(Language::parse(value).ok_or(())?);
                }
            }
        }
    }
    Ok(language)
}
