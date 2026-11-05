//! STD source selection and offline synthesis run per real phrase, not per song.
//! Section identity is the first source note, so edits keep other phrases cached.
use super::{GlobalBlick, OwnerInput};

pub(super) fn sections(input: &OwnerInput) -> Vec<(Option<u64>, OwnerInput)> {
    let seconds = |local: i64| input.tempo.seconds_at(GlobalBlick(
        local.saturating_add(input.content_offset.0)));
    let active: Vec<_> = input.notes.iter().enumerate().filter(|(_, note)|
        note.attributes.get("muted").and_then(serde_json::Value::as_bool) != Some(true)).collect();
    let mut ranges = Vec::new();
    let mut first = 0;
    let mut covered_end = active.first().map_or(0, |(_, note)|
        note.onset.0.saturating_add(note.duration));
    for i in 1..active.len() {
        let (_, next) = active[i];
        // Never sever a pending word/hold chain, including the kana hold alias.
        let continuation = next.phonemes.trim().is_empty()
            && matches!(next.lyrics.trim(), "+" | "-" | "ー" | "!con");
        // Both native adapters retain a half-second source preroll. Requiring
        // two such margins keeps the shared boundary outside either phrase's
        // singing phones; shorter rests stay inside the same source query.
        if !continuation && seconds(next.onset.0) - seconds(covered_end) >= 1.0 {
            ranges.push((first..i, covered_end));
            first = i;
        }
        covered_end = covered_end.max(next.onset.0.saturating_add(next.duration));
    }
    if ranges.is_empty() { return vec![(None, input.clone())]; }
    ranges.push((first..active.len(), covered_end));
    let (publication_begin, publication_end) = input.publication_bounds();
    let mut boundaries = Vec::with_capacity(ranges.len() - 1);
    for pair in ranges.windows(2) {
        let previous_end = seconds(pair[0].1);
        let next_start = seconds(active[pair[1].0.start].1.onset.0);
        // Divide only the actual rest. At large gaps retain at least half a
        // second of source preroll/tail on each side, without synthesizing the
        // intervening silence. Round the shared boundary once for both owners.
        let boundary = (previous_end + next_start) * 0.5;
        boundaries.push(input.tempo.position_at(boundary));
    }
    ranges.iter().enumerate().map(|(index, (range, _))| {
        let (start, first_note) = active[range.start];
        let (end, _) = active[range.end - 1];
        let mut section = input.clone();
        section.notes = input.notes[start..=end].to_vec();
        let begin = if index == 0 { publication_begin } else {
            GlobalBlick(publication_begin.0.max(boundaries[index - 1].0))
        };
        let end = boundaries.get(index).map_or(publication_end, |boundary|
            Some(GlobalBlick(publication_end.map_or(boundary.0, |end| end.0.min(boundary.0)))));
        section.section_bounds = Some((begin, end));
        (Some(first_note.id), section)
    }).collect()
}
