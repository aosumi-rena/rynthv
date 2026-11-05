mod inputs;

pub use inputs::{SamplerControls, SamplerInputs};

use crate::synthesis::voice::{PreparedTree, Result, VoiceError};

use super::statistics::{SamplerStatistics, control_weight};

pub const SAMPLER_TYPE_ID: [u8; 16] = [
    0x0e, 0x64, 0x34, 0x36, 0xe9, 0xda, 0xce, 0x03, 0x7f, 0x58, 0x55, 0xa1, 0x67, 0xd4, 0x2f, 0x4a,
];
const CONDITIONAL_TYPE_ID: [u8; 16] = [
    0xf4, 0x3c, 0xe8, 0x05, 0xf7, 0x73, 0x0c, 0xd4, 0xa2, 0x3d, 0x9d, 0x7a, 0x02, 0x61, 0xdd, 0x7a,
];

#[derive(Clone, Debug)]
pub struct F0Sampler {
    children: [usize; 13],
    positional_features: usize,
    positional_scale: f32,
}

pub struct SamplerOutput {

    pub normalized_pitch: Vec<f32>,

    pub predicted_controls: SamplerStatistics,
}

impl F0Sampler {
    #[cfg(feature = "acoustic-diagnostic")]
    pub(super) fn child_slot(&self,index:usize)->Option<usize> {self.children.iter().position(|child|*child==index)}

    pub fn prepare(tree: &PreparedTree, index: usize) -> Result<Self> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("f0.sampler.prepare");
        let node = tree.node(index)?;
        let config = tree.payload(index)?;
        if node.type_id != SAMPLER_TYPE_ID || config.len() != 4 {
            return Err(VoiceError::new("Invalid F0 sampler type or configuration"));
        }

        let children: [usize; 13] =
            node.children.as_slice().try_into().map_err(|_| {
                VoiceError::new("Unsupported F0 sampler layout")
            })?;
        for &child in &children[9..11] {
            if tree.node(child)?.type_id != CONDITIONAL_TYPE_ID {
                return Err(VoiceError::new("Unexpected F0 conditional-network type"));
            }
        }
        let positional_features = u16::from_le_bytes([config[0], config[1]]) as usize;
        let positional_scale = u16::from_le_bytes([config[2], config[3]]) as f32;
        if positional_features == 0
            || !positional_features.is_multiple_of(2)
            || positional_scale == 0.0
        {
            return Err(VoiceError::new("Invalid F0 positional encoding dimensions"));
        }
        Ok(Self {
            children,
            positional_features,
            positional_scale,
        })
    }

    pub fn predict(
        &self,
        input: &SamplerInputs<'_>,
        mut execute: impl FnMut(usize, &[f32], Option<&[f32]>, usize) -> Result<Vec<f32>>,
        mut denormalize: impl FnMut(&[f32]) -> Result<Vec<f32>>,
    ) -> Result<SamplerOutput> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("f0.sampler.predict");
        input.validate()?;
        let frames = input.frames;
        let width = input.frame_features;
        if frames == 0 {
            return Ok(SamplerOutput {
                normalized_pitch: Vec::new(),
                predicted_controls: SamplerStatistics::estimate(
                    &[],
                    input.vowel_mask,
                    input.syllables,
                )?,
            });
        }
        let positional = input.positional(self.positional_features, self.positional_scale)?;
        let base_condition = inputs::concatenate(input.frame_condition, &positional, frames)?;
        let initial = execute(self.children[0], input.source_condition, None, frames)?;
        inputs::check_output(&initial, frames, "F0 source projection")?;
        let hidden = {
            #[cfg(feature = "synth-profile")]
            let _profile = crate::synthesis::profiling::span("f0.sampler.initial_pass");
            execute(self.children[9], &initial, Some(&base_condition), frames)?
        };
        inputs::check_frames(&hidden, frames, "F0 initial hidden features")?;
        let preliminary = execute(self.children[11], &hidden, None, frames)?;
        inputs::check_output(&preliminary, frames, "F0 preliminary pitch")?;
        let pitch = denormalize(&preliminary)?;
        inputs::check_output(&pitch, frames, "F0 denormalized preliminary pitch")?;
        let statistics = SamplerStatistics::estimate(&pitch, input.vowel_mask, input.syllables)?;

        let mut embedding = vec![0.0; input.frame_condition.len()];
        let slope: Vec<_> = input
            .controls
            .slope
            .iter()
            .map(|x| missing_to_zero(*x))
            .collect();
        let offset: Vec<_> = input
            .controls
            .offset
            .iter()
            .map(|x| missing_to_zero(*x))
            .collect();
        let slope_weights = inferred_weights(input.controls.slope, &statistics.slope, 10.0);
        let offset_weights = inferred_weights(input.controls.offset, &statistics.offset, 5.0);

        let projected = execute(self.children[4], &slope, None, frames)?;
        accumulate(&mut embedding, &projected, Some(&slope_weights), width)?;
        let projected = execute(self.children[5], &offset, None, frames)?;
        accumulate(&mut embedding, &projected, Some(&offset_weights), width)?;
        let projected = execute(
            self.children[1],
            input.controls.expressiveness,
            None,
            frames,
        )?;
        accumulate(&mut embedding, &projected, None, width)?;
        let projected = execute(self.children[2], input.style_condition, None, frames)?;
        accumulate(
            &mut embedding,
            &projected,
            Some(input.controls.style_weight),
            width,
        )?;
        let projected = execute(self.children[3], input.controls.strength, None, frames)?;
        accumulate(&mut embedding, &projected, None, width)?;
        let projected = execute(
            self.children[6],
            input.controls.absolute_control,
            None,
            frames,
        )?;
        accumulate(
            &mut embedding,
            &projected,
            Some(input.controls.control_weight),
            width,
        )?;
        let projected = execute(self.children[7], input.controls.accent, None, frames)?;
        accumulate(&mut embedding, &projected, None, width)?;

        let projected_input = inputs::concatenate(input.frame_condition, &embedding, frames)?;
        let mut adjusted = execute(self.children[8], &projected_input, None, frames)?;
        inputs::check_output(
            &adjusted,
            input.frame_condition.len(),
            "F0 adjusted condition",
        )?;
        for (value, base) in adjusted.iter_mut().zip(input.frame_condition) {
            *value += base;
        }
        let condition = inputs::concatenate(&adjusted, &positional, frames)?;
        let final_hidden = {
            #[cfg(feature = "synth-profile")]
            let _profile = crate::synthesis::profiling::span("f0.sampler.final_pass");
            execute(self.children[10], &hidden, Some(&condition), frames)?
        };
        inputs::check_frames(&final_hidden, frames, "F0 final hidden features")?;
        let normalized_pitch = execute(self.children[12], &final_hidden, None, frames)?;
        inputs::check_output(&normalized_pitch, frames, "F0 final normalized pitch")?;
        Ok(SamplerOutput {
            normalized_pitch,
            predicted_controls: statistics,
        })
    }
}

fn missing_to_zero(value: f32) -> f32 {
    if value.is_nan() { 0.0 } else { value }
}

fn inferred_weights(controls: &[f32], predictions: &[f32], scale: f32) -> Vec<f32> {
    controls
        .iter()
        .zip(predictions)
        .map(|(&control, &predicted)| {
            control_weight(
                if control.is_nan() {
                    0.0
                } else {
                    control - predicted
                },
                scale,
            )
        })
        .collect()
}

fn accumulate(
    destination: &mut [f32],
    projected: &[f32],
    weights: Option<&[f32]>,
    width: usize,
) -> Result<()> {
    inputs::check_output(projected, destination.len(), "F0 control projection")?;
    for (frame, (destination, projected)) in destination
        .chunks_exact_mut(width)
        .zip(projected.chunks_exact(width))
        .enumerate()
    {
        let weight = weights.map_or(1.0, |weights| weights[frame]);
        for (target, value) in destination.iter_mut().zip(projected) {
            *target += weight * value;
        }
    }
    Ok(())
}
