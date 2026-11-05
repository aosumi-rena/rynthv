mod controls;
mod r1;
mod r1_sections;
mod r2;
mod output;
mod stream;
pub use stream::StreamEvent;

use std::{path::Path,sync::{Arc,atomic::AtomicBool}};
use crate::synthesis::{voice::{Result,standard::{StdVoice,StdResources}},plan::OwnerInput,rendered::RenderedClip,execution,score};

#[derive(Clone)]
struct NativePhone {note_id:Option<u64>,token_index:usize,symbol:String,start:f64,end:f64}
struct NativeAudio {samples:Vec<f32>,rate:u32,origin:f64,pitch_origin:f64,pitch_hop:f64,pitch:Vec<(f64,f64)>,phones:Vec<NativePhone>,note_errors:std::collections::BTreeMap<u64,String>}
enum Backend {R1(crate::synthesis::r1_std::pipeline::StdBank),R2(crate::synthesis::r2_std::NativeRenderer)}

/// A bank-bound offline renderer shared by editor scheduling and CLI export.
pub struct StandardRenderer {descriptor:StdVoice,backend:Backend}
impl StandardRenderer {
    pub fn open(profile:&Path)->Result<Self> {Self::from_voice(StdVoice::load(profile)?)}
    pub(crate) fn from_voice(descriptor:StdVoice)->Result<Self> {
        let backend=match descriptor.resources.clone() {
            StdResources::R1 {metadata,data,language_directory,renderer_library}=>Backend::R1(r1::open(metadata,data,language_directory,renderer_library)?),
            StdResources::R2 {bank,classifier_directory,f0_model}=>Backend::R2(r2::open(bank,classifier_directory,f0_model)?),
        };
        Ok(Self {descriptor,backend})
    }
    pub fn render(&mut self,input:&OwnerInput,cancel:&AtomicBool)->Result<Arc<RenderedClip>> {
        self.render_stream(input,cancel,&mut |_|Ok(()))
    }
    pub fn render_stream(&mut self,input:&OwnerInput,cancel:&AtomicBool,emit:&mut dyn FnMut(StreamEvent)->Result<()>)->Result<Arc<RenderedClip>> {
        execution::checkpoint()?;
        score::validate_voice(input,&self.descriptor.configuration.name,&self.descriptor.configuration.version,&self.descriptor.database_id)?;
        let mut errors=crate::synthesis::plan::note_issues::for_input(input);
        for note in &input.notes {
            if note.attributes.get("muted").and_then(serde_json::Value::as_bool)==Some(true) {continue;}
            if self.descriptor.configuration.family==crate::synthesis::voice::VoiceFamily::R2Std && note.pitch.saturating_add(input.pitch_offset)==0 {
                errors.insert(note.id,"音符音高超出范围".into());
            }
            if note.attributes.get("phonemes").is_some_and(|value|value.as_array().is_none_or(|rows|!rows.is_empty())) {
                errors.insert(note.id,"This voice does not support manual phoneme timing".into());
            }
            let start=note.onset.0.saturating_add(input.content_offset.0);
            let begin=input.tempo.seconds_at(crate::synthesis::plan::GlobalBlick(start));
            let end=input.tempo.seconds_at(crate::synthesis::plan::GlobalBlick(start.saturating_add(note.duration)));
            let minimum=if self.descriptor.configuration.family==crate::synthesis::voice::VoiceFamily::R1Std {0.01}else{0.005};
            if end-begin<minimum {errors.insert(note.id,"音符时长无效".into());}
        }
        let mut clean=crate::synthesis::plan::note_issues::muted(input,&errors);
        clean.notes.sort_by_key(|note|(note.onset.0,note.id));
        let mut sink=stream::Sink::new(input,errors.clone(),emit);
        let mut observer=|event:stream::NativeEvent<'_>|sink.update(event);
        let mut audio=if clean.notes.iter().all(|note|note.attributes.get("muted").and_then(serde_json::Value::as_bool)==Some(true)) {
            let coverage=input.coverage().ok_or_else(||crate::synthesis::voice::VoiceError::new("没有可合成的音符"))?;
            let origin=input.tempo.seconds_at(coverage.start).max(0.0);
            let end=input.tempo.seconds_at(coverage.end);
            let count=((end-origin)*f64::from(input.sample_rate)).ceil().max(1.0) as usize;
            if count>32*1024*1024 {return Err(crate::synthesis::voice::VoiceError::new("音频区间过长"));}
            NativeAudio {samples:vec![0.0;count],rate:input.sample_rate,origin,pitch_origin:origin,pitch_hop:0.005,pitch:Vec::new(),phones:Vec::new(),note_errors:Default::default()}
        }else{match &mut self.backend {Backend::R1(bank)=>r1_sections::render(bank,&clean,cancel,&mut observer)?,Backend::R2(renderer)=>r2::render(renderer,&clean,cancel,&mut observer)?}};
        audio.note_errors.extend(errors);
        execution::checkpoint()?;
        let streamed=sink.finish()?;
        output::clip(audio,input,cancel,streamed)
    }
}
