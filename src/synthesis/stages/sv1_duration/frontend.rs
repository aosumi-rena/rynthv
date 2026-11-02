use crate::synthesis::{
    stages::duration::{DurationRecord, PhoneFeatures},
    voice::{PreparedTree, Result, VoiceError, embedded::types},
};

pub(super) fn validate(tree: &PreparedTree, index: usize) -> Result<(i32, i32)> {
    let node = tree.node(index)?;
    let payload = tree.payload(index)?;
    if node.type_id != types::DURATION_FRONTEND_TYPE_ID
        || node.children.len() != 4
        || payload.len() != 8
    {
        return Err(VoiceError::new("Invalid original SV1 duration frontend"));
    }
    let low = i32::from_le_bytes(payload[..4].try_into().unwrap());
    let high = i32::from_le_bytes(payload[4..].try_into().unwrap());
    if high <= low {
        return Err(VoiceError::new(
            "SV1 duration pitch bounds are not increasing",
        ));
    }
    for &child in &node.children[2..] {
        if tree.range_normalizer(child)?.channels() != 1 {
            return Err(VoiceError::new(
                "SV1 duration normalizers require one channel",
            ));
        }
    }
    Ok((low, high))
}

pub(super) fn prepare(
    tree: &PreparedTree,
    index: usize,
    records: &[DurationRecord],
) -> Result<Vec<PhoneFeatures>> {
    let (low, high) = validate(tree, index)?;
    let normalizer = tree.range_normalizer(tree.node(index)?.children[2])?;
    let mut output = Vec::new();
    for (record_index, record) in records.iter().enumerate() {
        let (_, seconds) = timing(record)?;
        let pitch =
            ((record.note.pitch + record.pitch_offset) - low as f32) / (high as f32 - low as f32);
        if !pitch.is_finite() {
            return Err(VoiceError::new("Non-finite SV1 duration pitch"));
        }
        let duration = normalizer
            .normalize_scalar(0, (seconds + f64::from(0.01_f32)).ln() as f32, true)
            .map_err(|error| VoiceError::new(error.to_string()))?;
        for (left_index, phone) in record.phones.iter().enumerate() {

            if phone
                .onset_offset_seconds
                .is_some_and(|offset| offset != 0.0)
            {
                return Err(VoiceError::new(
                    "SV1 synthesis does not yet support phoneme onset offsets",
                ));
            }
            let resolved = tree.resolve_phone_alphabet(&phone.language, &phone.token,phone.xsampa)?;
            output.push(PhoneFeatures {
                unified_phone_index: resolved.unified_phone_index,
                language_index: resolved.language_index,
                left_index,
                right_index: record.phones.len() - left_index - 1,
                pitch,
                duration,
                record_index,
                used_silence_fallback: resolved.used_silence_fallback,
            });
        }
    }
    Ok(output)
}

pub(super) fn timing(record: &DurationRecord) -> Result<(f64, f64)> {
    if record.note.sample_rate == 0
        || !record.note.hop_samples.is_finite()
        || record.note.hop_samples <= 0.0
        || !record.note.duration_seconds.is_finite()
        || record.note.duration_seconds < 0.0
        || record.parts == 0
        || record.part_index >= record.parts
        || record
            .linked_durations_seconds
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(VoiceError::new("Invalid SV1 duration record timing"));
    }

    let seconds = record.linked_durations_seconds.iter().fold(
        record.note.duration_seconds / f64::from(record.parts),
        |sum, value| sum + value,
    );
    let step = f64::from((record.note.hop_samples / f64::from(record.note.sample_rate)) as f32);
    if !seconds.is_finite()
        || !step.is_finite()
        || step <= 0.0
        || seconds / step > f64::from(i32::MAX)
        || (seconds == 0.0 && !record.phones.is_empty())
    {
        return Err(VoiceError::new(
            "SV1 duration record exceeds its supported timing range",
        ));
    }
    Ok((step, seconds))
}
