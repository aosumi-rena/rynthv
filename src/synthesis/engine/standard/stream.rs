use super::{NativePhone,output};
use crate::synthesis::{audio::{RenderedAudio,ChannelLayout,conversion::stream::Mono},
    engine::RenderStage,plan::OwnerInput,rendered::{PhonemeTrack,RenderVisuals,Waveform},voice::{Result,VoiceError}};
use std::{collections::BTreeMap,sync::Arc,ops::Range};

pub enum StreamEvent {
    Stage(RenderStage),
    Prepared {phones:PhonemeTrack,model:Range<i64>},
    Audio {audio:Arc<RenderedAudio>,visuals:Arc<RenderVisuals>},
}
pub(super) enum NativeEvent<'a> {
    Stage(RenderStage),
    Prepared {origin:f64,rate:u32,hop:f64,phones:Vec<NativePhone>,errors:BTreeMap<u64,String>,pitch:Vec<(f64,f64)>},
    Pitch(&'a [(f64,f64)]),
    Pcm {origin:f64,samples:&'a [f32]},
}
pub(super) type NativeObserver<'a>=dyn FnMut(NativeEvent<'_>)->Result<()>+'a;

pub(super) struct Sink<'a> {
    input:&'a OwnerInput,emit:&'a mut dyn FnMut(StreamEvent)->Result<()>,
    origin:Option<f64>,rate:u32,hop:f64,raw_cursor:usize,first:usize,stop:Option<usize>,
    pcm:Option<Mono>,converted:Vec<f32>,phones:Vec<NativePhone>,errors:BTreeMap<u64,String>,
    track:PhonemeTrack,pitch:Vec<(f64,f64)>,
}
impl<'a> Sink<'a> {
    pub fn new(input:&'a OwnerInput,errors:BTreeMap<u64,String>,emit:&'a mut dyn FnMut(StreamEvent)->Result<()>)->Self {
        Self {input,emit,origin:None,rate:0,hop:0.005,raw_cursor:0,first:0,stop:None,pcm:None,
            converted:Vec::new(),phones:Vec::new(),errors,track:PhonemeTrack(Arc::from([])),pitch:Vec::new()}
    }
    pub fn update(&mut self,event:NativeEvent<'_>)->Result<()> {
        match event {
            NativeEvent::Stage(stage)=>(self.emit)(StreamEvent::Stage(stage)),
            NativeEvent::Prepared{origin,rate,hop,phones,errors,pitch}=>{
                if self.origin.is_none() {
                    self.origin=Some(origin);self.rate=rate;self.hop=hop;
                    let(begin,end)=self.input.publication_bounds();
                    self.first=((self.input.tempo.seconds_at(begin).max(0.0)-origin)*f64::from(rate)).ceil().max(0.0) as usize;
                    self.stop=end.map(|end|((self.input.tempo.seconds_at(end)-origin)*f64::from(rate)).floor().max(0.0) as usize);
                    self.pcm=Some(Mono::new(rate,self.input.sample_rate));
                } else if self.rate!=rate {return Err(VoiceError::new("STD stream sample-rate changed"));}
                self.phones.extend(phones);self.errors.extend(errors);self.pitch.extend(pitch);
                self.track=output::phone_track(self.phones.clone(),self.errors.clone(),self.input);
                let origin=self.origin.unwrap()+self.first as f64/f64::from(rate);
                let end=self.input.coverage().map_or(0,|c|c.sample_end.0);
                (self.emit)(StreamEvent::Prepared {phones:self.track.clone(),model:(origin*self.input.sample_rate as f64).round() as i64..end})
            },
            NativeEvent::Pitch(pitch)=>{self.pitch.extend_from_slice(pitch);Ok(())},
            NativeEvent::Pcm{origin,samples}=>{
                let base=self.origin.ok_or_else(||VoiceError::new("STD PCM arrived before metadata"))?;
                let start=((origin-base)*f64::from(self.rate)).round();
                if !start.is_finite() || start<0.0 || start>32.0*1024.0*1024.0 {return Err(VoiceError::new("STD stream clock exceeds capacity"));}
                let start=start as usize;
                if start<self.raw_cursor {return Err(VoiceError::new("STD stream PCM overlaps"));}
                // Gaps are genuine source absence from the R1 scheduler, not
                // synthesized or fitted silence masks. Keep the aggregate SRC clock.
                let zeros=[0.0;4096];
                while self.raw_cursor<start {let count=(start-self.raw_cursor).min(zeros.len());self.append(&zeros[..count])?;}
                for block in samples.chunks(4096) {self.append(block)?;}
                Ok(())
            },
        }
    }
    fn append(&mut self,samples:&[f32])->Result<()> {
        let start=self.raw_cursor;let end=start+samples.len();
        let first=start.max(self.first);let stop=end.min(self.stop.unwrap_or(usize::MAX));
        if first<stop {self.pcm.as_mut().unwrap().push(&samples[first-start..stop-start]);}
        self.raw_cursor=end;
        self.drain(self.stop.is_some_and(|stop|end>=stop))
    }
    fn drain(&mut self,finished:bool)->Result<()> {
        while let Some((first,samples))=self.pcm.as_mut().and_then(|pcm|pcm.next_chunk(finished)) {
            crate::synthesis::execution::checkpoint()?;
            // The live mixer rounds an audio origin to its integer sample
            // grid. Anchor ONCE, then add integer offsets: independently
            // rounding fractional origins can leave one-sample holes.
            let origin=self.origin.unwrap()+self.first as f64/f64::from(self.rate);
            let frame=(origin*f64::from(self.input.sample_rate)).round() as i64+first as i64;
            let start=frame as f64/f64::from(self.input.sample_rate);
            self.converted.extend_from_slice(&samples);
            let audio=Arc::new(RenderedAudio::new(self.input.sample_rate,start,ChannelLayout::Mono,samples).map_err(|e|VoiceError::new(e.to_string()))?);
            let left=self.pitch.partition_point(|(t,_)|*t<start-self.hop);
            let right=self.pitch.partition_point(|(t,_)|*t<audio.end_seconds()+self.hop);
            let values=&self.pitch[left..right];
            let base=self.origin.unwrap();let pitch_origin=base+((start-base)/self.hop).floor()*self.hop;
            let pitch=output::pitch_track(values,pitch_origin,self.hop,audio.end_seconds())?;
            let visuals=Arc::new(RenderVisuals {pitch,phonemes:self.track.clone(),waveform:Waveform::new(&audio)});
            (self.emit)(StreamEvent::Audio {audio,visuals})?;
        }
        Ok(())
    }
    pub fn finish(mut self)->Result<Option<RenderedAudio>> {
        if self.origin.is_none() {return Ok(None);}
        self.drain(true)?;
        if self.converted.is_empty(){return Err(VoiceError::new("STD output has no visible audio"));}
        RenderedAudio::new(self.input.sample_rate,self.origin.unwrap()+self.first as f64/f64::from(self.rate),ChannelLayout::Mono,self.converted)
            .map(Some).map_err(|e|VoiceError::new(e.to_string()))
    }
}
