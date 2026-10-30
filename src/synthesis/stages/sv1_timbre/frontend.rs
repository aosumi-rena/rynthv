mod context;
mod quadrature;
mod language;
mod input;

use super::Contract;

pub(super) use context::{Context, ContextInput};

use crate::synthesis::{
    inference::{Sequence, ops::range_normalizer::RangeNormalizer},
    voice::{
        PhoneSet, PreparedModule, PreparedTree, Result, VoiceError,
        embedded::types::{TIMBRE_FRONTEND_TYPE_ID, TIMBRE_FRONTEND_V2_TYPE_ID},
    },
};

pub(super) struct Frontend<'a> {
    contract: Contract,
    languages: Vec<&'a PhoneSet>,
    unified: &'a PhoneSet,
    linguistic: &'a RangeNormalizer,
    pitch: &'a RangeNormalizer,
    pitch_bounds: Option<[f32; 2]>,
    acoustic: &'a RangeNormalizer,
    width: usize,
    output_width: usize,
    aperiodicity_start: usize,
    pub(super) frame_seconds: f32,
}

pub(super) struct PhoneInput {
    pub normalized: Sequence,
    pub language_ids: Vec<usize>,
    pub noise_ids: Vec<i32>,
    pub silence:Vec<bool>,
}

impl<'a> Frontend<'a> {
    pub fn prepare(tree: &'a PreparedTree, index: usize, contract: Contract) -> Result<Self> {
        let node = tree.node(index)?;
        if !matches!(
            node.type_id,
            TIMBRE_FRONTEND_TYPE_ID | TIMBRE_FRONTEND_V2_TYPE_ID
        ) || node.children.len() != 6
        {
            return Err(VoiceError::new(
                "SV1 timbre frontend requires its six typed children",
            ));
        }
        let payload = tree.payload(index)?;
        if payload.len() != 32 {
            return Err(VoiceError::new(
                "SV1 timbre frontend configuration must contain 32 bytes",
            ));
        }
        let word = |index: usize| {
            u32::from_le_bytes(payload[index * 4..index * 4 + 4].try_into().unwrap())
        };
        let width = word(0) as usize;
        let output_width = word(2) as usize;
        let spectral_start = word(3) as usize;
        let aperiodicity_start = word(4) as usize;
        let frame_seconds = f32::from_bits(word(6));
        if word(5) != 0 || word(7) != 0 {
            return Err(VoiceError::new(
                "SV1 acoustic DCT/projection configuration is unsupported",
            ));
        }
        if width < 2
            || word(1) != 1
            || spectral_start != 2
            || aperiodicity_start <= spectral_start
            || aperiodicity_start.checked_add(5) != Some(output_width)
            || !frame_seconds.is_finite()
            || frame_seconds <= 0.0
        {
            return Err(VoiceError::new(
                "SV1 acoustic frontend configuration is invalid",
            ));
        }
        let languages = tree
            .node(node.children[0])?
            .children
            .iter()
            .map(|&child| phone_set(tree, child))
            .collect::<Result<Vec<_>>>()?;
        let unified = phone_set(tree, node.children[1])?;
        let linguistic = tree.range_normalizer(node.children[3])?;
        let pitch = tree.range_normalizer(node.children[4])?;
        let acoustic = tree.range_normalizer(node.children[5])?;
        if languages.is_empty()
            || unified.symbols.len() >= width
            || linguistic.channels() != width
            || pitch.channels() != 1
            || acoustic.channels() != output_width
        {
            return Err(VoiceError::new(
                "SV1 acoustic frontend tables and dimensions disagree",
            ));
        }

        let pitch_bounds = if contract == Contract::Compatible {
            let bounds = [0.0_f64, 127.0].map(|midi| {
                let hz = (440.0 * ((midi - 69.0) / 12.0).exp2()) as i32;
                f64::from(hz).ln() as f32
            });
            let bounds = [
                pitch.normalize_scalar(0, bounds[0], true).map_err(error)?,
                pitch.normalize_scalar(0, bounds[1], true).map_err(error)?,
            ];
            if bounds[0] > bounds[1] {
                return Err(VoiceError::new("SV1 normalized pitch bounds are reversed"));
            }
            Some(bounds)
        } else {
            None
        };
        Ok(Self {
            contract,
            languages,
            unified,
            linguistic,
            pitch,
            pitch_bounds,
            acoustic,
            width,
            output_width,
            aperiodicity_start,
            frame_seconds,
        })
    }

    pub fn normalize_pitch(&self, log_hz: &[f32]) -> Result<Vec<f32>> {
        if log_hz.iter().any(|value| !value.is_finite()) {
            return Err(VoiceError::new(
                "SV1 acoustic log-Hz contains a non-finite value",
            ));
        }
        let mut output = vec![0.0; log_hz.len()];
        if self.contract == Contract::Native {
            self.pitch.normalize_native(log_hz, &mut output, true)
        } else {
            self.pitch.normalize(log_hz, &mut output, true)
        }.map_err(error)?;
        if let Some(bounds) = self.pitch_bounds {
            for value in &mut output {
                *value = value.clamp(bounds[0], bounds[1]);
            }
        }
        Ok(output)
    }

    pub fn inverse_output(&self, normalized: &Sequence) -> Result<Sequence> {
        normalized.validate()?;
        if normalized.channels != self.output_width
            || normalized.values.iter().any(|value| !value.is_finite())
        {
            return Err(VoiceError::new(
                "SV1 acoustic output dimensions or values are invalid",
            ));
        }
        let mut output = Sequence::zeros(normalized.frames, self.output_width)?;
        self.acoustic
            .denormalize(&normalized.values, &mut output.values)
            .map_err(error)?;
        for row in output.values.chunks_exact_mut(self.output_width) {
            row[0] = if row[0] > 0.5 { 1.0 } else { 0.0 };
            row[1] = row[1].clamp(0.01, 3.0);
            for value in &mut row[self.aperiodicity_start..self.aperiodicity_start + 5] {
                *value = value.clamp(0.0, 1.0);
            }
        }
        Ok(output)
    }
}

fn phone_set(tree: &PreparedTree, index: usize) -> Result<&PhoneSet> {
    match &tree.node(index)?.module {
        PreparedModule::PhoneSet(table) => Ok(table),
        _ => Err(VoiceError::new("SV1 frontend child is not a phone table")),
    }
}

fn error(error: impl std::fmt::Display) -> VoiceError {
    VoiceError::new(error.to_string())
}
