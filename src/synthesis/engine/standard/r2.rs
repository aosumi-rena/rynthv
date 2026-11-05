//! Frozen RynthV score/control adapter. Musical ticks and project time origin
//! stay separate; the native bank pipeline owns phonetics, selection and F0.
use std::{path::PathBuf,sync::atomic::AtomicBool};
use crate::synthesis::{
    r2_std::{self,NativeRenderer,Resources,Phrase,Note,Tempo,ControlProvider,ControlSample,PitchMode},
    plan::OwnerInput,voice::{Result,VoiceError},
};
use super::{controls::Controls,NativeAudio,NativePhone};

pub(super) fn open(bank:PathBuf,classifier_directory:PathBuf,f0_model:PathBuf)->Result<NativeRenderer>{
    NativeRenderer::open(Resources{bank_path:bank,clf_directory:classifier_directory,shared_f0_path:f0_model})
        .map_err(|e|VoiceError::new(e.to_string()))
}
struct FrozenControls {controls:Controls,offset:f64}
impl ControlProvider for FrozenControls {
    fn pitch_mode(&self)->PitchMode {PitchMode::SharedAutomatic}
    fn sample(&self,seconds:f64)->r2_std::Result<ControlSample>{
        let global=seconds+self.offset;
        let midi_pitch=if let Some(pitch)=&self.controls.pitch{
            let value=pitch.sample(global,1.0,1).map_err(|e|r2_std::Error::Text(e.to_string()))?[0];
            value.is_finite().then_some(f64::from(value))
        }else{None};
        let generated_shift=if midi_pitch.is_none(){self.controls.transpose}else{0.0};
        Ok(ControlSample{midi_pitch,
            pitch_deviation_semitones:f64::from(self.controls.value("pitchDelta",global,0.0)*0.01+generated_shift),
            loudness_db:f64::from(self.controls.loudness(global)),
            tension:self.controls.expression("tension",global,self.controls.parameters.tension,2.0),
            breathiness:self.controls.expression("breathiness",global,self.controls.parameters.breathiness,2.0),
            gender:self.controls.expression("gender",global,self.controls.parameters.gender,2.0),
            voicing:f64::from(self.controls.value("voicing",global,1.0).clamp(0.0,1.0)),
            tone_shift_cents:f64::from(self.controls.parameters.tone_shift+self.controls.value("toneShift",global,0.0))})
    }
}
pub(super) fn render(renderer:&mut NativeRenderer,input:&OwnerInput,cancel:&AtomicBool,emit:&mut super::stream::NativeObserver<'_>)->Result<NativeAudio>{
    let controls=Controls::new(input)?;
    if controls.parameters.tone_shift!=0.0||controls.automation.as_ref().is_some_and(|c|c.has("toneShift")){
        return Err(VoiceError::new("Tone shift is unavailable in the R2 STD unit-source renderer"));
    }
    if controls.parameters.consonant_strength!=0.0||controls.parameters.consonant_duration!=0.0||input.notes.iter().any(|note|
        note.attributes.get("muted").and_then(serde_json::Value::as_bool)!=Some(true) && note.attributes.get("phonemes").is_some_and(|v|v.as_array().is_none_or(|rows|!rows.is_empty()))){
        return Err(VoiceError::new("R2 STD manual phoneme timing/strength and consonant controls are not implemented"));
    }
    let mut notes=Vec::new();let mut identities=Vec::new();
    for note in &input.notes{
        if note.attributes.get("muted").and_then(serde_json::Value::as_bool)==Some(true){continue;}
        let start=note.onset.0.checked_add(input.content_offset.0).ok_or_else(||VoiceError::new("STD note onset overflows"))?;
        let midi=note.pitch.checked_add(input.pitch_offset).filter(|p|(1..=127).contains(p)).ok_or_else(||VoiceError::new("R2 STD note pitch exceeds supported MIDI range"))?;
        notes.push(Note{local_tick:start,duration_ticks:note.duration,midi_pitch:midi as u8,
            lyric:if note.lyrics.trim().is_empty(){"a".into()}else{note.lyrics.clone()},
            pronunciation:(!note.phonemes.trim().is_empty()).then(||note.phonemes.trim().to_owned())});
        identities.push(note.id);
    }
    if notes.is_empty(){return Err(VoiceError::new("R2 STD phrase has no active notes"));}
    if !input.tempo.origin_seconds.is_finite(){return Err(VoiceError::new("R2 STD project origin is not finite"));}
    let phrase=Phrase{anchor_tick:0,notes,tempos:input.tempo.points.iter().map(|t|Tempo{tick:t.position.0,bpm:t.bpm}).collect()};
    let frozen=FrozenControls{controls,offset:input.tempo.origin_seconds};
    // The native renderer combines its note baseline with the shared F0 model;
    // explicit host pitch overrides only the drawn regions.
    let offset=input.tempo.origin_seconds;
    let mut observer=|event:r2_std::RenderProgress<'_>| {
        use r2_std::RenderProgress as P;
        use super::stream::NativeEvent as E;
        let converted=match event {
            P::Stage(stage)=>E::Stage(match stage {0=>crate::synthesis::engine::RenderStage::Duration,
                1=>crate::synthesis::engine::RenderStage::Pitch,2=>crate::synthesis::engine::RenderStage::Timbre,_=>crate::synthesis::engine::RenderStage::Vocoder}),
            P::Prepared{origin,hop,phones,failures}=>E::Prepared {origin:origin+offset,rate:44100,hop,
                phones:phones.iter().map(|p|NativePhone {note_id:p.note_index.and_then(|i|identities.get(i).copied()),
                    token_index:p.token_index.unwrap_or(0),symbol:p.symbol.clone(),start:p.start_seconds+offset,end:p.end_seconds+offset}).collect(),
                errors:failures.iter().filter_map(|e|identities.get(e.note_index).copied().map(|id|(id,e.message.clone()))).collect(),pitch:Vec::new()},
            P::Pcm{origin,samples,pitch_origin,hop,pitch}=>{
                let points=pitch.iter().enumerate().filter_map(|(i,&hz)|(hz.is_finite()&&hz>0.0).then(||
                    (pitch_origin+offset+i as f64*hop,69.0+12.0*(f64::from(hz)/440.0).log2()))).collect::<Vec<_>>();
                emit(E::Pitch(&points)).map_err(|e|r2_std::Error::Text(e.to_string()))?;
                E::Pcm {origin:origin+offset,samples}
            },
        };
        emit(converted).map_err(|e|r2_std::Error::Text(e.to_string()))
    };
    let result=renderer.render_source_stream(&phrase,&frozen,cancel,&mut observer).map_err(|e|VoiceError::new(e.to_string()))?;
    let offset=input.tempo.origin_seconds;
    let pitch=result.generated_pitch_hz.iter().enumerate().filter_map(|(i,&hz)|{
        (hz.is_finite()&&hz>0.0).then(||(result.pitch_origin_seconds+offset+i as f64*result.pitch_hop_seconds,
            69.0+12.0*(f64::from(hz)/440.0).log2()))
    }).collect();
    let phones=result.phones.into_iter().map(|p|NativePhone{
        note_id:p.note_index.and_then(|i|identities.get(i).copied()),token_index:p.token_index.unwrap_or(0),symbol:p.symbol,
        start:p.start_seconds+offset,end:p.end_seconds+offset}).collect();
    let note_errors=result.note_failures.into_iter().filter_map(|error|identities.get(error.note_index).copied().map(|id|(id,error.message))).collect();
    Ok(NativeAudio{samples:result.samples,rate:result.sample_rate,origin:result.origin_seconds+offset,
        pitch_origin:result.pitch_origin_seconds+offset,pitch_hop:result.pitch_hop_seconds,pitch,phones,note_errors})
}
