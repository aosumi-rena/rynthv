mod reader;
mod models;
mod bank;
mod data;
mod language;
mod renderer;
mod wave;
mod score;
mod selection;
mod guides;
mod pitch;
mod warp;
mod frame_ops;
mod spectrum;
mod synthesis;
mod transform;
pub(crate) use synthesis::render;

use std::{path::PathBuf,sync::atomic::{AtomicBool,Ordering}};
pub(crate) use bank::StdBank;
pub(super) type Result<T> = std::result::Result<T,StdError>;

#[derive(Debug,Clone)]
pub(crate) enum StdError { Invalid(String), Unsupported(String), Io(String), Cancelled, Native(String) }
impl std::fmt::Display for StdError {
 fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result { match self {
  Self::Invalid(s)|Self::Unsupported(s)|Self::Io(s)|Self::Native(s)=>f.write_str(s),Self::Cancelled=>f.write_str("R1 synthesis cancelled")
 }}
}
impl std::error::Error for StdError{}
impl From<std::io::Error> for StdError {fn from(e:std::io::Error)->Self{Self::Io(e.to_string())}}
pub(super) fn ensure(ok:bool,message:&str)->Result<()> {if ok{Ok(())}else{Err(StdError::Invalid(message.into()))}}
pub(super) fn cancel(flag:&AtomicBool)->Result<()> {if flag.load(Ordering::Relaxed){Err(StdError::Cancelled)}else{Ok(())}}

#[derive(Clone,Debug)]
pub(crate) struct StdBankPaths {pub metadata:PathBuf,pub data:PathBuf,pub language_dir:PathBuf,pub renderer_library:PathBuf}
#[derive(Clone,Debug)]
pub(crate) struct StdNote {pub start_seconds:f64,pub end_seconds:f64,pub midi:u8,pub lyric:String}
#[derive(Clone,Debug,Default)]
pub(crate) struct StdCurve {pub points:Vec<(f64,f64)>}
impl StdCurve {
 pub(super) fn sample_absolute(&self,time:f64)->f64 {
  let i=self.points.partition_point(|p|p.0<time);
  if i<self.points.len()&&self.points[i].0==time{return self.points[i].1;}
  if i==0||i==self.points.len(){return f64::NAN;}
  let(x,a)=self.points[i-1];let(y,b)=self.points[i];
  if a.is_nan()||b.is_nan(){f64::NAN}else{a+(b-a)*((time-x)/(y-x))}
 }
 pub(super) fn sample(&self,time:f64,default:f64)->f64 {
  if self.points.is_empty(){return default;}
  let i=self.points.partition_point(|p|p.0<time);
  if i==0{return self.points[0].1;} if i==self.points.len(){return self.points[i-1].1;}
  let (x,a)=self.points[i-1];let(y,b)=self.points[i];a+(b-a)*((time-x)/(y-x))
 }
 pub(super) fn validate(&self)->Result<()> {
  self.validate_missing(false)
 }
 pub(super) fn validate_missing(&self,allow_missing:bool)->Result<()> {
  ensure(self.points.len()<=1_000_000 && self.points.iter().all(|p|p.0.is_finite()&&(p.1.is_finite()||(allow_missing&&p.1.is_nan())))&&
   self.points.windows(2).all(|p|p[0].0<p[1].0),"Invalid R1 control curve")
 }
}
#[derive(Clone,Debug,Default)]
pub(crate) struct StdControls {
 pub request_origin_seconds:Option<f64>,
 pub absolute_pitch:StdCurve,
 pub pitch_deviation:StdCurve,
 pub generated_pitch_shift:f64,
 pub tension:StdCurve,pub breathiness:StdCurve,pub gender:StdCurve
}
#[derive(Clone,Debug)]
pub(crate) struct StdPhoneme {pub symbol:String,pub start_seconds:f64,pub duration_seconds:f64,pub note_index:usize,pub leading:bool}
#[derive(Clone,Debug)]
pub(crate) struct StdNoteFailure {pub note_index:usize,pub message:String}
#[derive(Clone,Debug)]
pub(crate) struct StdAudio {pub sample_rate:u32,pub start_seconds:f64,pub mono:Vec<f32>,pub pitch:Vec<(f64,f64)>,pub phonemes:Vec<StdPhoneme>,pub note_failures:Vec<StdNoteFailure>}

pub(crate) enum RenderProgress<'a> {
 Stage(u8),
 Prepared {origin:f64,hop:f64,rate:u32,phones:&'a [StdPhoneme],failures:&'a [StdNoteFailure],pitch:&'a [(f64,f64)]},
 Pcm {origin:f64,samples:&'a [f32]},
}
