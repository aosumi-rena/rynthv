use super::{Runtime, events::WakeOnExit};
use crate::synthesis::{
    settings::{SynthesisSettings, VoiceSelection},
    voice::{VoiceAvailability, VoiceCatalog, VoiceCatalogEntry},
};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

pub struct VoiceLibrary {
    pub settings: SynthesisSettings,
    catalog: Option<VoiceCatalog>,
    scan: Option<Receiver<LibraryScan>>,
    catalog_error: Option<String>,
    settings_error: Option<String>,
    invalid_settings: bool,
}

struct LibraryScan {
    catalog: VoiceCatalog,
    error: Option<String>,
}

fn scan_roots(settings: &SynthesisSettings) -> LibraryScan {
    let mut catalog = VoiceCatalog::default();
    let mut errors = Vec::new();
    for (label, root) in settings.database_roots() {
        match VoiceCatalog::scan(root) {
            Ok(found) => {
                catalog.entries.extend(found.entries);
                catalog.warnings.extend(
                    found
                        .warnings
                        .into_iter()
                        .map(|warning| format!("{label} {}：{warning}", root.display())),
                );
            }
            Err(error) => errors.push(format!(
                "无法扫描 {label} 声库目录 {}：{error}",
                root.display()
            )),
        }
    }
    catalog.entries.sort_by(|left, right| {
        left.database_id
            .cmp(&right.database_id)
            .then_with(|| left.revision.cmp(&right.revision))
            .then_with(|| left.directory.cmp(&right.directory))
    });
    catalog.entries.dedup_by(|left, right| {
        left.database_id == right.database_id
            && left.revision == right.revision
            && left.directory == right.directory
    });
    for duplicates in catalog.entries.chunk_by_mut(|left, right| {
        left.database_id == right.database_id && left.revision == right.revision
    }) {
        if duplicates.len() > 1 {
            let message = format!(
                "声库 {}/{} 对应多个文件，请检查声库目录配置。\n{}",
                duplicates[0].database_id, duplicates[0].revision,
                duplicates.iter().map(|e|e.directory.display().to_string()).collect::<Vec<_>>().join("\n")
            );
            errors.push(message.clone());
            for entry in duplicates {
                entry.availability = VoiceAvailability::Unsupported(message.clone());
            }
        }
    }
    catalog.entries.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then_with(|| left.database_id.cmp(&right.database_id))
            .then_with(|| right.revision.cmp(&left.revision))
    });
    LibraryScan {
        catalog,
        error: (!errors.is_empty()).then(|| errors.join("\n")),
    }
}

impl VoiceLibrary {
    pub(super) fn new(repaint: eframe::egui::Context) -> Self {
        let (settings, settings_error) = match SynthesisSettings::load() {
            Ok(settings) => (settings, None),
            Err(error) => (SynthesisSettings::default(), Some(error)),
        };
        let invalid_settings = settings_error.is_some();
        settings.experiments.publish_ui();
        let mut library = Self {
            settings,
            catalog: None,
            scan: None,
            catalog_error: None,
            settings_error,
            invalid_settings,
        };
        if !invalid_settings {
            library.rescan(repaint);
        }
        library
    }

    pub fn catalog(&self) -> Option<&VoiceCatalog> {
        self.catalog.as_ref()
    }

    pub fn is_scanning(&self) -> bool {
        self.scan.is_some()
    }

    pub fn catalog_error(&self) -> Option<&str> {
        self.catalog_error.as_deref()
    }

    pub fn settings_error(&self) -> Option<&str> {
        self.settings_error.as_deref()
    }

    fn update(&mut self, settings: SynthesisSettings) -> bool {
        if self.invalid_settings {
            return false;
        }
        match settings.save() {
            Ok(()) => {
                self.settings = settings;
                self.settings_error = None;
                true
            }
            Err(error) => {
                self.settings_error = Some(error);
                false
            }
        }
    }

    pub fn entry(&self, selection: &VoiceSelection) -> Option<&VoiceCatalogEntry> {
        self.catalog.as_ref()?.entry(selection)
    }
    fn rescan(&mut self, repaint: eframe::egui::Context) {

        self.scan = None;
        self.catalog = None;
        self.catalog_error = None;
        if self.invalid_settings {
            return;
        }
        if !self.settings.has_databases() {
            return;
        }
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("voice-library-scan".into())
            .spawn(move || {
                let _wake = WakeOnExit(repaint);
                let sender = tx;
                let result = scan_roots(&settings);
                let _ = sender.send(result);
            }) {
            Ok(_) => self.scan = Some(rx),
            Err(error) => self.catalog_error = Some(format!("无法启动声库扫描：{error}")),
        }
    }

    fn poll(&mut self) -> bool {
        let Some(scan) = &self.scan else {
            return false;
        };
        match scan.try_recv() {
            Ok(result) => {
                self.scan = None;
                self.catalog = Some(result.catalog);
                self.catalog_error = result.error;
                true
            }
            Err(TryRecvError::Disconnected) => {
                self.scan = None;
                self.catalog_error = Some("声库扫描线程意外终止。".into());
                true
            }
            Err(TryRecvError::Empty) => false,
        }
    }
}

impl Runtime {
    pub fn set_voice_sources(&mut self,paths:Vec<PathBuf>) {
        let mut settings=self.library.settings.clone();
        settings.databases=None;
        settings.sv1_databases=None;
        settings.voice_sources=paths;
        if settings!=self.library.settings && self.library.update(settings) {self.rescan_voices();}
    }
    pub fn set_experiments(&mut self, experiments:crate::synthesis::experiments::ExperimentalSettings) {
        let mut settings=self.library.settings.clone();
        settings.experiments=experiments;
        if self.library.update(settings) {
            experiments.publish_ui();
            self.request_serial=self.request_serial.wrapping_add(1);
            self.last_revision=None;
            self.export_owners.clear();
            self.playback.clear();
            for voice in self.voices.values_mut() {
                if let Some(worker)=&voice.worker {worker.send(super::worker::Command::Reset);}
                voice.plan=Default::default();voice.live=None;voice.audio=None;
                voice.owners.clear();voice.failures.clear();voice.stats=Default::default();
                voice.progress=None;voice.revision=self.request_serial;
                if voice.ready {voice.error=None;}
            }
            self.visuals.clear();
            self.phonemes.clear();
        }
    }
    pub fn set_voice_duration_key_override(&mut self, voice: VoiceSelection, enabled: bool, value: u64) {
        let mut settings = self.library.settings.clone();
        settings.set_voice_duration_key_override(voice.clone(), enabled, value);
        if settings == self.library.settings { return; }
        if self.library.update(settings) {

            self.visuals.retain(|owner, _| self.routes.get(owner) != Some(&voice));
            self.phonemes.retain(|owner, _| self.routes.get(owner) != Some(&voice));
            self.reload_voice(&voice);
        }
    }

    pub fn set_duration_key_override(&mut self, enabled: bool, value: u64) {
        let mut settings = self.library.settings.clone();
        settings.duration_key_override_enabled = enabled;
        settings.duration_key_override_value = value;
        if self.library.update(settings) {
            self.clear_voices();
            self.visuals.clear();
            self.phonemes.clear();
        }
    }

    pub fn dictionaries(&self) -> Option<&Path> {
        self.library.settings.dictionaries.as_deref()
    }
    pub fn set_audio_converter(&mut self, path: Option<PathBuf>) {
        let mut settings = self.library.settings.clone();
        settings.audio_converter = path;
        self.library.update(settings);
    }
    pub fn set_databases(&mut self, path: PathBuf) {
        let mut settings = self.library.settings.clone();
        settings.databases = Some(path);
        if self.library.update(settings) {
            self.rescan_voices();
        }
    }
    pub fn set_sv1_databases(&mut self, path: PathBuf) {
        let mut settings = self.library.settings.clone();
        settings.sv1_databases = Some(path);
        if self.library.update(settings) {
            self.rescan_voices();
        }
    }
    pub fn set_dictionaries(&mut self, path: PathBuf) {
        let mut settings = self.library.settings.clone();
        settings.dictionaries = Some(path);
        if self.library.update(settings) {
            self.clear_voices();
        }
    }
    pub fn rescan_voices(&mut self) {
        self.clear_voices();
        self.library.rescan(self.repaint.clone());
    }
    pub(super) fn poll_library(&mut self) {
        if self.library.poll() {
            self.last_revision = None;
        }
    }
}
