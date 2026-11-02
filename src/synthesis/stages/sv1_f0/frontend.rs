use super::{Contract, timing};

use crate::synthesis::{
    inference::{Sequence, ops::standard_normalizer::StandardNormalizer},
    stages::f0::{
        TimedPhone,
        frontend::{FrontendNote, duration_seconds},
    },
    voice::{PhoneSet, PreparedModule, PreparedTree, Result, VoiceError},
};

use crate::synthesis::voice::embedded::types::F0_FRONTEND_TYPE_ID;

pub(super) struct Frontend<'a> {
    languages: Vec<&'a PhoneSet>,
    unified: &'a PhoneSet,
    categories: bool,
    pub frame_seconds: f32,
    pub normalizer: &'a StandardNormalizer,
}

pub(super) struct Encoded {
    pub notes: Sequence,
    pub phone_ids: Vec<f32>,
    pub language_ids: Vec<f32>,
    pub vowel_mask: Vec<bool>,
}

impl<'a> Frontend<'a> {
    pub fn prepare(tree: &'a PreparedTree, index: usize) -> Result<Self> {
        let node = tree.node(index)?;
        let payload = tree.payload(index)?;
        if node.type_id != F0_FRONTEND_TYPE_ID || node.children.len() != 3 || payload.len() != 8 {
            return Err(VoiceError::new(
                "Expected the compatible ten-channel F0 frontend",
            ));
        }
        let categories = u32::from_le_bytes(payload[..4].try_into().unwrap());
        let frame_seconds = f32::from_le_bytes(payload[4..].try_into().unwrap());
        if categories > 1 || !frame_seconds.is_finite() || frame_seconds <= 0.0 {
            return Err(VoiceError::new(
                "Invalid compatible F0 frontend configuration",
            ));
        }
        let languages = tree
            .node(node.children[0])?
            .children
            .iter()
            .map(|&index| phone_set(tree, index))
            .collect::<Result<Vec<_>>>()?;
        let unified = phone_set(tree, node.children[1])?;
        let normalizer = tree.standard_normalizer(node.children[2])?;
        if languages.is_empty() || normalizer.channels() != 10 {
            return Err(VoiceError::new(
                "Compatible F0 frontend has invalid language or normalization dimensions",
            ));
        }
        Ok(Self {
            languages,
            unified,
            categories: categories != 0,
            frame_seconds,
            normalizer,
        })
    }

    pub fn encode(
        &self,
        notes: &[FrontendNote],
        phones: &[TimedPhone],
        phone_frames: &[usize],
        transpose: f32,
        contract: Contract,
    ) -> Result<Encoded> {
        if phone_frames.len() != phones.len() {
            return Err(VoiceError::new(
                "Compatible F0 phone timings differ from its phone records",
            ));
        }
        let mut rows = Sequence::zeros(notes.len(), 10)?;
        for (index, note) in notes.iter().enumerate() {
            let duration = match contract {
                Contract::Compatible => duration_seconds(note)?,
                Contract::Native { .. } => timing::duration_seconds(note)?,
            };
            let pitch = note.pitch + note.pitch_offset + transpose;
            let maximum_kind = match contract {
                Contract::Compatible => 2,
                Contract::Native { .. } => 1,
            };
            if !pitch.is_finite() || note.kind > maximum_kind {
                return Err(VoiceError::new(
                    "Invalid DDS F0 note kind or pitch",
                ));
            }
            if matches!(contract, Contract::Native { .. })
                && note.tag.as_ref().is_some_and(|tag| !tag.is_empty())
            {
                return Err(VoiceError::new(
                    "Native DDS F0 special-mode note tags are not supported",
                ));
            }
            let row = &mut rows.values[index * 10..(index + 1) * 10];
            let breath = note.phones.first().is_some_and(|phone| phone == "br")
                && match contract {

                    Contract::Compatible => true,
                    Contract::Native { .. } => note.phones.len() == 1,
                };
            row[0] = f32::from(note.kind == 1 || breath);

            row[1] = f32::from(note.kind == 2);
            row[2] = pitch;
            row[3] = (duration + 0.001).min(5.0).ln() as f32;
            let tones_enabled = match contract {
                Contract::Compatible => {

                    row[4] = f32::from(note.flag);
                    true
                }
                Contract::Native { .. } => {

                    false
                }
            };
            if tones_enabled
                && let Some(tag) = note.tag.as_deref()
                && let Some(tone) = ["1", "2", "3", "4", "5"]
                    .iter()
                    .position(|value| *value == tag)
            {
                row[5 + tone] = 1.0;
            }
        }
        let mut normalized = vec![0.0; rows.values.len()];
        self.normalizer
            .normalize(&rows.values, &mut normalized)
            .map_err(error)?;
        rows.values = normalized;
        let mut phone_ids = Vec::with_capacity(phones.len());
        let mut language_ids = Vec::with_capacity(phones.len());
        let mut vowel_mask = Vec::new();
        let mut previous_note = 0;
        for (phone, &frames) in phones.iter().zip(phone_frames) {
            if phone.note_index >= notes.len() || phone.note_index < previous_note {
                return Err(VoiceError::new(
                    "Compatible F0 phone records are not in note order",
                ));
            }
            previous_note = phone.note_index;
            let resolved =
                crate::synthesis::voice::resolve_alphabet(&self.languages, self.unified, &phone.language, &phone.token,phone.xsampa)?;
            let unified = self
                .unified
                .lookup(resolved.unified_symbol)
                .ok_or_else(|| VoiceError::new("Compatible F0 unified phone disappeared"))?;
            phone_ids.push(if self.categories {
                unified.class_index
            } else {
                unified.stored_index
            } as f32);
            language_ids.push(resolved.language_index as f32);
            vowel_mask.extend(std::iter::repeat_n(
                matches!(resolved.source_class, Some("vowel" | "diphthong")),
                frames,
            ));
        }
        Ok(Encoded {
            notes: rows,
            phone_ids,
            language_ids,
            vowel_mask,
        })
    }
}

fn phone_set(tree: &PreparedTree, index: usize) -> Result<&PhoneSet> {
    match &tree.node(index)?.module {
        PreparedModule::PhoneSet(value) => Ok(value),
        _ => Err(VoiceError::new(
            "Compatible F0 frontend child is not a phone table",
        )),
    }
}

pub(super) fn note_frame_counts(notes: &[FrontendNote], step: f32) -> Result<Vec<usize>> {
    let mut elapsed = 0.0_f64;
    let step = f64::from(step);
    let bias = f64::from_bits(0x3fdf_ffff_ffff_ffff);
    notes
        .iter()
        .map(|note| {
            let left = frame((elapsed / step + bias).trunc())?;
            elapsed += duration_seconds(note)?;
            let right = frame((elapsed / step + bias).trunc())?;
            right
                .checked_sub(left)
                .ok_or_else(|| VoiceError::new("Compatible F0 note clock moved backwards"))
        })
        .collect()
}

fn frame(value: f64) -> Result<usize> {
    if !value.is_finite() || value < 0.0 || value >= i32::MAX as f64 {
        return Err(VoiceError::new(
            "Compatible F0 frame count exceeds its native range",
        ));
    }
    Ok(value as usize)
}

fn error(error: impl std::fmt::Display) -> VoiceError {
    VoiceError::new(error.to_string())
}
