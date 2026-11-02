use super::topology::Mapping;
use crate::synthesis::{
    engine::OwnerSynthesis,
    score::PreparedPhrase,
    stages::{
        duration::{DurationResult, DurationStage},
        f0::TimedPhone,
    },
    voice::{PreparedVoice, Result, VoiceError},
};

pub(super) struct Prepared {
    pub duration: DurationResult,
    pub phones: Vec<TimedPhone>,
    pub phone_rows: Vec<Option<usize>>,
}

pub(super) fn prepare(
    voice: &PreparedVoice,
    identity: u64,
    phrase: &PreparedPhrase,
    base: &OwnerSynthesis,
    mapping: &Mapping,
) -> Result<Prepared> {
    let seeds: Vec<_> = phrase
        .retake_seeds
        .iter()
        .map(|seeds| seeds.unwrap_or_default().duration)
        .collect();
    let mut current = DurationStage::prepare(voice.model("duration")?, identity, voice.speaker)?
        .predict_explicit_seeded(&phrase.records, &seeds)?;
    if phrase.voice_parameters.consonant_duration!=0.0 {
        current.durations=crate::synthesis::stages::duration::frontend::postprocess_controlled(
            voice.model("duration")?,&phrase.records,&current.prediction,
            phrase.voice_parameters.consonant_duration)?;
    }
    crate::synthesis::score::phonemes::apply(phrase,&mut current)?;
    let old = &base.duration;
    validate_arrays(&current)?;
    validate_arrays(old)?;
    let old_offsets = phone_offsets(&base.prepared.phrase)?;
    if old_offsets.last().copied() != Some(old.durations.len())
        || base.prepared.phones.len() != old.durations.len()
    {
        return Err(VoiceError::new(
            "Saved duration phone rows differ from their prepared phrase",
        ));
    }
    let mut phone_rows = Vec::with_capacity(current.durations.len());
    for (index, now) in current.durations.iter_mut().enumerate() {
        let old_record = mapping
            .note_rows
            .get(now.record_index)
            .ok_or_else(|| VoiceError::new("Duration record index is outside its phrase"))?;
        let Some(old_record) = *old_record else {
            phone_rows.push(None);
            continue;
        };
        let old_index = old_offsets[old_record]
            .checked_add(now.phone_index)
            .filter(|&index| index < old_offsets[old_record + 1])
            .ok_or_else(|| VoiceError::new("Reused duration phone index is outside its record"))?;
        let previous = &old.durations[old_index];
        let previous_phone = &base.prepared.phones[old_index];
        let explicit = &phrase.records[now.record_index].phones[now.phone_index];
        if previous.record_index != old_record
            || previous.phone_index != now.phone_index
            || previous_phone.note_index != old_record
            || previous_phone.frames != previous.frames
            || previous_phone.language != explicit.language
            || previous_phone.token != explicit.token
        {
            return Err(VoiceError::new(
                "Saved duration phone correspondence is invalid",
            ));
        }

        now.frames = previous.frames;
        now.duration_seconds = previous.duration_seconds;
        current.natural_frames[index]=old.natural_frames[old_index];
        current.prediction.scalars[index] = old.prediction.scalars[old_index];
        current.prediction.impulse_values[index] = old.prediction.impulse_values[old_index];
        current.prediction.impulse_positions[index] = old.prediction.impulse_positions[old_index];
        phone_rows.push(Some(old_index));
    }
    let phones = timed_phones(phrase, &current, mapping)?;
    Ok(Prepared {
        duration: current,
        phones,
        phone_rows,
    })
}

fn validate_arrays(value: &DurationResult) -> Result<()> {
    let count = value.durations.len();
    if [
        value.natural_frames.len(),
        value.prediction.phones.len(),
        value.prediction.scalars.len(),
        value.prediction.impulse_values.len(),
        value.prediction.impulse_positions.len(),
    ]
    .iter()
    .any(|&length| length != count)
        || value
            .prediction
            .phones
            .iter()
            .any(|phone| phone.used_silence_fallback)
    {
        return Err(VoiceError::new(
            "Duration snapshot contains invalid phone arrays",
        ));
    }
    Ok(())
}

fn phone_offsets(phrase: &PreparedPhrase) -> Result<Vec<usize>> {
    let mut output = Vec::with_capacity(phrase.records.len() + 1);
    output.push(0_usize);
    for record in &phrase.records {
        let end = output
            .last()
            .expect("initial boundary")
            .checked_add(record.phones.len())
            .ok_or_else(|| VoiceError::new("Duration phone row count overflow"))?;
        output.push(end);
    }
    Ok(output)
}

fn timed_phones(
    phrase: &PreparedPhrase,
    duration: &DurationResult,
    mapping: &Mapping,
) -> Result<Vec<TimedPhone>> {
    let mut total=0_usize;
    let mut phones = Vec::with_capacity(duration.durations.len());
    for allocation in &duration.durations {
        let record = phrase
            .records
            .get(allocation.record_index)
            .ok_or_else(|| VoiceError::new("Duration record index is outside its phrase"))?;
        let phone = record
            .phones
            .get(allocation.phone_index)
            .ok_or_else(|| VoiceError::new("Duration phone index is outside its record"))?;
        total = total
            .checked_add(allocation.frames)
            .ok_or_else(|| VoiceError::new("Duration record frame count overflow"))?;
        phones.push(TimedPhone {
            xsampa:phone.xsampa,
            note_index: allocation.record_index,
            language: phone.language.clone(),
            token: phone.token.clone(),
            frames: allocation.frames,
        });
    }
    if total!=*mapping.record_frames.last().expect("validated note clock") {
        return Err(VoiceError::new(
            "Local phone clock does not cover the complete note clock",
        ));
    }
    Ok(phones)
}
