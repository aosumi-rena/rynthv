mod attention;
pub(crate) mod context;
pub(crate) mod encoding;
pub mod frontend;
mod heads;
mod sampling;
#[cfg(feature = "acoustic-diagnostic")]
pub mod diagnostic;

use crate::synthesis::voice::{
    DURATION_ROOT_TYPE_ID, DurationConfiguration, PreparedModule, PreparedTree, Result, VoiceError,
};

use serde::Serialize;

pub use frontend::{DurationNote, DurationRecord, ExplicitPhone, PhoneDuration, PhoneFeatures};

pub(crate) fn conditioning(tree: &PreparedTree, node: usize, speaker: Option<usize>) -> Result<&[f32]> {
    let selected = match (speaker, tree.tensor(node)?) {
        (None, crate::synthesis::voice::TensorStorage::FloatMatrix { .. }) => Some(0),
        (selected, _) => selected,
    };
    crate::synthesis::voice::speakers::vector(tree, node, selected)
        .map_err(|error| VoiceError::new(format!("Duration model: {error}")))
}

pub struct DurationStage<'a> {
    tree: &'a PreparedTree,
    children: [usize; 14],
    configuration: DurationConfiguration,
    identity_bias: Vec<f32>,
    speaker: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct DurationPrediction {
    pub phones: Vec<PhoneFeatures>,
    pub scalars: Vec<f32>,

    pub impulse_values: Vec<f64>,

    pub impulse_positions: Vec<f32>,
}

#[derive(Debug, Serialize)]
pub struct DurationResult {
    pub durations: Vec<PhoneDuration>,

    pub natural_frames: Vec<usize>,
    pub prediction: DurationPrediction,
}

impl<'a> DurationStage<'a> {

    pub fn prepare(tree: &'a PreparedTree, identity: u64, speaker: Option<usize>) -> Result<Self> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("duration.prepare");
        let root = tree.node(tree.root_index())?;
        if root.type_id != DURATION_ROOT_TYPE_ID {
            return Err(VoiceError::new(
                "The supplied tree is not the local duration model",
            ));
        }
        let children: [usize; 14] = root.children.as_slice().try_into().map_err(|_| {
            VoiceError::new("Duration model requires its fourteen observed children")
        })?;
        let configuration = DurationConfiguration::parse(&tree.resource_bytes(tree.root_index())?)?;
        let PreparedModule::Affine(input) = &tree.node(children[5])?.module else {
            return Err(VoiceError::new("Duration input projection is not prepared"));
        };
        let constant_width = conditioning(tree, children[1], speaker)?.len();
        configuration.validate_input_projection(
            input.output_features,
            input.input_features,
            constant_width,
        )?;

        let identity_bits: Vec<f32> = (0..64)
            .map(|index| ((identity >> (63 - index)) & 1) as f32)
            .collect();
        let identity_bias = tree.execute_node(children[6], &identity_bits)?;
        if identity_bias.len() != input.output_features {
            return Err(VoiceError::new(
                "Duration identity conditioning width differs from its projection",
            ));
        }
        Ok(Self {
            tree,
            children,
            configuration,
            identity_bias,
            speaker,
        })
    }

    pub fn predict_explicit(&self, records: &[DurationRecord]) -> Result<DurationResult> {
        let phones = frontend::prepare_explicit(self.tree, records)?;
        let prediction = self.predict(phones)?;
        let durations = frontend::postprocess(self.tree, records, &prediction.scalars)?;
        Ok(DurationResult {
            natural_frames: durations.iter().map(|phone|phone.frames).collect(),
            durations,
            prediction,
        })
    }

    pub fn predict_explicit_seeded(
        &self,
        records: &[DurationRecord],
        seeds: &[u32],
    ) -> Result<DurationResult> {
        if records.len() != seeds.len() {
            return Err(VoiceError::new(
                "Duration seed count differs from the explicit record count",
            ));
        }
        let phones = frontend::prepare_explicit(self.tree, records)?;
        let prediction = self.predict_with_seeds(phones, Some(seeds))?;
        let durations = frontend::postprocess(self.tree, records, &prediction.scalars)?;
        Ok(DurationResult {
            natural_frames: durations.iter().map(|phone|phone.frames).collect(),
            durations,
            prediction,
        })
    }

    pub fn predict(&self, phones: Vec<PhoneFeatures>) -> Result<DurationPrediction> {
        self.predict_with_seeds(phones, None)
    }

    fn predict_with_seeds(
        &self,
        phones: Vec<PhoneFeatures>,
        seeds: Option<&[u32]>,
    ) -> Result<DurationPrediction> {
        if phones.is_empty() {
            return Ok(DurationPrediction {
                phones,
                scalars: Vec::new(),
                impulse_values: Vec::new(),
                impulse_positions: Vec::new(),
            });
        }
        let constant = conditioning(self.tree, self.children[1], self.speaker)?;
        let features = encoding::encode(
            self.tree,
            &self.children,
            self.configuration,
            &phones,
            constant,
        )?;
        let width = self.identity_bias.len();
        #[cfg(feature = "acoustic-diagnostic")]
        diagnostic::capture("input_features", phones.len(), &features)?;
        let mut encoded = self.tree.execute_node(self.children[5], &features)?;
        #[cfg(feature = "acoustic-diagnostic")]
        diagnostic::capture("input_projection", phones.len(), &encoded)?;
        if encoded.len() != phones.len() * width {
            return Err(VoiceError::new("Duration projection frame count changed"));
        }
        for frame in encoded.chunks_exact_mut(width) {
            for (value, bias) in frame.iter_mut().zip(&self.identity_bias) {
                *value += bias;
            }
        }
        #[cfg(feature = "acoustic-diagnostic")]
        diagnostic::capture("conditioned_projection", phones.len(), &encoded)?;
        for node in &self.tree.node(self.children[7])?.children {
            encoded = context::encode(self.tree, *node, &encoded, width)?;
            #[cfg(feature = "acoustic-diagnostic")]
            diagnostic::capture(&format!("context_node_{node}"), phones.len(), &encoded)?;
        }
        #[cfg(feature = "acoustic-diagnostic")]
        diagnostic::capture("context_to_scalar_head", phones.len(), &encoded)?;
        let scalars =
            sampling::predict(self.tree, &self.children, &encoded, width, &phones, seeds)?;
        if scalars.len() != phones.len() || scalars.iter().any(|v| !v.is_finite()) {
            return Err(VoiceError::new(
                "Duration prediction is not one finite scalar per phone",
            ));
        }

        let mut head_input = self.tree.execute_node(self.children[11], &scalars)?;
        if head_input.len() != encoded.len() {
            return Err(VoiceError::new(
                "Duration scalar projection width differs from context",
            ));
        }
        for (value, context) in head_input.iter_mut().zip(encoded) {
            *value += context;
        }
        let boundary = self.tree.execute_node(self.children[12], &head_input)?;
        let transition = self.tree.execute_node(self.children[13], &head_input)?;
        let impulse_values = heads::maximum_index(
            &boundary,
            self.configuration.unassigned_words[1] as usize,
            phones.len(),
        )?;
        let impulse_positions = heads::expected_index(
            &transition,
            self.configuration.unassigned_words[2] as usize,
            phones.len(),
        )?;
        Ok(DurationPrediction {
            phones,
            scalars,
            impulse_values,
            impulse_positions,
        })
    }
}
