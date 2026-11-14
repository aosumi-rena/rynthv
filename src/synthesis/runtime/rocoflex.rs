use super::{Runtime, events::WakeOnExit, mixer};
use crate::{
    model::{Extra, Project, RocoflexTrack},
    synthesis::{audio::{AudioLayer, MorphLayer, RenderedAudio}, plan::{Coverage, OwnerId, RunEpoch},
        render_cache::PublishedOwner, rendered::Waveform, rocoflex::{InputUpdate, Provider, RenderCheck, render}},
};
use std::{collections::BTreeMap, ops::Range, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, thread};

#[derive(Default)]
pub(super) struct Morphs {
    provider: Option<Provider>,
    model_check: RenderCheck,
    tracks: BTreeMap<u64, Track>,
    entries: BTreeMap<u64, Entry>,
    partial: BTreeMap<OwnerId, Partial>,
    job: Option<Job>,
    pub(super) dirty: bool,
}

impl Morphs {
    pub fn busy(&self) -> bool { self.job.is_some() }
    pub fn provider_available(&self) -> bool { self.provider.is_some() }

    pub fn reset(&mut self) {
        self.tracks.clear();
        self.entries.clear();
        self.partial.clear();
        self.model_check = RenderCheck::Idle;
        self.dirty = true;
        if let Some(job) = &self.job { job.cancel.store(true, Ordering::Relaxed); }
    }

    pub fn source(&mut self, id: OwnerId, epoch: RunEpoch, coverage: Coverage,
        pcm: Option<&Arc<RenderedAudio>>, start: Option<i64>)
    {
        let Ok(range) = mixer::morph_range(coverage) else { return; };
        let partial = self.partial.entry(id).or_insert_with(|| Partial::new(epoch, range.start));
        if partial.epoch != epoch { *partial = Partial::new(epoch, range.start); }
        let previous = partial.until;
        if let Some(start) = start && !partial.prepared {
            partial.until = partial.until.max((start.max(0) as usize).clamp(range.start, range.end));
            partial.prepared = true;
        }
        if let Some(pcm) = pcm && let Some(span) = mixer::morph_span(pcm, range) {
            if span.end > partial.until { partial.spans.insert(span.start, span); }
            while let Some(end) = partial.spans.range(..=partial.until).next_back()
                .map(|(_, span)| span.end).filter(|&end| end > partial.until)
            {
                partial.until = end;
            }
        }
        self.dirty |= partial.until != previous;
    }

    pub fn forget_voice(&mut self, owners: impl IntoIterator<Item = OwnerId>) {
        for owner in owners {
            self.partial.remove(&owner);
            for entry in self.entries.values_mut().filter(|entry| entry.sources.contains_key(&owner)) {
                entry.clear_audio();
                entry.status = Status::Rendering;
            }
            if let Some(job) = &self.job && self.entries.get(&job.track).is_some_and(|entry| entry.sources.contains_key(&owner)) {
                job.cancel.store(true, Ordering::Relaxed);
            }
        }
        self.dirty = true;
    }

    pub fn retry(&mut self) {
        self.entries.retain(|_, entry| !matches!(&entry.status, Status::Failed(_)));
        self.model_check = RenderCheck::Idle;
        self.dirty = true;
    }
}

struct Track {
    preset: Arc<RocoflexTrack>,
    mixer: Extra,
    gains: [f32; 2],
}

#[derive(Clone, PartialEq, Eq)]
struct Source {
    epoch: RunEpoch,
    coverage: Coverage,
    range: Range<usize>,
}

struct Partial {
    epoch: RunEpoch,
    until: usize,
    prepared: bool,
    spans: BTreeMap<usize, mixer::MorphSpan>,
}

impl Partial {
    fn new(epoch: RunEpoch, start: usize) -> Self {
        Self { epoch, until: start, prepared: false, spans: BTreeMap::new() }
    }
}

struct Chunk {
    audio: Arc<RenderedAudio>,
    waveform: Waveform,
}

enum Status { Rendering, Ready, Failed(String) }

struct Entry {
    preset: Arc<RocoflexTrack>,
    sources: BTreeMap<OwnerId, Source>,
    chunks: Vec<Chunk>,
    ready_until: usize,
    range: Range<usize>,
    layer: MorphLayer,
    status: Status,
}

struct Job {
    track: u64,
    cancel: Arc<AtomicBool>,
    receive: mpsc::Receiver<Event>,
    send: mpsc::Sender<InputUpdate<(OwnerId, usize), mixer::MorphSpan>>,
    admitted: BTreeMap<OwnerId, Arc<RenderedAudio>>,
    next_source: BTreeMap<OwnerId, usize>,
    sent_until: usize,
}

enum Event { Audio(Chunk), Finished(Result<(), String>) }

impl Drop for Job {
    fn drop(&mut self) { self.cancel.store(true, Ordering::Relaxed); }
}

impl Entry {
    fn matches(&self, preset: &Arc<RocoflexTrack>, sources: &BTreeMap<OwnerId, Source>) -> bool {
        Arc::ptr_eq(&self.preset, preset) && self.sources == *sources
    }

    fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    fn fail(&mut self, error: String) {
        self.clear_audio();
        self.status = Status::Failed(error);
    }

    fn clear_audio(&mut self) {
        self.chunks.clear();
        self.ready_until = self.range.start;
        self.layer.reset(self.range.clone());
    }
}

impl Runtime {
    pub fn set_rocoflex_provider(&mut self, provider: Option<Provider>) {
        if self.morphs.provider == provider { return; }
        if self.morphs.provider.is_some() != provider.is_some() { self.last_revision = None; }
        self.morphs.provider = provider;
        self.morphs.model_check = RenderCheck::Idle;
        self.morphs.entries.clear();
        self.morphs.partial.clear();
        self.morphs.dirty = true;
        if let Some(job) = &self.morphs.job { job.cancel.store(true, Ordering::Relaxed); }
    }

    pub(super) fn sync_morph_tracks(&mut self, project: &Project) {
        let mut previous = std::mem::take(&mut self.morphs.tracks);
        if self.morphs.provider.is_some() {
            for track in &project.tracks {
                let Some(preset) = track.rocoflex.as_ref().filter(|preset| preset.enabled) else { continue; };
                let preset = previous.remove(&track.id).filter(|prev| prev.preset.as_ref() == preset)
                    .map(|prev| prev.preset).unwrap_or_else(|| Arc::new(preset.clone()));
                self.morphs.tracks.insert(track.id, Track {
                    preset, mixer: track.mixer.clone(),
                    gains: mixer::morph_gains(&track.mixer, &self.solo_tracks).unwrap_or([0.0; 2]),
                });
            }
        }
        self.morphs.dirty = true;
        self.update_morphs();
        for (&id, track) in &self.morphs.tracks {
            if let Some(entry) = self.morphs.entries.get(&id) {
                entry.layer.gains(track.gains);
            }
        }
    }

    pub(super) fn update_morphs(&mut self) {
        let mut changed = false;
        if self.morphs.model_check.poll() { self.morphs.dirty = true; }
        if std::mem::take(&mut self.morphs.dirty) && self.morphs.provider.is_some() {
            changed |= self.prepare_morphs();
        }
        if let Some(job) = &self.morphs.job {
            let mut finished = false;
            let gains = self.morphs.tracks.get(&job.track).map_or([0.0; 2], |track| track.gains);
            let entry = self.morphs.entries.get_mut(&job.track);
            let entry = if job.cancel.load(Ordering::Relaxed) { None } else { entry };
            let mut entry = entry.filter(|e| matches!(e.status, Status::Rendering));
            let mut batch = Vec::new();
            loop {
                let event = match job.receive.try_recv() {
                    Ok(event) => event,
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => Event::Finished(Err("Rocoflex render worker closed unexpectedly".into())),
                };
                match event {
                    Event::Audio(chunk) => {
                        if let Some(entry) = &mut entry {
                            let end = (chunk.audio.end_seconds() * 48_000.0).round() as usize;
                            entry.ready_until = end;
                            batch.push(mixer::morph_layer(job.track, gains, Arc::clone(&chunk.audio)));
                            entry.chunks.push(chunk);
                        }
                    }
                    Event::Finished(result) => {
                        if let Some(entry) = &mut entry {
                            match result {
                                Ok(()) if entry.ready_until == entry.range().end => entry.status = Status::Ready,
                                Ok(()) => entry.fail("Incomplete Rocoflex audio".into()),
                                Err(error) => entry.fail(error),
                            }
                        }
                        finished = true;
                        break;
                    }
                }
            }
            if let Some(entry) = entry && !matches!(entry.status, Status::Failed(_)) && !batch.is_empty() {
                entry.layer.publish(batch, entry.ready_until);
            }
            if finished {
                self.morphs.job = None;
                self.morphs.dirty = true;
            }
        }
        if changed { self.refresh_playback(); }
        if self.morphs.dirty { self.render_wake.request(); }
    }

    fn prepare_morphs(&mut self) -> bool {
        let mut changed = false;
        let mut new_preset = false;
        if self.morphs.tracks.is_empty() {
            let had_entries = !self.morphs.entries.is_empty();
            self.morphs.entries.clear();
            if let Some(job) = &self.morphs.job { job.cancel.store(true, Ordering::Relaxed); }
            return had_entries;
        }
        let plan = self.planner.plan();
        let mut sources = BTreeMap::<u64, BTreeMap<OwnerId, Source>>::new();
        for (&id, request) in &plan.requests {
            let Some(out) = plan.output_controls.get(&id) else { continue; };
            if !out.rocoflex || out.reference_muted || !self.morphs.tracks.contains_key(&out.track_id)
                || !(self.routes.contains_key(&id.reference()) || self.pending_voices.contains(&id.reference())
                    || self.binding_errors.contains_key(&id.reference()))
            { continue; }
            let Ok(range) = mixer::morph_range(request.coverage) else { continue; };
            if !range.is_empty() {
                sources.entry(out.track_id).or_default().insert(id, Source {
                    epoch: request.epoch, coverage: request.coverage, range,
                });
            }
        }
        let ready: BTreeMap<_, _> = self.export_owners.iter().map(|o| (o.owner, o)).collect();
        self.morphs.partial.retain(|id, partial| {
            plan.requests.get(id).is_some_and(|request| request.epoch == partial.epoch)
                && plan.output_controls.get(id).is_some_and(|out| out.rocoflex && !out.reference_muted
                    && self.morphs.tracks.contains_key(&out.track_id))
                && ready.get(id).is_none_or(|pcm| plan.requests[id].coverage != pcm.coverage)
        });
        let partials = &self.morphs.partial;
        for (&tid, track) in &self.morphs.tracks {
            let sources = sources.remove(&tid).unwrap_or_default();
            let prior = self.morphs.entries.get(&tid);
            let unchanged = prior.is_some_and(|e| e.matches(&track.preset, &sources));
            if !unchanged {
                if let Some(job) = self.morphs.job.as_ref().filter(|job| job.track == tid) {
                    job.cancel.store(true, Ordering::Relaxed);
                }
                let status = if sources.is_empty() { Status::Ready } else { Status::Rendering };
                new_preset |= !sources.is_empty() && prior.is_none_or(|e|
                    e.sources.is_empty() || !Arc::ptr_eq(&e.preset, &track.preset));
                let range = sources.values().map(|s| s.range.start).min().unwrap_or(0)
                    ..sources.values().map(|s| s.range.end).max().unwrap_or(0);
                let layer = MorphLayer::default();
                layer.reset(range.clone());
                self.morphs.entries.insert(tid, Entry { preset: Arc::clone(&track.preset), sources,
                    chunks: Vec::new(), ready_until: range.start, range, layer, status });
                changed = true;
            }
        }
        let count = self.morphs.entries.len();
        self.morphs.entries.retain(|id, _| self.morphs.tracks.contains_key(id));
        changed |= self.morphs.entries.len() != count;
        let active = self.morphs.job.as_ref().filter(|job| !job.cancel.load(Ordering::Relaxed)).map(|job| job.track);
        let waiting = |id, entry: &Entry| active != Some(id) && matches!(entry.status, Status::Rendering);
        if self.morphs.entries.iter().any(|(&id, entry)| waiting(id, entry)) {
            let provider = self.morphs.provider.as_ref().unwrap();
            self.morphs.model_check.request(provider, &self.repaint, new_preset);
            if let Some(error) = self.morphs.model_check.error() {
                for (&id, entry) in &mut self.morphs.entries {
                    if waiting(id, entry) { entry.fail(error.to_owned()); changed = true; }
                }
            }
        }
        if let Some(job) = &mut self.morphs.job {
            if !self.morphs.entries.contains_key(&job.track) {
                job.cancel.store(true, Ordering::Relaxed);
            }
            if !job.cancel.load(Ordering::Relaxed) {
                let entry = self.morphs.entries.get_mut(&job.track).unwrap();
                if job.admitted.iter().any(|(id, prior)| ready.get(id).is_none_or(|o| !Arc::ptr_eq(&o.audio, prior))) {
                    job.cancel.store(true, Ordering::Relaxed);
                    entry.clear_audio();
                } else if let Some(error) = failed_source(entry, &self.failures) {
                    job.cancel.store(true, Ordering::Relaxed);
                    entry.fail(error);
                    changed = true;
                } else if let Err(error) = supply(job, entry, &ready, partials) {
                    job.cancel.store(true, Ordering::Relaxed);
                    entry.fail(error);
                    changed = true;
                }
            }
            return changed;
        }
        if !self.morphs.model_check.ready() { return changed; }
        for (&track, entry) in &mut self.morphs.entries {
            if !matches!(entry.status, Status::Rendering) { continue; }
            if let Some(error) = failed_source(entry, &self.failures) {
                entry.fail(error);
                changed = true;
                continue;
            }
            let range = entry.range();
            if range.len() > crate::synthesis::rocoflex::MAX_FRAMES {
                entry.fail("Rocoflex track exceeds the offline render limit".into());
                changed = true;
                continue;
            }
            if available_until(entry, &ready, partials) <= range.start { continue; }
            let (send, updates) = mpsc::channel();
            let (events, receive) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&cancel);
            let repaint = self.repaint.clone();
            let wake = self.render_wake.clone();
            let provider = self.morphs.provider.clone().unwrap();
            let preset = Arc::clone(&entry.preset);
            let start = range.start;
            let launch = thread::Builder::new().name("rocoflex-offline".into()).spawn(move || {
                let _wake = WakeOnExit(repaint.clone());
                let result = render(&provider, &preset, range, updates, &flag, mixer::mix_morph,
                    |owner, at| owner.end > at, |audio| {
                        if flag.load(Ordering::Relaxed) { return; }
                        let waveform = Waveform::new(&audio);
                        if events.send(Event::Audio(Chunk { audio: Arc::new(audio), waveform })).is_ok() {
                            wake.request();
                        }
                    });
                let _ = events.send(Event::Finished(result));
            });
            match launch {
                Ok(_) => {
                    let mut job = Job { track, cancel, receive, send, admitted: BTreeMap::new(), next_source: BTreeMap::new(), sent_until: start };
                    if let Err(error) = supply(&mut job, entry, &ready, partials) {
                        job.cancel.store(true, Ordering::Relaxed);
                        entry.fail(error);
                    }
                    self.morphs.job = Some(job);
                }
                Err(error) => entry.fail(error.to_string()),
            }
            return changed;
        }
        changed
    }

    pub fn morph_enabled(&self, track: u64) -> bool { self.morphs.tracks.contains_key(&track) }

    pub fn morph_pending(&self, track: u64) -> bool {
        self.morph_enabled(track) && self.morphs.entries.get(&track).is_none_or(|e| matches!(e.status, Status::Rendering))
    }

    pub fn morph_error(&self, track: u64) -> Option<&str> {
        match &self.morphs.entries.get(&track)?.status {
            Status::Failed(error) => Some(error),
            _ => None,
        }
    }

    pub fn morph_waveforms(&self, track: u64) -> impl Iterator<Item = &Waveform> {
        self.morphs.entries.get(&track).into_iter().flat_map(|e| e.chunks.iter().map(|part| &part.waveform))
    }

    pub fn morph_waveforms_in(&self, track: u64, seconds: Range<f64>) -> impl Iterator<Item = &Waveform> {
        self.morphs.entries.get(&track).into_iter().flat_map(move |entry| {
            // The offline stream publishes consecutive PCM chunks in order.
            let first = entry.chunks.partition_point(|chunk| chunk.waveform.end_seconds() <= seconds.start);
            entry.chunks[first..].iter()
                .take_while(move |chunk| chunk.waveform.start_seconds() < seconds.end)
                .map(|chunk| &chunk.waveform)
        })
    }

    pub(super) fn morph_ready(&self) -> bool {
        self.morphs.tracks.iter().filter(|(id, track)| self.morph_audible(**id, track)).all(|(id, _)| {
            self.morphs.entries.get(id).is_some_and(|e| matches!(e.status, Status::Ready))
        })
    }

    fn morph_audible(&self, id: u64, track: &Track) -> bool {
        (self.solo_tracks.is_empty() || self.solo_tracks.contains(&id))
            && track.mixer.get("mute").and_then(serde_json::Value::as_bool) != Some(true)
    }

    fn morph_groups(&self) -> impl Iterator<Item = (u64, [f32; 2], &Entry)> {
        self.morphs.tracks.iter().filter(|(id, track)| self.morph_audible(**id, track))
            .filter_map(|(&id, track)| Some((id, track.gains, self.morphs.entries.get(&id)?)))
    }

    pub(super) fn morph_layers(&self) -> Vec<AudioLayer> {
        self.morph_groups().flat_map(|(id, gains, entry)| entry.chunks.iter().map(move |part|
            mixer::morph_layer(id, gains, Arc::clone(&part.audio)))).collect()
    }

    pub(super) fn morph_playback(&self) -> Vec<MorphLayer> {
        self.morph_groups().map(|(_, _, entry)| entry.layer.clone()).collect()
    }
}

fn failed_source(entry: &Entry, failures: &[(OwnerId, String)]) -> Option<String> {
    failures.iter().find(|(id, _)| entry.sources.contains_key(id)
        || id.1.is_none() && entry.sources.keys().any(|source| source.reference() == *id))
        .map(|(_, error)| error.clone())
}

fn available_until(entry: &Entry, ready: &BTreeMap<OwnerId, &PublishedOwner>, partials: &BTreeMap<OwnerId, Partial>) -> usize {
    entry.sources.iter().filter_map(|(id, source)| {
        if ready.get(id).is_some_and(|o| o.coverage == source.coverage) { return None; }
        Some(partials.get(id).filter(|p| p.epoch == source.epoch)
            .map_or(source.range.start, |p| p.until.min(source.range.end)))
            .filter(|&until| until < source.range.end)
    }).min().unwrap_or(entry.range.end)
}

fn supply(job: &mut Job, entry: &Entry, ready: &BTreeMap<OwnerId, &PublishedOwner>,
    partials: &BTreeMap<OwnerId, Partial>) -> Result<(), String>
{
    let until = available_until(entry, ready, partials);
    if until <= job.sent_until { return Ok(()); }
    let mut owners = BTreeMap::new();
    for (&id, source) in &entry.sources {
        let mut at = job.next_source.get(&id).copied().unwrap_or(source.range.start);
        if at >= until || at >= source.range.end { continue; }
        if let Some(pcm) = ready.get(&id).filter(|o| o.coverage == source.coverage) {
            if pcm.audio.sample_rate() != 48_000 { return Err("Rocoflex needs 48 kHz track audio".into()); }
            if let Some(span) = mixer::morph_span(&pcm.audio, at..source.range.end) {
                owners.insert((id, span.start), span);
            }
            job.admitted.insert(id, Arc::clone(&pcm.audio));
            at = source.range.end;
        } else if let Some(partial) = partials.get(&id).filter(|p| p.epoch == source.epoch) {
            for (_, span) in partial.spans.range(..until).filter(|(_, span)| span.end > at) {
                let start = span.start.max(at);
                owners.insert((id, start), span.clipped(start, span.end.min(until)));
            }
            at = until;
        } else {
            return Err("Missing ready Rocoflex input".into());
        }
        job.next_source.insert(id, at);
    }
    if job.send.send(InputUpdate { until, owners }).is_ok() { job.sent_until = until; }
    Ok(())
}
