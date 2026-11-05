use super::{Error, NoteFailure, Phrase, Result};
use std::collections::VecDeque;

pub(super) enum Decoded {
    Word(Vec<Vec<String>>),
    Hold,
    Plus,
    Rest,
    Failure(String),
}
pub(super) struct Row {
    pub phones: Vec<String>,
    pub held: bool,
    pub silent: bool,
}
impl Row {
    fn rest() -> Self { Self { phones: vec!["sil".into()], held: false, silent: true } }
    fn word(phones: Vec<String>) -> Self { Self { phones, held: false, silent: false } }
}
pub(super) struct Assignment {
    pub rows: Vec<Row>,
    pub failures: Vec<NoteFailure>,
}

pub(super) fn assign(phrase: &Phrase, decoded: Vec<Decoded>) -> Result<Assignment> {
    if decoded.len() != phrase.notes.len() { return Err(Error::Format("lyric assignment extent")); }
    let mut rows: Vec<_> = (0..decoded.len()).map(|_| Row::rest()).collect();
    let mut messages: Vec<Option<String>> = vec![None; decoded.len()];
    let mut pending = VecDeque::new();
    let mut owner = None;
    let mut initialized = false;
    let mut last_emitted = None;
    let mut last_consumer = None;
    let mut inactive_plus_tail = false;
    for (i, value) in decoded.into_iter().enumerate() {
        let contiguous = i > 0 && phrase.notes[i].local_tick ==
            phrase.notes[i-1].local_tick + phrase.notes[i-1].duration_ticks;
        if !contiguous {
            flush(&mut owner, &mut pending, &mut rows, &mut last_emitted, false)?;
            pending.clear(); initialized = false; last_emitted = None;
            inactive_plus_tail = false; last_consumer = None;
        }
        match value {
            Decoded::Failure(message) => {
                flush(&mut owner, &mut pending, &mut rows, &mut last_emitted, false)?;
                messages[i] = Some(message); pending.clear(); initialized = false;
                last_emitted = None; last_consumer = None; inactive_plus_tail = false;
            }
            Decoded::Rest => {
                flush(&mut owner, &mut pending, &mut rows, &mut last_emitted, false)?;
                pending.clear(); initialized = false; last_emitted = None;
                last_consumer = None; inactive_plus_tail = false;
            }
            Decoded::Word(units) => {
                flush(&mut owner, &mut pending, &mut rows, &mut last_emitted, false)?;
                if !pending.is_empty() && !inactive_plus_tail {
                    fail_tail(last_consumer, &mut rows, &mut messages);
                }
                pending = units.into(); owner = Some(i); initialized = true;
                last_consumer = None; inactive_plus_tail = false;
            }
            Decoded::Plus => {
                if !initialized {
                    messages[i] = Some("No syllable is available for +".into());
                    last_emitted = None; continue;
                }
                flush(&mut owner, &mut pending, &mut rows, &mut last_emitted, true)?;
                if let Some(unit) = pending.pop_front() {
                    rows[i] = Row::word(unit); last_emitted = Some(i);
                    inactive_plus_tail = !pending.is_empty(); last_consumer = Some(i);
                } else {
                    initialized = false; last_emitted = None;
                    inactive_plus_tail = false; last_consumer = None;
                }
            }
            Decoded::Hold => {
                if owner.is_some() || phrase.notes[i].lyric != "-" { inactive_plus_tail = false; }
                flush(&mut owner, &mut pending, &mut rows, &mut last_emitted, true)?;
                if contiguous && last_emitted.is_some() {
                    rows[i] = Row { phones: Vec::new(), held: true, silent: false };
                    if !pending.is_empty() { last_consumer = Some(i); }
                } else {
                    messages[i] = Some("No preceding syllable is available for -".into());
                    pending.clear(); initialized = false; last_emitted = None;
                    inactive_plus_tail = false; last_consumer = None;
                }
            }
        }
    }
    flush(&mut owner, &mut pending, &mut rows, &mut last_emitted, false)?;
    if !pending.is_empty() { fail_tail(last_consumer, &mut rows, &mut messages); }
    let failures = messages.into_iter().enumerate().filter_map(|(note_index, message)|
        message.map(|message| NoteFailure { note_index, message })).collect();
    Ok(Assignment { rows, failures })
}

fn flush(owner: &mut Option<usize>, pending: &mut VecDeque<Vec<String>>, rows: &mut [Row],
    last: &mut Option<usize>, first_only: bool) -> Result<()> {
    if let Some(index) = owner.take() {
        if pending.is_empty() { return Err(Error::Format("empty owned pending word")); }
        let phones = if first_only { pending.pop_front().unwrap() }
            else { pending.drain(..).flatten().collect() };
        rows[index] = Row::word(phones); *last = Some(index);
    }
    Ok(())
}
fn fail_tail(index: Option<usize>, rows: &mut [Row], messages: &mut [Option<String>]) {
    if let Some(index) = index {
        rows[index] = Row::rest();
        messages[index] = Some("Word syllables remain after the last + or -".into());
    }
}
