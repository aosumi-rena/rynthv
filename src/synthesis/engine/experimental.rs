mod weights;
pub(super) mod dds;
use super::{PcmComponents,RenderResult,RenderStage};
use crate::synthesis::{
    experiments::{BlendTarget,ExperimentControls},
    plan::OwnerInput,score::{LyricFrontend,PreparedPhrase,PreparedAutomation,SAMPLE_RATE},
    voice::{PreparedVoice,Result,VoiceError,VoiceFamily},
};

pub fn prepare_modes(voice:&PreparedVoice,input:&OwnerInput,phrase:&mut PreparedPhrase)->Result<()> {
    let settings=input.experiments.settings;
    if !settings.active(settings.random_styles) {return Ok(());}
    let mut controls=ExperimentControls::effective(&input.voice.instance_options,input.experiments.settings)?;
    controls.random_signed &= settings.negative_styles;
    if !controls.random_enabled {return Ok(());}
    controls.validate()?;
    let mut names=voice.vocal_modes.clone().unwrap_or_default();
    for member in voice.member_vocal_modes.iter() {names.extend(member.public_names.iter().cloned());}
    let random=controls.random_modes(&names);
    let range=settings.style_range();
    let modes=phrase.vocal_modes.get_or_insert_with(Default::default);
    for (name,extra) in random {
        let current=modes.entry(name).or_default();
        for (value,added) in [(&mut current.pitch,extra.pitch),(&mut current.timbre,extra.timbre),(&mut current.pronunciation,extra.pronunciation)] {
            *value=Some((value.unwrap_or(0.0)+added.unwrap_or(0.0)).clamp(range.0,range.1));
        }
    }
    Ok(())
}

pub fn needs_complete(input:&OwnerInput,_family:VoiceFamily)->Result<bool> {
    let s=input.experiments.settings;
    if !s.enabled {return Ok(false);}
    let c=ExperimentControls::effective(&input.voice.instance_options,input.experiments.settings)?;
    c.validate()?;
    let active=c.voices.iter().map(|v|v.active(&input.automation,input.notes.iter().map(|n|&n.extensions))).collect::<Result<Vec<_>>>()?;
    Ok(s.dds_controls || (s.voice_blending && active.into_iter().any(|v|v)) || (s.component_effects
        && (c.harmonic_gain!=0.0 || c.aspiration_gain!=0.0 || c.ring_depth!=0.0)))
}

pub fn synthesize(voice:&PreparedVoice,identity:Option<u64>,input:&OwnerInput,
    phrase:&PreparedPhrase,lexicon:&LyricFrontend,mut checkpoint:impl FnMut(RenderStage)->Result<()>)->Result<RenderResult> {
    let controls=ExperimentControls::effective(&input.voice.instance_options,input.experiments.settings)?;
    controls.validate()?;
    let dds_enabled=input.experiments.settings.active(input.experiments.settings.dds_controls);
    let speaker_vectors=dds_enabled && voice.family!=VoiceFamily::Sv2 && controls.dds.speaker_vectors;
    let selected_vocoder=if dds_enabled {
        controls.dds.vocoder.as_ref().map(|key| {
            let source=input.experiments.sources.iter().find(|s|&s.voice==key).ok_or_else(||VoiceError::new("Selected vocoder voice is not loaded"))?;
            voice.with_vocoder(&source.prepared)
        }).transpose()?
    }else{None};
    let voice=selected_vocoder.as_ref().unwrap_or(voice);
    let mut sources=Vec::new();
    let curves=PreparedAutomation::for_experiments(input);
    if input.experiments.settings.voice_blending {
        for target in &controls.voices {
            if !target.active(&input.automation,input.notes.iter().map(|n|&n.extensions))? {continue;}
            let source=input.experiments.sources.iter().find(|s|s.voice==target.voice)
                .ok_or_else(||VoiceError::new("混合歌声尚未加载；请检查所选声库版本及其合成上下文。"))?;
            if source.prepared.family!=voice.family {return Err(VoiceError::new("请在同一代歌声之间混合。"));}
            let member_id=target.speaker_id.or_else(||source.prepared.speakers.first().map(|s|s.id as i32));
            let selected=if let Some(id)=member_id {
                Some(source.prepared.member(source.prepared.speakers.iter().find(|s|s.id==id as usize)
                    .ok_or_else(||VoiceError::new("找不到所选合唱成员。"))?)?)
            }else{None};
            sources.push((target,source,selected));
        }
    }
    let mut primary=if controls.acoustic_blend && !speaker_vectors && !sources.is_empty() {
        let targets=sources.iter().map(|(t,s,member)|super::acoustic_mix::AcousticMixTarget {
            voice:member.as_ref().unwrap_or(&s.prepared),target:t,curves:curves.as_ref(),inherit_modes:t.inherit_modes,
        }).collect::<Vec<_>>();
        super::acoustic_mix::synthesize_acoustic_mix(voice,identity,phrase,&targets,dds_enabled.then_some((input,&controls.dds)),&mut checkpoint)?
    }else{
        let mut primary=if dds_enabled {dds::synthesize(voice,identity,input,phrase,&controls.dds,&mut checkpoint)?}
            else{super::synthesize(voice,identity,phrase,&mut checkpoint)?};
        let mut targets=Vec::new();
        for (target,source,member) in &sources {
            if speaker_vectors {continue;}
            let target_voice=member.as_ref().unwrap_or(&source.prepared);
            let identity=if target_voice.family.requires_identity() {Some(match source.identity_override {
                Some(v)=>v,None=>crate::synthesis::voice::session_identity::saved_identity(&source.name,target_voice.family.model_version())?,
            })}else{None};
            let mut other=input.clone();
            other.experiments.sources.clear();
            for database in [&mut other.voice.database,&mut other.voice.track_default_database] {
                let fields=database.as_object_mut().ok_or_else(||VoiceError::new("Invalid target voice database options"))?;
                fields.insert("id".into(),serde_json::json!(source.voice.database_id));
                fields.insert("name".into(),serde_json::json!(source.name));
                fields.insert("version".into(),serde_json::json!(source.voice.revision.to_string()));
            }
            other.pitch_offset=other.pitch_offset.checked_add(target.transpose).ok_or_else(||VoiceError::new("Harmony pitch overflow"))?;
            other.voice.instance_options.remove("rynthvExperiments");
            if let Some(voice)=other.voice.instance_options.get_mut("voice").and_then(|v|v.as_object_mut()) {
                voice.insert("choirNumStems".into(),serde_json::json!(1));
            }
            if !target.inherit_modes {
                for options in [&mut other.voice.instance_options,&mut other.voice.track_default_options] {
                    if let Some(v)=options.get_mut("voice").and_then(|v|v.as_object_mut()) {v.remove("vocalModeParams");}
                }
                other.automation.retain(|key,_|!key.starts_with("vocalMode"));
            }
            let frontend=lexicon.for_voice(&source.default_language,&source.supported_languages)?;
            let mut target_phrase=crate::synthesis::score::prepare_owner(&other,&frontend,target_voice.model("duration")?,target_voice.family)?;
            if target.inherit_modes {target_phrase.vocal_modes=phrase.vocal_modes.clone();}
            let rendered=super::synthesize(target_voice,identity,&target_phrase,&mut checkpoint)?;
            targets.push((*target,rendered));
        }
        if !targets.is_empty() {mix_rendered(&mut primary,phrase,input,&targets,curves.as_ref())?;}
        primary
    };
    if input.experiments.settings.component_effects {apply_effects(&mut primary,phrase,&controls)?;}
    if dds_enabled && controls.dds.output!=crate::synthesis::experiments::ComponentOutput::Complete {
        use crate::synthesis::experiments::ComponentOutput;
        let pcm=&mut primary.state.components;
        if controls.dds.output==ComponentOutput::Harmonics {pcm.secondary=vec![0.0;pcm.secondary.len()].into();}
        else {pcm.primary=vec![0.0;pcm.primary.len()].into();}
        republish(&mut primary,phrase)?;
    }
    Ok(primary)
}

pub(super) fn weight_at(target:&BlendTarget,curves:Option<&PreparedAutomation>,time:f64)->f32 {
    let key=format!("voiceBlend_{}:{}",target.voice.database_id,target.voice.revision);
    (target.weight+curves.map_or(0.0,|c|c.value(&key,time,0.0))).clamp(0.0,1.0)
}

fn mix_rendered(primary:&mut RenderResult,phrase:&PreparedPhrase,input:&OwnerInput,targets:&[(&BlendTarget,RenderResult)],curves:Option<&PreparedAutomation>)->Result<()> {
    let local=targets.iter().map(|(target,_)|weights::LocalWeights::new(input,primary,target)).collect::<Result<Vec<_>>>()?;
    let keys:Vec<_>=targets.iter().map(|(target,_)|format!("voiceBlend_{}:{}",target.voice.database_id,target.voice.revision)).collect();
    let weight=|index:usize,time:f64| {
        let target=targets[index].0;
        (local[index].base(time,target.weight)+curves.map_or(0.0,|c|c.value(&keys[index],time,0.0))).clamp(0.0,1.0)
    };
    let rate=f64::from(SAMPLE_RATE);
    let streams=std::iter::once((&primary.state.components,None))
        .chain(targets.iter().map(|(t,r)|(&r.state.components,Some(*t)))).collect::<Vec<_>>();
    let first=streams.iter().map(|(p,_)|(p.start_seconds*rate).round() as i64).min().unwrap();
    let end=streams.iter().map(|(p,_)|(p.start_seconds*rate).round() as i64+p.primary.len() as i64).max().unwrap();
    let count=usize::try_from(end-first).map_err(|_|VoiceError::new("Blend timeline overflow"))?;
    if count>90_000*SAMPLE_RATE as usize/200 {return Err(VoiceError::new("混合片段过长。"));}
    let mut result=[vec![0.0;count],vec![0.0;count]];
    for i in 0..count {
        if i%4096==0 {crate::synthesis::execution::checkpoint()?;}
        let time=(first+i as i64) as f64/rate;
        let total:f32=(0..targets.len()).map(|index|weight(index,time)).sum();
        for (index,(pcm,_)) in streams.iter().enumerate() {
            let weight=if index==0 {(1.0-total).max(0.0)}else{weight(index-1,time)/total.max(1.0)};
            let sample=first+i as i64-(pcm.start_seconds*rate).round() as i64;
            if let Ok(sample)=usize::try_from(sample) {if sample<pcm.primary.len() {
                result[0][i]+=pcm.primary[sample]*weight;result[1][i]+=pcm.secondary[sample]*weight;
            }}
        }
    }
    primary.state.components=PcmComponents {start_seconds:first as f64/rate,
        primary:std::mem::take(&mut result[0]).into(),secondary:std::mem::take(&mut result[1]).into()};
    republish(primary,phrase)
}

fn apply_effects(output:&mut RenderResult,phrase:&PreparedPhrase,c:&ExperimentControls)->Result<()> {
    if c.harmonic_gain==0.0 && c.aspiration_gain==0.0 && c.ring_depth==0.0 {return Ok(());}
    let harmonic=10.0_f32.powf(c.harmonic_gain/20.0);let aspiration=10.0_f32.powf(c.aspiration_gain/20.0);
    let origin=output.state.components.start_seconds;
    output.state.components.primary=output.state.components.primary.iter().enumerate().map(|(i,v)| {
        let value=v*harmonic;
        if c.ring_depth==0.0 {value}else{let t=origin+i as f64/f64::from(SAMPLE_RATE);
            value*(1.0-c.ring_depth+c.ring_depth*(std::f64::consts::TAU*f64::from(c.ring_frequency)*t).cos() as f32)}
    }).collect::<Vec<_>>().into();
    output.state.components.secondary=output.state.components.secondary.iter().map(|v|v*aspiration).collect::<Vec<_>>().into();
    republish(output,phrase)
}
fn republish(output:&mut RenderResult,phrase:&PreparedPhrase)->Result<()> {
    if output.state.components.primary.iter().chain(output.state.components.secondary.iter()).any(|v|!v.is_finite()) {
        return Err(VoiceError::new("Experimental audio contains nonfinite samples"));
    }
    output.audio=output.state.components.publish(phrase.visible_begin_seconds,phrase.visible_end_seconds)?;
    output.visuals.waveform=crate::synthesis::rendered::Waveform::new(&output.audio);
    Ok(())
}
