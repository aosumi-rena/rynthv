use crate::{app::App, model::{Language, Note, PhonemeCatalog, database_language}};
use eframe::egui::{self, *};
use std::sync::Arc;

pub(super) fn catalog(app: &App, ctx: &Context) -> Result<Arc<PhonemeCatalog>, String> {
    let path = app.synthesis.dictionaries().ok_or("请设置 2.3 发音词典目录")?;
    let stamp = std::fs::metadata(path.join("phoneme-table.tsv")).ok()
        .and_then(|m|m.modified().ok());
    let id = Id::new(("phoneme-table", path, stamp));
    ctx.data_mut(|data| {
        if let Some(value) = data.get_temp::<Result<Arc<PhonemeCatalog>, String>>(id) { return value; }
        let value = PhonemeCatalog::load(path).map(Arc::new);
        data.insert_temp(id, value.clone());
        value
    })
}

pub(super) fn inherited_language(app: &App) -> Option<Language> {
    let track = &app.editor.current_track().main_ref;
    let target=app.editor.active_ref().unwrap_or(track);
    let native=app.voice_inventory().and_then(|voice|voice.configuration.as_ref()).and_then(|config|Language::parse(&config.default_language))
        .or_else(||target.database.get("language").and_then(serde_json::Value::as_str).and_then(Language::parse))
        .or_else(||track.database.get("language").and_then(serde_json::Value::as_str).and_then(Language::parse))?;
    let inherited = database_language(&track.database, native).ok()?;
    let inherited = database_language(&app.editor.active_ref().unwrap_or(track).database, inherited).ok()?;
    Some(inherited)
}

pub(super) fn language(app: &App, note: &Note) -> Option<Language> {
    match note.attributes.get("languageOverride").and_then(serde_json::Value::as_str).filter(|s|!s.is_empty()) {
        Some(value)=>Language::parse(value),None=>inherited_language(app),
    }
}

#[derive(Clone, Default)]
struct Selection { query: String, start: usize, index: usize, dismissed: bool }

#[derive(Default)]
pub(super) struct Outcome { pub handled: bool, pub changed: bool }

/// Replace only the token under the caret; all other pronunciation tokens survive.
pub(super) fn show(app: &App, ui: &mut Ui, output: &egui::text_edit::TextEditOutput,
    text: &mut String, language: Option<Language>, xsampa: bool) -> Outcome {
    let mut result = Outcome::default();
    let Some(language) = language else { return result; };
    if !output.response.has_focus() && !output.response.lost_focus() { return result; }
    let Ok(catalog) = catalog(app, ui.ctx()) else { return result; };
    let caret = output.cursor_range.map_or(text.chars().count(), |range|range.primary.index.into());
    let byte = text.char_indices().nth(caret).map_or(text.len(), |(i,_)|i);
    let start = text[..byte].char_indices().rev().find(|(_,ch)|ch.is_whitespace()).map_or(0, |(i,ch)|i+ch.len_utf8());
    let end = text[byte..].find(char::is_whitespace).map_or(text.len(), |i|byte+i);
    let query = text[start..end].to_owned();
    if query.is_empty() { return result; }
    let symbol = |entry: &crate::model::PhonemeEntry| if xsampa {entry.xsampa.clone()} else {entry.symbol.clone()};
    let inventory=if xsampa {catalog.unified()} else {catalog.for_language(language).collect()};
    let mut entries: Vec<_> = inventory.into_iter().filter(|entry| {
        let value = symbol(entry);
        value.contains(&query) || near(&value, &query)
            || entry.example.replace('*', "").to_lowercase().contains(&query.to_lowercase())
    }).collect();
    entries.sort_by_key(|entry| {
        let value = symbol(entry);
        (if value == query {0} else if value.starts_with(&query) {1} else if value.contains(&query) {2} else {3}, value.len(), value.to_owned())
    });
    entries.truncate(8);
    if entries.is_empty() { return result; }
    let id = output.response.id.with("phone-candidates");
    let mut state = ui.ctx().data_mut(|data|data.get_temp::<Selection>(id).unwrap_or_default());
    if state.query != query || state.start != start { state = Selection {query, start, ..Default::default()}; }
    state.index = state.index.min(entries.len()-1);
    if ui.input_mut(|i|i.consume_key(Modifiers::NONE, Key::Escape)) && !state.dismissed {
        state.dismissed = true; result.handled = true; output.response.request_focus();
    }
    if state.dismissed {
        ui.ctx().data_mut(|data|data.insert_temp(id,state)); return result;
    }
    if ui.input_mut(|i|i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
        state.index = (state.index+1)%entries.len(); result.handled = true;
    }
    if ui.input_mut(|i|i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
        state.index = (state.index+entries.len()-1)%entries.len(); result.handled = true;
    }
    let accept = ui.input_mut(|i|i.consume_key(Modifiers::NONE, Key::Tab) || i.consume_key(Modifiers::NONE, Key::Enter));
    let mut chosen = accept.then_some(state.index);
    let height = entries.len() as f32*25.0+12.0;
    let screen=ui.ctx().content_rect();
    let top=output.response.rect.top().clamp(screen.top(),(screen.bottom()-height).max(screen.top()));
    Area::new(id).order(Order::Foreground)
        // Screen-edge constraints must not slide the popup back over the editor.
        .constrain(false)
        .fixed_pos(pos2(output.response.rect.right()+8.0,top))
        .show(ui.ctx(), |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(250.0);
                for (index, entry) in entries.iter().enumerate() {
                    let label = format!("{}   {}   {}", symbol(entry), category_label(entry), entry.example);
                    if ui.selectable_label(index == state.index, label).clicked() { chosen = Some(index); }
                }
            });
        });
    if let Some(index) = chosen {
        let value = symbol(entries[index]);
        text.replace_range(start..end, &value);
        let caret = text[..start].chars().count()+value.chars().count();
        if let Some(mut edit) = TextEdit::load_state(ui.ctx(),output.response.id) {
            edit.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(caret))));
            edit.store(ui.ctx(), output.response.id);
        }
        state.dismissed = true; result.handled = true; result.changed = true;
        output.response.request_focus();
    }
    ui.ctx().data_mut(|data|data.insert_temp(id,state));
    result
}

fn category_label(entry: &crate::model::PhonemeEntry) -> &str {
    let key = match entry.symbol.as_str() {
        "cl" => "Phoneme_Closure",
        "vf" => "Phoneme_Vocal Fry",
        "br" => "Phoneme_Breath",
        _ => match entry.category.as_str() {
            "vowel" => "Phoneme_Vowel",
            "semivowel" => "Phoneme_Semivowel",
            "diphthong" => "Phoneme_Diphthong",
            "stop" => "Phoneme_Stop",
            "fricative" => "Phoneme_Fricative",
            "affricate" => "Phoneme_Affricate",
            "nasal" => "Phoneme_Nasal",
            "liquid" => "Phoneme_Liquid",
            "coda" => "Phoneme_Coda",
            "silence" => "Phoneme_Silence",
            _ => return &entry.category,
        },
    };
    let translated = crate::i18n::t(key);
    translated.strip_prefix("Phoneme_").unwrap_or(translated)
}

fn near(left: &str, right: &str) -> bool {
    let a: Vec<_> = left.chars().collect(); let b: Vec<_> = right.chars().collect();
    if a.len().abs_diff(b.len()) > 1 { return false; }
    let (mut i,mut j,mut edits)=(0,0,0);
    while i<a.len() && j<b.len() {
        if a[i]==b[j] {i+=1;j+=1;continue;}
        edits+=1; if edits>1 {return false;}
        if a.len()>=b.len() {i+=1;} if b.len()>=a.len() {j+=1;}
    }
    edits+(a.len()-i)+(b.len()-j)<=1
}
