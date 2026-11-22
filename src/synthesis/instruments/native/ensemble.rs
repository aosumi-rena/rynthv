use super::{Result, program::Program, render, score::Note, wave_stream::WaveStream};
use super::super::{EffectiveNote, Entry, InstrumentSettings};
use std::{collections::VecDeque, sync::{Arc, atomic::AtomicBool}};

struct Member {
    stream: WaveStream,
    pending: VecDeque<f32>,
    complete: bool,
}
pub struct EnsembleStream {
    members: Vec<Member>,
    program: Option<Program>,
    published: usize,
    pub pitch: Vec<f32>,
    pub member_pitch: Vec<Vec<f32>>,
    pub controls: Vec<EffectiveNote>,
}

fn fnv(bytes: &[u8], mut state: u64) -> u64 {
    for &byte in bytes { state=(state^u64::from(byte)).wrapping_mul(0x100000001b3); }
    state
}
fn combine(a:u64,b:u64)->u64 {
    a^b.wrapping_add(a<<6).wrapping_add(a>>2).wrapping_add(0x9e3779b9)
}
fn pitch_offsets(notes:&[Note],member:u32)->Result<Vec<f64>> {
    let raw=notes.iter().map(|note| {
        if note.duration_ticks==0 {return Err("Instrument ensemble requires original note duration".into());}
        let pitch=note.pitch as f32;
        let pitch=if pitch==0. {0.}else{pitch};
        let a=fnv(&note.duration_ticks.to_le_bytes(),0xcbf29ce484222325);
        let b=fnv(&pitch.to_le_bytes(),0xcbf29ce484222325);
        let seed=note.pitch_seed.wrapping_add(member.wrapping_mul(7919));
        let h=combine(combine(a,b),fnv(&seed.to_le_bytes(),0xffff9ce484222325));
        let draw=(h.wrapping_mul(0x5deece66d).wrapping_add(11)>>16) as u32;
        Ok(((draw as f32*2f32.powi(-32)).min(0.9999998807907104)-0.5) as f64)
    }).collect::<Result<Vec<_>>>()?;
    Ok(notes.iter().enumerate().map(|(i,n)| {
        let mut total=0f64;
        for delta in -2isize..=2 {
            total+=raw[(i as isize+delta).clamp(0,raw.len() as isize-1) as usize];
        }
        (total*f64::from(n.pitch_dispersion))/5.
    }).collect())
}

pub fn prepare(settings:&InstrumentSettings,entry:&Entry,notes:Vec<serde_json::Value>,
    ambient:u32,players:usize,cancel:Arc<AtomicBool>,
    waveform:Option<&std::path::Path>)->Result<EnsembleStream> {
    if !(1..=128).contains(&players){return Err("Invalid instrument player count".into());}
    let notes=notes.into_iter().map(serde_json::from_value)
        .collect::<std::result::Result<Vec<Note>,_>>().map_err(|e|format!("Instrument score: {e}"))?;
    let main = super::archive::Archive::open(&entry.main, settings.accept_any_v4_signature)?;
    let auxiliary = super::auxiliary::open(entry, settings.accept_any_v4_signature)?;
    let wave_archive = match waveform {
        Some(path) => super::open_waveform(path, settings.accept_any_v4_signature)?,
        None => main.clone(),
    };
    let mut members=Vec::with_capacity(players);
    let mut member_pitch=Vec::with_capacity(players);
    let mut controls=Vec::new();
    for member in 0..players {
        let mut input=notes.clone();
        if players>1 {
            let offsets=pitch_offsets(&notes,member as u32)?;
            for (note,offset) in input.iter_mut().zip(offsets) {
                note.pitch=((note.pitch as f32 as f64+offset) as f32) as f64;
                let delta=(member as u32).wrapping_mul(7919);
                for seed in [&mut note.pitch_seed,&mut note.timbre_seed,
                    &mut note.articulation_seed,&mut note.dynamics_seed] {
                    *seed=seed.wrapping_add(delta);
                }
            }
        }
        let stream=render::prepare_member(settings,entry,input,
            ambient.wrapping_add((member as u32).wrapping_mul(3)),players>1,cancel.clone(),&main,&auxiliary,wave_archive.clone())?;
        if member==0 {controls=stream.controls.clone();}
        if member_pitch.first().is_some_and(|p:&Vec<f32>|p.len()!=stream.pitch.len()) {
            return Err("Instrument member frame clocks disagree".into());
        }
        member_pitch.push(stream.pitch.clone());
        members.push(Member {stream,pending:VecDeque::new(),complete:false});
    }
    Ok(EnsembleStream {pitch:member_pitch[0].clone(),member_pitch,controls,
        members,program:None,published:0})
}
impl EnsembleStream {
    pub fn ready(&self)->usize {self.published}
    pub fn advance(&mut self,cancel:Arc<AtomicBool>,
        emit:&mut impl FnMut(usize,Vec<f32>)->Result<()>)->Result<bool> {
        let count=self.members.len();
        let index=self.members.iter().enumerate().filter(|(_,m)|!m.complete)
            .min_by_key(|(_,m)|m.stream.ready()).map(|(i,_)|i);
        if let Some(index)=index {
            let member=&mut self.members[index];
            member.stream.program=self.program.take();
            member.complete=member.stream.advance(cancel,&mut |at,data| {
                if at!=self.published+member.pending.len()/8 {
                    return Err("Discontinuous instrument member stream".into());
                }
                member.pending.extend(data);Ok(())
            })?;
            self.program=member.stream.program.take();
        }
        let available=self.members.iter().map(|m|m.pending.len()/8).min().unwrap_or(0);
        if available>0 {
            let mut output=Vec::with_capacity(available*8*count);
            for _ in 0..available {
                for member in &mut self.members {
                    for _ in 0..8 {output.push(member.pending.pop_front().unwrap());}
                }
            }
            emit(self.published,output)?;
            self.published+=available;
        }
        let complete=self.members.iter().all(|m|m.complete);
        if complete&&self.members.iter().any(|m|!m.pending.is_empty()) {
            return Err("Instrument member source lengths disagree".into());
        }
        Ok(complete)
    }
}
