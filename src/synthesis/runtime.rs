mod context;
mod control;
mod events;
mod export;
mod library;
mod metronome;
mod mixer;
mod playback;
mod recordings;
mod routing;
mod transport;
mod visuals;
mod voices;
mod worker;
#[cfg(feature="acoustic-diagnostic")]
pub use worker::scheduling_diagnostic;

use super::{
    plan::{OwnerId, Planner},
    render_cache::{CacheStats, PublishedOwner},
    rendered::VisualTimeline,
    settings::VoiceSelection,
    voice::VoiceInventory,
};
pub use context::{ContextImport, ContextImportDialog, ContextImportPhase, ContextImportSource};
pub use export::{ExportRequest, export_directory};
pub use library::VoiceLibrary;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::mpsc::{Receiver, TryRecvError},
    time::Duration,
};

pub enum VoiceState {
    Unloaded,
    Loading,
    Loaded(Box<VoiceInventory>),
    Failed(String),
}

pub struct Runtime {
    pub library: VoiceLibrary,
    pub context_import: ContextImport,
    pub planner: Planner,
    pub stats: CacheStats,
    pub progress: Option<String>,
    pub error: Option<String>,
    pub failures: Vec<(OwnerId, String)>,
    pub export_result: Option<Result<Vec<PathBuf>, String>>,
    pub instrumentals: super::instrumental::AudioLibrary,
    pub recording_error: Option<String>,
    voices: BTreeMap<VoiceSelection, voices::VoiceSlot>,
    routes: BTreeMap<OwnerId, VoiceSelection>,
    binding_errors: BTreeMap<OwnerId, String>,
    pending_voices: BTreeSet<OwnerId>,
    project_error: Option<String>,
    recordings: mixer::RecordingMix,
    last_audio_generation: u64,
    metronome: metronome::MetronomeState,
    playback: playback::Playback,
    export_owners: Vec<PublishedOwner>,
    visuals: BTreeMap<OwnerId, VisualTimeline>,
    phonemes: BTreeMap<OwnerId, crate::synthesis::rendered::PhonemeSnapshot>,
    export: Option<Receiver<Result<Vec<PathBuf>, String>>>,
    last_revision: Option<u64>,
    request_serial: u64,
    desired_playing: bool,
    desired_seconds: f64,
    loop_range: Option<(f64, f64)>,
    demand_block: Option<u64>,
    solo_tracks: BTreeSet<u64>,
    repaint: eframe::egui::Context,
}

impl Runtime {
    pub fn new(repaint: eframe::egui::Context) -> Self {
        Self {
            library: VoiceLibrary::new(repaint.clone()),
            context_import: ContextImport::default(),
            planner: Planner::default(),
            stats: CacheStats::default(),
            progress: None,
            error: None,
            failures: Vec::new(),
            export_result: None,
            instrumentals: super::instrumental::AudioLibrary::default(),
            recording_error: None,
            voices: BTreeMap::new(),
            routes: BTreeMap::new(),
            binding_errors: BTreeMap::new(),
            pending_voices: BTreeSet::new(),
            project_error: None,
            recordings: mixer::RecordingMix::default(),
            last_audio_generation: 0,
            metronome: metronome::MetronomeState::default(),
            playback: playback::Playback::new(repaint.clone()),
            export_owners: Vec::new(),
            visuals: BTreeMap::new(),
            phonemes: BTreeMap::new(),
            export: None,
            last_revision: None,
            request_serial: 0,
            desired_playing: false,
            desired_seconds: 0.0,
            loop_range: None,
            demand_block: None,
            solo_tracks: BTreeSet::new(),
            repaint,
        }
    }

    pub fn poll(&mut self) {
        self.instrumentals.poll();
        self.poll_library();
        self.poll_context_import();
        self.poll_voices();
        if let Some(rx) = &self.export {
            match rx.try_recv() {
                Ok(result) => {
                    self.export_result = Some(result);
                    self.export = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.export_result = Some(Err("音频导出线程意外终止。".into()));
                    self.export = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    pub fn voice_state(&self, key: &VoiceSelection) -> &VoiceState {
        self.voices
            .get(key)
            .map_or(&VoiceState::Unloaded, |v| &v.state)
    }
    pub fn inventory(&self, key: &VoiceSelection) -> Option<&VoiceInventory> {
        match self.voice_state(key) {
            VoiceState::Loaded(v) => Some(v),
            _ => None,
        }
    }
    pub fn voice_error(&self, key: &VoiceSelection) -> Option<&str> {
        self.voices.get(key).and_then(|v| match &v.state {
            VoiceState::Failed(error) => Some(error.as_str()),
            _ => v.error.as_deref(),
        })
    }
    pub fn load_elapsed(&self, key: &VoiceSelection) -> Option<Duration> {
        self.voices.get(key).and_then(|v| v.elapsed)
    }
    pub fn reload_voice(&mut self, key: &VoiceSelection) {
        self.voices.remove(key);
        self.last_revision = None;
        self.export_owners
            .retain(|owner| self.routes.get(&owner.owner.reference()) != Some(key));
        self.refresh_playback();
    }
    pub fn reset_project(&mut self) {
        self.audition_key(None);
        self.clear_voices();
        self.instrumentals = super::instrumental::AudioLibrary::default();
        self.recordings = mixer::RecordingMix::default();
        self.recording_error = None;
        self.planner.reset();
        self.routes.clear();
        self.binding_errors.clear();
        self.pending_voices.clear();
        self.project_error = None;
        self.solo_tracks.clear();
        self.visuals.clear();
        self.phonemes.clear();
        self.desired_playing = false;
        self.desired_seconds = 0.0;
        self.loop_range = None;
        let _ = self.playback.set_loop_range(None);
        self.playback.clear();
    }
    pub fn retry(&mut self) {
        let mut reload = Vec::new();
        for (key, voice) in &mut self.voices {
            if let Some(worker) = voice.worker.as_ref().filter(|_| voice.ready) {
                voice.error = None;
                voice.waiting = worker.send(worker::Command::Retry);
            } else {
                reload.push(key.clone());
            }
        }
        for key in reload {
            self.reload_voice(&key);
        }
        self.last_revision = None;
        self.aggregate_voices();
    }
    pub fn playback_error(&self) -> Option<String> {
        self.recording_error
            .clone()
            .or_else(|| self.playback.snapshot().error)
    }
    pub fn audition_key(&self, note: Option<u8>) {
        self.playback.audition_key(note);
    }
    pub fn is_loading(&self) -> bool {
        self.voices
            .values()
            .any(|v| matches!(v.state, VoiceState::Loading))
    }
    pub fn is_busy(&self) -> bool {
        self.library.is_scanning()
            || self.export.is_some()
            || self.voices.values().any(|v| {
                v.waiting
                    || (v.worker.is_some() && !v.ready)
                    || v.stats.running > 0
                    || v.stats.queued > 0
            })
    }
    pub fn has_audio(&self) -> bool {
        self.voices.values().any(|v| v.audio.is_some()) || !self.recordings.layers.is_empty()
    }
    pub fn is_synthesizing(&self) -> bool {
        self.voices.values().any(|voice|voice.ready && (voice.waiting || voice.stats.running>0 || voice.stats.queued>0))
    }
    pub fn visuals(&self, owner: OwnerId) -> impl Iterator<Item = &VisualTimeline> {
        self.visuals.iter().filter_map(move |(id,visuals)| (id.0==owner.0).then_some(visuals))
    }
    pub fn phonemes(&self, owner: OwnerId) -> impl Iterator<Item = &crate::synthesis::rendered::PhonemeSnapshot> {
        self.phonemes.iter().filter_map(move |(id,phones)| (id.0==owner.0).then_some(phones))
    }
    pub fn is_exporting(&self) -> bool {
        self.export.is_some()
    }
    pub fn audio_duration(&self) -> Option<f64> {
        let seconds = self
            .voices
            .values()
            .filter_map(|v| v.audio.as_ref())
            .map(|a| a.duration_seconds())
            .fold(self.recordings.end as f64 / 48_000.0, f64::max);
        (seconds > 0.0).then_some(seconds)
    }
    pub fn status(&self) -> &'static str {
        if self.recording_error.is_some() {
            "伴奏混音失败 · 查看渲染面板"
        } else if !self.recordings.pending.is_empty() {
            "伴奏未就绪 · 查看伴奏轨"
        } else if self.library.is_scanning() {
            "正在扫描声库目录"
        } else if self.is_loading() {
            "正在加载声库"
        } else if self.voices.values().any(|voice| voice.context_required) {
            "歌声需要导入模型上下文"
        } else if self.desired_playing && self.playback.snapshot().buffering {
            "正在准备播放位置的音频"
        } else if self.stats.running > 0 || self.stats.queued > 0 {
            "正在合成歌声"
        } else if self.error.is_some() || self.stats.failed > 0 {
            "歌声合成失败 · 查看渲染面板"
        } else if self.has_audio() {
            "音频已就绪"
        } else if self.voices.values().any(|v| v.ready) {
            "等待音符"
        } else if self
            .voices
            .values()
            .any(|v| matches!(v.state, VoiceState::Loaded(_)))
        {
            "正在准备合成环境"
        } else {
            "音轨尚未指定歌声"
        }
    }
}
