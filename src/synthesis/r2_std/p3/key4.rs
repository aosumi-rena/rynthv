// SPDX-License-Identifier: MIT
use super::{ring::Ring, resample, FFT, RING};
use super::super::{Error, Result, frames::NoiseWave};

struct Entry { samples: Vec<f32>, cursor: usize, gain: f32 }
pub(super) struct State { entries: Vec<Option<Entry>> }
impl State {
    pub fn new() -> Self {Self {entries: Vec::from([None])}}
    pub fn apply(&mut self, previous: Option<&NoiseWave>, ring: &mut Ring, lookahead: usize) -> Result<()> {
        let incoming=previous.map(resample::shape).transpose()?.map_or(0,|(_,n)|n);
        if incoming!=0 && self.entries.len()==8 && self.entries.iter().all(Option::is_some) {
            return Err(Error::Unsupported("embedded wave pending-slot budget"));
        }
        let retained=self.entries.iter().flatten().map(|e|e.samples.len()).sum::<usize>()+incoming;
        let (count,skip,lag)=if let Some(previous)=previous {
            let scaled=(FFT as f32/2.0/44100.0+previous.offset)*44100.0;
            let offset=(f64::from((-scaled).floor())*(-1.0/44100.0)) as f32;
            let raw=(-(44100.0*offset)).floor();
            if !raw.is_finite() || raw<i32::MIN as f32 || raw>=2147483648.0 {
                return Err(Error::Format("embedded wave ring offset"));
            }
            let count=(raw as i32).max(0) as usize;
            if count+FFT>=RING && count>0 {
                let next=1-RING as isize;let old=-((count+FFT) as isize);
                ((-(next+FFT as isize)) as usize,(next-old) as usize,next)
            } else { (count,0,-((count+FFT) as isize)) }
        } else {(0,0,0)};
        let source_count=previous.map_or(0,|p|p.samples.len());
        if count>8192 || retained>8*8192 || retained+4*source_count+2*incoming+count+lookahead+4096>131072 {
            return Err(Error::Unsupported("embedded wave operation budget"));
        }
        if let Some(previous)=previous {
            let samples=resample::convert(previous)?;
            let mut entry=Entry {samples,cursor:0,gain:previous.gain};
            if count>0 {
                let mut early=ring.read(lag,count)?;
                for (i,value) in early.iter_mut().enumerate() {
                    let at=i+skip;let weight=ramp(at,entry.samples.len(),i)*entry.gain;
                    if i<entry.samples.len() {
                        *value=blend(*value,entry.samples[at.min(entry.samples.len()-1)],weight)?;
                    }
                }
                ring.write(lag,&early,false)?;
                entry.cursor=count; // Source does not add the skipped prefix.
            }
            if let Some(index)=self.entries.iter().position(Option::is_none) {self.entries[index]=Some(entry);}
            else {self.entries.push(Some(entry));}
        }
        let mut trailing=ring.read(-(FFT as isize),lookahead)?;
        for slot in &mut self.entries {
            let Some(entry)=slot else {continue;};
            for (i,value) in trailing.iter_mut().enumerate() {
                let at=entry.cursor+i;
                if at<entry.samples.len() {
                    *value=blend(*value,entry.samples[at],ramp(at,entry.samples.len(),at)*entry.gain)?;
                }
            }
            entry.cursor+=lookahead;
            if entry.cursor>=entry.samples.len() {*slot=None;}
        }
        ring.write(-(FFT as isize),&trailing,false)
    }
}
fn ramp(cursor: usize, count: usize, front: usize) -> f32 {
    let mut value=if front<64 {front as f32*(1.0/64.0)} else {1.0};
    if cursor as isize>count as isize-64 {
        value=(count as isize-cursor as isize) as f32*(1.0/64.0);
    }
    value
}
fn blend(old: f32, sample: f32, weight: f32) -> Result<f32> {
    let value=weight*sample+(1.0-weight)*old;
    if value.is_finite() {Ok(value)} else {Err(Error::Format("embedded wave finite blend"))}
}
