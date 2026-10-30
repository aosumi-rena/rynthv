use super::{Runtime, voices::VoiceSlot, worker};
use crate::{
    model::{GroupRef, Project, Track},
    synthesis::{
        plan::{OwnerId, RenderPlan},
        settings::VoiceSelection,
    },
};
use std::collections::{BTreeMap, BTreeSet};

impl Runtime {
    pub fn voice_not_installed(&self,track:&Track,reference:&GroupRef)->bool {
        !track.is_instrumental() && self.library.catalog().and_then(|catalog|
            catalog.is_bank_installed(&track.main_ref.database,&reference.database))==Some(false)
    }
    pub fn resolve_voice(
        &self,
        track: &Track,
        reference: &GroupRef,
    ) -> Result<Option<VoiceSelection>, String> {
        if track.is_instrumental() {
            return Ok(None);
        }
        match self.library.catalog() {
            Some(catalog) => catalog.resolve(&track.main_ref.database, &reference.database),
            None => {
                let db =
                    crate::model::singer_database(&track.main_ref.database, &reference.database);
                let assigned = ["id", "name"].iter().any(|key| {
                    db.get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|s| !s.is_empty())
                });
                if assigned && !self.library.is_scanning() {
                    Err(self
                        .library
                        .settings_error()
                        .or_else(|| self.library.catalog_error())
                        .unwrap_or("请设置 databases 目录以加载音轨歌手")
                        .to_owned())
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn reconcile_voices(&mut self, project: &Project) {
        self.routes.clear();
        self.binding_errors.clear();
        self.pending_voices.clear();
        let mut required = BTreeSet::new();
        for track in &project.tracks {
            if track.is_instrumental() {
                continue;
            }
            for reference in track.references() {
                let features=self.library.settings.experiments;
                if features.enabled {
                    if let Ok(controls)=crate::synthesis::experiments::ExperimentControls::effective(&reference.extra,features) {
                        if let Some(group)=project.group(&reference.group_id) {
                            if features.voice_blending {required.extend(controls.voices.into_iter().filter(|v|v.active(&group.parameters,group.notes.iter().map(|n|&n.extra)).unwrap_or(false)).map(|v|v.voice));}
                            if features.dds_controls {required.extend(controls.dds.vocoder);}
                        }
                    }
                }
                match self.resolve_voice(track, reference) {
                    Ok(Some(key)) => {
                        required.insert(key.clone());
                        self.routes.insert(OwnerId::new(reference.id), key);
                    }
                    Ok(None) => {
                        let database = crate::model::singer_database(
                            &track.main_ref.database,
                            &reference.database,
                        );
                        if ["id", "name"].iter().any(|key| {
                            database
                                .get(key)
                                .and_then(serde_json::Value::as_str)
                                .is_some_and(|s| !s.is_empty())
                        }) {
                            self.pending_voices.insert(OwnerId::new(reference.id));
                        }
                    }
                    Err(error) => {
                        self.binding_errors
                            .insert(OwnerId::new(reference.id), format!("{}：{error}", track.name));
                    }
                }
            }
        }

        self.voices.retain(|key, _| required.contains(key));
        for key in required {
            if self.voices.contains_key(&key) {
                continue;
            }
            let worker = (|| {
                let entry = self.library.entry(&key).ok_or("声库不在当前目录中")?;
                let dictionaries = self
                    .dictionaries()
                    .filter(|p| p.is_dir())
                    .ok_or("请设置有效的 clf-data 词典目录")?;
                worker::Worker::spawn(
                    entry.directory.clone(),
                    dictionaries.to_owned(),
                    self.library.settings.duration_key_override_for(&key),
                    self.repaint.clone(),
                )
            })();
            let slot = VoiceSlot::new(worker);
            if let Some(worker) = &slot.worker {
                worker.send(worker::Command::Demand {
                    seconds: self.desired_seconds,
                    playing: self.desired_playing,
                    seek: true,
                });
            }
            self.voices.insert(key, slot);
        }
    }

    pub fn sync_project(&mut self, project: &Project, revision: u64) {
        self.instrumentals.reconcile(project, &self.repaint);
        if self.last_revision == Some(revision)
            && self.last_audio_generation == self.instrumentals.generation
        {
            return;
        }
        self.reconcile_voices(project);
        let identities: BTreeMap<_, _> = self
            .routes
            .iter()
            .filter_map(|(&owner, key)| {
                self.inventory(key)
                    .map(|voice| (owner, voice.source_identity.clone()))
            })
            .collect();
        self.metronome.update(project);
        self.planner.experiments.settings=self.library.settings.experiments;
        self.planner.experiments.sources=self.voices.iter().filter_map(|(key,_)| {
            let inventory=self.inventory(key)?;
            let config=inventory.configuration.as_ref()?;
            Some(crate::synthesis::experiments::BlendSource {
                voice:key.clone(),source_identity:inventory.source_identity.clone(),name:config.name.clone(),
                default_language:config.default_language.clone(),supported_languages:config.supported_languages.clone(),
                prepared:inventory.preparation()?.clone(),identity_override:self.library.settings.duration_key_override_for(key),
            })
        }).collect();
        self.planner
            .update(project, revision, |owner| identities.get(&owner).cloned());
        self.sync_visuals();
        let solo = crate::synthesis::mixing_options::validate_project_mixer(
            project.extra.get("projectMixer"),
        )
        .and_then(|()| {
            project
                .tracks
                .iter()
                .try_fold(BTreeSet::new(), |mut solo, track| {
                    crate::synthesis::mixing_options::validate_mixer(&track.mixer)
                        .map_err(|error| format!("音轨 {}：{error}", track.name))?;
                    match track.mixer.get("solo").and_then(serde_json::Value::as_bool) {
                        Some(true) => {
                            solo.insert(track.id);
                        }
                        Some(false) => {}
                        None => {
                            return Err(format!("音轨 {} 的独奏字段缺失或类型无效。", track.name));
                        }
                    }
                    Ok(solo)
                })
        });
        self.last_revision = Some(revision);
        self.last_audio_generation = self.instrumentals.generation;
        self.request_serial = self.request_serial.wrapping_add(1);
        self.project_error = None;
        match solo {
            Ok(solo) => self.solo_tracks = solo,
            Err(error) => {
                self.project_error = Some(error);
                self.recordings = super::mixer::RecordingMix::default();
                for voice in self.voices.values_mut() {
                    if let Some(worker) = &voice.worker {
                        worker.send(worker::Command::Reset);
                    }
                    voice.live = None;
                    voice.audio = None;
                    voice.owners.clear();
                    voice.plan = RenderPlan::default();
                    voice.revision = self.request_serial;
                }
                self.playback.clear();
                self.aggregate_voices();
                return;
            }
        }
        self.dispatch_voices();
        self.aggregate_voices();
        self.prepare_recordings(project);
        self.refresh_playback();
    }

    fn dispatch_voices(&mut self) {
        let plan = self.planner.plan();
        for (key, voice) in &mut self.voices {
            let requests: BTreeMap<_, _> = plan
                .requests
                .iter()
                .filter(|(id, _)| self.routes.get(&id.reference()) == Some(key))
                .map(|(&id, r)| (id, r.clone()))
                .collect();
            let output_controls = plan
                .output_controls
                .iter()
                .filter(|(id, _)| requests.contains_key(id))
                .map(|(&id, c)| (id, c.clone()))
                .collect();
            let unchanged = voice.solo == self.solo_tracks
                && voice.plan.output_controls == output_controls
                && voice.plan.requests.len() == requests.len()
                && requests.iter().all(|(id, r)| {
                    voice
                        .plan
                        .requests
                        .get(id)
                        .is_some_and(|old| old.epoch == r.epoch)
                });
            if unchanged {
                continue;
            }
            let subset = RenderPlan {
                changes: plan
                    .changes
                    .iter()
                    .filter(|c| {
                        requests.contains_key(&c.owner)
                            || voice.plan.requests.contains_key(&c.owner)
                    })
                    .cloned()
                    .collect(),
                obsolete_clear: plan
                    .obsolete_clear
                    .iter()
                    .filter(|(id, _)| voice.plan.requests.contains_key(id))
                    .map(|(&id, &c)| (id, c))
                    .collect(),
                requests,
                output_controls,
            };
            voice.revision = self.request_serial;
            voice.solo = self.solo_tracks.clone();
            voice.live = None;
            voice.audio = None;
            voice.owners.clear();
            voice.failures.clear();
            voice.stats = Default::default();
            voice.progress = None;
            if voice.ready {
                voice.error = None;
            }

            if let Some(worker) = &voice.worker
                && !matches!(voice.state, super::VoiceState::Loading)
            {
                voice.waiting = worker.send(worker::Command::Plan {
                    plan: subset.clone(),
                    revision: voice.revision,
                    solo_tracks: self.solo_tracks.clone(),
                });
            }
            voice.plan = subset;
        }
    }
}
