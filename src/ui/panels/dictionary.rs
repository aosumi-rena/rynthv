use super::controls::*;
use crate::{
    app::App,
    model::{Language, dictionary_name},
    ui::theme::*,
};
use eframe::egui::*;

#[derive(Clone)]
struct State {
    target: u64,
    dictionary: String,
    word: String,
    phones: String,
    xsampa: bool,
    new_name: String,
    language: Language,
    error: Option<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            target: 0,
            dictionary: String::new(),
            word: String::new(),
            phones: String::new(),
            xsampa: false,
            new_name: String::new(),
            language: Language::Mandarin,
            error: None,
        }
    }
}

pub(super) fn show(app: &mut App, ui: &mut Ui, viewport_height: f32) {
    let content_top = ui.next_widget_position().y;
    let id = ui.make_persistent_id("dictionary-state");
    let mut state = ui
        .ctx()
        .data_mut(|d| d.get_temp::<State>(id).unwrap_or_default());
    let track = app.editor.current_track();
    let target = app.editor.active_ref().unwrap_or(&track.main_ref);
    let target_id = target.id;
    let original = target
        .extra
        .get("dictionary")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let name = dictionary_name(track, target).to_owned();
    if state.target!=target_id && let Some(language)=crate::ui::phoneme_candidates::inherited_language(app) {
        state.language=language;
    }
    if state.target != target_id || state.dictionary != name {
        state.target = target_id;
        state.dictionary.clone_from(&name);
        state.word.clear();
        state.phones.clear();
        state.xsampa=false;
        state.error = None;
    }
    padded(ui, |ui| {
        ui.weak(crate::i18n::t("当前词典"));
        let mut selected = original.clone();
        ComboBox::from_id_salt("dictionary-select")
            .width(ui.available_width())
            .selected_text(if original.is_empty() {
                crate::i18n::t("（继承音轨默认值）")
            } else {
                &original
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, String::new(), crate::i18n::t("（继承音轨默认值）"));
                for (name, dictionary) in &app.editor.project.dictionaries {
                    ui.selectable_value(&mut selected, name.clone(), name)
                        .on_hover_text(crate::i18n::t(dictionary.language.label()));
                }
            });
        if selected != original {
            app.commit_lyric();
            app.editor.select_dictionary(&selected);
        }
        pair(ui, |ui, width| {
            let create = ui.add_sized(vec2(width, 28.0), Button::new(crate::i18n::t("新建…")));
            Popup::from_toggle_button_response(&create)
                .width(240.0)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    ui.label(crate::i18n::t("新建发音词典"));
                    field(ui, &mut state.new_name);
                    let options: Vec<_> =
                        Language::ALL.into_iter().map(|l| (l, crate::i18n::t(l.label()))).collect();
                    combo(ui, "dictionary-language", &mut state.language, &options);
                    if ui.button(crate::i18n::t("创建")).clicked() {
                        app.commit_lyric();
                        match app
                            .editor
                            .create_dictionary(&state.new_name, state.language)
                        {
                            Ok(()) => {
                                state.new_name.clear();
                                state.error = None;
                                ui.close();
                            }
                            Err(message) => state.error = Some(message),
                        }
                    }
                    if let Some(message) = &state.error {
                        error(ui, message);
                    }
                });
            if ui
                .add_enabled(
                    !name.is_empty(),
                    Button::new(crate::i18n::t("移除")).min_size(vec2(width, 28.0)),
                )
                .clicked()
            {
                app.commit_lyric();
                app.editor.remove_dictionary(&name);
            }
        });
    });
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.next_widget_position().y,
        Stroke::new(1.0, EDGE),
    );
    padded(ui, |ui| {
        let dictionary = app.editor.project.dictionaries.get(&name).cloned();
        ui.add_enabled_ui(dictionary.is_some(), |ui| {
            ui.weak(crate::i18n::t("字词"));
            field(ui, &mut state.word);
            ui.weak(crate::i18n::t("音素"));
            field(ui, &mut state.phones);
            if ui.checkbox(&mut state.xsampa,crate::i18n::t("Use X-SAMPA phonemes")).changed()
                && let Some(dict)=dictionary.as_ref()
                && let Ok(catalog)=crate::ui::phoneme_candidates::catalog(app,ui.ctx()) {
                state.phones=state.phones.split_whitespace().map(|token|
                    catalog.convert(dict.language,token,state.xsampa).unwrap_or_else(|_|token.to_owned()))
                    .collect::<Vec<_>>().join(" ");
            }
            pair(ui, |ui, width| {
                if ui
                    .add_enabled(
                        !state.word.trim().is_empty() && !state.phones.trim().is_empty(),
                        Button::new(crate::i18n::t("Set")).min_size(vec2(width, 28.0)),
                    )
                    .clicked()
                {
                    app.commit_lyric();
                    state.error = app
                        .editor
                        .set_dictionary_entry(&name, &state.word, Some(&state.phones),state.xsampa)
                        .err();
                }
                let exists = dictionary
                    .as_ref()
                    .is_some_and(|dict| dict.entries.contains_key(state.word.trim()));
                if ui
                    .add_enabled(exists, Button::new(crate::i18n::t("移除")).min_size(vec2(width, 28.0)))
                    .clicked()
                {
                    app.commit_lyric();
                    state.error = app
                        .editor
                        .set_dictionary_entry(&name, &state.word, None,false)
                        .err();
                    state.word.clear();
                    state.phones.clear();
                }
            });
        });
        if let Some(message) = &state.error {
            error(ui, message);
        }

        let height =
            (viewport_height - (ui.next_widget_position().y - content_top) - 32.0).max(210.0);
        Frame::new()
            .fill(BACKGROUND)
            .inner_margin(8)
            .show(ui, |ui| {
                ui.set_min_height(height);
                ui.set_width(ui.available_width());
                let half = (ui.available_width() * 0.3).max(25.0);
                let phone_width=(ui.available_width()*0.42).max(25.0);
                ui.horizontal(|ui| {
                    ui.add_sized(
                        vec2(half, 24.0),
                        Label::new(RichText::new(crate::i18n::t("字词")).color(MUTED)),
                    );
                    ui.add_sized(vec2(phone_width,24.0),Label::new(crate::i18n::t("发音")));
                    ui.weak(crate::i18n::t("Phoneset"));
                });
                ScrollArea::vertical()
                    .id_salt("dictionary-entries")
                    .max_height(height - 28.0)
                    .show(ui, |ui| {
                        if let Some(dictionary) = app.editor.project.dictionaries.get(&name) {
                            for (word, phones) in &dictionary.entries {
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_sized(
                                            vec2(half, 26.0),
                                            Button::selectable(state.word == *word, word),
                                        )
                                        .clicked()
                                    {
                                        state.word.clone_from(word);
                                        state.phones.clone_from(phones);
                                        state.xsampa=dictionary.xsampa_entries.contains(word);
                                    }
                                    if ui.add_sized(vec2(phone_width,26.0),Label::new(phones).sense(Sense::click())).clicked() {
                                        state.word.clone_from(word);
                                        state.phones.clone_from(phones);
                                        state.xsampa=dictionary.xsampa_entries.contains(word);
                                    }
                                    ui.weak(if dictionary.xsampa_entries.contains(word){"X-SAMPA"}else{""});
                                });
                            }
                        }
                    });
            });
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, state));
}
