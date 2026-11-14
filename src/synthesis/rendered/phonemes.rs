use super::super::{
    score::{PhoneSource, PreparedPhrase},
    stages::duration::DurationResult,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub struct RenderedPhone {
    pub token: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub predicted_start_seconds: f64,
    pub predicted_end_seconds: f64,
    pub source: Option<PhoneSource>,
    pub boundary_strength: f64,
    pub boundary_position: f64,
    pub has_activity: bool,
    pub strength: f32,
    pub error: Option<Arc<str>>,
    pub predicted_activity: Option<f64>,
    pub predicted_position: f64,
}
#[derive(Clone, Debug)]
pub struct PhonemeTrack(pub Arc<[RenderedPhone]>, Arc<BTreeMap<u64, Vec<usize>>>);
#[derive(Clone, Debug)]
pub struct PhonemeSnapshot {
    pub input: Arc<crate::synthesis::plan::OwnerInput>,
    pub phones: PhonemeTrack,
}
impl PhonemeTrack {
    pub(crate) fn from_phones(phones: Arc<[RenderedPhone]>) -> Self {
        let mut by_note = BTreeMap::<u64, Vec<usize>>::new();
        for (index, phone) in phones.iter().enumerate() {
            if let Some(source) = phone.source {
                by_note.entry(source.note_id).or_default().push(index);
            }
        }
        Self(phones, Arc::new(by_note))
    }

    pub fn for_note(&self, id: u64) -> impl Iterator<Item = &RenderedPhone> {
        self.1.get(&id).into_iter().flatten().map(|&index| &self.0[index])
    }

    pub fn new(phrase: &PreparedPhrase, duration: &DurationResult) -> Self {
        let mut seconds = phrase.start_seconds;
        let mut natural_seconds=phrase.start_seconds;
        Self::from_phones(
            duration
                .durations
                .iter()
                .enumerate()
                .map(|(index, phone)| {
                    let start_seconds = seconds;
                    seconds += phone.duration_seconds;
                    let predicted_start_seconds=natural_seconds;
                    natural_seconds+=duration.natural_frames[index] as f64*crate::synthesis::score::FRAME_SECONDS;
                    let source = phrase.phone_sources[phone.record_index][phone.phone_index];
                    let control = source.and_then(|s| phrase.phoneme_controls.get(&s));
                    RenderedPhone {
                        token: source.and_then(|s|phrase.phone_labels.get(&s)).cloned().unwrap_or_else(||
                            phrase.records[phone.record_index].phones[phone.phone_index].token.clone()),
                        start_seconds,
                        end_seconds: seconds,
                        predicted_start_seconds,
                        predicted_end_seconds:natural_seconds,
                        source,
                        error: source.and_then(|s| phrase.note_errors.get(&s.note_id))
                            .map(|error| Arc::from(error.as_str())),
                        predicted_activity: duration.prediction.impulse_values.get(index).copied().filter(|v|*v>0.0),
                        predicted_position: duration.prediction.impulse_positions.get(index).copied().map_or(0.0,f64::from),
                        boundary_strength: crate::synthesis::score::phonemes::activity(
                            phrase, duration, index,
                        )
                        .unwrap_or(0.0),
                        has_activity: crate::synthesis::score::phonemes::activity(
                            phrase, duration, index,
                        )
                        .is_some(),
                        strength: control.and_then(|c| c.strength).unwrap_or(0.0),
                        boundary_position: control
                            .and_then(|c| c.position)
                            .or_else(|| duration.prediction.impulse_positions.get(index).copied())
                            .map_or(0.0, f64::from),
                    }
                })
                .collect(),
        )
    }
    pub fn memory_bytes(&self) -> usize {
        std::mem::size_of_val(self.0.as_ref())
            + self.0.iter().map(|p| p.token.capacity()).sum::<usize>()
            + self.1.values().map(|indices| std::mem::size_of::<(u64, Vec<usize>)>()
                + indices.capacity() * std::mem::size_of::<usize>()).sum::<usize>()
    }
}
