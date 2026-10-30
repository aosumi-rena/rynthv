use super::{RenderResult, RenderStage};
use crate::synthesis::{
    score::PreparedPhrase,
    stages::timbre::{TimbreOutput,TimbreRequest,records::AcousticNote},
    voice::{PreparedVoice,Result,VoiceError},
};

pub struct AcousticMixTarget<'a> {
    pub voice:&'a PreparedVoice,
    pub target:&'a crate::synthesis::experiments::BlendTarget,
    pub curves:Option<&'a crate::synthesis::score::PreparedAutomation>,
    pub inherit_modes:bool,
}

pub fn synthesize_acoustic_mix(voice:&PreparedVoice,identity:Option<u64>,phrase:&PreparedPhrase,targets:&[AcousticMixTarget<'_>],dds:Option<(&crate::synthesis::plan::OwnerInput,&crate::synthesis::experiments::DdsControls)>,mut checkpoint:impl FnMut(RenderStage)->Result<()>)->Result<RenderResult> {
    if targets.is_empty() {return super::synthesize(voice,identity,phrase,checkpoint);}
    if !voice.speakers.is_empty() || phrase.unison.count>1 {
        return Err(VoiceError::new("Acoustic feature mixing needs a single primary member; use rendered voice mixing for ensembles"));
    }
    if targets.len()>3 || targets.iter().any(|t| !t.target.weight.is_finite() || !(0.0..=1.0).contains(&t.target.weight) || t.target.transpose!=0 || t.voice.family!=voice.family || !t.voice.speakers.is_empty()) {
        return Err(VoiceError::new("Invalid acoustic mix target or generation"));
    }
    let mut primary=super::acoustics::synthesize_acoustics(voice,identity,phrase,&mut checkpoint)?;
    if let Some((input,controls))=dds {super::experimental::dds::prepare(voice,input,controls,&mut primary)?;}
    let mut rendered=Vec::new();
    for target in targets {
        if target.target.weight!=0.0 || target.curves.is_some() {rendered.push((target,target_timbre(target.voice,&primary,target.inherit_modes)?));}
    }
    if !rendered.is_empty() {
        let mut values=primary.timbre.acoustics.values.clone();
        for (_,other) in &rendered {
            if other.first_frame!=primary.timbre.first_frame || other.acoustics.frames!=primary.timbre.acoustics.frames || other.acoustics.channels!=primary.timbre.acoustics.channels {
                return Err(VoiceError::new("Target acoustic frame format differs; use rendered voice mixing"));
            }
        }
        for frame in 0..primary.timbre.acoustics.frames {
            let time=phrase.start_seconds+(primary.timbre.first_frame+frame) as f64*crate::synthesis::score::FRAME_SECONDS;
            let total: f32=rendered.iter().map(|(t,_)|weight_at(t,time)).sum();
            for channel in 0..primary.timbre.acoustics.channels {
                let i=frame*primary.timbre.acoustics.channels+channel;
                values[i]*=(1.0-total).max(0.0);
                for (target,other) in &rendered {
                    let weight=weight_at(target,time)/total.max(1.0);
                    if weight!=0.0 {values[i]+=other.acoustics.values[i]*weight;}
                }
            }
        }
        if values.iter().any(|v|!v.is_finite()) {return Err(VoiceError::new("Experimental feature mix is nonfinite"));}
        primary.timbre.acoustics.values=values;
    }
    super::render::publish_acoustic(voice,phrase,primary,checkpoint)
}

fn weight_at(target:&AcousticMixTarget<'_>,time:f64)->f32 {
    super::experimental::weight_at(target.target,target.curves,time)
}

fn target_timbre(voice:&PreparedVoice,primary:&super::AcousticRender,inherit_modes:bool)->Result<TimbreOutput> {
    let phrase=&primary.prepared.phrase;
    let coefficients=if inherit_modes {crate::synthesis::vocal_modes::for_phrase(voice,phrase)?}else{None};
    let mut pitch=if voice.family==crate::synthesis::voice::VoiceFamily::Sv1 {primary.pitch.pitch.clone()}
        else {super::pitch::assemble_note_curves(phrase,&primary.pitch)?};
    if voice.family!=crate::synthesis::voice::VoiceFamily::Sv2 && phrase.experimental_dds_expression {
        for (frame,value) in pitch.iter_mut().enumerate() {
            let ratio=crate::synthesis::parameters::frame(phrase,phrase.start_seconds+frame as f64*primary.pitch.frame_seconds).tone_ratio;
            if ratio!=1.0 {*value+=12.0*ratio.log2();}
        }
    }
    let mut note_frames=vec![0;phrase.notes.len()];
    if voice.family!=crate::synthesis::voice::VoiceFamily::Sv2 {
        for phone in &primary.prepared.phones {note_frames[phone.note_index]+=phone.frames;}
    }else{
        for (frames,note) in note_frames.iter_mut().zip(&phrase.notes) {*frames=crate::synthesis::stages::f0::frontend::frame_count(note)?;}
    }
    let modulation=phrase.notes.iter().zip(&phrase.retake_seeds).zip(note_frames).map(|((note,seeds),frames)|AcousticNote {
        frames,timbre_seed:seeds.map(|s|s.timbre),kind:note.kind,factor:None,
    }).collect::<Vec<_>>();
    let impulses=primary.duration.durations.iter().enumerate().map(|(index,phone)| {
        let control=phrase.phone_sources[phone.record_index][phone.phone_index].and_then(|s|phrase.phoneme_controls.get(&s));
        let value=crate::synthesis::score::phonemes::activity(phrase,&primary.duration,index).unwrap_or(0.0);
        let position=control.and_then(|c|c.position).unwrap_or(primary.duration.prediction.impulse_positions[index]);
        (value>0.0).then_some(crate::synthesis::stages::timbre::records::PhoneImpulse{value,position:f64::from(position)})
    }).collect::<Vec<_>>();
    let controls=(voice.family==crate::synthesis::voice::VoiceFamily::Sv2).then(||super::acoustics::conditioning::frame_controls_for(phrase,&primary.duration,&primary.pitch)).flatten();
    let request=TimbreRequest { dds:None,mode_coefficients:coefficients.as_ref(),notes:&phrase.notes,phones:&primary.prepared.phones,midi_pitch:&pitch,note_modulation:&modulation,phone_impulses:(voice.family==crate::synthesis::voice::VoiceFamily::Sv2).then_some(impulses.as_slice()),frame_controls:controls.as_deref(),requested_frame:0};
    if voice.family!=crate::synthesis::voice::VoiceFamily::Sv2 {
        let mut acoustic_pitch=primary.pitch.clone();acoustic_pitch.pitch=pitch.clone();
        return super::models::dds_timbre(voice)?.render(&request,&acoustic_pitch);
    }
    if voice.family==crate::synthesis::voice::VoiceFamily::Sv2 {
        let network=crate::synthesis::stages::timbre::TimbreNetwork::prepare(voice.model("timbre")?,voice.speaker,voice.retained_acoustic_noise_only)?;
        let features=network.prepare_features(&phrase.notes,&primary.prepared.phones)?;
        return network.render_prepared(&request,&crate::synthesis::stages::timbre::records::RecordsConfig::default(),&features);
    }
    Err(VoiceError::new("Target voice generation is unavailable in this renderer"))
}
