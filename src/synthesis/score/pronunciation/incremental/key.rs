use crate::{model::Language,synthesis::{plan::{NoteInput,OwnerInput},voice::{VoiceFamily,Result}}};
#[derive(Clone,Copy,PartialEq)]
pub(super) struct Settings {pub relaxed:bool,pub european:bool}
impl Settings {
    pub fn new(input:&OwnerInput,inherited:Language,family:VoiceFamily)->Self {
        let setting=|key,default|match input.voice.instance_options.get("voice").and_then(|v|v.get(key)).and_then(|v|v.as_str()) {
            Some("true")=>true,Some("false")=>false,_=>default,
        };
        Self {relaxed:family==VoiceFamily::Sv2 && setting("relaxedPronunciation",inherited==Language::English),
            european:setting("europeanPronunciation",false)}
    }
}
#[derive(PartialEq)]
pub(super) struct Key {
    notes:Vec<Note>,links:Vec<bool>,settings:Settings,inherited:Language,start:usize,len:usize,
}
#[derive(PartialEq)]
struct Note {
    id:u64,lyrics:String,phonemes:String,language:std::result::Result<Language,String>,
    xsampa:bool,even:bool,dictionary:Option<(String,bool)>,duration:Option<i64>,
}
impl Key {
    pub fn new(notes:&[&NoteInput],joined:&[NoteInput],languages:&[Result<Language>],input:&OwnerInput,
        inherited:Language,settings:Settings,start:usize,len:usize)->Self {
        let has_english=languages.iter().any(|l|l.as_ref().is_ok_and(|l|*l==Language::English));
        let has_spanish=languages.iter().any(|l|l.as_ref().is_ok_and(|l|*l==Language::Spanish));
        let settings=Settings{relaxed:settings.relaxed&&has_english,european:settings.european&&has_spanish};
        let rows=notes.iter().zip(joined).zip(languages).map(|((note,joined),language)| {
            let lang=language.as_ref().copied().map_err(ToString::to_string);
            let dictionary=input.voice.dictionary.as_ref().filter(|dict|Some(dict.language)==lang.as_ref().ok().copied())
                .and_then(|dict|dict.entries.get(joined.lyrics.trim()).map(|phones|
                    (phones.clone(),dict.xsampa_entries.contains(joined.lyrics.trim()))));
            Note {id:note.id,lyrics:note.lyrics.clone(),phonemes:note.phonemes.clone(),language:lang,
                xsampa:note.attributes.get("phoneset").and_then(|v|v.as_str())==Some("xsampa"),
                even:note.attributes.get("evenSyllableDuration").and_then(|v|v.as_bool())!=Some(false),
                dictionary,duration:(settings.relaxed && language.as_ref().is_ok_and(|l|*l==Language::English)).then_some(note.duration)}
        }).collect();
        let links=notes.windows(2).map(|pair|pair[0].onset.0.checked_add(pair[0].duration)==Some(pair[1].onset.0)).collect();
        Self {notes:rows,links,settings,inherited,start,len}
    }
}
