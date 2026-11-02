use crate::synthesis::{
    stages::duration::{DurationRecord, PhoneDuration},
    voice::{PreparedTree, Result, VoiceError},
};

use super::frontend;

pub(super) fn execute(
    tree: &PreparedTree,
    frontend: usize,
    records: &[DurationRecord],
    scalars: &[f32],
) -> Result<(Vec<PhoneDuration>, Vec<f64>)> {
    let normalizer = tree.range_normalizer(tree.node(frontend)?.children[3])?;
    let mut values = vec![0.0; scalars.len()];
    normalizer
        .denormalize(scalars, &mut values)
        .map_err(|error| VoiceError::new(error.to_string()))?;
    let mut result = Vec::with_capacity(scalars.len());
    let mut record_seconds = Vec::with_capacity(records.len());
    let mut cursor = 0_usize;
    let mut carry = 0.0_f64;
    for (record_index, record) in records.iter().enumerate() {
        let (_, seconds) = frontend::timing(record)?;
        let count = record.phones.len();
        let end = cursor
            .checked_add(count)
            .filter(|end| *end <= values.len())
            .ok_or_else(|| VoiceError::new("SV1 duration scalar count differs from its records"))?;
        let predictions = &mut values[cursor..end];
        cursor = end;
        if predictions.is_empty() {
            record_seconds.push(borrow_onset(seconds, &[], &[], &mut carry));
            continue;
        }

        for value in predictions.iter_mut() {
            *value = value.exp();
        }
        let sum = predictions.iter().fold(0.0_f32, |sum, value| sum + value);
        if !sum.is_finite() || sum <= 0.0 {
            return Err(VoiceError::new(
                "SV1 duration prediction cannot be normalized",
            ));
        }
        let reciprocal = sum.recip();
        let mut seconds_by_phone: Vec<_> = predictions
            .iter()
            .map(|value| f64::from((f64::from(*value * reciprocal) * seconds) as f32))
            .collect();
        let classes = record
            .phones
            .iter()
            .map(|phone| {
                tree.resolve_phone_alphabet(&phone.language, &phone.token,phone.xsampa)
                    .map(|phone| phone.source_class)
            })
            .collect::<Result<Vec<_>>>()?;
        let target = borrow_onset(seconds, &seconds_by_phone, &classes, &mut carry);
        record_seconds.push(target);
        redistribute(&mut seconds_by_phone, &classes, target)?;
        result.extend(seconds_by_phone.into_iter().enumerate().map(
            |(phone_index, duration_seconds)| PhoneDuration {
                record_index,
                phone_index,
                duration_seconds,
                frames: 0,
            },
        ));
    }
    if cursor != values.len() {
        return Err(VoiceError::new(
            "SV1 duration has surplus scalar predictions",
        ));
    }
    if let Some(first) = records.first() {
        let reciprocal_step = frontend::timing(first)?.0.recip();
        let mut accumulated = 0.0;
        let mut previous = 0_usize;
        for phone in &mut result {
            accumulated += phone.duration_seconds;
            let boundary = (accumulated * reciprocal_step).trunc();
            if !boundary.is_finite() || boundary < 0.0 || boundary >= f64::from(i32::MAX) {
                return Err(VoiceError::new(
                    "SV1 acoustic phone boundary exceeds its frame range",
                ));
            }
            let end = (boundary as usize).max(previous + 1);
            phone.frames = end - previous;
            previous = end;
        }
    }
    Ok((result, record_seconds))
}

fn borrow_onset(seconds: f64, phones: &[f64], classes: &[Option<&str>], carry: &mut f64) -> f64 {

    let mut target = seconds;
    if *carry > 0.0 {
        if seconds <= *carry * 4.0 {
            *carry += seconds * -0.25;
            target = seconds * 0.75;
        } else {
            target -= *carry;
            *carry = 0.0;
        }
    }
    if classes.ends_with(&[Some("stop"), Some("semivowel")]) {
        let borrowed = phones[phones.len() - 1] * 0.5;
        target += borrowed;
        *carry += borrowed;
    }
    target
}

fn redistribute(phones: &mut [f64], classes: &[Option<&str>], target: f64) -> Result<()> {
    let sustained =
        |class: Option<&str>| matches!(class, Some("vowel" | "diphthong" | "coda" | "silence"));
    let (mut sustained_sum, mut other_sum) = (0.0, 0.0);
    for (&seconds, &class) in phones.iter().zip(classes) {
        if sustained(class) {
            sustained_sum += seconds;
        } else {
            other_sum += seconds;
        }
    }
    let reserved = (target * 0.3).max(target - other_sum);
    let other_target = target - if sustained_sum == 0.0 { 0.0 } else { reserved };
    for (seconds, &class) in phones.iter_mut().zip(classes) {
        let (target, sum) = if sustained(class) {
            (reserved, sustained_sum)
        } else {
            (other_target, other_sum)
        };
        *seconds = target * *seconds / sum;
    }
    let sum: f64 = phones.iter().sum();
    if !sum.is_finite() || sum <= 0.0 {
        return Err(VoiceError::new(
            "SV1 phone class duration allocation is invalid",
        ));
    }
    let pairs = phones.len() & !1;
    for (index, seconds) in phones.iter_mut().enumerate() {

        *seconds = if index < pairs {
            *seconds * (target / sum)
        } else {
            (*seconds * target) * sum.recip()
        };
        if !seconds.is_finite() || *seconds < 0.0 {
            return Err(VoiceError::new(
                "SV1 phone duration is not finite and nonnegative",
            ));
        }
    }
    Ok(())
}
