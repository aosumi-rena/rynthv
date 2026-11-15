mod processing;
use super::{Runtime,mixer};
use crate::synthesis::{audio::{AudioLayer,MorphLayer,RenderedAudio},effects::{Configuration,voice::VoiceProcessor},
    plan::{OwnerId,RunEpoch},runtime::mixer::live::spans::ReadySpans};
use std::{collections::BTreeMap,ops::Range,sync::{Arc,atomic::{AtomicBool,Ordering},mpsc}};

#[derive(Clone,PartialEq,Eq)]
struct Source {epoch:RunEpoch,range:Range<usize>}
struct Input {spec:Source,ready:ReadySpans,completed:Option<Arc<RenderedAudio>>}
struct Track {
    configuration:Configuration,sources:BTreeMap<OwnerId,Source>,range:Range<usize>,cursor:usize,
    processor:Option<VoiceProcessor>,layer:MorphLayer,parts:Vec<AudioLayer>,gains:[f32;2],error:Option<String>,
}
enum Event {Audio(Arc<RenderedAudio>),Done(VoiceProcessor,usize),Failed(String)}
struct Job {track:u64,cancel:Arc<AtomicBool>,events:mpsc::Receiver<Event>}
impl Drop for Job {fn drop(&mut self){self.cancel.store(true,Ordering::Release);}}
#[derive(Default)]
pub(super) struct Effects {tracks:BTreeMap<u64,Track>,inputs:BTreeMap<OwnerId,Input>,job:Option<Job>}
impl Effects {
    pub fn forget_sources(&mut self, ids:impl Iterator<Item=OwnerId>) {
        let ids=ids.collect::<std::collections::BTreeSet<_>>();
        self.inputs.retain(|id,_|!ids.contains(id));
        self.tracks.retain(|_,track|!track.sources.keys().any(|id|ids.contains(id)));
        if self.job.as_ref().is_some_and(|job|!self.tracks.contains_key(&job.track)){self.job=None;}
    }
    pub fn retry(&mut self) {
        self.tracks.retain(|_,track|track.error.is_none());
        if self.job.as_ref().is_some_and(|job|!self.tracks.contains_key(&job.track)){self.job=None;}
    }
    pub fn busy(&self)->bool {self.tracks.values().any(|t|t.cursor<t.range.end&&t.error.is_none())}
    pub fn ready(&self)->bool {self.tracks.values().all(|t|t.cursor==t.range.end&&t.error.is_none())}
    pub fn contains(&self,id:u64)->bool {self.tracks.contains_key(&id)}
    pub fn source(&mut self,id:OwnerId,epoch:RunEpoch,pcm:Option<&Arc<RenderedAudio>>,start:Option<i64>,complete:bool) {
        let Some(input)=self.inputs.get_mut(&id).filter(|i|i.spec.epoch==epoch)else{return};
        if complete && pcm.is_some_and(|p|input.completed.as_ref().is_some_and(|old|Arc::ptr_eq(old,p))){return;}
        if let Some(begin)=start {
            let stop=(begin.max(0)as usize).clamp(input.spec.range.start,input.spec.range.end);
            input.ready.insert_silence(input.spec.range.start..stop);
        }
        if complete {input.ready.insert_silence(input.spec.range.clone());}
        if let Some(pcm)=pcm {let _=input.ready.insert_audio(Arc::clone(pcm),&input.spec.range);}
        if complete {input.completed=pcm.cloned();}
    }
    pub fn playback(&self)->impl Iterator<Item=MorphLayer>+'_ {self.tracks.values().map(|t|t.layer.clone())}
    pub fn layers(&self)->impl Iterator<Item=AudioLayer>+'_ {self.tracks.values().flat_map(|t|t.parts.iter().cloned())}
    pub fn error(&self)->Option<&str>{self.tracks.values().find_map(|t|t.error.as_deref())}
}

impl Runtime {
    pub fn track_effect_progress(&self)->impl Iterator<Item=(u64,usize,bool)>+'_ {
        self.effects.tracks.iter().map(|(&id,t)|
            (id,t.parts.iter().map(|p|p.audio.frames()).sum(),t.cursor==t.range.end&&t.error.is_none()))
    }
    pub(super) fn sync_effects(&mut self) {
        let plan=self.planner.plan();let mut specifications=BTreeMap::<u64,(Configuration,BTreeMap<OwnerId,Source>,[f32;2])>::new();
        for (&id,request) in &plan.requests {
            let Some(out)=plan.output_controls.get(&id)else{continue};
            if out.reference_muted||out.rocoflex||!mixer::has_effects(&out.mixer){continue;}
            let Some(gains)=mixer::morph_gains(&out.mixer,&self.solo_tracks)else{continue};
            let configuration=Configuration::read(&out.mixer["fxParams"]).expect("validated mixer");
            let Ok(range)=mixer::morph_range(request.coverage)else{continue};
            if range.is_empty(){continue;}
            let source=Source {epoch:request.epoch,range};
            let input=self.effects.inputs.entry(id).or_insert_with(||Input {spec:source.clone(),ready:ReadySpans::default(),completed:None});
            if input.spec!=source {*input=Input {spec:source.clone(),ready:ReadySpans::default(),completed:None};}
            specifications.entry(out.track_id).or_insert_with(||(configuration,BTreeMap::new(),gains)).1.insert(id,source);
        }
        self.effects.inputs.retain(|id,_|specifications.values().any(|(_,sources,_)|sources.contains_key(id)));
        self.effects.tracks.retain(|id,_|specifications.contains_key(id));
        for (id,(configuration,sources,gains)) in specifications {
            let same=self.effects.tracks.get(&id).is_some_and(|t|t.configuration==configuration&&t.sources==sources);
            if same {
                let track=self.effects.tracks.get_mut(&id).unwrap();track.gains=gains;track.layer.gains(gains);
                for part in &mut track.parts {part.gains=gains;}
                continue;
            }
            if self.effects.job.as_ref().is_some_and(|j|j.track==id){self.effects.job=None;}
            let first=sources.values().map(|s|s.range.start).min().unwrap();
            let last=sources.values().map(|s|s.range.end).max().unwrap();
            let tail=processing::tail(&configuration);
            let range=first..last.saturating_add(tail);
            let layer=MorphLayer::default();layer.reset(range.clone());
            self.effects.tracks.insert(id,Track {processor:Some(VoiceProcessor::new(&configuration)),configuration,sources,range,cursor:first,layer,parts:Vec::new(),gains,error:None});
        }
        if self.effects.job.as_ref().is_some_and(|j|!self.effects.tracks.contains_key(&j.track)){self.effects.job=None;}
        for owner in &self.export_owners {
            if let Some(request)=plan.requests.get(&owner.owner){self.effects.source(owner.owner,request.epoch,Some(&owner.audio),None,true);}
        }
    }

    pub(super) fn update_effects(&mut self) {
        for track in self.effects.tracks.values_mut() {
            if track.error.is_none() {
                if let Some((_,error))=self.failures.iter().find(|(id,_)|track.sources.contains_key(id)) {
                    track.error=Some(error.clone());track.layer.reset(0..0);
                }
            }
        }
        if self.effects.job.as_ref().is_some_and(|j|self.effects.tracks[&j.track].error.is_some()){self.effects.job=None;}
        self.effects.poll(&self.repaint);
        if let Some(error)=self.effects.error(){self.error=Some(error.to_owned());}
    }
}
