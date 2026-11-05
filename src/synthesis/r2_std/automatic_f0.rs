mod model;
mod network;
mod baseline;
mod packed;
use super::{Error,Phrase,Result};
use model::{Node,float,strings,text};
use network::{Network,Normal};
use std::{collections::HashMap,path::Path,sync::atomic::{AtomicBool,Ordering}};

pub(super) struct Record {duration:f64,pitch:f32,mode:u8,phones:Vec<String>,boundary:bool,left:f64,right:f64}
struct PhoneSet {name:String,classes:HashMap<String,String>}
pub(crate) struct PitchCurve {pub points:Vec<(f64,f32)>}
impl PitchCurve {
    pub fn sample(&self,seconds:f64)->f32{
        let at=self.points.partition_point(|(t,_)|*t<=seconds);
        if at==0{return 0.0;}if at==self.points.len(){return self.points[at-1].1;}
        let (a,x)=self.points[at-1];let(b,y)=self.points[at];
        x+(y-x)*((seconds-a)/(b-a))as f32
    }
}
pub(crate) struct AutomaticF0 {network:Network,sets:Vec<PhoneSet>,style:Vec<f32>,minimum:Vec<f32>,maximum:Vec<f32>,target:Vec<f32>,hop:f32,margin:f32}
impl AutomaticF0 {
    pub fn open(path:&Path)->Result<Self>{
        let bytes=std::fs::read(path)?;
        if bytes.len()!=1748149{return Err(Error::Unsupported("shared loose F0 model geometry"));}
        let root=Node::parse(&bytes)?;root.require("_g4fgv1",2)?;
        let core=&root.children[0];core.require("_f0g4v1",6)?;
        let formatter=&core.children[0];formatter.require("_ff0fv1",4)?;
        if formatter.body.len()!=16||model::word(&formatter.body,0)?!=6{return Err(Error::Format("F0 formatter width"));}
        let hop=float(&formatter.body,4)?;let margin=float(&formatter.body,8)?;
        if !(0.001..=0.02).contains(&hop){return Err(Error::Format("F0 formatter hop"));}
        let mut sets=Vec::new();
        for node in &formatter.children[..3]{
            node.require("_psv1",0)?;let mut at=0;let name=text(&node.body,&mut at)?;
            let phones=strings(&node.body,&mut at)?;let classes=strings(&node.body,&mut at)?;let _vocabulary=strings(&node.body,&mut at)?;
            if at!=node.body.len()||phones.len()!=classes.len(){return Err(Error::Format("F0 phone set extent"));}
            sets.push(PhoneSet{name,classes:phones.into_iter().zip(classes).collect()});
        }
        let norm=&formatter.children[3];norm.require("cmpu0",3)?;
        let minimum=norm.children[0].vector()?;let maximum=norm.children[1].vector()?;let target=norm.children[2].vector()?;
        root.children[1].require("_stbkv0",4)?;let style=root.children[1].children[0].vector()?;
        if minimum.len()!=6||maximum.len()!=6||target.len()!=2||style.len()!=8||minimum.iter().zip(&maximum).any(|(a,b)|a>=b){return Err(Error::Format("F0 normalization geometry"));}
        Ok(Self{network:Network::read(core)?,sets,style,minimum,maximum,target,hop,margin})
    }
    pub fn predict(&self,phrase:&Phrase,prepared:&[Vec<String>],language:&str,cancel:&AtomicBool)->Result<PitchCurve>{
        phrase.validate()?;if prepared.len()!=phrase.notes.len(){return Err(Error::Format("F0 prepared note mapping"));}
        let set=self.sets.iter().find(|s|s.name.contains(language)).ok_or(Error::Unsupported("shared F0 language"))?;
        let mut normal=Normal::new(0);let mut points=Vec::new();let mut first=0;
        for end in 1..=phrase.notes.len(){
            if end<phrase.notes.len(){let previous=&phrase.notes[end-1];let next=&phrase.notes[end];
                if next.local_tick-(previous.local_tick+previous.duration_ticks)<=352800000{continue;}}
            if cancel.load(Ordering::Relaxed){return Err(Error::Cancelled);}
            let boundary=|pitch:u8|Record{duration:0.5,pitch:pitch as f32,mode:0,phones:vec!["sil".into()],boundary:true,left:0.1,right:0.1};
            let mut records=vec![boundary(phrase.notes[first].midi_pitch)];
            let bpm_at=|tick:i64|phrase.tempos.iter().rev().find(|t|t.tick<=tick).unwrap().bpm;
            for i in first..end {
                let note=&phrase.notes[i];let start=phrase.anchor_tick+note.local_tick;
                if i>first {let previous=&phrase.notes[i-1];let gap=note.local_tick-(previous.local_tick+previous.duration_ticks);
                    if gap>0 {let inherited=phrase.anchor_tick+previous.local_tick;
                        records.push(Record{duration:phrase.seconds_at(inherited+gap)?-phrase.seconds_at(inherited)?,pitch:previous.midi_pitch as f32,
                            mode:2,phones:vec!["sil".into()],boundary:false,left:12.0/bpm_at(inherited),right:12.0/bpm_at(inherited)});}}
                let held=matches!(note.lyric.as_str(),"-"|"ー");
                records.push(Record{duration:phrase.seconds_at(start+note.duration_ticks)?-phrase.seconds_at(start)?,pitch:note.midi_pitch as f32,
                    mode:if held{1}else{2},phones:if held{Vec::new()}else{prepared[i].clone()},boundary:false,left:12.0/bpm_at(start),right:12.0/bpm_at(start)});
            }
            records.push(boundary(phrase.notes[end-1].midi_pitch));
            let (rows,counts)=self.format(&records,set)?;
            let heads=self.network.forward(&rows,&counts,&self.style,&mut normal,cancel)?;
            let preliminary=self.filter(&heads);let base=baseline::build(&records,f64::from(self.hop),preliminary.len())?;
            let first_note=phrase.seconds_at(phrase.anchor_tick+phrase.notes[first].local_tick)?;
            let last=&phrase.notes[end-1];let stop=phrase.seconds_at(phrase.anchor_tick+last.local_tick+last.duration_ticks)?+0.1;
            let mut accepted=first_note-0.1;let mut previous=0.0f32;
            for (i,(&pitch,&baseline))in preliminary.iter().zip(&base).enumerate(){
                let seconds=i as f64*f64::from(self.hop)+first_note-0.5;let delta=pitch-baseline;
                if seconds>accepted&&(delta-previous).abs()>0.01 {points.push((seconds,delta));accepted=seconds;previous=delta;}
                if seconds>stop{break;}
            }
            first=end;
        }
        if points.iter().any(|(t,v)|!t.is_finite()||!v.is_finite()){return Err(Error::Format("F0 output finiteness"));}
        Ok(PitchCurve{points})
    }
    fn format(&self,records:&[Record],set:&PhoneSet)->Result<(Vec<Vec<f32>>,Vec<usize>)>{
        let span=self.maximum[4]-self.minimum[4];let pad=span*self.margin;let low=self.minimum[4]-pad;let inverse=1.0/(self.maximum[4]+pad-low);
        let mut rows=Vec::new();let mut counts=Vec::new();let mut carry=records[0].duration as f32;
        for r in records{
            let mut vowel=false;let mut other=false;
            for p in &r.phones {match set.classes.get(p).map(String::as_str).unwrap_or("silence"){
                "vowel"|"diphthong"=>{vowel=true;break;},"breath"|"silence"=>{},_=>other=true}}
            let gate=r.mode==0||(r.mode!=1&&!other&&!vowel);
            let mut row=vec![if gate{1.0}else{0.0},if r.mode==1{1.0}else{0.0},if vowel{1.0}else{0.0},if r.mode==1{1.0}else{r.phones.len()as f32},r.pitch,((r.duration as f32 as f64)+0.001).ln()as f32];
            row[4]=((row[4]-low)*inverse)*span+self.minimum[4];
            for(c,value)in row.iter_mut().enumerate(){*value=(((*value-self.minimum[c])/(self.maximum[c]-self.minimum[c])).clamp(0.0,1.0))*(self.target[1]-self.target[0])+self.target[0];}
            let next=carry+r.duration as f32;let count=((next/self.hop).round()-(carry/self.hop).round()) as usize;
            if !next.is_finite()||count>100_000{return Err(Error::Format("F0 raster extent"));}
            counts.push(count);rows.push(row);carry=next;
        }Ok((rows,counts))
    }
    fn filter(&self,input:&[f32])->Vec<f32>{
        const WORDS:[u32;15]=[0xbd909091,0xbc40c0c5,0x3d1baf61,0x3da13ec8,0x3de21d31,0x3e083974,0x3e161ff8,0x3e1ac224,0x3e161ff8,0x3e083974,0x3de21d31,0x3da13ec8,0x3d1baf61,0xbc40c0c5,0xbd909091];
        if input.is_empty(){return Vec::new();}
        let extent=self.maximum[4]-self.minimum[4];let pad=extent*self.margin;let low=self.minimum[4]-pad;let width=(self.maximum[4]+pad)-low;
        (0..input.len()+7).map(|i|{let mut y=0.0;for(t,&bits)in WORDS.iter().enumerate(){y+=input[(i+t).saturating_sub(7).min(input.len()-1)]*f32::from_bits(bits);}y*width+low}).collect()
    }
}
