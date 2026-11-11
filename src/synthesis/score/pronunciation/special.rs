use super::{ResolvedNote, NoteInput};
use crate::synthesis::voice::{PreparedTree, Result};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Marker { Syllable, Sustain }

pub(super) fn spelling(lyrics: &str) -> &str {
    lyrics.trim().strip_prefix(['\'', '’']).unwrap_or(lyrics.trim())
}
pub(super) fn marker(lyrics: &str) -> Option<Marker> {
    match spelling(lyrics) {
        "+" | "＋" => Some(Marker::Syllable),
        "-" | "ー" => Some(Marker::Sustain),
        _ => None,
    }
}
pub(super) fn direct(lyrics: &str) -> Option<&str> {
    let word = spelling(lyrics);
    word.strip_prefix('.').or_else(|| {
        (matches!(word, "br" | "cl" | "vf") || numbered_breath(word)).then_some(word)
    })
}
pub(in crate::synthesis::score) fn numbered_breath(word:&str)->bool {
    ["brl", "br"].iter().any(|prefix| word.strip_prefix(prefix)
        .and_then(|tail| tail.as_bytes().first()).is_some_and(|b| matches!(b, b'1'..=b'9')))
}
fn fail(row: &mut ResolvedNote, message: impl Into<String>) {
    row.error = Some(message.into());
    row.kind = 1;
    row.syllables = vec![vec!["sil".into()]];
}
fn adjacent(notes: &[&NoteInput], rows: &[ResolvedNote], left: usize, right: usize) -> bool {
    notes[left].onset.0.checked_add(notes[left].duration) == Some(notes[right].onset.0)
        && rows[left].language == rows[right].language
}

pub(super) fn distribute(notes: &[&NoteInput], rows: &mut [ResolvedNote], tree: &PreparedTree, frontend: &super::LyricFrontend, default_language: crate::model::Language) -> Result<()> {

    let mut pending = std::collections::VecDeque::new();
    let mut previous_tail: Option<Vec<String>> = None;
    let mut previous_tail_alphabet=None;
    let mut previous_tail_owner:Option<usize>=None;
    for index in 0..rows.len() {
        let linked = index > 0 && adjacent(notes, rows, index-1, index);
        if !linked { pending.clear(); previous_tail = None; previous_tail_owner=None;previous_tail_alphabet=None; }
        let reattack = notes[index].lyrics.trim().starts_with(['\'', '’']);
        match marker(&notes[index].lyrics) {
            Some(Marker::Syllable) => {
                if let Some((syllable,alphabet)) = pending.pop_front() {
                    rows[index].syllables = vec![syllable];
                    rows[index].dictionary_alphabet=alphabet;
                    if let Some(xsampa)=alphabet {rows[index].xsampa=xsampa;}
                } else if reattack {
                    rows[index].syllables=vec![Vec::new()];rows[index].kind=0;rows[index].error=None;
                } else { fail(&mut rows[index], "前面的歌词没有可继续分配的音节"); }
            }
            Some(Marker::Sustain) => {
                if !linked {
                    fail(&mut rows[index],"延音前缺少相连的音符");

                    rows[index].placeholder_language=Some(default_language);
                }
                else {

                    if reattack && let (Some(owner),Some(tail))=(previous_tail_owner,previous_tail.as_ref()) {
                        if let Some(last)=rows[owner].syllables.last_mut() {
                            last.truncate(last.len().saturating_sub(tail.len().saturating_sub(1)));
                        }
                    }
                    rows[index].syllables = if reattack {
                        vec![previous_tail.clone().unwrap_or_else(||vec!["a".into()])]
                    } else {vec![Vec::new()]};
                    rows[index].kind = if reattack {0} else {2};
                    if reattack {
                        rows[index].dictionary_alphabet=previous_tail_alphabet;
                        if let Some(xsampa)=previous_tail_alphabet {rows[index].xsampa=xsampa;}
                    }
                    rows[index].error = None;
                }
            }
            None => {
                pending.clear();
                if reattack && spelling(&notes[index].lyrics).is_empty() {
                    rows[index].syllables=vec![Vec::new()];rows[index].kind=0;rows[index].error=None;
                }
                let mut end = index+1;
                while end < rows.len() && adjacent(notes, rows, end-1, end)
                    && marker(&notes[end].lyrics).is_some() { end+=1; }
                let pluses = (index+1..end).filter(|&i| marker(&notes[i].lyrics)==Some(Marker::Syllable)).count();
                if pluses > 0 && rows[index].syllables.len()>1 {
                    let remaining = rows[index].syllables.split_off(1);
                    pending.extend(remaining.into_iter().map(|syllable|(syllable,rows[index].dictionary_alphabet)));
                }
            }
        }
        if reattack && rows[index].kind == 0 {
            if let Some(first) = rows[index].syllables.first_mut() { first.insert(0,"cl".into()); }
        }
        if rows[index].kind != 2 && rows[index].error.is_none() {
            validate(&mut rows[index],tree,frontend)?;
            if rows[index].error.is_some() {previous_tail=None;previous_tail_owner=None;previous_tail_alphabet=None;pending.clear();}
            else {
                let phones:Vec<_>=rows[index].syllables.iter().flatten().cloned().collect();
                for (i,token) in phones.iter().enumerate() {
                    if matches!(frontend.phone_class(rows[index].language,token,tree)?,Some("vowel"|"diphthong")) {
                        previous_tail=Some(phones[i..].to_vec());
                        previous_tail_alphabet=rows[index].dictionary_alphabet;
                        previous_tail_owner=Some(index);
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate(row:&mut ResolvedNote,tree:&PreparedTree,frontend:&super::LyricFrontend)->Result<()> {
    if row.kind==2 || row.error.is_some() {return Ok(());}
    for token in row.syllables.iter().flatten() {
        if frontend.converting() {frontend.phone_class(row.language,token,tree)?;continue;}
        if frontend.resolve_token(row.language,token,tree)?.used_silence_fallback {
            if frontend.unmapped_phone_class(row.language,token)?.is_some() {

                continue;
            }

            if row.syllables.len()==1 && row.syllables[0].len()==1 && numbered_breath(token) {
                row.kind=1;return Ok(());
            }
            let error=format!("不支持的音素：{token}");fail(row,error);return Ok(());
        }
    }
    if row.syllables.iter().all(Vec::is_empty) {fail(row,"歌词没有可合成的音素");}
    Ok(())
}
