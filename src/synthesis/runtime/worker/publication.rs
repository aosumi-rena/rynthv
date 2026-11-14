use super::{
    Event,
    state::{Session, WorkState, equivalent_owners},
    visuals,
};
use crate::synthesis::{
    plan::OwnerId,
    runtime::{
        events::Events,
        mixer::{self, LiveMixer},
    },
};
use std::sync::Arc;

pub(super) fn morph_input(plan: &crate::synthesis::plan::RenderPlan, events: &Events, revision: u64,
    mirrors: &[OwnerId], pcm: Option<Arc<crate::synthesis::audio::RenderedAudio>>, start: Option<i64>)
{
    let owners = mirrors.iter().copied().filter(|id| plan.output_controls.get(id)
        .is_some_and(|out| out.rocoflex && !out.reference_muted)).collect::<Vec<_>>();
    if !owners.is_empty() {
        let _ = events.send(Event::MorphInput { revision, owners, pcm, start });
    }
}

impl<'network, 'tree> WorkState<'network, 'tree> {
    pub fn publish(&mut self, events: &Events) {
        let publication = self.cache.publication_revision();
        let changed = publication != self.published_revision;
        let snapshot = if self.rebuild_source || changed {
            Some(self.cache.snapshot())
        } else {
            None
        };
        if self.rebuild_source {
            self.rebuild_source = false;
            let built = (|| {
                let mut live = LiveMixer::new(&self.plan, &self.solo_tracks)?;
                for owner in snapshot.as_deref().unwrap_or_default() {
                    live.complete(owner.owner, Arc::clone(&owner.audio))?;
                }
                for (&owner, session) in &self.sessions {
                    let mirrors = equivalent_owners(&self.plan, owner);
                    for &mirror in &mirrors {
                        live.prepared(mirror, session.prepared.model_samples())?;
                    }
                    morph_input(&self.plan, events, self.revision, &mirrors, None,
                        Some(session.prepared.model_samples().start));
                    for audio in session
                        .prepared
                        .ready_audio()
                        .map_err(|error| error.to_string())?
                    {
                        let audio = Arc::new(audio);
                        live.publish_for(&mirrors, Arc::clone(&audio))?;
                        morph_input(&self.plan, events, self.revision, &mirrors, Some(audio), None);
                    }
                }
                Ok::<_, String>(live)
            })();
            match built {
                Ok(live) => self.live = Some(live),
                Err(error) => {
                    self.live = None;
                    let _ = events.send(Event::Error {
                        revision: Some(self.revision),
                        error: format!("预览混音准备失败：{error}"),
                    });
                }
            }
            let _ = events.send(Event::Source {
                revision: self.revision,
                source: self.live.as_ref().and_then(LiveMixer::source),
            });

            for (&owner, session) in &self.sessions {
                visuals::publish(
                    events,
                    &self.plan,
                    self.revision,
                    owner,
                    session.visuals.snapshot(),
                );
            }
        } else if changed && let Some(live) = &mut self.live {
            for owner in snapshot.as_deref().unwrap_or_default() {
                if let Err(error) = live.complete(owner.owner, Arc::clone(&owner.audio)) {
                    let _ = events.send(Event::Error {
                        revision: Some(self.revision),
                        error: format!("预览混音失败：{error}"),
                    });
                }
            }
        }
        if self.force_publication || changed {
            self.force_publication = false;
            self.published_revision = publication;
            let stats = self.cache.stats();
            let complete = stats.queued == 0 && stats.running == 0 && stats.failed == 0;
            let mut owners = snapshot.unwrap_or_else(|| self.cache.snapshot());
            if !complete { owners.retain(|owner| owner.controls.rocoflex); }
            let audio = if complete {
                match mixer::mix(&owners, &self.solo_tracks) {
                    Ok(audio) => audio,
                    Err(error) => {
                        let _ = events.send(Event::Error {
                            revision: Some(self.revision),
                            error: format!("音轨混音失败：{error}"),
                        });
                        None
                    }
                }
            } else {
                None
            };
            let _ = events.send(Event::Publication {
                revision: self.revision,
                audio,
                owners,
                visuals: self.cache.visual_snapshot(),
            });
        }
        self.publish_status(events);
    }

    pub fn publish_prepared(&mut self, owner: OwnerId, events: &Events) -> Result<(), String> {
        if let Some(session) = self.sessions.get_mut(&owner) {
            let mirrors = equivalent_owners(&self.plan, owner);
            morph_input(&self.plan, events, self.revision, &mirrors, None, Some(session.prepared.model_samples().start));
            let phones=session.prepared.phoneme_track();
            for &mirror in &mirrors {
                let _=events.send(Event::Phonemes {
                    revision:self.revision,owner:mirror,
                    snapshot:crate::synthesis::rendered::PhonemeSnapshot {
                        input:Arc::clone(&session.job.request.input),phones:phones.clone(),
                    },
                });
            }
            if !self.rebuild_source
                && let Some(live) = &mut self.live
            {
                for &mirror in &mirrors {
                    live.prepared(mirror, session.prepared.model_samples())?;
                }
            }
            for audio in session
                .prepared
                .ready_audio()
                .map_err(|error| error.to_string())?
            {
                session.visuals.audio(&audio, false);
                let audio = Arc::new(audio);
                if !self.rebuild_source
                    && let Some(live) = &mut self.live
                {
                    live.publish_for(&mirrors, Arc::clone(&audio))?;
                }
                morph_input(&self.plan, events, self.revision, &mirrors, Some(audio), None);
            }
            visuals::publish(
                events,
                &self.plan,
                self.revision,
                owner,
                session.visuals.snapshot(),
            );
        }
        Ok(())
    }

    pub fn publish_chunks(
        &mut self,
        session: &mut Session<'network, 'tree>,
        chunks: Vec<crate::synthesis::audio::RenderedAudio>,
        events: &Events,
    ) -> Result<(), String> {

        let owner = session.job.request.owner;
        let mirrors = equivalent_owners(&self.plan, owner);
        let mut patches = Vec::new();
        for audio in chunks {
            if let Some(patch) = session.visuals.audio(&audio, true) {
                patches.push(patch);
            }
            let audio = Arc::new(audio);
            if !self.rebuild_source
                && let Some(live) = &mut self.live
            {
                live.publish_for(&mirrors, Arc::clone(&audio))?;
            }
            morph_input(&self.plan, events, self.revision, &mirrors, Some(audio), None);
        }
        visuals::publish(events, &self.plan, self.revision, owner, patches);
        Ok(())
    }

    fn publish_status(&mut self, events: &Events) {
        let stats = self.cache.stats();
        let unchanged = self.last_status_revision == Some(self.revision)
            && self.last_stats.is_some_and(|old| {
                old.queued == stats.queued
                    && old.running == stats.running
                    && old.ready == stats.ready
                    && old.failed == stats.failed
                    && old.current_render_bytes == stats.current_render_bytes
                    && old.discarded_completions == stats.discarded_completions
            });
        if unchanged {
            return;
        }
        self.last_stats = Some(stats);
        self.last_status_revision = Some(self.revision);
        let failures = self
            .plan
            .requests
            .keys()
            .filter_map(|owner| {
                self.cache
                    .failure(*owner)
                    .map(|error| (*owner, error.to_owned()))
            })
            .collect();
        let _ = events.send(Event::Status {
            revision: self.revision,
            stats,
            failures,
        });
    }
}
