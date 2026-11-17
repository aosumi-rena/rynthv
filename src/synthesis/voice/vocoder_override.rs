mod source;
mod r3;
pub use source::VocoderSource;
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

pub(crate) fn load(source: &VocoderSource) -> Result<Arc<VocoderOverride>> {
    let source = ResolvedSource::resolve(source)?;
    let model = {
        let mut cache = CACHE.lock().map_err(|_| VoiceError::new("VOCODER_OVERRIDE_INVALID"))?;
        if cache.as_ref().is_none_or(|cached| cached.source != source) {
            *cache = Some(Cached { source: source.clone(), model: Arc::new(OnceLock::new()) });
        }
        Arc::clone(&cache.as_ref().expect("initialized cache").model)
    };
    model.get_or_init(|| prepare(source)).clone()
}

fn prepare(source: ResolvedSource) -> Result<Arc<VocoderOverride>> {
    let (tree, identity) = source.prepare()?;
    let expected = if source.r3 { crate::synthesis::stages::vocoder::configuration::ROOT_TYPE }
        else { super::embedded::types::VOCODER_TYPE_ID };
    if tree.node(tree.root_index())?.type_id != expected { return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID")); }
    let tree = Arc::new(tree);
    let stage = VocoderStage::prepare(&tree)?;
    stage.into_stream()?;
    source.validate()?;
    Ok(Arc::new(VocoderOverride { tree, source, identity }))
}
