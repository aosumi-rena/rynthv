use super::{StdBank,StdControls,Result,cancel,ensure,guides::Guide,pitch::Pitch,data::Frame,
    frame_ops as f,spectrum as s,renderer::SourcePhases};
use std::sync::atomic::AtomicBool;
use rayon::prelude::*;
mod fp;

pub(super) fn apply(frames:&mut [Frame],guide:&Guide,bank:&StdBank,controls:&StdControls,
    pitch:&Pitch,axis:&[f32],origin:f64,phases:SourcePhases,cancellation:&AtomicBool)->Result<()> {
    let environment=fp::get();
    let process=|i,frame:&mut Frame|->Result<()> {
        let config=&bank.data.config;
        let coefficient=((guide.first+guide.second)/(guide.source_end-guide.source_start)).min(1.);
cancel(cancellation)?;let local=i as f32*config.hop+guide.start;let global=origin+local as f64;
   let absolute=controls.absolute_pitch.sample_absolute(global);let deviation=controls.pitch_deviation.sample(global,0.);
   ensure((absolute.is_nan()||(0.0..=127.0).contains(&absolute))&&deviation.abs()<=48.,"R1 pitch control exceeds MIDI/deviation domain")?;
   let generated=if absolute.is_nan(){pitch.at(local)*2_f32.powf(controls.generated_pitch_shift as f32/12.)}else{440_f32*2_f32.powf((absolute as f32-69.)/12.)};
   let target=(generated*2_f32.powf(deviation as f32/12.)+frame.contour*coefficient).max(30.);
   ensure(target.is_finite()&&target<=config.nyquist,"R1 target pitch exceeds spectrum domain")?;
   let tension=controls.tension.sample(global,0.);let breath=controls.breathiness.sample(global,0.);let gender=controls.gender.sample(global,0.);
   ensure([tension,breath,gender].iter().all(|v|(-1.2..=1.2).contains(v)),"R1 expression exceeds -1.2 to 1.2 domain")?;
   let noise_amplitude=10_f64.powf(breath*7.5/20.)as f32;
   if frame.f0>0.{let original=frame.f0;let spectrum=s::expand(frame,config)?;let mut before=s::uniform(&spectrum,config.nyquist,&axis);let mut shifted=s::pitch_warp(&spectrum,original,target,config.nyquist)?;
    let desired=frame.phases.len().max((config.nyquist/target)as usize);if desired>frame.phases.len(){let mut extended=phases.evaluate(frame.rd,original,target,desired)?;extended[..frame.phases.len()].copy_from_slice(&frame.phases);frame.phases=extended;}
    let expression_scale=(-gender*0.5).exp()as f32;let mut rd=frame.rd;
    if let Some(timbre)=&bank.data.timbre{rd=s::timbre(timbre,original,target,rd,&mut shifted,&mut frame.noise.psd,&axis,expression_scale,config)?;}
    else{shifted=s::warp_expression(&shifted,target,expression_scale,config)?;}
    let amplitude=10_f64.powf((-tension*6.-breath.max(0.)*5.5)/20.)as f32;
    let gain=((f::log2(amplitude)*0.6931471824645996)as f64*8.685889611636938+(f::log2(original/target)*0.6931471824645996)as f64*8.685889611636938)as f32;
    for v in &mut shifted{*v+=gain;}for v in &mut before{*v+=gain;}let after=s::uniform(&shifted,config.nyquist,&axis);
    for j in 0..frame.noise.psd.len(){frame.noise.psd[j]=(frame.noise.psd[j]as f64+((after[j]-before[j])as f64).min(3.))as f32;}
    if bank.data.timbre.is_none()&&original>target{rd=(rd as f64+(1.-(target/original)as f64)*0.5*(3.-rd as f64))as f32;}
    let delta=(tension*if tension>0.{0.5}else{0.8}).clamp(-1.,1.);if delta!=0.{rd=(rd as f64+delta.abs()*(if delta>0.{0.1}else{3.}-rd as f64))as f32;}
    let spacing=target/3.;let query=(0..frame.phases.len()*3).map(|j|((j as f64+1.)*spacing as f64)as f32).collect::<Vec<_>>();frame.vt=s::uniform(&shifted,config.nyquist,&query);frame.f0=target;frame.rd=rd;
   }
   if noise_amplitude!=1.{let gain=((f::log2(noise_amplitude)*0.6931471824645996)as f64*8.685889611636938)as f32;
    for(j,v)in frame.noise.psd.iter_mut().enumerate(){*v+=(axis[j]/3000.).min(1.)*gain;}if let Some(wave)=&mut frame.wave{for value in &mut wave.values{*value*=noise_amplitude;}}}
   frame.contour=0.;
  
        Ok(())
    };
    if frames.len()<32 {for(i,frame)in frames.iter_mut().enumerate(){process(i,frame)?;}return Ok(());}
    crate::synthesis::inference::compute_pool().map_err(super::StdError::Native)?.install(|| {
        frames.par_chunks_mut(16).enumerate().try_for_each(|(block,frames)| {
            let _environment=fp::Guard::enter(environment);
            for(i,frame)in frames.iter_mut().enumerate(){process(block*16+i,frame)?;}Ok(())
        })
    })
}
