use std::sync::atomic::AtomicBool;
use crate::synthesis::{r1_std::pipeline::{self,StdBank,StdBankPaths,StdNote,StdControls,StdCurve},plan::{OwnerInput,GlobalBlick},voice::{Result,VoiceError}};
use super::{controls::Controls,NativeAudio,NativePhone};

pub(super) fn open(metadata:std::path::PathBuf,data:std::path::PathBuf,language_dir:std::path::PathBuf,renderer_library:std::path::PathBuf)->Result<StdBank> {
    StdBank::load(StdBankPaths {metadata,data,language_dir,renderer_library}).map_err(|error|VoiceError::new(error.to_string()))
}
pub(super) fn render(bank:&StdBank,input:&OwnerInput,cancel:&AtomicBool,emit:&mut super::stream::NativeObserver<'_>)->Result<NativeAudio> {
    let controls=Controls::new(input)?;
    if controls.parameters.consonant_strength!=0.0 || controls.parameters.consonant_duration!=0.0 || input.notes.iter().any(|note|
        note.attributes.get("muted").and_then(serde_json::Value::as_bool)!=Some(true) && note.attributes.get("phonemes").is_some_and(|value|value.as_array().is_none_or(|rows|!rows.is_empty()))) {
        return Err(VoiceError::new("R1 STD manual phoneme timing/strength and consonant controls are not implemented"));
    }
    if controls.parameters.tone_shift!=0.0 || controls.automation.as_ref().is_some_and(|c|c.has("toneShift")||c.has("voicing")) {
        return Err(VoiceError::new("R1 STD currently supports loudness, tension, breathiness and gender; tone shift/voicing are unavailable"));
    }
    let mut notes=Vec::new();let mut sources=Vec::new();
    for note in &input.notes {
        if note.attributes.get("muted").and_then(serde_json::Value::as_bool)==Some(true){continue;}
        let start=note.onset.0.checked_add(input.content_offset.0).ok_or_else(||VoiceError::new("STD note onset overflows"))?;
        let end=start.checked_add(note.duration).ok_or_else(||VoiceError::new("STD note end overflows"))?;
        let midi=note.pitch.checked_add(input.pitch_offset).filter(|p|(0..=127).contains(p)).ok_or_else(||VoiceError::new("STD note pitch exceeds MIDI range"))? as u8;
        let lyric=if !note.phonemes.trim().is_empty(){format!(".{}",note.phonemes.trim())}else if note.lyrics.trim().is_empty(){".a".into()}else{note.lyrics.clone()};
        notes.push(StdNote {start_seconds:input.tempo.seconds_at(GlobalBlick(start)),end_seconds:input.tempo.seconds_at(GlobalBlick(end)),midi,lyric});
        sources.push(note.id);
    }
    if notes.is_empty(){return Err(VoiceError::new("STD phrase has no active notes"));}
    let hop=bank.frame_seconds();
    let origin=notes[0].start_seconds-1.0;
    let duration=notes.last().expect("active notes").end_seconds-origin+1.0;
    let count=(duration/hop).ceil() as usize+1;
    if count>200_000{return Err(VoiceError::new("R1 STD phrase is too long; split it at a rest"));}
    let curve=|key:&str,constant:f32,scale:f32|StdCurve {points:(0..count).map(|i|{
        let seconds=origin+i as f64*hop;
        let value=if matches!(key,"tension"|"breathiness"|"gender") {
            controls.expression(key,seconds,constant,1.2)
        }else{f64::from(constant+controls.value(key,seconds,0.0)*scale)};
        (seconds,value)}).collect()};
    let absolute_pitch=if let Some(pitch)=&controls.pitch {
        StdCurve {points:pitch.sample(origin,hop,count)?.into_iter().enumerate().map(|(i,value)|(origin+i as f64*hop,f64::from(value))).collect()}
    }else{StdCurve::default()};
    // The request window contains real score time before a delayed consonant.
    // Publication can trim this window later, but synthesis must retain it.
    let available_begin=input.tempo.seconds_at(input.publication_bounds().0).max(0.0);
    let request_origin=available_begin.max(notes[0].start_seconds-0.5).min(notes[0].start_seconds);
    let frozen=StdControls {request_origin_seconds:Some(request_origin),absolute_pitch,pitch_deviation:curve("pitchDelta",0.0,0.01),generated_pitch_shift:f64::from(controls.transpose),
        tension:curve("tension",controls.parameters.tension,1.0),breathiness:curve("breathiness",controls.parameters.breathiness,1.0),gender:curve("gender",controls.parameters.gender,1.0)};
    let mut audio_origin=0.0;
    let mut observer=|event:pipeline::RenderProgress<'_>| {
        use pipeline::RenderProgress as P;
        use super::stream::NativeEvent as E;
        let converted=match event {
            P::Stage(stage)=>E::Stage(match stage {0=>crate::synthesis::engine::RenderStage::Duration,
                1=>crate::synthesis::engine::RenderStage::Pitch,2=>crate::synthesis::engine::RenderStage::Timbre,_=>crate::synthesis::engine::RenderStage::Vocoder}),
            P::Prepared {origin,hop,rate,phones,failures,pitch}=>{
                audio_origin=origin;
                let mut counters=std::collections::BTreeMap::<usize,usize>::new();
                let phones=phones.iter().map(|p| {
                    let index=counters.entry(p.note_index).or_default();let token_index=*index;*index+=1;
                    NativePhone {note_id:sources.get(p.note_index).copied(),token_index,symbol:p.symbol.clone(),
                        start:p.start_seconds,end:p.start_seconds+p.duration_seconds}
                }).collect();
                E::Prepared {origin,rate,hop,phones,pitch:pitch.to_vec(),errors:failures.iter().filter_map(|e|
                    sources.get(e.note_index).copied().map(|id|(id,e.message.clone()))).collect()}
            },
            P::Pcm {origin,samples}=>{
                if cancel.load(std::sync::atomic::Ordering::Acquire) {return Err(pipeline::StdError::Cancelled);}
                let first=((origin-audio_origin)*f64::from(bank.output_rate())).round() as usize;
                let values=samples.iter().enumerate().map(|(i,&v)|v*10.0f32.powf(
                    controls.loudness(audio_origin+(first+i) as f64/f64::from(bank.output_rate()))/20.0)).collect::<Vec<_>>();
                return emit(E::Pcm {origin,samples:&values}).map_err(|e|pipeline::StdError::Native(e.to_string()));
            }
        };
        emit(converted).map_err(|e|pipeline::StdError::Native(e.to_string()))
    };
    let output=pipeline::render(bank,&notes,&frozen,cancel,&mut observer).map_err(|error|VoiceError::new(error.to_string()))?;
    let mut samples=output.mono;
    for(i,sample)in samples.iter_mut().enumerate(){
        let db=controls.loudness(output.start_seconds+i as f64/f64::from(output.sample_rate));
        if !db.is_finite()||!(-48.0..=24.0).contains(&db){return Err(VoiceError::new("STD loudness exceeds -48..24 dB"));}
        *sample*=10.0f32.powf(db/20.0);
    }
    let mut counters=std::collections::BTreeMap::<usize,usize>::new();
    let phones=output.phonemes.into_iter().map(|phone|{
        let index=counters.entry(phone.note_index).or_default();let token_index=*index;*index+=1;
        NativePhone {note_id:sources.get(phone.note_index).copied(),token_index,symbol:phone.symbol,start:phone.start_seconds,end:phone.start_seconds+phone.duration_seconds}
    }).collect();
    let note_errors=output.note_failures.into_iter().filter_map(|error|sources.get(error.note_index).copied().map(|id|(id,error.message))).collect();
    Ok(NativeAudio {samples,rate:output.sample_rate,origin:output.start_seconds,pitch_origin:output.start_seconds,pitch_hop:hop,pitch:output.pitch,phones,note_errors})
}
