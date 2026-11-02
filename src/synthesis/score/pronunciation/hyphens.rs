use crate::{model::Language, synthesis::plan::NoteInput};

/// Join hyphenated word fragments for pronunciation lookup.
pub(super) fn join(notes: &[&NoteInput], languages: &[Language]) -> Vec<NoteInput> {
    let mut resolved: Vec<_> = notes.iter().map(|note| (*note).clone()).collect();
    let mut start = 0;
    while start < notes.len() {
        let language = languages[start];
        if !matches!(language, Language::English | Language::Spanish | Language::French
            | Language::German | Language::Portuguese | Language::Italian)
            || !fragment(&notes[start].lyrics) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < notes.len() && languages[end] == language
            && notes[end-1].onset.0.checked_add(notes[end-1].duration) == Some(notes[end].onset.0)
            && fragment(&notes[end].lyrics)
            && (notes[end-1].lyrics.trim().ends_with('-') || notes[end].lyrics.trim().starts_with('-')) {
            end += 1;
        }
        if end > start + 1 {
            resolved[start].lyrics = notes[start..end].iter()
                .map(|note| note.lyrics.trim().trim_matches('-')).collect::<String>();
            for note in &mut resolved[start+1..end] { note.lyrics = "+".into(); }
        }
        start = end;
    }
    resolved
}

fn fragment(lyrics: &str) -> bool {
    let word = lyrics.trim();
    !word.trim_matches('-').is_empty() && super::special::marker(word).is_none()
        && super::special::direct(word).is_none()
        && !word.chars().any(char::is_whitespace)
        && !word.starts_with(['\'', '’'])
}
