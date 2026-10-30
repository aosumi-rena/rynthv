use crate::synthesis::{
    inference::Sequence,
    voice::{PreparedModule, PreparedTree, Result, VoiceError},
};

use super::{
    condition::VocoderCondition,
    configuration::{Configuration, Modules},
    excitation::ExcitationFrame,
    output::FrameMetadata,
    sampling::PositionalGru,
    stream_parameters::ParameterStream,
};

#[derive(Clone)]
pub struct VocoderStage<'a> {
    pub(crate) tree: &'a PreparedTree,
    pub(crate) config: Configuration,
    pub(crate) modules: Modules,
    pub(crate) condition: VocoderCondition,
    pub(crate) positional: Option<PositionalGru>,
    pub(crate) effects:Option<super::effects::EffectTimeline>,
    pub(crate) compatible_dispatch:bool,
    pub(crate) native_dds: bool,
    pub(crate) independent_branches:bool,
    pub(crate) streamed_frame_noise:bool,
    pub(crate) editor_seed_source:Option<std::sync::Arc<[u64]>>,
}

pub struct VocoderParameters {
    pub normalized: Sequence,
    pub spectra: [Sequence; 3],
    pub harmonic_parameters: Sequence,
    pub excitation: Vec<ExcitationFrame>,
    pub metadata: Vec<FrameMetadata>,
    pub effects:Vec<Option<super::effects::FrameEffect>>,
}

impl<'a> VocoderStage<'a> {
    pub fn prepare(tree: &'a PreparedTree) -> Result<Self> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("vocoder.prepare");
        let root = tree.root_index();
        let config = Configuration::parse(&tree.payload(root)?)?;
        let modules = Modules::prepare(tree, root, config)?;
        let condition = VocoderCondition::prepare(tree, modules.conditions)?;
        if config.bands != 8 || modules.input_projection.is_none() {
            return Err(VoiceError::new("Unsupported vocoder source variant"));
        }
        let main = match &tree.node(modules.conditioned_network)?.module {
            PreparedModule::Conditional(config) => *config,
            _ => {
                return Err(VoiceError::new(
                    "Vocoder main network must be a prepared conditional stack",
                ));
            }
        };
        let positional = modules
            .recurrent_sampler
            .map(|sampler| PositionalGru::prepare(tree, sampler))
            .transpose()?;

        let condition_channels = match &positional {
            None => 0,
            Some(positional) if positional.output_channels() == 192 => 192,
            Some(_) => {
                return Err(VoiceError::new(
                    "Unsupported vocoder positional condition width",
                ));
            }
        };
        if main.input_channels != 72
            || main.output_channels != 256
            || main.condition_channels != condition_channels
            || condition.frame_samples != 240
        {
            return Err(VoiceError::new("Unexpected vocoder condition dimensions"));
        }
        Ok(Self {
            tree,
            config,
            modules,
            condition,
            positional,
            effects:None,
            compatible_dispatch:false,
            native_dds: false,
            independent_branches:false,
            streamed_frame_noise:false,
            editor_seed_source:None,
        })
    }

    pub fn set_editor_controls(&mut self,phrase:&crate::synthesis::score::PreparedPhrase,origin:f64) {
        self.effects=Some(super::effects::EffectTimeline::new(phrase,origin));
    }

    pub fn set_compatible_dispatch(&mut self) {self.compatible_dispatch=true;self.independent_branches=true;}
    pub fn set_native_dds(&mut self) {
        self.native_dds = true;
        self.independent_branches = true;
    }
    pub fn set_editor_seed_source(&mut self,seeds:std::sync::Arc<[u64]>) {
        self.editor_seed_source=Some(seeds);self.streamed_frame_noise=true;self.independent_branches=true;
    }

    pub fn parameters(
        &self,
        physical: &Sequence,
        controls: Option<&Sequence>,
    ) -> Result<VocoderParameters> {
        ParameterStream::new(self.clone())?.process(physical, controls, true)
    }
}
