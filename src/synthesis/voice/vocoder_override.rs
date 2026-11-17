mod source;
pub use source::R2VocoderSource;
use source::ResolvedSource;
use super::{PreparedTree, Result, VoiceError};
use crate::synthesis::stages::vocoder::VocoderStage;
use std::sync::{Arc, Mutex, OnceLock};

pub(crate) struct VocoderOverride {
    pub tree: Arc<PreparedTree>,
    pub identity: String,
    source: ResolvedSource,
}

impl VocoderOverride {
    pub(super) fn validate_current(&self) -> Result<()> { self.source.validate() }
}

struct Cached {
    source: ResolvedSource,
    model: Arc<OnceLock<Result<Arc<VocoderOverride>>>>,
}

static CACHE: Mutex<Option<Cached>> = Mutex::new(None);

pub(crate) fn clear() {
    if let Ok(mut cache) = CACHE.lock() { *cache = None; }
}

pub(crate) fn load(source: &R2VocoderSource) -> Result<Arc<VocoderOverride>> {
    let source = ResolvedSource::resolve(source)?;
    let model = {
        let mut cache = CACHE.lock().map_err(|_| VoiceError::new("R2_VOCODER_INVALID"))?;
        if cache.as_ref().is_none_or(|cached| cached.source != source) {
            *cache = Some(Cached { source: source.clone(), model: Arc::new(OnceLock::new()) });
        }
        Arc::clone(&cache.as_ref().expect("initialized cache").model)
    };
    model.get_or_init(|| prepare(source)).clone()
}

fn prepare(source: ResolvedSource) -> Result<Arc<VocoderOverride>> {
    let bytes = source.read()?;
    if super::embedded::root_type(&bytes)? != super::embedded::types::VOCODER_TYPE_ID {
        return Err(VoiceError::new("R2_VOCODER_INVALID"));
    }
    let tree = Arc::new(PreparedTree::prepare_embedded_execution(&bytes, true)?);
    let stage = VocoderStage::prepare(&tree)?;
    if stage.positional.is_some() { return Err(VoiceError::new("R2_VOCODER_INVALID")); }
    stage.into_stream()?;
    source.validate()?;
    Ok(Arc::new(VocoderOverride { tree, source, identity: blake3::hash(&bytes).to_hex().to_string() }))
}
