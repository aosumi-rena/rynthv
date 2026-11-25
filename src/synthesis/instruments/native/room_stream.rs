use super::{archive::Archive,Microphones,Result};
use crate::synthesis::instruments::{microphones::Mix,rooms::{self,hall::Hall}};
use std::{collections::VecDeque,path::Path,sync::atomic::{AtomicBool,Ordering}};

pub(crate) enum RoomStream { Recorded(Microphones), Spatial(Box<Spatial>) }
pub(crate) struct Spatial {
    hall:Hall, close:Option<Microphones>, pending:VecDeque<f32>, close_pending:VecDeque<f32>,
    close_complete:bool, emitted:usize, members:usize,
    layout:Vec<Vec<(usize,usize)>>,
}
impl RoomStream {
    pub fn new(path:&Path,selected:&[Mix],room:&str,pack:Option<&Path>,accept:bool,groups:&[usize])->Result<Self> {
        let resources=pack.map(rooms::load).transpose()?;
        let hall=resources.as_ref().and_then(|p|p.room(room)).is_some_and(|r|r.microphones.iter().any(|m|m.responses.iter().any(|r|r.channels==0)));
        if !hall {return Microphones::from_room(path,selected,Some(room),pack,accept).map(Self::Recorded);}
        let resources=resources.unwrap();let program=resources.program(room)?;
        let center: [f64;2]=program.values(0x109,&[2])?.try_into().unwrap();
        let mut positions=Vec::new();let mut layout:Vec<Vec<(usize,usize)>>=Vec::new();
        for (group,&members) in groups.iter().enumerate() {
            for (member,position) in Hall::positions(center,members)?.into_iter().enumerate() {
                let lane=if let Some(i)=positions.iter().position(|p|*p==position) {i}else {
                    positions.push(position);layout.push(Vec::new());positions.len()-1
                };
                layout[lane].push((group,member));
            }
        }
        let members=positions.len();
        let hall=Hall::new(program,selected,&positions)?;
        let close:Vec<_>=selected.iter().filter(|v|v.name.starts_with("Close")).cloned().collect();
        let close=if close.is_empty() {None}else{let archive=Archive::open(path,accept)?;Some(Microphones::new(&archive,&close)?)};
        Ok(Self::Spatial(Box::new(Spatial{hall,close,pending:VecDeque::new(),close_pending:VecDeque::new(),close_complete:false,emitted:0,members,layout})))
    }
    pub fn spatial(&self)->bool {matches!(self,Self::Spatial(_))}
    pub fn layout(&self)->Vec<Vec<(usize,usize)>> {match self {Self::Spatial(p)=>p.layout.clone(),Self::Recorded(_)=>Vec::new()}}
    pub fn feed(&mut self,input:&[Vec<[f32;4]>],frames:usize,last:bool,cancel:&AtomicBool,
        emit:&mut impl FnMut(usize,Vec<f32>)->Result<()>)->Result<()> {
        match self {
            Self::Recorded(p)=>p.feed_mixed(input.first().ok_or("Instrument mix has no sources")?,last,cancel,emit),
            Self::Spatial(p)=>p.feed(input,frames,last,cancel,emit),
        }
    }
}
impl Spatial {
    fn publish(&mut self,emit:&mut impl FnMut(usize,Vec<f32>)->Result<()>)->Result<()> {
        let count=if self.close.is_none()||self.close_complete {self.pending.len()}else{self.pending.len().min(self.close_pending.len())};
        if count==0 {return Ok(());}
        let mut output=Vec::with_capacity(count);
        for _ in 0..count {output.push(self.pending.pop_front().unwrap()+self.close_pending.pop_front().unwrap_or(0.));}
        emit(self.emitted,output)?;self.emitted+=count/2;Ok(())
    }
    fn feed(&mut self,input:&[Vec<[f32;4]>],frames:usize,last:bool,cancel:&AtomicBool,
        emit:&mut impl FnMut(usize,Vec<f32>)->Result<()>)->Result<()> {
        if cancel.load(Ordering::Acquire) {return Err("Instrument mix cancelled".into());}
        self.pending.extend(self.hall.process(input,frames)?);
        if let Some(close)=&mut self.close {
            let mut sum=vec![[0f32;4];frames];
            for member in input {for (dst,src) in sum.iter_mut().zip(member) {for c in 0..4 {dst[c]+=src[c];}}}
            close.feed_mixed(&sum,last,cancel,&mut |_,pcm| {self.close_pending.extend(pcm);Ok(())})?;
        }
        self.close_complete=last;self.publish(emit)?;
        if last {
            let zeros=vec![vec![[0f32;4];256];self.members];let mut tail=0;
            while !self.hall.settled() || !self.close_pending.is_empty() {
                if cancel.load(Ordering::Acquire) {return Err("Instrument mix cancelled".into());}
                self.pending.extend(self.hall.process(&zeros,256)?);self.publish(emit)?;tail+=256;
                if tail>48000*60 {return Err("Hall feedback did not settle".into());}
            }
        }
        Ok(())
    }
}
