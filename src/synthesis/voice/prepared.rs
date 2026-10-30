mod affine;
mod convolution;
mod duration;
mod factory;
mod phones;
mod source;
mod summary;
mod transposed_convolution;

pub use affine::PreparedAffine;
pub use convolution::PreparedConvolution;
pub use duration::DurationInputLayout;
pub use summary::*;
pub use transposed_convolution::PreparedTransposedConvolution;

use std::{collections::HashMap, sync::Arc};

use crate::synthesis::inference::{
    ModelRuntime, Sequence, UnaryOperator,
    affine::{AFFINE_TYPE_ID, Affine},
    convolution::{CONVOLUTION_TYPE_ID, TRANSPOSED_CONVOLUTION_TYPE_ID},
    ops::{
        attention::MultiHeadAttention,
        attention_relative::{AttentionOptions, RelativeAttention},
        bidirectional::BidirectionalGru,
        conditional::ConditionalConfig,
        embedding::Embedding,
        gated::GatedConfig,
        normalization::{ChannelAffine, LayerNorm},
        rms_normalization::RmsNorm,
        parameterized::LeakyRelu,
        range_normalizer::RangeNormalizer,
        standard_normalizer::StandardNormalizer,
    },
    pooling::PoolConfig,
    recurrent::Gru,
};

use super::{
    DURATION_ROOT_TYPE_ID, DurationConfiguration, Result, VoiceError,
    archive::Archive,
    array, hex,
    phones::{PHONE_SET_TYPE_ID, PhoneSet},
    tensor::{self, TensorLayout, TensorStorage},
    tree::ModuleTree,
};

pub struct PreparedTree {
    legacy_numeric: bool,
    nodes: Vec<PreparedNode>,
    root: usize,
    duration_input: Option<DurationInputLayout>,

    source: source::ModelSource,
}

pub struct PreparedNode {
    pub resource_index: usize,
    pub type_id: [u8; 16],
    pub children: Vec<usize>,
    pub payload_bytes: usize,
    pub module: PreparedModule,
}

pub enum PreparedModule {
    Tensor(Arc<TensorStorage>),
    Unary(UnaryOperator),
    LeakyRelu(LeakyRelu),
    Affine(PreparedAffine),
    PhoneSet(PhoneSet),
    Convolution(PreparedConvolution),
    TransposedConvolution(PreparedTransposedConvolution),
    Gru(Gru),
    BidirectionalGru(Box<BidirectionalGru>),
    Pool(PoolConfig),
    Sequential,
    ModuleList,
    Embedding(Embedding),
    LayerNorm(LayerNorm),
    RmsNorm(RmsNorm),
    ChannelAffine(ChannelAffine),
    RangeNormalizer(RangeNormalizer),
    StandardNormalizer(StandardNormalizer),
    RelativeAttention(RelativeAttention),
    Attention(MultiHeadAttention),
    Conditional(ConditionalConfig),
    Gated(GatedConfig),
    SamplerNetwork,
    Unsupported,
}

impl PreparedTree {
    pub(super) fn prepare_execution(archive:Arc<Archive>,source:&ModuleTree,legacy_numeric:bool)->Result<Self> {
        let records = source
            .nodes
            .iter()
            .map(|node| {
                let index = archive
                    .index(node.resource)
                    .ok_or_else(|| VoiceError::new("Prepared node resource is absent"))?;
                Ok((
                    index,
                    node.children.iter().map(|child| child.index()).collect(),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Self::prepare_source(
            source::ModelSource::Indexed(archive),
            records,
            source.root.index(),
            legacy_numeric,
        )
    }

    pub(crate) fn prepare_embedded(bytes: &[u8]) -> Result<Self> {
        Self::prepare_embedded_execution(bytes,false)
    }

    pub(super) fn prepare_embedded_execution(bytes:&[u8],legacy_numeric:bool)->Result<Self> {
        let records = super::embedded::parse(bytes)?;
        Self::prepare_records(records,legacy_numeric)
    }

    pub(super) fn prepare_records(records:Vec<super::embedded::Record>,legacy_numeric:bool)->Result<Self> {
        if records.is_empty() {return Err(VoiceError::new("Model has no records"));}
        let root = records.len() - 1;
        let mut nodes = Vec::with_capacity(records.len());
        let mut resources = Vec::with_capacity(records.len());
        for (index, record) in records.into_iter().enumerate() {
            nodes.push((index, record.children));
            resources.push(record.bytes);
        }
        Self::prepare_source(source::ModelSource::Inline(resources), nodes, root,legacy_numeric)
    }

    fn prepare_source(
        source: source::ModelSource,
        records: Vec<(usize, Vec<usize>)>,
        root: usize,
        legacy_numeric: bool,
    ) -> Result<Self> {
        if root >= records.len() {
            return Err(VoiceError::new("Prepared root is outside its module arena"));
        }
        let mut tensors = HashMap::<usize, Arc<TensorStorage>>::new();
        let mut nodes = Vec::with_capacity(records.len());
        for (resource_index, children) in records {
            let bytes = source.read(resource_index)?;
            let type_id = array(&bytes, 0)?;
            if children.iter().any(|index| *index >= nodes.len()) {
                return Err(VoiceError::new("Module arena is not in checked postorder"));
            }
            let payload_bytes = bytes.len() - 16;
            let module = if tensor::is_known(&bytes) {
                require_leaf(&children, "Tensor")?;
                let storage = match tensors.get(&resource_index) {
                    Some(storage) => Arc::clone(storage),
                    None => {
                        let mut storage=tensor::load(&bytes)?;
                        if let TensorStorage::QuantizedMatrix{legacy_integer_execution,..}=&mut storage {
                            *legacy_integer_execution=legacy_numeric;
                        }
                        let storage = Arc::new(storage);
                        tensors.insert(resource_index, Arc::clone(&storage));
                        storage
                    }
                };
                PreparedModule::Tensor(storage)
            } else if type_id == PHONE_SET_TYPE_ID {
                require_leaf(&children, "Phone set")?;
                PreparedModule::PhoneSet(PhoneSet::parse(&bytes)?)
            } else if let Ok(operator) = UnaryOperator::from_type_id(type_id) {
                require_empty_payload(payload_bytes)?;
                require_leaf(&children, "Unary operator")?;
                PreparedModule::Unary(operator)
            } else if type_id == AFFINE_TYPE_ID {
                require_empty_payload(payload_bytes)?;
                PreparedModule::Affine(PreparedAffine::prepare(&nodes, &children)?)
            } else if type_id == CONVOLUTION_TYPE_ID {
                PreparedModule::Convolution(PreparedConvolution::prepare(
                    &bytes[16..],
                    &nodes,
                    &children,
                )?)
            } else if type_id == TRANSPOSED_CONVOLUTION_TYPE_ID {
                PreparedModule::TransposedConvolution(PreparedTransposedConvolution::prepare(
                    &bytes[16..],
                    &nodes,
                    &children,
                )?)
            } else {
                factory::prepare_operator(type_id, &bytes[16..], &nodes, &children,legacy_numeric)?
                    .unwrap_or(PreparedModule::Unsupported)
            };
            nodes.push(PreparedNode {
                resource_index,
                type_id,
                children,
                payload_bytes,
                module,
            });
        }
        let duration_input = if nodes[root].type_id == DURATION_ROOT_TYPE_ID {
            let bytes = source.read(nodes[root].resource_index)?;
            Some(DurationInputLayout::prepare(
                DurationConfiguration::parse(&bytes)?,
                &nodes[root],
                &nodes,
            )?)
        } else {
            None
        };
        Ok(Self {
            legacy_numeric,
            nodes,
            root,
            duration_input,
            source,
        })
    }

    pub fn nodes(&self) -> &[PreparedNode] {
        &self.nodes
    }

    pub(crate) fn legacy_numeric(&self)->bool {self.legacy_numeric}

    pub fn root_index(&self) -> usize {
        self.root
    }

    pub fn node(&self, index: usize) -> Result<&PreparedNode> {
        self.nodes
            .get(index)
            .ok_or_else(|| VoiceError::new("Model node index is out of range"))
    }

    pub fn resource_bytes(&self, index: usize) -> Result<std::borrow::Cow<'_, [u8]>> {
        self.source.read(self.node(index)?.resource_index)
    }

    pub fn payload(&self, index: usize) -> Result<std::borrow::Cow<'_, [u8]>> {
        use std::borrow::Cow;
        Ok(match self.resource_bytes(index)? {
            Cow::Borrowed(bytes) => Cow::Borrowed(&bytes[16..]),
            Cow::Owned(bytes) => Cow::Owned(bytes[16..].to_vec()),
        })
    }

    pub fn tensor(&self, index: usize) -> Result<&TensorStorage> {
        match &self.node(index)?.module {
            PreparedModule::Tensor(tensor) => Ok(tensor),
            _ => Err(VoiceError::new("Expected a tensor storage node")),
        }
    }

    pub fn affine(&self, index: usize) -> Result<&Affine> {
        match &self.node(index)?.module {
            PreparedModule::Affine(layer) => Ok(layer.operator()),
            _ => Err(VoiceError::new("Expected an affine node")),
        }
    }

    pub fn range_normalizer(&self, index: usize) -> Result<&RangeNormalizer> {
        match &self.node(index)?.module {
            PreparedModule::RangeNormalizer(layer) => Ok(layer),
            _ => Err(VoiceError::new("Expected a range normalizer node")),
        }
    }

    pub fn standard_normalizer(&self, index: usize) -> Result<&StandardNormalizer> {
        match &self.node(index)?.module {
            PreparedModule::StandardNormalizer(layer) => Ok(layer),
            _ => Err(VoiceError::new("Expected a standard normalizer node")),
        }
    }

    pub fn attention(&self, index: usize) -> Result<&MultiHeadAttention> {
        match &self.node(index)?.module {
            PreparedModule::Attention(layer) => Ok(layer),
            _ => Err(VoiceError::new("Expected an attention node")),
        }
    }

    pub fn execute_attention(
        &self,
        index: usize,
        queries: &[f32],
        keys: &[f32],
        values: &[f32],
        options: AttentionOptions<'_>,
    ) -> Result<Vec<f32>> {
        self.attention(index)?
            .execute(queries, keys, values, options)
            .map_err(|error| VoiceError::new(error.to_string()))
    }

    pub fn execute_node(&self, index: usize, input: &[f32]) -> Result<Vec<f32>> {
        let mut runtime = ModelRuntime::new(self);
        let channels = match runtime.input_channels(index)? {
            Some(channels) => channels,
            None if runtime.flat_elementwise(index)? => 1,
            None => {
                return Err(VoiceError::new(format!(
                    "Model node {index} type {} needs an explicitly shaped Sequence",
                    hex(&self.node(index)?.type_id)
                )));
            }
        };
        let input = Sequence::from_values(channels, input.to_vec())?;
        Ok(runtime.execute(index, &input, true)?.values)
    }
}

fn require_empty_payload(bytes: usize) -> Result<()> {
    if bytes != 0 {
        return Err(VoiceError::new(
            "Recovered parameterless module contains a payload",
        ));
    }
    Ok(())
}

fn require_leaf(children: &[usize], kind: &str) -> Result<()> {
    if !children.is_empty() {
        return Err(VoiceError::new(format!(
            "{kind} unexpectedly contains child modules"
        )));
    }
    Ok(())
}
