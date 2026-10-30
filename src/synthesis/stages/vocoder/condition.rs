use crate::synthesis::{
    inference::{Sequence, ops::range_normalizer::RangeNormalizer},
    voice::{PreparedTree, Result, VoiceError},
};

pub const CONDITION_TYPE_ID: [u8; 16] = [
    0xd4, 0x14, 0xf7, 0xf0, 0xb2, 0xbe, 0x11, 0x5e, 0xd5, 0xc5, 0xf9, 0xde, 0xa6, 0x2e, 0x70, 0xd1,
];
const ORIGINAL_CONDITION_TYPE_ID: [u8; 16] = [
    0x97, 0x90, 0x3c, 0x66, 0x53, 0xa0, 0x7d, 0xa6, 0xe8, 0x6e, 0x27, 0x2b, 0x00, 0x55, 0xce, 0x76,
];
const IDENTITY_TYPE_ID: [u8; 16] = [
    0xd8, 0x9e, 0x1b, 0x8a, 0x6c, 0x01, 0xd9, 0x3d, 0x18, 0xa8, 0xd1, 0xa7, 0x40, 0x1e, 0x1c, 0x0e,
];

pub const CHANNELS: usize = 72;
pub const PITCH_CHANNEL: usize = 1;

#[derive(Clone)]
pub struct VocoderCondition {
    normalizer: RangeNormalizer,
    pub sample_rate: u32,
    pub frame_seconds: f32,
    pub frame_samples: usize,

    pub sample_normalizer_node: usize,
}

impl VocoderCondition {
    pub fn prepare(tree: &PreparedTree, index: usize) -> Result<Self> {
        let node = tree.node(index)?;
        let payload_bytes = match node.type_id {
            ORIGINAL_CONDITION_TYPE_ID => 48,
            CONDITION_TYPE_ID => 52,
            _ => {
                return Err(VoiceError::new("Unsupported vocoder condition factory"));
            }
        };
        if node.children.len() != 4 {
            return Err(VoiceError::new(
                "Vocoder condition requires the recovered four-child type",
            ));
        }
        let payload = tree.payload(index)?;
        if payload.len() != payload_bytes {
            return Err(VoiceError::new(
                "Vocoder condition payload does not match its registered factory",
            ));
        }
        let words: Vec<_> = payload
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| u32::from_le_bytes(*bytes))
            .collect();

        if words[0] != CHANNELS as u32
            || words[1] != 1
            || words[4..12] != [72, 1, 1, 0, 1, 1, 1, 0]
            || (words.len() == 13 && words[12] != 0)
        {
            return Err(VoiceError::new("Unsupported vocoder condition layout"));
        }
        let sample_rate = words[2];
        let frame_seconds = f32::from_bits(words[3]);
        if sample_rate != 48_000 || frame_seconds.to_bits() != 0.005_f32.to_bits() {
            return Err(VoiceError::new(
                "Unsupported vocoder sample rate or frame period",
            ));
        }
        let normalizer = tree.range_normalizer(node.children[0])?.clone();
        if normalizer.channels() != CHANNELS {
            return Err(VoiceError::new(
                "Vocoder condition normalizer must have 72 channels",
            ));
        }

        for &child in &node.children[2..] {
            if tree.node(child)?.type_id != IDENTITY_TYPE_ID
                || !tree.node(child)?.children.is_empty()
                || !tree.payload(child)?.is_empty()
            {
                return Err(VoiceError::new(
                    "Unsupported optional vocoder condition transform",
                ));
            }
        }
        tree.range_normalizer(node.children[1])?;
        Ok(Self {
            normalizer,
            sample_rate,
            frame_seconds,
            frame_samples: (sample_rate as f32 * frame_seconds).round() as usize,
            sample_normalizer_node: node.children[1],
        })
    }

    pub fn normalize(&self, physical: &Sequence) -> Result<Sequence> {
        self.normalize_profile(physical, false)
    }

    pub(crate) fn normalize_native(&self, physical: &Sequence) -> Result<Sequence> {
        self.normalize_profile(physical, true)
    }

    fn normalize_profile(&self, physical: &Sequence, native: bool) -> Result<Sequence> {
        physical.validate()?;
        if physical.channels != CHANNELS {
            return Err(VoiceError::new(
                "Vocoder condition input must have 72 channels",
            ));
        }
        let mut normalized = Sequence::zeros(physical.frames, CHANNELS)?;
        for (frame_index, frame) in physical.values.as_chunks::<CHANNELS>().0.iter().enumerate() {
            if frame.iter().any(|value| !value.is_finite()) || frame[PITCH_CHANNEL] <= 0.0 {
                return Err(VoiceError::new(
                    "Vocoder conditions require finite values and positive F0",
                ));
            }
            if native {
                let mut row = *frame;
                row[PITCH_CHANNEL] = row[PITCH_CHANNEL].ln();
                self.normalizer.normalize_native(&row,
                    &mut normalized.values[frame_index * CHANNELS..(frame_index + 1) * CHANNELS], true)
                    .map_err(|error| VoiceError::new(error.to_string()))?;
                continue;
            }
            for (channel, &value) in frame.iter().enumerate() {
                let value = if channel == PITCH_CHANNEL {
                    value.ln()
                } else {
                    value
                };
                normalized.values[frame_index * CHANNELS + channel] = self
                    .normalizer
                    .normalize_scalar(channel, value, true)
                    .map_err(|error| VoiceError::new(error.to_string()))?;
            }
        }
        Ok(normalized)
    }
}
