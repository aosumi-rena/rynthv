use std::sync::Arc;

use crate::synthesis::{
    execution::checkpoint,
    inference::{
        Sequence,
        affine::Affine,
        recurrent::{Gru, GruState},
    },
    voice::{PreparedModule, PreparedTree, Result, VoiceError},
};

pub const POSITIONAL_GRU_TYPE_ID: [u8; 16] = [
    0x4e, 0xac, 0x7e, 0x2f, 0xe6, 0x91, 0x45, 0x1e, 0xfc, 0xa6, 0x70, 0xcb, 0x9e, 0xd2, 0x1e, 0x40,
];
const PROJECTION_TYPE_ID: [u8; 16] = [
    0xde, 0x2c, 0xa5, 0xe2, 0xde, 0x53, 0xd6, 0xc8, 0x6f, 0x83, 0xcb, 0x27, 0x36, 0xb5, 0x39, 0x59,
];
const INPUT_CHANNELS: usize = 72;
const SPECTRAL_CHANNELS: usize = 64;
const SPECTRAL_START: usize = 3;

#[derive(Clone)]
pub struct PositionalGru {
    projection: Affine,
    retained_channels: usize,
    phase_divisor: u32,
    frequencies: Arc<[f32]>,
    recurrent: Gru,
}

impl PositionalGru {
    pub fn prepare(tree: &PreparedTree, index: usize) -> Result<Self> {
        Self::prepare_layout(tree, index, false)
    }

    pub(super) fn prepare_compact(tree: &PreparedTree, index: usize) -> Result<Self> {
        Self::prepare_layout(tree, index, true)
    }

    fn prepare_layout(tree: &PreparedTree, index: usize, compact: bool) -> Result<Self> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("vocoder.positional_gru.prepare");
        let node = tree.node(index)?;
        if node.type_id != POSITIONAL_GRU_TYPE_ID || node.children.len() != 3 {
            return Err(VoiceError::new(
                "Vocoder positional GRU requires the recovered three-child type",
            ));
        }
        let payload = tree.payload(index)?;

        if payload.len() != 12
            || (!compact
                && (u32::from_le_bytes(payload[4..8].try_into().unwrap()) != 0
                    || u32::from_le_bytes(payload[8..12].try_into().unwrap())
                        != INPUT_CHANNELS as u32))
        {
            return Err(VoiceError::new(
                "Unsupported vocoder positional condition configuration",
            ));
        }
        let declared_channels = u16::from_le_bytes(payload[0..2].try_into().unwrap()) as usize;
        let packed_width = u16::from_le_bytes(payload[2..4].try_into().unwrap());
        let phase_divisor = if compact {
            u32::from_le_bytes(payload[4..8].try_into().unwrap())
        } else {
            u32::from(packed_width)
        };
        if declared_channels == 0 || phase_divisor == 0 {
            return Err(VoiceError::new(
                "Vocoder positional dimensions and phase divisor must be positive",
            ));
        }
        let frequencies: Arc<[f32]> = tree
            .tensor(node.children[0])?
            .vector()
            .ok_or_else(|| {
                VoiceError::new("Vocoder positional frequencies must be a float vector")
            })?
            .into();
        if frequencies.is_empty() || frequencies.iter().any(|value| !value.is_finite()) {
            return Err(VoiceError::new("Invalid vocoder positional frequencies"));
        }
        let retained_channels = if compact {
            declared_channels
                .checked_sub(frequencies.len() * 2)
                .filter(|&count| count > 0)
                .ok_or_else(|| VoiceError::new("Invalid compact vocoder feature dimensions"))?
        } else {
            declared_channels
        };
        let projection_node = tree.node(node.children[1])?;
        if projection_node.type_id != PROJECTION_TYPE_ID
            || projection_node.children.len() != 2
            || !tree.payload(node.children[1])?.is_empty()
        {
            return Err(VoiceError::new(
                "Vocoder spectral projection requires the two-matrix module",
            ));
        }
        let matrix = tree.tensor(projection_node.children[0])?.dense()?;
        let second_shape = tree.tensor(projection_node.children[1])?.matrix_shape();
        if matrix.columns() != SPECTRAL_CHANNELS
            || matrix.rows() < retained_channels
            || second_shape != Some((matrix.columns(), matrix.rows()))
        {
            return Err(VoiceError::new(
                "Vocoder spectral projection matrix dimensions do not match",
            ));
        }

        let projection =
            Affine::new(matrix, None).map_err(|error| VoiceError::new(error.to_string()))?;
        let recurrent = match &tree.node(node.children[2])?.module {
            PreparedModule::Gru(gru) => gru.clone(),
            _ => {
                return Err(VoiceError::new(
                    "Vocoder positional child must be a prepared GRU",
                ));
            }
        };
        let features = frequencies
            .len()
            .checked_mul(2)
            .and_then(|count| count.checked_add(retained_channels))
            .ok_or_else(|| VoiceError::new("Vocoder positional feature count overflows"))?;
        if recurrent.input_channels() != features
            || (compact
                && (recurrent.output_channels() != usize::from(packed_width)
                    || frequencies.len()
                        != u32::from_le_bytes(payload[8..12].try_into().unwrap()) as usize))
        {
            return Err(VoiceError::new(
                "Vocoder positional features do not match the GRU input",
            ));
        }
        Ok(Self {
            projection,
            retained_channels,
            phase_divisor,
            frequencies,
            recurrent,
        })
    }

    pub fn output_channels(&self) -> usize {
        self.recurrent.output_channels()
    }

    pub fn new_state(&self) -> GruState {
        self.recurrent.new_state()
    }

    pub fn execute(
        &self,
        input: &Sequence,
        positions: &[u32],
        state: &mut GruState,
    ) -> Result<Sequence> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("vocoder.positional_gru.execute");
        input.validate()?;
        if input.channels != INPUT_CHANNELS || positions.len() != input.frames {
            return Err(VoiceError::new(
                "Vocoder positional input needs 72 channels and one position per frame",
            ));
        }
        if state.hidden().len() != self.output_channels()
            || input.values.iter().any(|value| !value.is_finite())
        {
            return Err(VoiceError::new("Invalid vocoder positional state or input"));
        }
        let mut output = Sequence::zeros(input.frames, self.output_channels())?;
        let mut projected = vec![0.0; self.projection.output_channels()];
        let mut features = vec![0.0; self.recurrent.input_channels()];
        for (frame_index, (frame, &position)) in input
            .values
            .as_chunks::<INPUT_CHANNELS>()
            .0
            .iter()
            .zip(positions)
            .enumerate()
        {
            checkpoint()?;
            self.projection
                .execute(
                    &frame[SPECTRAL_START..SPECTRAL_START + SPECTRAL_CHANNELS],
                    &mut projected,
                )
                .map_err(|error| VoiceError::new(error.to_string()))?;
            features[..self.retained_channels]
                .copy_from_slice(&projected[..self.retained_channels]);

            let phase = (position as f64 * std::f64::consts::TAU) / self.phase_divisor as f64;
            for (frequency_index, &frequency) in self.frequencies.iter().enumerate() {
                let angle = phase * frequency as f64;
                features[self.retained_channels + frequency_index] = angle.sin() as f32;
                features[self.retained_channels + self.frequencies.len() + frequency_index] =
                    angle.cos() as f32;
            }
            self.recurrent
                .step(&features, state)
                .map_err(|error| VoiceError::new(error.to_string()))?;
            let start = frame_index * self.output_channels();
            output.values[start..start + self.output_channels()].copy_from_slice(state.hidden());
        }
        Ok(output)
    }
}
