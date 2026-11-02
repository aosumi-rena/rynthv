use serde::{Deserialize, Serialize};

use crate::synthesis::{
    inference::ops::softmax,
    voice::{PreparedTree, Result, VoiceError},
};

const FRONTEND_TYPE: [u8; 16] = [
    0x94, 0xe2, 0xba, 0x14, 0x8e, 0xe1, 0xf6, 0x8d, 0x07, 0x43, 0x4d, 0xcb, 0xf5, 0xa4, 0x04, 0xab,
];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExplicitPhone {
    #[serde(default)]
    pub xsampa: bool,
    pub language: String,
    pub token: String,

    pub onset_offset_seconds: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct DurationNote {

    pub sample_rate: u32,

    pub hop_samples: f64,

    pub duration_seconds: f64,
    pub pitch: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DurationRecord {
    pub phones: Vec<ExplicitPhone>,
    pub note: DurationNote,

    pub linked_durations_seconds: Vec<f64>,

    pub parts: u32,
    pub part_index: u32,
    pub pitch_offset: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhoneFeatures {
    pub unified_phone_index: usize,
    pub language_index: usize,
    pub left_index: usize,
    pub right_index: usize,
    pub pitch: f32,
    pub duration: f32,
    pub record_index: usize,
    pub used_silence_fallback: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhoneDuration {
    pub record_index: usize,
    pub phone_index: usize,

    pub duration_seconds: f64,

    pub frames: usize,
}

impl DurationRecord {
    fn timing(&self) -> Result<(f64, f64)> {
        if self.note.sample_rate == 0
            || !self.note.hop_samples.is_finite()
            || self.note.hop_samples <= 0.0
            || !self.note.duration_seconds.is_finite()
            || self.note.duration_seconds < 0.0
            || self.parts == 0
            || self.part_index >= self.parts
            || self
                .linked_durations_seconds
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(VoiceError::new("Invalid duration frontend record timing"));
        }
        let step = self.note.hop_samples / f64::from(self.note.sample_rate);
        let part = f64::from(self.part_index);

        let start =
            rounded_frames(self.note.duration_seconds * (part / f64::from(self.parts)) / step);
        let end = rounded_frames(
            self.note.duration_seconds * ((part + 1.0) / f64::from(self.parts)) / step,
        );
        let seconds = self
            .linked_durations_seconds
            .iter()
            .fold((end - start) * step, |sum, v| sum + v);
        if !seconds.is_finite() || seconds / step > i32::MAX as f64 {
            return Err(VoiceError::new(
                "Duration frontend record exceeds supported frame range",
            ));
        }
        Ok((step, seconds))
    }
}

fn frontend_node(tree: &PreparedTree, expected_type: [u8; 16]) -> Result<usize> {
    let root = tree.node(tree.root_index())?;
    let index = *root
        .children
        .first()
        .ok_or_else(|| VoiceError::new("Duration tree lacks its note frontend"))?;
    if tree.node(index)?.type_id != expected_type {
        return Err(VoiceError::new(
            "Duration root child 0 is not its typed note frontend",
        ));
    }
    Ok(index)
}

pub fn prepare_explicit(
    tree: &PreparedTree,
    records: &[DurationRecord],
) -> Result<Vec<PhoneFeatures>> {
    prepare_explicit_typed(tree, records, FRONTEND_TYPE)
}

pub(crate) fn prepare_explicit_typed(
    tree: &PreparedTree,
    records: &[DurationRecord],
    expected_type: [u8; 16],
) -> Result<Vec<PhoneFeatures>> {
    let frontend = frontend_node(tree, expected_type)?;
    let node = tree.node(frontend)?;
    if node.children.len() != 4 {
        return Err(VoiceError::new("Duration frontend requires four children"));
    }
    let payload = tree.payload(frontend)?;
    if payload.len() != 8 {
        return Err(VoiceError::new(
            "Duration frontend requires two pitch bounds",
        ));
    }
    let low = i32::from_le_bytes(payload[..4].try_into().unwrap());
    let high = i32::from_le_bytes(payload[4..].try_into().unwrap());
    if high <= low {
        return Err(VoiceError::new(
            "Duration frontend pitch bounds are not increasing",
        ));
    }
    let mut output = Vec::new();
    for (record_index, record) in records.iter().enumerate() {
        let (_, seconds) = record.timing()?;
        if record.phones.is_empty() {
            continue;
        }
        let pitch =
            ((record.note.pitch + record.pitch_offset) - low as f32) / (high as f32 - low as f32);
        if !pitch.is_finite() {
            return Err(VoiceError::new("Non-finite duration frontend pitch"));
        }
        let normalizer = tree.range_normalizer(node.children[2])?;
        if normalizer.channels() != 1 {
            return Err(VoiceError::new(
                "Duration frontend normalizer must have one channel",
            ));
        }
        let duration = normalizer
            .normalize_scalar(0, (seconds + 0.01).ln() as f32, true)
            .map_err(|error| VoiceError::new(error.to_string()))?;
        for (left_index, phone) in record.phones.iter().enumerate() {
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

pub fn postprocess(
    tree: &PreparedTree,
    records: &[DurationRecord],
    raw_scalar: &[f32],
) -> Result<Vec<PhoneDuration>> {

    let allocation_scalar = sv2_allocation_scalars(raw_scalar);
    postprocess_typed(tree, records, &allocation_scalar, FRONTEND_TYPE)
}

fn sv2_allocation_scalars(raw: &[f32]) -> Vec<f32> {
    raw.iter()
        .map(|&value| {

            let scaled = value * 0.5;
            scaled + 0.5
        })
        .collect()
}

pub(crate) fn postprocess_typed(
    tree: &PreparedTree,
    records: &[DurationRecord],
    raw_scalar: &[f32],
    expected_type: [u8; 16],
) -> Result<Vec<PhoneDuration>> {
    postprocess_biased(tree,records,raw_scalar,expected_type,None)
}

pub fn postprocess_controlled(
    tree:&PreparedTree,records:&[DurationRecord],prediction:&super::DurationPrediction,control:f32,
)->Result<Vec<PhoneDuration>> {
    if !control.is_finite() || prediction.impulse_values.len()!=prediction.scalars.len() {
        return Err(VoiceError::new("Invalid consonant duration conditions"));
    }
    let adjustment=(1.0+control.clamp(-0.9,0.9)).ln();
    let biases:Vec<_>=prediction.impulse_values.iter().map(|&value|if value>0.0 {adjustment}else{0.0}).collect();
    postprocess_biased(tree,records,&sv2_allocation_scalars(&prediction.scalars),FRONTEND_TYPE,Some(&biases))
}

fn postprocess_biased(
    tree:&PreparedTree,records:&[DurationRecord],raw_scalar:&[f32],expected_type:[u8;16],biases:Option<&[f32]>,
)->Result<Vec<PhoneDuration>> {
    let frontend = tree.node(frontend_node(tree, expected_type)?)?;
    if frontend.children.len() != 4 {
        return Err(VoiceError::new("Duration frontend requires four children"));
    }
    let count = records
        .iter()
        .try_fold(0_usize, |sum, record| sum.checked_add(record.phones.len()))
        .ok_or_else(|| VoiceError::new("Duration phone count overflows"))?;
    if raw_scalar.len() != count || raw_scalar.iter().any(|v| !v.is_finite()) {
        return Err(VoiceError::new(
            "Duration scalar output does not match phone records",
        ));
    }
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let normalizer = tree.range_normalizer(frontend.children[3])?;
    if normalizer.channels() != 1 {
        return Err(VoiceError::new(
            "Duration output normalizer must have one channel",
        ));
    }
    let mut transformed = vec![0.0; raw_scalar.len()];
    normalizer
        .denormalize(raw_scalar, &mut transformed)
        .map_err(|error| VoiceError::new(error.to_string()))?;
    if transformed.len() != count || transformed.iter().any(|v| !v.is_finite()) {
        return Err(VoiceError::new(
            "Duration output normalization returned invalid scalars",
        ));
    }
    if let Some(biases)=biases {
        if biases.len()!=count || biases.iter().any(|v|!v.is_finite()) {
            return Err(VoiceError::new("Duration allocation bias differs from phone records"));
        }
        for (value,bias) in transformed.iter_mut().zip(biases) {*value+=bias;}
    }
    let mut output = Vec::with_capacity(count);
    let mut cursor = 0;
    for (record_index, record) in records.iter().enumerate() {
        let (step, seconds) = record.timing()?;
        let scalars = &transformed[cursor..cursor + record.phones.len()];
        cursor += record.phones.len();
        if scalars.is_empty() {
            continue;
        }
        let total_frames = seconds / step;
        let mut weights = scalars.to_vec();
        softmax::execute(&mut weights).map_err(|error| VoiceError::new(error.to_string()))?;
        let mut accumulated = 0.0;
        let mut previous = 0_usize;
        for (phone_index, weight) in weights.iter().enumerate() {
            accumulated += rounded_frames(total_frames) * f64::from(*weight);
            let end = if phone_index + 1 == weights.len() {
                rounded_frames(total_frames)
            } else {
                rounded_frames(accumulated)
            };
            let end = end as usize;
            let frames = end
                .checked_sub(previous)
                .ok_or_else(|| VoiceError::new("Duration allocation has decreasing boundaries"))?;
            previous = end;
            output.push(PhoneDuration {
                record_index,
                phone_index,
                duration_seconds: frames as f64 * step,
                frames,
            });
        }
    }

    let (step, _) = records[0].timing()?;
    let phones: Vec<_> = records.iter().flat_map(|record| &record.phones).collect();
    let mut seconds = 0.0;
    let mut previous = 0_usize;
    for (index, phone) in output.iter_mut().enumerate() {
        seconds += phone.duration_seconds;
        let offset = phones
            .get(index + 1)
            .and_then(|p| p.onset_offset_seconds)
            .unwrap_or(0.0);
        if !offset.is_finite() {
            return Err(VoiceError::new("Non-finite phoneme onset offset"));
        }
        let boundary = rounded_frames((seconds + offset) / step);
        if !boundary.is_finite() || boundary > i32::MAX as f64 {
            return Err(VoiceError::new(
                "Phoneme boundary exceeds supported frame range",
            ));
        }
        let minimum = if index == 0 { 0 } else { previous + 1 };
        let end = (boundary.max(0.0) as usize).max(minimum);
        phone.frames = end - previous;
        previous = end;
    }
    Ok(output)
}

fn rounded_frames(value: f64) -> f64 {

    (value + f64::from_bits(0x3fdf_ffff_ffff_ffff).copysign(value)).trunc()
}

#[cfg(test)]
mod sv2_allocation_tests {
    use super::sv2_allocation_scalars;

    #[test]
    fn conversion_is_a_copy_and_retains_raw_backprojection_input() {
        let raw = [-1.0_f32, -0.5, -0.0, 0.0, 0.5, 1.0];
        let original_bits = raw.map(f32::to_bits);
        let mut allocation = sv2_allocation_scalars(&raw);
        assert_eq!(allocation, [0.0, 0.25, 0.5, 0.5, 0.75, 1.0]);
        assert_eq!(raw.map(f32::to_bits), original_bits);

        allocation[0] = 123.0;
        assert_eq!(raw.map(f32::to_bits), original_bits);
    }

    #[test]
    fn no_clipping_or_bulk_boundary_exception() {
        let raw = [-3.0_f32, -2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0, 3.0];
        let expected = [-1.0_f32, -0.5, 0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0];
        for count in [0, 1, 7, 8, 9] {
            assert_eq!(sv2_allocation_scalars(&raw[..count]), expected[..count]);
        }
    }

    #[test]
    fn invalid_scalars_are_not_repaired_before_typed_validation() {
        let allocation = sv2_allocation_scalars(&[f32::NAN, f32::INFINITY, f32::NEG_INFINITY]);
        assert!(allocation[0].is_nan());
        assert_eq!(allocation[1], f32::INFINITY);
        assert_eq!(allocation[2], f32::NEG_INFINITY);
    }
}
