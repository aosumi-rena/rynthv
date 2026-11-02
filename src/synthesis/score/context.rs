use super::{ExplicitScore, FRAME_SECONDS, SAMPLE_RATE, ScoreNote, ScorePhone};
use crate::synthesis::voice::VoiceFamily;

const CONTEXT_SECONDS: f64 = 0.5;
const ALIGNMENT_FRAMES: f64 = 8.0;

pub(super) struct ContextNotes {
    pub start_seconds: f64,
    pub notes: Vec<ScoreNote>,
    pub has_alignment_prefix: bool,
}

pub(super) fn padded(score: &ExplicitScore, family: VoiceFamily, default_language: &str) -> ContextNotes {
    match family {
        VoiceFamily::Sv1 => padded_sv1(score),
        VoiceFamily::Sv2Compatible | VoiceFamily::Sv2 => {
            let mut context = padded_sv2(score);

            if family == VoiceFamily::Sv2 {
                let trailing = context.notes.len().checked_sub(1);
                let leading_count = 1 + usize::from(context.has_alignment_prefix);
                for (index, note) in context.notes.iter_mut().enumerate() {
                    if index < leading_count || Some(index) == trailing {
                        note.phones[0].language = default_language.to_owned();
                    }
                }
            }
            context
        },
    }
}

fn padded_sv1(score: &ExplicitScore) -> ContextNotes {
    let Some(first) = score.notes.first() else {
        return ContextNotes {
            start_seconds: score.start_seconds,
            notes: Vec::new(),
            has_alignment_prefix: false,
        };
    };
    let last = score.notes.last().expect("nonempty score");
    let mut leading = silence(first, CONTEXT_SECONDS);
    let mut trailing = silence(last, CONTEXT_SECONDS);
    leading.flag = true;
    trailing.flag = true;
    let mut notes = Vec::with_capacity(score.notes.len() + 2);
    notes.push(leading);
    notes.extend(score.notes.iter().cloned());
    notes.push(trailing);
    ContextNotes {
        start_seconds: ((score.start_seconds - CONTEXT_SECONDS) * f64::from(SAMPLE_RATE)).round()
            / f64::from(SAMPLE_RATE),
        notes,
        has_alignment_prefix: false,
    }
}

fn padded_sv2(score: &ExplicitScore) -> ContextNotes {
    let Some(first) = score.notes.first() else {
        return ContextNotes {
            start_seconds: score.start_seconds,
            notes: Vec::new(),
            has_alignment_prefix: false,
        };
    };
    let last = score.notes.last().expect("nonempty score");
    let origin = score.start_seconds - CONTEXT_SECONDS;
    let aligned =
        (origin / FRAME_SECONDS / ALIGNMENT_FRAMES).floor() * FRAME_SECONDS * ALIGNMENT_FRAMES;
    let remainder = origin - aligned;
    let mut notes = Vec::with_capacity(score.notes.len() + 2);

    let mut leading = silence(first, CONTEXT_SECONDS + remainder);
    leading.performance.choir_pitch = first.performance.choir_pitch;
    notes.push(leading);
    notes.extend(score.notes.iter().cloned());
    let mut trailing = silence(last, CONTEXT_SECONDS);
    trailing.performance.choir_onset = last.performance.choir_tail;
    trailing.performance.choir_pitch = last.performance.choir_pitch;
    notes.push(trailing);
    ContextNotes {
        start_seconds: aligned,
        notes,
        has_alignment_prefix: false,
    }
}

fn silence(edge: &ScoreNote, duration_seconds: f64) -> ScoreNote {
    ScoreNote {
        source_note_id: None,
        note_error: None,
        retake_seeds: Default::default(),
        performance: Default::default(),
        duration_seconds,
        pitch: edge.pitch,
        phones: vec![ScorePhone {
            vocoder_token:None,
            xsampa:false,
            language: edge
                .phones
                .first()
                .map_or_else(String::new, |phone| phone.language.clone()),
            token: "sil".into(),
            display_token: None,
            onset_offset_seconds: None,
        }],
        kind: 1,
        flag: edge.flag,
        tag: None,
        parts: 1,
        part_index: 0,
    }
}
