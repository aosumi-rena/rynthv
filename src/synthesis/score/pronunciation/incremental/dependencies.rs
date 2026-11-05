use crate::{model::Language,synthesis::{plan::{NoteInput,OwnerInput},score::LyricFrontend,voice::{PreparedTree,Result}}};
use std::ops::Range;
use super::super::{special,automatic,split_syllables,dictionary_entry};

fn linked(left:&NoteInput,right:&NoteInput,language:Language,next:Language)->bool {
    language==next && left.onset.0.checked_add(left.duration)==Some(right.onset.0)
}
pub(super) fn units(notes:&[&NoteInput],joined:&[NoteInput],languages:&[Language],frontend:&LyricFrontend,
    tree:&PreparedTree,input:&OwnerInput,european:bool)->Result<Vec<Range<usize>>> {
    let mut units:Vec<Range<usize>>=Vec::new();
    for index in 0..notes.len() {
        if index>0 && linked(notes[index-1],notes[index],languages[index-1],languages[index])
            && special::marker(&joined[index].lyrics).is_some() {
            units.last_mut().unwrap().end=index+1;
        } else {units.push(index..index+1)}
    }
    let mut index=0;
    while index<units.len() {
        let reattack=joined[units[index].clone()].iter().any(|note|
            note.lyrics.trim().starts_with(['\'','’']) && special::marker(&note.lyrics)==Some(special::Marker::Sustain));
        if reattack {
            while index>0 {
                let first=units[index].start;
                if !linked(notes[first-1],notes[first],languages[first-1],languages[first])
                    || tail_boundary(&joined[first],languages[first],frontend,tree,input,european)? {break}
                let end=units.remove(index).end;
                index-=1;units[index].end=end;
            }
        }
        index+=1;
    }
    Ok(units)
}

fn tail_boundary(note:&NoteInput,language:Language,frontend:&LyricFrontend,tree:&PreparedTree,input:&OwnerInput,european:bool)->Result<bool> {
    if note.lyrics.trim().starts_with(['\'','’']) && special::spelling(&note.lyrics).is_empty() {return Ok(false)}
    let direct=special::direct(&note.lyrics);
    let result=if let Some(phones)=direct {split_syllables(phones,language,tree,frontend,
        note.attributes.get("phoneset").and_then(|v|v.as_str())==Some("xsampa"))}
        else {automatic(note,language,frontend,european,tree).and_then(|parts|parts.into_iter().map(|part|
            part.iter().map(|token|frontend.model_token(language,token,tree)).collect()).collect())};
    let result=if let Some((phones,xsampa))=dictionary_entry(note,language,input.voice.dictionary.as_ref()) {
        split_syllables(phones,language,tree,frontend,xsampa)
    }else{result};
    let Ok(parts)=result else {return Ok(true)};
    for token in parts.iter().flatten() {
        let phone=frontend.resolve_token(language,token,tree)?;
        if phone.used_silence_fallback || matches!(phone.source_class,Some("vowel"|"diphthong")) {return Ok(true)}
    }
    Ok(false)
}

pub(super) fn contextual(unit:&Range<usize>,joined:&[NoteInput],languages:&[Language],frontend:&LyricFrontend,tree:&PreparedTree,european:bool)->bool {
    if !matches!(languages[unit.start],Language::French|Language::Portuguese) {return true}
    joined[unit.clone()].iter().any(|note|special::marker(&note.lyrics).is_none() && special::direct(&note.lyrics).is_none()
        && automatic(note,languages[unit.start],frontend,european,tree).is_ok())
}

pub(super) fn context(index:usize,units:&[Range<usize>],notes:&[&NoteInput],languages:&[Language],eligible:&[bool],relaxed:bool)->Range<usize> {
    let unit=&units[index];let language=languages[unit.start];
    let radius=if eligible[index] {match language {Language::English if relaxed=>1,Language::French=>1,Language::Portuguese=>2,_=>0}}else{0};
    let mut left=index;let mut right=index;
    for _ in 0..radius {
        while left>0 {
            let at=units[left].start;
            if !linked(notes[at-1],notes[at],languages[at-1],language) {break}
            left-=1;if eligible[left] {break}
        }
        while right+1<units.len() {
            let at=units[right].end;
            if !linked(notes[at-1],notes[at],language,languages[at]) {break}
            right+=1;if eligible[right] {break}
        }
    }
    units[left].start..units[right].end
}
