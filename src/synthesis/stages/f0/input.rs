use crate::synthesis::voice::{Result, VoiceError};
use super::features::FeaturePhones;

use super::{
    F0Stage, TimedPhone,
    frontend::{FrontendNote, NoteFeatures},
};

pub(super) struct PitchInput {
    pub notes: Vec<FrontendNote>,
    pub note_features: NoteFeatures,
    pub normalized_notes: Vec<f32>,
    pub fixed_style: Vec<f32>,
    pub phone_features: FeaturePhones,
    pub phone_to_note: Vec<usize>,
    pub phone_frames: Vec<usize>,
    pub tokens: Vec<String>,
    pub vowel_mask: Vec<bool>,
}

impl F0Stage<'_> {

    pub(super) fn prepare_input(
        &self,
        notes: &[FrontendNote],
        phones: &[TimedPhone],
        transpose: &[f32],
    ) -> Result<PitchInput> {
        if notes.len() != transpose.len() || transpose.iter().any(|v| !v.is_finite()) {
            return Err(VoiceError::new(
                "F0 saved transpositions differ from note records",
            ));
        }
        let mut notes = notes.to_vec();
        for (note, &transpose) in notes.iter_mut().zip(transpose) {
            note.pitch_offset += transpose;
        }
        let note_features = self.frontend.encode(&notes)?;
        let frames = note_features.frame_positions.len();
        let mut normalized_notes = vec![0.0; note_features.values.len()];
        self.note_normalizer
            .normalize(&note_features.values, &mut normalized_notes, true)
            .map_err(|e| VoiceError::new(e.to_string()))?;
        let mut fixed_style = Vec::with_capacity(notes.len() * self.fixed_style.len());
        for _ in &notes {
            fixed_style.extend_from_slice(&self.fixed_style);
        }
        let phone_features=self.features.prepare_phones(phones)?;
        let mut phone_to_note = Vec::with_capacity(phones.len());
        let mut phone_frames = Vec::with_capacity(phones.len());
        let tokens = phone_features.noise_tokens(phones)?;
        let mut vowel_mask = Vec::with_capacity(frames);
        let mut last_note = 0;
        let mut per_note_frames = vec![0_usize; notes.len()];
        for (phone,&is_vowel) in phones.iter().zip(&phone_features.vowels) {
            if phone.note_index >= notes.len() || phone.note_index < last_note {
                return Err(VoiceError::new(
                    "F0 phones are not ordered by their prepared note records",
                ));
            }
            last_note = phone.note_index;
            per_note_frames[phone.note_index] = per_note_frames[phone.note_index]
                .checked_add(phone.frames)
                .ok_or_else(|| VoiceError::new("F0 phone duration sum overflow"))?;
            vowel_mask.extend(std::iter::repeat_n(is_vowel, phone.frames));
            phone_to_note.push(phone.note_index);
            phone_frames.push(phone.frames);
        }

        if vowel_mask.len() != frames {
            return Err(VoiceError::new(
                "F0 corrected phone frames do not cover their notes",
            ));
        }
        Ok(PitchInput {
            notes,
            note_features,
            normalized_notes,
            fixed_style,
            phone_features,
            phone_to_note,
            phone_frames,
            tokens,
            vowel_mask,
        })
    }
}
