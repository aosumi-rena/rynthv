mod decoder;
mod frontend;
mod noise;
mod pitch;
mod prepared;
mod queue;
mod records;
mod sampling;
mod silence;
mod stream;

use std::sync::Arc;

pub use prepared::PreparedSv1TimbreFrames;
pub use stream::Sv1TimbreStream;

use crate::synthesis::{
    stages::{
        f0::PitchResult,
        timbre::{TimbreOutput, TimbreRequest, TimbreStyles},
    },
    voice::{PreparedTree, Result, VoiceError, embedded::types},
};
use frontend::{Context, Frontend};
use sampling::Sampler;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Contract {
    Native,
    Compatible,
}

pub struct Sv1TimbreNetwork<'a> {
    contract: Contract,
    tree: &'a PreparedTree,
    frontend: Frontend<'a>,
    context: Context<'a>,
    sampler: Sampler,
    bank: Vec<f32>,
    styles: TimbreStyles,
    style_node:usize,
    decoder: [usize; 4],
}

impl<'a> Sv1TimbreNetwork<'a> {
    pub fn prepare(tree: &'a PreparedTree) -> Result<Self> {
        Self::prepare_contract(tree, Contract::Compatible)
    }

    pub fn prepare_native(tree: &'a PreparedTree) -> Result<Self> {
        Self::prepare_contract(tree, Contract::Native)
    }

    pub fn prepare_native_with_language_precision(tree: &'a PreparedTree, half: bool) -> Result<Self> {
        Self::prepare_execution(tree, Contract::Native, half)
    }

    fn prepare_contract(tree: &'a PreparedTree, contract: Contract) -> Result<Self> {
        Self::prepare_execution(tree, contract, false)
    }

    fn prepare_execution(tree: &'a PreparedTree, contract: Contract, half_language: bool) -> Result<Self> {
        let root = tree.node(tree.root_index())?;
        if root.type_id != types::TIMBRE_TYPE_ID
            || !tree.payload(tree.root_index())?.is_empty()
            || root.children.len() != 2
        {
            return Err(VoiceError::new(
                "Not a supported version-1 compatible timbre root",
            ));
        }
        let program = tree.node(root.children[0])?;
        if program.type_id != types::TIMBRE_SAMPLER_TYPE_ID
            || !tree.payload(root.children[0])?.is_empty()
        {
            return Err(VoiceError::new("Unsupported compatible acoustic program"));
        }
        let nodes: [usize; 8] =
            program.children.as_slice().try_into().map_err(|_| {
                VoiceError::new("Compatible acoustic program requires eight children")
            })?;
        let bank = tree
            .tensor(nodes[1])?
            .vector()
            .ok_or_else(|| VoiceError::new("Compatible acoustic bank is not a vector"))?
            .to_vec();
        Ok(Self {
            contract,
            tree,
            frontend: Frontend::prepare(tree, nodes[0], contract)?,
            context: Context::prepare(tree, nodes[2], contract == Contract::Native, half_language)?,
            sampler: Sampler::prepare(tree, nodes[3])?,
            bank,
            styles: TimbreStyles::prepare(tree, root.children[1])?,
            style_node:root.children[1],
            decoder: nodes[4..].try_into().unwrap(),
        })
    }

    pub fn prepare_frame_features(
        &self,
        request: &TimbreRequest<'_>,
        pitch: &PitchResult,
    ) -> Result<PreparedSv1TimbreFrames> {
        PreparedSv1TimbreFrames::prepare(self, request, pitch)
    }

    pub fn start_prepared<'network>(
        &'network self,
        request: TimbreRequest<'_>,
        frames: Arc<PreparedSv1TimbreFrames>,
    ) -> Result<Sv1TimbreStream<'network, 'a>> {
        Sv1TimbreStream::new(self, &request, frames)
    }

    pub fn render(&self, request: &TimbreRequest<'_>, pitch: &PitchResult) -> Result<TimbreOutput> {
        let frames = Arc::new(self.prepare_frame_features(request, pitch)?);
        Sv1TimbreStream::new(self, request, frames)?.collect()
    }
}
