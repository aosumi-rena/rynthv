use crate::synthesis::{
    score::PreparedPhrase,
    stages::{
        duration::DurationResult,
        f0::{PitchResult, PreparedPitchFeatures, TimedPhone, features::FeatureEncoding},
        timbre::PreparedTimbreFeatures,
    },
    voice::{Result, VoiceError},
};

#[derive(Debug)]
pub struct PreparedAcoustics {
    pub phrase: PreparedPhrase,
    pub phones: Vec<TimedPhone>,
    pub features: PreparedModelFeatures,
}

#[derive(Debug)]
pub enum PreparedModelFeatures {
    Sv1Local,
    Sv2 {
        pitch: PreparedPitchFeatures,
        timbre: PreparedTimbreFeatures,
    },
}

impl PreparedAcoustics {
    pub fn sv2_features(&self) -> Result<(&PreparedPitchFeatures, &PreparedTimbreFeatures)> {
        match &self.features {
            PreparedModelFeatures::Sv2 { pitch, timbre } => Ok((pitch, timbre)),
            PreparedModelFeatures::Sv1Local => Err(VoiceError::new(
                "SV1 acoustic state cannot enter the SV2 incremental model program",
            )),
        }
    }
}

#[derive(Debug)]
pub struct OwnerSynthesis {
    pub prepared: PreparedAcoustics,
    pub duration: DurationResult,
    pub pitch: PitchResult,
    pub components: super::PcmComponents,
}

impl OwnerSynthesis {

    pub fn memory_bytes(&self) -> usize {
        let prepared = &self.prepared;
        let phrase = &prepared.phrase;
        let mut bytes = std::mem::size_of_val(self)
            .saturating_add(phrase.unison.part.capacity())
            .saturating_add(vector_bytes(&phrase.records))
            .saturating_add(vector_bytes(&phrase.notes))
            .saturating_add(vector_bytes(&phrase.source_note_ids))
            .saturating_add(vector_bytes(&phrase.phone_sources))
            .saturating_add(phrase.vocoder_phone_labels.values().map(|s|std::mem::size_of::<crate::synthesis::score::PhoneSource>()+std::mem::size_of::<String>()+s.capacity()).sum::<usize>())
            .saturating_add(phrase.phone_sources.iter().map(|p|vector_bytes(p)).sum::<usize>())
            .saturating_add(vector_bytes(&phrase.retake_seeds))
            .saturating_add(vector_bytes(&prepared.phones));
        if let Some(controls) = &phrase.performance_controls {
            bytes = bytes.saturating_add(controls.memory_bytes());
        }
        for record in &phrase.records {
            bytes = bytes
                .saturating_add(vector_bytes(&record.phones))
                .saturating_add(vector_bytes(&record.linked_durations_seconds));
            for phone in &record.phones {
                bytes = bytes
                    .saturating_add(phone.language.capacity())
                    .saturating_add(phone.token.capacity());
            }
        }
        for note in &phrase.notes {
            bytes = bytes
                .saturating_add(vector_bytes(&note.phones))
                .saturating_add(vector_bytes(&note.appended_durations))
                .saturating_add(note.tag.as_ref().map_or(0, String::capacity));
            for phone in &note.phones {
                bytes = bytes.saturating_add(phone.capacity());
            }
        }
        for phone in &prepared.phones {
            bytes = bytes
                .saturating_add(phone.language.capacity())
                .saturating_add(phone.token.capacity());
        }
        if let PreparedModelFeatures::Sv2 { pitch, timbre } = &prepared.features {
            bytes = bytes
                .saturating_add(encoding_bytes(&pitch.encoding))
                .saturating_add(std::mem::size_of_val(pitch.transpose.as_ref()))
                .saturating_add(encoding_bytes(timbre));
        }
        let prediction = &self.duration.prediction;
        bytes
            .saturating_add(self.components.memory_bytes())
            .saturating_add(vector_bytes(&self.duration.durations))
            .saturating_add(vector_bytes(&self.duration.natural_frames))
            .saturating_add(vector_bytes(&prediction.phones))
            .saturating_add(vector_bytes(&prediction.scalars))
            .saturating_add(vector_bytes(&prediction.impulse_values))
            .saturating_add(vector_bytes(&prediction.impulse_positions))
            .saturating_add(vector_bytes(&self.pitch.pitch))
            .saturating_add(vector_bytes(&self.pitch.predicted_controls.slope))
            .saturating_add(vector_bytes(&self.pitch.predicted_controls.offset))
    }
}

fn encoding_bytes(encoding: &FeatureEncoding) -> usize {
    std::mem::size_of_val(encoding.notes.as_ref())
        .saturating_add(std::mem::size_of_val(encoding.phones.as_ref()))
}

fn vector_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity().saturating_mul(std::mem::size_of::<T>())
}
