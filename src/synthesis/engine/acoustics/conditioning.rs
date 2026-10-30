use super::PreparedRenderAcoustics;
use crate::synthesis::stages::{
    timbre::records::{AcousticNote, PhoneImpulse},
};

impl PreparedRenderAcoustics {
    pub(crate) fn acoustic_pitch(&self)->crate::synthesis::voice::Result<Vec<f32>> {
        if self.family != crate::synthesis::voice::VoiceFamily::Sv1 {
            crate::synthesis::engine::pitch::assemble_note_curves(&self.prepared.phrase,&self.pitch)
        } else {
            let mut values=self.pitch.pitch.clone();
            if self.prepared.phrase.experimental_dds_expression {
                for (frame,value) in values.iter_mut().enumerate() {
                    let seconds=self.prepared.phrase.start_seconds+frame as f64*self.pitch.frame_seconds;
                    let ratio=crate::synthesis::parameters::frame(&self.prepared.phrase,seconds).tone_ratio;
                    if ratio!=1.0 {*value+=12.0*ratio.log2();}
                }
            }
            Ok(values)
        }
    }
    pub fn frame_controls(&self)->Option<Vec<crate::synthesis::stages::timbre::records::FrameControl>> {
        if self.family!=crate::synthesis::voice::VoiceFamily::Sv2 {return None;}
        frame_controls_for(&self.prepared.phrase,&self.duration,&self.pitch)
    }
    pub fn modulation(&self) -> Vec<AcousticNote> {
        let mut note_frames = vec![0_usize; self.prepared.phrase.notes.len()];
        if self.family == crate::synthesis::voice::VoiceFamily::Sv2Compatible {

            for phone in &self.prepared.phones {
                note_frames[phone.note_index] += phone.frames;
            }
        } else {
            for (frames,note) in note_frames.iter_mut().zip(&self.prepared.phrase.notes) {
                *frames=crate::synthesis::stages::f0::frontend::frame_count(note)
                    .expect("prepared note has a validated model clock");
            }
        }
        self.prepared
            .phrase
            .notes
            .iter()
            .zip(note_frames)
            .zip(&self.prepared.phrase.retake_seeds)
            .map(|((note, frames), seeds)| AcousticNote {
                frames,
                timbre_seed: seeds.map(|seeds| seeds.timbre),
                kind: note.kind,

                factor: None,
            })
            .collect()
    }

    pub fn impulses(&self) -> Vec<Option<PhoneImpulse>> {
        self.duration
            .prediction
            .impulse_values
            .iter()
            .zip(&self.duration.prediction.impulse_positions)
            .enumerate()
            .map(|(index,(&_value, &position))| {

                let phone=&self.duration.durations[index];
                let phrase=&self.prepared.phrase;
                let control=phrase.phone_sources[phone.record_index][phone.phone_index]
                    .and_then(|s|phrase.phoneme_controls.get(&s));
                let value=crate::synthesis::score::phonemes::activity(phrase,&self.duration,index).unwrap_or(0.0);
                let position=control.and_then(|c|c.position).unwrap_or(position);
                (value > 0.0).then_some(PhoneImpulse {
                    value,
                    position: f64::from(position),
                })
            })
            .collect()
    }
}

pub(crate) fn frame_controls_for(phrase:&crate::synthesis::score::PreparedPhrase,
    duration:&crate::synthesis::stages::duration::DurationResult,pitch:&crate::synthesis::stages::f0::PitchResult)
    ->Option<Vec<crate::synthesis::stages::timbre::records::FrameControl>> {
        let animated=phrase.automation.as_ref().is_some_and(|c|c.has("toneShift"));
        let mouth=phrase.automation.as_ref().is_some_and(|c|c.has("mouthOpening"));
        let strength=crate::synthesis::score::phonemes::StrengthEnvelope::new(phrase,duration);
        if phrase.voice_parameters.tone_shift==0.0 && !animated && !mouth && strength.is_none() {return None;}
        Some((0..pitch.pitch.len()).map(|frame| {
            let seconds=phrase.start_seconds+frame as f64*pitch.frame_seconds;
            let parameters=crate::synthesis::parameters::frame(phrase,seconds);

            let opening=phrase.automation.as_ref().map_or(0.0,|c|c.value("mouthOpening",seconds,0.0));
            let opening=if f64::from(opening)>-0.01 && f64::from(opening)<0.01 {0.0}else{opening};
            crate::synthesis::stages::timbre::records::FrameControl {pitch_ratio:parameters.tone_ratio,scalars:[opening,strength.as_ref().map_or(0.0,|c|c.sample(seconds))]}
        }).collect())
}
