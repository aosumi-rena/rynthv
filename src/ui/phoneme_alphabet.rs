use crate::{app::App,model::{Note,GroupRef,PronunciationDictionary,Language},synthesis::plan::NoteInput};

/// Match timing snapshots to the current pronunciation and language.
pub(super) fn matches(note:&Note,source:&NoteInput)->bool {
    note.id==source.id && note.onset==source.onset.0 && note.duration==source.duration
        && note.lyrics==source.lyrics && note.phonemes==source.phonemes
        && note.attributes.get("languageOverride")==source.attributes.get("languageOverride")
}

fn alphabet(lyrics:&str,phonemes:&str,attributes:&crate::model::Extra,
    dictionary:Option<&PronunciationDictionary>,language:Option<Language>)->bool {
    let note=attributes.get("phoneset").and_then(serde_json::Value::as_str)==Some("xsampa");
    if !phonemes.trim().is_empty(){return note;}
    dictionary.filter(|d|Some(d.language)==language && d.entries.contains_key(lyrics.trim()))
        .map_or(note,|d|d.xsampa_entries.contains(lyrics.trim()))
}

pub(super) fn label(app:&App,reference:&GroupRef,note:&Note,source:&NoteInput,
    source_dictionary:Option<&PronunciationDictionary>,token:&str)->Result<String,String> {
    if !super::phoneme_candidates::xsampa_enabled(app) {return Ok(token.to_owned())}
    if app.synthesis.resolve_voice(app.editor.current_track(),reference).ok().flatten()
        .and_then(|key|app.synthesis.library.entry(&key)).and_then(|entry|entry.family)
        .is_some_and(|family|family.is_standard()) {return Ok(token.to_owned());}
    let language=super::phoneme_candidates::language(app,note);
    let dictionary=app.editor.project.dictionary_for(app.editor.current_track(),reference);
    let xsampa=alphabet(&note.lyrics,&note.phonemes,&note.attributes,dictionary,language);
    let previous=alphabet(&source.lyrics,&source.phonemes,&source.attributes,source_dictionary,language);
    if previous==xsampa {return Ok(token.to_owned());}
    let language=language.ok_or_else(||"未设置音素语言".to_owned())?;
    let (path,format)=super::phoneme_candidates::resources(app)?;
    crate::model::PhonemeCatalog::cached(path,format)?.convert(language,token,xsampa)
}
