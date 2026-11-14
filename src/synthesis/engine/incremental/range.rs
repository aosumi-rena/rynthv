mod input;
mod records;

use crate::{
    model::QUARTER,
    synthesis::{
        plan::{GlobalBlick, OwnerInput},
        score::PreparedPhrase,
        voice::Result,
    },
};
use records::RecordLayout;
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct IncrementalEdit {
    pub previous_records: Range<usize>,
    pub work_records: Range<usize>,
    pub write_frames: Range<usize>,
    pub first_block: usize,
    pub stop_block: usize,
}

impl IncrementalEdit {

    pub fn between(
        old: &OwnerInput,
        new: &OwnerInput,
        previous: &PreparedPhrase,
        phrase: &PreparedPhrase,
    ) -> Result<Option<Self>> {
        if new.system_pitch_delta.is_some() {
            return Ok(None);
        }
        let Some(changed) = input::changed_notes(old, new) else {
            return Ok(None);
        };

        if previous.notes.iter().zip(&previous.source_note_ids)
            .chain(phrase.notes.iter().zip(&phrase.source_note_ids))
            .any(|(note,source)|source.is_some() && note.kind!=0)
            || !previous.note_errors.is_empty() || !phrase.note_errors.is_empty() {
            return Ok(None);
        }
        let notes = &new.notes;
        let start = notes[changed.start].onset.0;
        let end = notes[changed.end - 1]
            .onset
            .0
            .saturating_add(notes[changed.end - 1].duration);

        let mut first = changed.start.saturating_sub(1);
        let mut left_time = start;
        while first > 0 && left_time >= start.saturating_sub(2 * QUARTER) {
            first -= 1;
            left_time = notes[first].onset.0;
        }
        let mut last = changed.end - 1;
        let mut right_time = end;
        while last + 1 < notes.len() && right_time <= end.saturating_add(2 * QUARTER) {
            last += 1;
            right_time = notes[last].onset.0.saturating_add(notes[last].duration);
        }
        let core_begin = notes[first].onset.0;
        let core_end = notes[last].onset.0.saturating_add(notes[last].duration);

        let mut input_first = first;
        while input_first > 0 {
            input_first -= 1;
            let note = &notes[input_first];
            if !matches!(note.lyrics.as_str(), "-" | "+")
                && note.onset.0.saturating_add(note.duration)
                    < core_begin.saturating_sub(2 * QUARTER)
            {
                break;
            }
        }
        let mut input_last = last;
        while input_last + 1 < notes.len() {
            input_last += 1;
            let note = &notes[input_last];
            if !matches!(note.lyrics.as_str(), "-" | "+")
                && note.onset.0 > core_end.saturating_add(2 * QUARTER)
            {
                break;
            }
        }
        let previous_layout = RecordLayout::new(previous, &old.notes)?;
        let layout = RecordLayout::new(phrase, notes)?;
        let previous_records = previous_layout.records(input_first..input_last + 1);
        let work_records = layout.records(input_first..input_last + 1);
        let write_frames = layout.frames(&layout.records(first..last + 1));

        let begin_seconds=phrase.start_seconds+write_frames.start as f64*crate::synthesis::score::FRAME_SECONDS;
        let end_seconds=phrase.start_seconds+write_frames.end as f64*crate::synthesis::score::FRAME_SECONDS;
        for source in previous.phoneme_controls.keys().chain(phrase.phoneme_controls.keys()) {
            let old=previous.phoneme_controls.get(source).and_then(|v|v.onset_seconds);
            let new=phrase.phoneme_controls.get(source).and_then(|v|v.onset_seconds);
            if old!=new && old.into_iter().chain(new).any(|seconds|seconds<begin_seconds || seconds>=end_seconds) {
                return Ok(None);
            }
        }

        if previous.start_seconds != phrase.start_seconds
            || previous.visible_begin_seconds != phrase.visible_begin_seconds
            || previous.visible_end_seconds != phrase.visible_end_seconds
            || previous_layout.frames(&previous_records) != layout.frames(&work_records)
            || previous_layout.total_frames() != layout.total_frames()
        {
            return Ok(None);
        }
        let seconds = |blick: i64| {
            new.tempo
                .seconds_at(GlobalBlick(blick.saturating_add(new.content_offset.0)))
        };
        let first_block = (seconds(core_begin).max(0.0) * 2.0).floor() as usize;
        let stop_block = (seconds(core_end).max(0.0) * 2.0).ceil() as usize;
        if work_records.is_empty() || write_frames.is_empty() || first_block >= stop_block {
            return Ok(None);
        }
        Ok(Some(Self {
            previous_records,
            work_records,
            write_frames,
            first_block,
            stop_block,
        }))
    }
}
