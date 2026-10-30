use crate::synthesis::{
    experiments::{DdsControls,DdsFrameControls},inference::Sequence,plan::OwnerInput,
    score::{PreparedPhrase,PreparedAutomation,FRAME_SECONDS},stages::timbre::{TimbreRequest,TimbreOutput,records::AcousticNote},
    voice::{PreparedVoice,PreparedTree,Result,VoiceError},
};

pub(in crate::synthesis::engine) fn prepare(voice:&PreparedVoice,input:&OwnerInput,c:&DdsControls,primary:&mut super::super::AcousticRender)->Result<()> {
    let phrase=&primary.prepared.phrase;
    let frames=primary.prepared.phones.iter().try_fold(0usize,|n,p|n.checked_add(p.frames)).ok_or_else(||VoiceError::new("DDS timeline overflow"))?;
    let curves=PreparedAutomation::for_experiments(input);
    let controls=super::ExperimentControls::effective(&input.voice.instance_options,input.experiments.settings)?;
    for (key,curve) in input.automation.iter().filter(|(k,_)|k.as_str()=="ddsDefaultStyle" || k.starts_with("ddsPhoneMix_")) {
        curve.validate().map_err(VoiceError::new)?;
        let range=if key=="ddsDefaultStyle" {-3.0..=3.0}else{0.0..=1.0};
        if curve.points.len()>90000 || curve.points.iter().any(|(_,v)|!range.contains(v)) {return Err(VoiceError::new("Invalid DDS automation curve"));}
    }
    let mut bank_rows=None;
    if voice.family!=crate::synthesis::voice::VoiceFamily::Sv2 && c.speaker_vectors && input.experiments.settings.voice_blending {
        let original=bank(voice)?;
        let mut targets=Vec::new();
        for target in &controls.voices {
            if !target.active(&input.automation,input.notes.iter().map(|n|&n.extensions))? {continue;}
            if target.transpose!=0 {return Err(VoiceError::new("Speaker vectors do not support harmony transposition"));}
            let source=input.experiments.sources.iter().find(|s|s.voice==target.voice).ok_or_else(||VoiceError::new("Speaker voice is not loaded"))?;
            if source.prepared.family!=voice.family {return Err(VoiceError::new("Speaker generation differs"));}
            if !c.cross_model_vectors && signature(voice.model("timbre")?)?!=signature(source.prepared.model("timbre")?)? {
                return Err(VoiceError::new("Speaker vectors require the same acoustic model; enable cross-model vectors to experiment"));
            }
            let vector=bank(&source.prepared)?;
            if vector.len()!=original.len() {return Err(VoiceError::new("Speaker vector widths differ"));}
            targets.push((target,vector));
        }
        if !targets.is_empty() {
            let mut rows=Sequence::from_values(original.len(),original.repeat(frames))?;
            for (frame,row) in rows.values.chunks_exact_mut(original.len()).enumerate() {
                let seconds=phrase.start_seconds+frame as f64*FRAME_SECONDS;
                let weights:Vec<_>=targets.iter().map(|(t,_)|super::weight_at(t,curves.as_ref(),seconds)).collect();
                let total:f32=weights.iter().sum();
                for value in row.iter_mut() {*value*=(1.0-total).max(0.0);}
                for ((_,vector),weight) in targets.iter().zip(weights) {for (value,target) in row.iter_mut().zip(vector) {*value+=target*weight/total.max(1.0);}}
            }
            bank_rows=Some(rows);
        }
    }
    let default_weights=c.default_as_style.then(||(0..frames).map(|frame| {
        let offset=curves.as_ref().map_or(0.0,|a|a.value("ddsDefaultStyle",phrase.start_seconds+frame as f64*FRAME_SECONDS,0.0));
        (c.default_weight+offset).clamp(-3.0,3.0)
    }).collect());
    let dds=DdsFrameControls {bank_rows,default_weights};
    let active=dds.bank_rows.is_some() || dds.default_weights.is_some() || (voice.family!=crate::synthesis::voice::VoiceFamily::Sv2
        && (phrase.voice_parameters.tone_shift!=0.0 || phrase.automation.as_ref().is_some_and(|a|a.has("toneShift"))));
    if active {primary.timbre=render(voice,primary,&primary.prepared.phones,Some(&dds))?;}
    let mut alternatives=Vec::new();
    for slot in &c.phonemes {
        if slot.token.trim().is_empty() {continue;}
        let key=format!("ddsPhoneMix_{}",slot.id);
        if slot.weight==0.0 && !input.automation.get(&key).is_some_and(|a|a.points.iter().any(|(_,v)|*v>0.0)) {continue;}
        let mut phones=primary.prepared.phones.clone();
        let mut mask=Vec::with_capacity(frames);
        for phone in &mut phones {
            let selected=!matches!(phone.token.as_str(),"sil"|"pau") && (slot.source.is_empty() || slot.source==phone.token);
            mask.extend(std::iter::repeat_n(selected,phone.frames));
            if selected {phone.token=slot.token.clone();if !slot.language.is_empty() {phone.language=slot.language.clone();}}
        }
        if !mask.iter().any(|v|*v) {continue;}
        let rendered=render(voice,primary,&phones,Some(&dds))?;
        if rendered.first_frame!=primary.timbre.first_frame || rendered.acoustics.frames!=primary.timbre.acoustics.frames || rendered.acoustics.channels!=primary.timbre.acoustics.channels {
            return Err(VoiceError::new("Phoneme mix frame formats differ"));
        }
        alternatives.push((slot,key,mask,rendered));
    }
    if !alternatives.is_empty() {
        for frame in 0..primary.timbre.acoustics.frames {
            let source_frame=frame+primary.timbre.first_frame;
            let seconds=phrase.start_seconds+source_frame as f64*FRAME_SECONDS;
            let weights:Vec<_>=alternatives.iter().map(|(slot,key,mask,_)|if mask.get(source_frame)==Some(&true) {
                (slot.weight+curves.as_ref().map_or(0.0,|a|a.value(key,seconds,0.0))).clamp(0.0,1.0)
            }else{0.0}).collect();
            let total:f32=weights.iter().sum();
            for channel in 0..primary.timbre.acoustics.channels {
                let i=frame*primary.timbre.acoustics.channels+channel;
                primary.timbre.acoustics.values[i]*=(1.0-total).max(0.0);
                for ((_,_,_,other),&weight) in alternatives.iter().zip(&weights) {if weight!=0.0 {primary.timbre.acoustics.values[i]+=other.acoustics.values[i]*weight/total.max(1.0);}}
            }
        }
    }
    Ok(())
}

fn render(voice:&PreparedVoice,primary:&super::super::AcousticRender,phones:&[crate::synthesis::stages::f0::TimedPhone],dds:Option<&DdsFrameControls>)->Result<TimbreOutput> {
    let phrase=&primary.prepared.phrase;
    let modern=voice.family==crate::synthesis::voice::VoiceFamily::Sv2;
    let mut counts=vec![0;phrase.notes.len()];
    if modern {for (count,note) in counts.iter_mut().zip(&phrase.notes) {*count=crate::synthesis::stages::f0::frontend::frame_count(note)?;}}
    else {for phone in phones {counts[phone.note_index]+=phone.frames;}}
    let modulation:Vec<_>=phrase.notes.iter().zip(&phrase.retake_seeds).zip(counts).map(|((note,seeds),frames)|AcousticNote {frames,timbre_seed:seeds.map(|s|s.timbre),kind:note.kind,factor:None}).collect();
    let mut pitch=if voice.family!=crate::synthesis::voice::VoiceFamily::Sv1 {super::super::pitch::assemble_note_curves(phrase,&primary.pitch)?}
        else {primary.pitch.pitch.clone()};
    if !modern {for (frame,v) in pitch.iter_mut().enumerate() {
        let time=phrase.start_seconds+frame as f64*primary.pitch.frame_seconds;
        let ratio=crate::synthesis::parameters::frame(phrase,time).tone_ratio;
        if ratio!=1.0 {*v+=12.0*ratio.log2();}
    }}
    let frames=modern.then(||super::super::acoustics::conditioning::frame_controls_for(phrase,&primary.duration,&primary.pitch)).flatten();
    let impulses=modern.then(||primary.duration.durations.iter().enumerate().map(|(index,phone)| {
        let control=phrase.phone_sources[phone.record_index][phone.phone_index].and_then(|s|phrase.phoneme_controls.get(&s));
        let value=crate::synthesis::score::phonemes::activity(phrase,&primary.duration,index).unwrap_or(0.0);
        let position=control.and_then(|c|c.position).unwrap_or(primary.duration.prediction.impulse_positions[index]);
        (value>0.0).then_some(crate::synthesis::stages::timbre::records::PhoneImpulse{value,position:f64::from(position)})
    }).collect::<Vec<_>>());
    let request=TimbreRequest {dds,mode_coefficients:phrase.mode_coefficients.as_ref(),notes:&phrase.notes,phones,midi_pitch:&pitch,
        note_modulation:&modulation,phone_impulses:impulses.as_deref(),frame_controls:frames.as_deref(),requested_frame:0};
    if modern {
        let network=crate::synthesis::stages::timbre::TimbreNetwork::prepare(voice.model("timbre")?,voice.speaker,voice.retained_acoustic_noise_only)?;
        let features=network.prepare_features(&phrase.notes,phones)?;
        return network.render_prepared(&request,&crate::synthesis::stages::timbre::records::RecordsConfig::default(),&features);
    }
    let mut acoustic_pitch=primary.pitch.clone();acoustic_pitch.pitch=pitch.clone();
    super::super::models::dds_timbre(voice)?.render(&request,&acoustic_pitch)
}

fn bank(voice:&PreparedVoice)->Result<Vec<f32>> {
    let tree=voice.model("timbre")?;
    let network=tree.node(tree.node(tree.root_index())?.children[0])?;
    tree.tensor(network.children[1])?.vector().map(|v|v.to_vec()).ok_or_else(||VoiceError::new("Speaker conditioning is not a vector"))
}
fn signature(tree:&PreparedTree)->Result<[u8;32]> {
    use sha2::{Digest,Sha256};
    fn visit(tree:&PreparedTree,index:usize,hash:&mut Sha256)->Result<()> {
        let node=tree.node(index)?;
        hash.update(node.type_id);hash.update((node.children.len() as u64).to_le_bytes());
        let bytes=tree.payload(index)?;hash.update((bytes.len() as u64).to_le_bytes());hash.update(&bytes);
        for &child in &node.children {visit(tree,child,hash)?;}
        Ok(())
    }
    let mut hash=Sha256::new();
    let network=tree.node(tree.node(tree.root_index())?.children[0])?;
    for (slot,&node) in network.children.iter().enumerate() {if slot!=1 {visit(tree,node,&mut hash)?;}}
    Ok(hash.finalize().into())
}

pub(super) fn synthesize(voice:&PreparedVoice,identity:Option<u64>,input:&OwnerInput,phrase:&PreparedPhrase,c:&DdsControls,mut checkpoint:&mut dyn FnMut(super::super::RenderStage)->Result<()>)->Result<super::RenderResult> {
    if !voice.speakers.is_empty() || phrase.unison.count>1 {
        return super::super::ensemble::synthesize_with(voice,identity,phrase,checkpoint,|v,i,p,stage|synthesize(v,i,input,p,c,stage));
    }
    let mut acoustic=super::super::acoustics::synthesize_acoustics(voice,identity,phrase,&mut checkpoint)?;
    prepare(voice,input,c,&mut acoustic)?;
    super::super::render::publish_acoustic(voice,phrase,acoustic,checkpoint)
}
