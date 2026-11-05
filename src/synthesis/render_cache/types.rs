use super::super::{
    audio::RenderedAudio,
    plan::{Coverage, OwnerId, OwnerInput, OwnerOutputControls, OwnerRequest},
    rendered::{RenderVisuals, RenderedClip},
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug)]
pub struct CompletedOwner {
    pub input: Arc<OwnerInput>,
    pub clip: Arc<RenderedClip>,
}

#[derive(Clone, Debug)]
pub struct RenderJob {
    pub request: Arc<OwnerRequest>,

    pub base: Option<Arc<CompletedOwner>>,
    pub(super) generation: u64,
    pub(super) serial: u64,
    pub(super) cancelled: Arc<AtomicBool>,
}

impl RenderJob {
    pub(crate) fn cancellation(&self) -> super::super::execution::Cancellation {
        super::super::execution::Cancellation::from_flag(Arc::clone(&self.cancelled))
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn cancellation_flag(&self) -> &AtomicBool { &self.cancelled }

    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

#[derive(Clone, Debug)]
pub struct PublishedOwner {
    pub owner: OwnerId,
    pub coverage: Coverage,
    pub controls: OwnerOutputControls,
    pub audio: Arc<RenderedAudio>,
    pub visuals: Arc<RenderVisuals>,
}

#[derive(Default, Debug)]
pub struct CacheUpdate {

    pub withdrawn: BTreeMap<OwnerId, Coverage>,
    pub publication_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completion {

    Published,
    Failed(String),

    Discarded,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct CacheStats {
    pub queued: usize,
    pub running: usize,
    pub ready: usize,
    pub failed: usize,

    pub current_render_bytes: usize,
    pub discarded_completions: u64,
}
