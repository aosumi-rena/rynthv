use crate::model::{Note, QUARTER};

/// Extend selections into neighboring gaps without crossing other notes.
pub(super) fn selected(mut notes: Vec<(Note, bool)>) -> Vec<(i64, i64)> {
    notes.sort_by_key(|(n, _)| n.onset);
    let midpoint = |a: i64, b: i64| ((i128::from(a) + i128::from(b)) / 2) as i64;
    let mut ranges: Vec<(i64, i64)> = Vec::new();
    for (index, (note, chosen)) in notes.iter().enumerate() {
        if !chosen {
            continue;
        }
        let end = note.onset.saturating_add(note.duration);
        let mut left = note.onset;
        let mut right = end;
        if index == 0 {
            left = left.saturating_sub(QUARTER / 2);
        } else {
            let previous = &notes[index - 1].0;
            let previous_end = previous.onset.saturating_add(previous.duration);
            if previous_end < left {
                left = left
                    .saturating_sub(QUARTER / 2)
                    .max(midpoint(previous_end, left));
            }
        }
        if let Some((next, _)) = notes.get(index + 1) {
            if end < next.onset {
                right = end
                    .saturating_add(QUARTER / 2)
                    .min(midpoint(end, next.onset));
            }
        } else {
            right = end.saturating_add(QUARTER / 2);
        }
        if let Some(last) = ranges.last_mut()
            && left <= last.1
        {
            last.1 = last.1.max(right);
        } else {
            ranges.push((left, right));
        }
    }
    let mut protected: Vec<(i64, i64)> = Vec::new();
    for (note, chosen) in &notes {
        if *chosen {
            continue;
        }
        let end = note.onset.saturating_add(note.duration);
        if let Some(last) = protected.last_mut()
            && note.onset <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            protected.push((note.onset, end));
        }
    }
    let mut result = Vec::new();
    let mut index = 0;
    for (mut begin, end) in ranges {
        while index < protected.len() && protected[index].1 <= begin {
            index += 1;
        }
        let mut current = index;
        while current < protected.len() && protected[current].0 < end {
            let (left, right) = protected[current];
            if begin < left {
                result.push((begin, left));
            }
            begin = begin.max(right);
            if begin >= end {
                break;
            }
            current += 1;
        }
        if begin < end {
            result.push((begin, end));
        }
    }
    result
}
