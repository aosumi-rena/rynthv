use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
use crate::synthesis::{audio::{RenderedAudio,ChannelLayout},plan::OwnerInput,rendered::{RenderedClip,RenderVisuals,PitchTrack,PhonemeTrack,RenderedPhone,Waveform},score::PhoneSource,voice::{Result,VoiceError}};
use super::NativeAudio;

pub(super) fn clip(output:NativeAudio,input:&OwnerInput,cancel:&AtomicBool,audio_override:Option<RenderedAudio>)->Result<Arc<RenderedClip>> {
    let checkpoint=|| {
        if cancel.load(Ordering::Acquire) {
            return Err(crate::synthesis::audio::AudioError::InvalidControl("STD synthesis cancelled".into()));
        }
        crate::synthesis::execution::checkpoint().map_err(|error|
            crate::synthesis::audio::AudioError::InvalidControl(error.to_string()))
    };
    checkpoint().map_err(|error|VoiceError::new(error.to_string()))?;
    let rate=f64::from(output.rate);
    if !output.origin.is_finite()||!output.pitch_origin.is_finite()||!output.pitch_hop.is_finite()||output.pitch_hop<=0.0||output.rate==0 {
        return Err(VoiceError::new("STD output has an invalid clock"));
    }
    let(begin,end)=input.publication_bounds();
    let begin=input.tempo.seconds_at(begin).max(0.0);
    let first=((begin-output.origin)*rate).ceil().clamp(0.0,output.samples.len()as f64)as usize;
    let stop=end.map_or(output.samples.len(),|end|((input.tempo.seconds_at(end)-output.origin)*rate).floor().clamp(0.0,output.samples.len()as f64)as usize);
    if first>=stop{return Err(VoiceError::new("STD output has no visible audio"));}
    let audio=if let Some(audio)=audio_override {audio} else {
    let audio=RenderedAudio::new(output.rate,output.origin+first as f64/rate,ChannelLayout::Mono,output.samples[first..stop].to_vec()).map_err(|error|VoiceError::new(error.to_string()))?;
    let audio=if audio.sample_rate()!=input.sample_rate {
        crate::synthesis::audio::conversion::convert_with_checkpoint(&audio,input.sample_rate,audio.layout(),checkpoint)
            .map_err(|error|VoiceError::new(error.to_string()))?
    }else{audio};
    audio
    };
    let pitch=pitch_track(&output.pitch,output.pitch_origin,output.pitch_hop,audio.end_seconds())?;
    let phones=phone_track(output.phones,output.note_errors,input);
    checkpoint().map_err(|error|VoiceError::new(error.to_string()))?;
    let visuals=RenderVisuals {pitch,phonemes:phones,waveform:Waveform::new(&audio)};
    Ok(Arc::new(RenderedClip {audio:Arc::new(audio),visuals:Arc::new(visuals),state:None}))
}

pub(super) fn phone_track(phones:Vec<super::NativePhone>,errors:std::collections::BTreeMap<u64,String>,input:&OwnerInput)->PhonemeTrack {
    let mut phones=phones.into_iter().filter(|phone|phone.note_id.is_none_or(|id|!errors.contains_key(&id))).map(|phone|RenderedPhone {
        token:phone.symbol,start_seconds:phone.start,end_seconds:phone.end,predicted_start_seconds:phone.start,predicted_end_seconds:phone.end,
        source:phone.note_id.map(|note_id|PhoneSource {note_id,phone_index:phone.token_index}),boundary_strength:0.0,boundary_position:0.0,
        has_activity:false,strength:1.0,error:None,predicted_activity:None,predicted_position:0.0,
    }).collect::<Vec<_>>();
    for (note_id,message) in errors {
        if let Some(note)=input.notes.iter().find(|note|note.id==note_id) {
            let start=input.tempo.seconds_at(crate::synthesis::plan::GlobalBlick(note.onset.0.saturating_add(input.content_offset.0)));
            let end=input.tempo.seconds_at(crate::synthesis::plan::GlobalBlick(note.onset.0.saturating_add(input.content_offset.0).saturating_add(note.duration.max(1))));
            phones.push(RenderedPhone {token:String::new(),start_seconds:start,end_seconds:end,predicted_start_seconds:start,predicted_end_seconds:end,
                source:Some(PhoneSource {note_id,phone_index:0}),boundary_strength:0.0,boundary_position:0.0,has_activity:false,strength:0.0,
                error:Some(Arc::from(message)),predicted_activity:None,predicted_position:0.0});
        }
    }
    PhonemeTrack(phones.into())
}

pub(super) fn pitch_track(points:&[(f64,f64)],origin:f64,hop:f64,end:f64)->Result<PitchTrack> {
    let count=((end-origin)/hop).ceil().max(0.0)as usize;
    if count>2_000_000{return Err(VoiceError::new("STD pitch display is too large"));}
    let mut values=vec![0.0;count];let mut present=vec![false;count];
    for(time,midi)in points.iter().copied() {
        if !time.is_finite()||!midi.is_finite(){return Err(VoiceError::new("STD pitch contains a nonfinite value"));}
        let index=((time-origin)/hop).round();
        if index>=0.0&&(index as usize)<count{values[index as usize]=midi as f32;present[index as usize]=true;}
    }
    let mut segments=Vec::new();let mut cursor=0;
    while cursor<count{if !present[cursor]{cursor+=1;continue;}let first=cursor;while cursor<count&&present[cursor]{cursor+=1;}segments.push(first..cursor);}
    Ok(PitchTrack {start_seconds:origin,frame_seconds:hop,values:values.into(),segments,rap_controls:Arc::from([]),unison_range:None})
}
