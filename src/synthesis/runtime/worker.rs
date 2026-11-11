mod publication;
mod runner;
mod scheduling;
mod state;
mod visuals;
mod standard;
#[cfg(feature="acoustic-diagnostic")]
pub use scheduling::diagnostic as scheduling_diagnostic;

use super::{
    VoiceState,
    control::Control,
    events::{Events, WakeOnExit},
};
use crate::synthesis::{
    audio::{LiveAudio, RenderedAudio},
    plan::{OwnerId, RenderPlan},
    render_cache::{CacheStats, PublishedOwner},
    rendered::{VisualPatch, VisualSnapshot},
};
use runner::run;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

pub(super) enum Command {
    Plan {
        plan: RenderPlan,
        revision: u64,
        solo_tracks: BTreeSet<u64>,
    },
    Demand {
        seconds: f64,
        playing: bool,
        seek: bool,
    },
    Reset,
    Retry,
}

pub(super) enum Event {
    Voice {
        state: VoiceState,
        elapsed: Duration,
    },
    Ready,
    ContextRequired {
        error: String,
    },
    Error {
        revision: Option<u64>,
        error: String,
    },
    Progress {
        revision: u64,
        owner: OwnerId,
        stage: String,
    },
    Source {
        revision: u64,
        source: Option<Arc<LiveAudio>>,
    },
    Publication {
        revision: u64,
        audio: Option<Arc<RenderedAudio>>,
        owners: Vec<PublishedOwner>,
        visuals: BTreeMap<OwnerId, VisualSnapshot>,
    },
    VisualPatches {
        revision: u64,
        patches: Vec<(OwnerId, VisualPatch)>,
    },
    Phonemes {
        revision: u64,
        owner: OwnerId,
        snapshot: crate::synthesis::rendered::PhonemeSnapshot,
    },
    Status {
        revision: u64,
        stats: CacheStats,
        failures: Vec<(OwnerId, String)>,
    },
}

pub(super) struct Worker {
    tx: Sender<Command>,
    pub rx: Receiver<Event>,
    alive: Arc<AtomicBool>,
    control: Arc<Control>,
}

impl Worker {
    pub fn spawn(
        path: PathBuf,
        dictionaries: PathBuf,
        phone_conversion: Option<PathBuf>,
        duration_key_override: Option<u64>,
        repaint: eframe::egui::Context,
    ) -> Result<Self, String> {
        let (tx, commands) = mpsc::channel();
        let (events, rx) = mpsc::channel();
        let alive = Arc::new(AtomicBool::new(true));
        let running = Arc::clone(&alive);
        let control = Arc::new(Control::default());
        let dispatch = Arc::clone(&control);
        thread::Builder::new()
            .name("synthesis-worker".into())
            .spawn(move || {
                let _wake = WakeOnExit(repaint.clone());
                run(
                    path,
                    dictionaries,
                    phone_conversion,
                    duration_key_override,
                    commands,
                    Events::new(events, repaint),
                    running,
                    dispatch,
                );
            })
            .map_err(|error| format!("无法启动合成线程：{error}"))?;
        Ok(Self {
            tx,
            rx,
            alive,
            control,
        })
    }

    pub fn send(&self, command: Command) -> bool {
        self.control.send(&self.tx, command)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);

        self.send(Command::Reset);
    }
}
