use super::{Entry, native::Microphones};
use crate::synthesis::{audio::{ChannelLayout, RenderedAudio}, effects::{Configuration, Processor}};
use std::{collections::BTreeMap, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, thread};

#[derive(Clone, Default)]
pub struct Source {
    pub origin: i64,
    pub expected_end: i64,
    pub frames: usize,
    pub complete: bool,
    pub control_frames: usize,
    pub players: usize,
    pub chunks: Vec<Arc<[f32]>>,
}
impl Source {
    pub fn append(&mut self, at: usize, samples: Arc<[f32]>) -> Result<(), String> {
        let width=8*self.players.max(1);
        if at != self.frames || samples.len() % width != 0 {
            return Err("Discontinuous instrument source stream".into());
        }
        self.frames += samples.len()/width;
        self.chunks.push(samples);
        Ok(())
    }
    fn add(&self, origin: i64, output: &mut [[f32;4]], performance: &super::performance::Performance) {
        let mut position = self.origin;
        let end = origin + output.len() as i64;
        let width=8*self.players.max(1);
        for chunk in &self.chunks {
            let chunk_end = position + (chunk.len()/width) as i64;
            let begin = origin.max(position);
            let stop = end.min(chunk_end);
            if stop > begin {
                let input = ((begin-position) as usize)*width;
                let destination = (begin-origin) as usize;
                performance.add(self.origin, self.control_frames, self.players.max(1), (begin-self.origin) as usize,
                    &chunk[input..input+(stop-begin) as usize*width],
                    &mut output[destination..destination+(stop-begin) as usize]);
            }
            position = chunk_end;
            if position >= end { break; }
        }
    }
}

pub struct Specification {
    pub id: u64,
    pub hash: u64,
    pub references: Vec<u64>,
    pub entry: Entry,
    pub microphones: Vec<super::microphones::Mix>,
    pub effects: Configuration,
    pub performance: Vec<super::performance::Performance>,
}

pub struct Track {
    spec: Specification,
    state: Option<State>,
    pub parts: Vec<Arc<RenderedAudio>>,
    pub pending: Option<std::ops::Range<usize>>,
    pub error: Option<String>,
    pub complete: bool,
}
struct State {
    effects: Processor,
    microphones: Microphones,
    origin: i64,
    cursor: i64,
}
struct Work {
    id: u64,
    hash: u64,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<(State, bool), String>>,
    pcm: mpsc::Receiver<Arc<RenderedAudio>>,
}
impl Drop for Work { fn drop(&mut self) { self.cancel.store(true, Ordering::Release); } }

#[derive(Default)]
pub struct Mixer { pub tracks: BTreeMap<u64, Track>, job: Option<Work> }
impl Mixer {
    pub fn busy(&self) -> bool { self.tracks.values().any(|t| !t.complete && t.error.is_none()) }
    pub fn reference_busy(&self, reference: u64) -> bool {
        self.tracks.values().any(|t|!t.complete && t.error.is_none() && t.spec.references.contains(&reference))
    }
    pub fn errors(&self) -> impl Iterator<Item=(u64, &String)> {
        self.tracks.values().filter_map(|t|t.error.as_ref().map(|e|(&t.spec.references,e)))
            .flat_map(|(ids,error)|ids.iter().map(move |id|(*id,error)))
    }
    pub fn clear(&mut self) { self.job=None;self.tracks.clear(); }
    pub fn sync(&mut self, specifications: Vec<Specification>, sources: &BTreeMap<u64, super::runtime::Output>) {
        let mut retained=std::collections::BTreeSet::new();
        for spec in specifications {
            retained.insert(spec.id);
            if self.tracks.get(&spec.id).is_some_and(|t|t.spec.hash==spec.hash) { continue; }
            if self.job.as_ref().is_some_and(|j|j.id==spec.id) { self.job=None; }
            let begin=spec.references.iter().filter_map(|r|sources.get(r)).map(|o|o.source.origin).min().unwrap_or(0).max(0) as usize;
            let end=spec.references.iter().filter_map(|r|sources.get(r)).map(|o|o.source.expected_end).max().unwrap_or(0).max(0) as usize;
            self.tracks.insert(spec.id,Track {spec,state:None,parts:Vec::new(),pending:Some(begin..end),error:None,complete:false});
        }
        self.tracks.retain(|id,_|retained.contains(id));
        if self.job.as_ref().is_some_and(|j|!retained.contains(&j.id)) { self.job=None; }
    }
    pub fn poll(&mut self, sources: &BTreeMap<u64, super::runtime::Output>, ctx: &eframe::egui::Context) -> bool {
        let mut changed=false;
        if let Some(job)=&self.job {
            let track=self.tracks.get_mut(&job.id).unwrap();
            while let Ok(audio)=job.pcm.try_recv() {
                if let Some(pending)=&mut track.pending { pending.start=(audio.end_seconds()*48000.).round() as usize;pending.end=pending.end.max(pending.start); }
                track.parts.push(audio);changed=true;
            }
            let result=match job.rx.try_recv() {
                Ok(result)=>Some(result), Err(mpsc::TryRecvError::Disconnected)=>Some(Err("Instrument mixer stopped".into())),_=>None,
            };
            if let Some(result)=result {
                while let Ok(audio)=job.pcm.try_recv() { track.parts.push(audio); }
                if track.spec.hash==job.hash {
                    match result {
                        Ok((state,done))=>{track.state=Some(state);track.complete=done;if done {track.pending=None;}},
                        Err(error)=>{track.error=Some(error);track.parts.clear();track.pending=None;},
                    }
                }
                self.job=None;changed=true;
            }
        }
        if self.job.is_none() {
            let candidate=self.tracks.iter().filter(|(_,t)|!t.complete && t.error.is_none())
                .filter_map(|(&id,t)| {
                    let input:Option<Vec<_>>=t.spec.references.iter().map(|r|sources.get(r).map(|o|o.source.clone())).collect();
                    let input=input?;
                    let origin=input.iter().map(|s|s.origin).min()?;
                    let complete=input.iter().all(|s|s.complete);
                    let available=input.iter().filter(|s|!s.complete).map(|s|s.origin+s.frames as i64).min().unwrap_or(i64::MAX);
                    let source_end=input.iter().map(|s|s.origin+s.frames as i64).max()?;
                    let tail=if t.spec.effects.enabled() {
                        (24000i64 + t.spec.effects.reverb.map_or(0,|p|(p.decay*48000.) as i64)).min(960000)
                    } else {0};
                    let end=source_end.saturating_add(if complete {tail} else {0});
                    let cursor=t.state.as_ref().map_or(origin,|s|s.cursor);
                    let until=available.min(end).min(cursor+65536);
                    if until<=cursor && !(complete && until==end) {return None;}
                    Some((id,origin,cursor,until,complete && until==end,input))
                }).min_by_key(|c|c.2);
            if let Some((id,origin,_,until,done,input))=candidate {
                let track=self.tracks.get_mut(&id).unwrap();
                let mut state=track.state.take();
                let main=track.spec.entry.main.clone();let mics=track.spec.microphones.clone();let settings=track.spec.effects.clone();
                let performance=track.spec.performance.clone();
                let hash=track.spec.hash;let cancel=Arc::new(AtomicBool::new(false));let stop=cancel.clone();let wake=ctx.clone();
                let (tx,rx)=mpsc::channel();let (pcm_tx,pcm)=mpsc::channel();
                thread::spawn(move || {
                    let result=(|| -> Result<(State,bool),String> {
                        let mut state=match state.take() {Some(s)=>s,None=>State {effects:Processor::new(&settings),microphones:Microphones::from_path(&main,&mics)?,origin,cursor:origin}};
                        let mut emit=|at:usize,mut samples:Vec<f32>|->Result<(),String> {
                            let position=state.origin+at as i64;
                            if position<0 {let skip=(position.unsigned_abs() as usize).saturating_mul(2).min(samples.len());samples.drain(..skip);}
                            if !samples.is_empty() {
                                let audio=RenderedAudio::new(48000,position.max(0) as f64/48000.,ChannelLayout::Stereo,samples).map_err(|e|e.to_string())?;
                                pcm_tx.send(Arc::new(audio)).map_err(|_|"Instrument mix cancelled")?;wake.request_repaint();
                            }Ok(())
                        };
                        while state.cursor<until {
                            if stop.load(Ordering::Acquire) {return Err("Instrument mix cancelled".into());}
                            let count=(until-state.cursor).min(4096) as usize;
                            let mut block=vec![[0.;4];count];
                            for (source, controls) in input.iter().zip(&performance) {source.add(state.cursor,&mut block,controls);}
                            state.effects.process(&mut block);
                            state.microphones.feed_mixed(&block,false,&stop,&mut emit)?;
                            state.cursor+=count as i64;
                        }
                        if done {state.microphones.feed_mixed(&[],true,&stop,&mut emit)?;}
                        Ok((state,done))
                    })();
                    let _=tx.send(result);wake.request_repaint();
                });
                self.job=Some(Work {id,hash,cancel,rx,pcm});
            }
        }
        changed
    }
}
