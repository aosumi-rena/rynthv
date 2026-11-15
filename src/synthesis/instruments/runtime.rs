use super::{Catalog, Entry, InstrumentSettings, backend};
use crate::model::Project;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
};

pub struct Output {
    pub track: u64,
    pub source: super::mixing::Source,
    pub pitch: Vec<f32>,
    pub member_pitch: Vec<Vec<f32>>,
    pub origin: f64,
    pub controls: BTreeMap<u64, super::EffectiveNote>,
}
struct Request {
    hash: u64,
    entry: Entry,
    notes: Vec<Value>,
    ambient: u32,
    players: usize,
    offset: i64,
    prepared: Option<Box<backend::EnsembleStream>>,
}
enum Update { Prepared(Vec<f32>,Vec<Vec<f32>>,Vec<super::EffectiveNote>), Source(usize, Arc<[f32]>), Yield(Request), Complete }
struct Job {
    reference: u64,
    hash: u64,
    cancel: Arc<AtomicBool>,
    rx: Receiver<Result<Update, String>>,
}
struct Scan {
    rx: Receiver<Result<Catalog, String>>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Scan {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

#[derive(Default)]
pub struct Instruments {
    pub catalog: Catalog,
    pub scan_error: Option<String>,
    pub errors: BTreeMap<u64, String>,
    pub outputs: BTreeMap<u64, Output>,
    pub mixer: super::mixing::Mixer,
    mix_errors: BTreeMap<u64,String>,
    settings: InstrumentSettings,
    hashes: BTreeMap<u64, u64>,
    requests: BTreeMap<u64, Request>,
    scan: Option<Scan>,
    scan_started: bool,
    job: Option<Job>,
    pub pending: BTreeMap<u64, std::ops::Range<usize>>,
}

impl Instruments {
    pub fn deferred(settings: InstrumentSettings) -> Self {
        Self {
            settings,
            ..Default::default()
        }
    }
    pub fn ensure_scan(&mut self, ctx: &eframe::egui::Context) {
        if !self.scan_started {
            self.rescan(ctx);
        }
    }
    pub fn new(settings: InstrumentSettings, ctx: &eframe::egui::Context) -> Self {
        let mut s = Self {
            settings,
            ..Default::default()
        };
        s.rescan(ctx);
        s
    }
    pub fn entry(&self, selection: &crate::model::InstrumentSelection) -> Option<&Entry> {
        self.catalog
            .entries
            .iter()
            .find(|e| e.selection == *selection)
    }
    pub fn for_track(&self, track: &crate::model::Track) -> Option<&Entry> {
        let config = track.instrument.as_ref()?;
        if let Some(selected) = &config.selection {
            return self.entry(selected);
        }
        let name = track.main_ref.database.get("name")?.as_str()?;
        let revision = track.main_ref.database.get("version")?.as_str()?;
        self.catalog
            .entries
            .iter()
            .filter(|entry| entry.name == name && entry.selection.revision == revision)
            .min_by_key(|entry| entry.auxiliary.is_none())
    }
    pub fn is_scanning(&self) -> bool {
        self.scan.is_some()
    }
    pub fn busy(&self) -> bool {
        self.job.is_some() || !self.requests.is_empty() || self.mixer.busy()
    }
    pub fn ready(&self) -> bool {
        !self.busy() && self.pending.is_empty() && self.errors.is_empty()
    }
    pub fn reference_busy(&self, id: u64) -> bool {
        self.requests.contains_key(&id) || self.job.as_ref().is_some_and(|j| j.reference == id) || self.mixer.reference_busy(id)
    }
    pub fn reset(&mut self) {
        self.job = None;
        self.requests.clear();
        self.hashes.clear();
        self.outputs.clear();
        self.errors.clear();
        self.mix_errors.clear();
        self.pending.clear();
        self.mixer.clear();
    }
    pub fn retry_errors(&mut self) {
        for id in self.errors.keys() {
            self.hashes.remove(id);
        }
        self.errors.clear();
    }
    pub fn rescan(&mut self, ctx: &eframe::egui::Context) {
        self.scan = None;
        self.scan_started = true;
        self.reset();
        self.catalog = Catalog::default();
        self.scan_error = None;
        let settings = self.settings.clone();
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        thread::spawn(move || {
            let _ = tx.send(backend::scan(&settings, stop));
            ctx.request_repaint();
        });
        self.scan = Some(Scan { rx, cancel });
    }
    pub fn configure(&mut self, settings: InstrumentSettings, ctx: &eframe::egui::Context) {
        let scan =
            settings.sources != self.settings.sources;
        if settings != self.settings {
            self.settings = settings;
            if scan {
                self.rescan(ctx);
            }
        }
    }
    pub fn poll(&mut self, ctx: &eframe::egui::Context) -> bool {
        let mut changed = false;
        if let Some(result) = self.scan.as_ref().and_then(|r| match r.rx.try_recv() {
            Ok(v) => Some(v),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("Instrument scanner stopped".into())),
            _ => None,
        }) {
            self.scan = None;
            match result {
                Ok(c) => self.catalog = c,
                Err(e) => self.scan_error = Some(e),
            };
            changed = true;
        }
        while let Some(result) = self.job.as_ref().and_then(|j| match j.rx.try_recv() {
            Ok(v) => Some(v),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Instrument renderer stopped".into()))
            }
            _ => None,
        }) {
            let job = self.job.as_ref().unwrap();
            let reference=job.reference;
            if self.hashes.get(&job.reference) == Some(&job.hash) {
                match result {
                    Ok(Update::Prepared(pitch,members,controls)) => {
                        if let Some(output)=self.outputs.get_mut(&reference) {
                            output.source.control_frames=pitch.len();output.pitch=pitch;
                            output.member_pitch=members;
                            output.controls=controls.into_iter().map(|c|(c.id,c)).collect();
                        }
                    }
                    Ok(Update::Source(at,audio)) => {
                        if let Some(output)=self.outputs.get_mut(&reference) {
                            if let Err(error)=output.source.append(at,audio) {self.errors.insert(reference,error);self.job=None;}
                        }
                    }
                    Ok(Update::Yield(request)) => {self.requests.insert(reference,request);self.job=None;}
                    Ok(Update::Complete) => {
                        if let Some(output)=self.outputs.get_mut(&reference){output.source.complete=true;}
                        self.pending.remove(&reference);self.errors.remove(&reference);self.job=None;
                    }
                    Err(e) => {
                        self.errors.insert(reference, e);self.outputs.remove(&reference);self.pending.remove(&reference);self.job=None;
                    }
                }
            } else {self.job=None;}
            changed = true;
        }
        if self.job.is_none()
            && let Some(reference) = self.requests.iter().min_by_key(|(_,r)|r.offset+r.prepared.as_ref().map_or(0,|s|s.ready() as i64)).map(|(id,_)|*id)
        {
            let mut request=self.requests.remove(&reference).unwrap();
            let settings = self.settings.clone();
            let cancel = Arc::new(AtomicBool::new(false));
            let stop = Arc::clone(&cancel);
            let wake = ctx.clone();
            let (tx, rx) = mpsc::channel();
            let hash = request.hash;
            thread::spawn(move || {
                let result=(||->Result<Update,String>{
                    if request.prepared.is_none(){
                        let prepared=backend::prepare(&settings,&request.entry,std::mem::take(&mut request.notes),request.ambient,request.players,stop.clone())?;
                        tx.send(Ok(Update::Prepared(prepared.pitch.clone(),prepared.member_pitch.clone(),prepared.controls.clone()))).map_err(|_|"Instrument render cancelled")?;wake.request_repaint();
                        request.prepared=Some(Box::new(prepared));
                    }
                    let prepared=request.prepared.as_mut().unwrap();
                    let finished=prepared.advance(stop,&mut |at,pcm|{
                        tx.send(Ok(Update::Source(at,Arc::from(pcm)))).map_err(|_|"Instrument render cancelled")?;wake.request_repaint();Ok(())
                    })?;
                    if finished {Ok(Update::Complete)}
                    else {Ok(Update::Yield(request))}
                })();
                let _=tx.send(result);
                wake.request_repaint();
            });
            self.job = Some(Job {
                reference,
                hash,
                cancel,
                rx,
            });
        }
        changed |= self.mixer.poll(&self.outputs, ctx);
        for (id,error) in self.mixer.errors() {self.errors.insert(id,error.clone());self.mix_errors.insert(id,error.clone());}
        changed
    }
    pub fn sync(
        &mut self,
        project: &Project,
        settings: InstrumentSettings,
        ctx: &eframe::egui::Context,
    ) {
        self.configure(settings, ctx);
        for (id,error) in std::mem::take(&mut self.mix_errors) {
            if self.errors.get(&id)==Some(&error){self.errors.remove(&id);}
        }
        if project.tracks.iter().any(|t| t.is_instrument()) {
            self.ensure_scan(ctx);
        }
        let mut retain = std::collections::BTreeSet::new();
        for track in &project.tracks {
            let Some(config) = &track.instrument else {
                continue;
            };
            let Some(entry) = self.for_track(track).cloned() else {
                for reference in track.references() {
                    self.outputs.remove(&reference.id);
                    self.hashes.remove(&reference.id);
                    self.requests.remove(&reference.id);
                    if self.job.as_ref().is_some_and(|j| j.reference == reference.id) {
                        self.job = None;
                    }
                    if config.selection.is_none() && track.main_ref.database.get("name").is_none() {
                        continue;
                    }
                    if reference.mute
                        || !project
                            .group(&reference.group_id)
                            .is_some_and(|g| g.notes.iter().any(|n| !n.muted()))
                    {
                        continue;
                    }
                    retain.insert(reference.id);
                    if self.is_scanning() {
                        self.pending.insert(reference.id, 0..1);
                    } else {
                        self.pending.remove(&reference.id);
                        self.errors.insert(
                            reference.id,
                            self.scan_error.clone().unwrap_or_else(|| {
                                "Instrument is not installed or selected".into()
                            }),
                        );
                    }
                }
                continue;
            };
            let selected = &entry.selection;
            for reference in track.references() {
                let id = reference.id;
                if reference.mute {
                    continue;
                }
                let Some(group) = project.group(&reference.group_id) else {
                    continue;
                };
                let (transpose,players)=match super::performance::transpose(reference).and_then(|t|super::players(reference).map(|n|(t,n))) {
                    Ok(value)=>value,
                    Err(error)=>{
                        retain.insert(id);self.outputs.remove(&id);self.pending.remove(&id);
                        self.hashes.remove(&id);self.requests.remove(&id);
                        if self.job.as_ref().is_some_and(|j|j.reference==id){self.job=None;}
                        self.errors.insert(id,error);continue;
                    }
                };
                let mut notes:Vec<_>=group.notes.iter().filter(|n|!n.muted()).filter_map(|n|{
                    let onset=n.onset+reference.blick_offset;let end=onset+n.duration;
                    if end<=reference.blick_absolute_begin||onset>=reference.visible_end(){return None}
                    let start=(project.time.seconds_at(onset)*200.0).round() as i64;
                    let length=(project.time.seconds_at(end)*200.0).round() as i64-start;
                    let takes=n.extra.get("takes");let active=takes.and_then(|v|v.get("activeTakeId")).and_then(Value::as_u64).unwrap_or(0);
                    let take=takes.and_then(|v|v.get("takes")).and_then(Value::as_array).and_then(|a|a.iter().find(|t|t.get("id").and_then(Value::as_u64)==Some(active)));
                    Some(json!({
                        "id":n.id,"start":start,"length":length.max(1),
                        "duration_ticks":n.duration,
                        "time_dispersion":n.attributes.get("cTimeDispersion").cloned().unwrap_or(json!(1.0)),
                        "pitch_dispersion":n.attributes.get("cPitchDispersion").cloned().unwrap_or(json!(0.0)),
                        "dynamic":n.attributes.get("dynamic").cloned().unwrap_or(json!(0.5)),
                        "automatic_dynamics":!n.attributes.get("dynamic").is_some_and(Value::is_number),
                        "automatic_articulations":n.attributes.get("articulationsFixed").and_then(Value::as_bool)==Some(false),
                        "direction":reference.extra.get("voice").and_then(|v|v.get("performanceDirection")).cloned().unwrap_or(json!("")),
                        "articulations":n.attributes.get("articulations").cloned().unwrap_or(json!([])),
                        "vibrato_depth":n.attributes.get("dF0VbrMod").cloned().unwrap_or(json!(1.0)),
                        "vibrato_frequency":n.attributes.get("fF0VbrMod").cloned().unwrap_or(json!(0.0)),
                        "vibrato_envelope":super::vibrato::envelope(group.parameters.get("vibratoEnv"),n,length.max(1) as usize),
                        "pitch":(n.pitch+reference.pitch_offset) as f64+n.extra.get("detune").and_then(Value::as_f64).unwrap_or(0.0)/100.0+transpose,
                        "pitch_seed":take.and_then(|t|t.get("seedPitch")).and_then(Value::as_u64).unwrap_or(0),
                        "timbre_seed":take.and_then(|t|t.get("seedTimbre")).and_then(Value::as_u64).unwrap_or(0)
                    }))
                }).collect();
                notes.sort_by_key(|n| n["start"].as_i64().unwrap_or(0));
                if notes.is_empty() {
                    continue;
                }
                retain.insert(id);
                if entry.auxiliary.is_none(){
                    self.outputs.remove(&id);self.pending.remove(&id);self.requests.remove(&id);
                    self.errors.insert(id,"Instrument auxiliary model is not installed in the selected version directory".into());
                    continue;
                }
                let first = notes[0]["start"].as_i64().unwrap();
                let offset = (first - 100) * 240;
                for n in &mut notes {
                    n["start"] = json!(n["start"].as_i64().unwrap() - first + 100);
                }
                let encoded = serde_json::to_vec(&(
                    selected,
                    offset,
                    &notes,
                    config.ambient_seed,
                    players,
                    self.settings.key(selected),
                    &entry.identity,
                    entry
                        .auxiliary
                        .as_ref()
                        ,
                    entry
                        .auxiliary
                        .as_ref()
                        .and_then(|p| std::fs::metadata(p).ok())
                        .map(|m| {
                            (
                                m.len(),
                                m.modified()
                                    .ok()
                                    .and_then(|d| d.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_nanos()),
                            )
                        }),
                ))
                .unwrap();
                let hash = xxhash_rust::xxh3::xxh3_64(&encoded);
                if self.hashes.get(&id) == Some(&hash) {
                    continue;
                }
                self.hashes.insert(id, hash);
                self.outputs.remove(&id);
                self.errors.remove(&id);
                if self.job.as_ref().is_some_and(|j| j.reference == id) {
                    self.job = None;
                }
                let last = notes.last().unwrap();
                let end =
                    ((last["start"].as_i64().unwrap() + last["length"].as_i64().unwrap() + 100)
                        * 240
                        + offset)
                        .max(0) as usize;
                self.pending.insert(id, offset.max(0) as usize..end);
                self.outputs.insert(id,Output {track:track.id,origin:offset as f64/48000.,pitch:Vec::new(),member_pitch:Vec::new(),controls:BTreeMap::new(),
                    source:super::mixing::Source {origin:offset,expected_end:end as i64,players,..Default::default()}});
                self.requests.insert(
                    id,
                    Request {
                        hash,
                        entry: entry.clone(),
                        notes,
                        ambient: config.ambient_seed,
                        players,
                        offset,
                        prepared:None,
                    },
                );
            }
        }
        self.pending.retain(|id, _| retain.contains(id));
        self.hashes.retain(|id, _| retain.contains(id));
        self.outputs.retain(|id, _| retain.contains(id));
        self.errors.retain(|id, _| retain.contains(id));
        self.requests.retain(|id, _| retain.contains(id));
        if self
            .job
            .as_ref()
            .is_some_and(|j| !retain.contains(&j.reference))
        {
            self.job = None;
        }
        let mut specifications=Vec::new();
        for track in &project.tracks {
            if track.instrument.is_none(){continue;}
            let Some(entry)=self.for_track(track).cloned() else {continue};
            let references:Vec<_>=track.references().filter(|r|!r.mute && self.outputs.contains_key(&r.id)).map(|r|r.id).collect();
            if references.is_empty(){continue;}
            let effects=match track.mixer.get("fxParams") {
                Some(value)=>crate::synthesis::effects::Configuration::read(value),None=>Ok(Default::default()),
            };
            let effects=match effects {Ok(v)=>v,Err(e)=>{for id in references {self.errors.insert(id,e.clone());self.mix_errors.insert(id,e.clone());}continue;}};
            let performance:Result<Vec<_>,String>=references.iter().map(|id| {
                let reference=track.references().find(|r|r.id==*id).ok_or("Instrument group reference missing")?;
                let group=project.group(&reference.group_id).ok_or("Instrument group missing")?;
                super::performance::Performance::read(&project.time,group,reference)
            }).collect();
            let performance=match performance {Ok(v)=>v,Err(e)=>{for id in references {self.errors.insert(id,e.clone());self.mix_errors.insert(id,e.clone());}continue;}};
            let microphones=super::microphones::Configuration::read(track.mixer.get("micParams"),&entry.microphones,entry.room.as_deref());
            let microphones=match microphones {Ok(v)=>v.mix(&entry.microphones),Err(e)=>{for id in references {self.errors.insert(id,e.clone());self.mix_errors.insert(id,e.clone());}continue;}};
            let inputs:Vec<_>=references.iter().map(|id|(id,self.hashes.get(id))).collect();
            let hash=xxhash_rust::xxh3::xxh3_64(&serde_json::to_vec(&(&inputs,&microphones,track.mixer.get("fxParams"),&performance)).unwrap());
            specifications.push(super::mixing::Specification {id:track.id,hash,references,entry,microphones,effects,performance});
        }
        self.mixer.sync(specifications,&self.outputs);
    }
}
