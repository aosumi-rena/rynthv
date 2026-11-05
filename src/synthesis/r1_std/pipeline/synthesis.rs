use super::{StdBank,StdNote,StdControls,StdAudio,StdPhoneme,Result,ensure,cancel,score::Score,selection,guides::{self,Guide,Window},pitch::Pitch,warp,frame_ops as f,spectrum as s,data::Frame,renderer::Native};
use std::{collections::HashMap,sync::atomic::AtomicBool};

pub(crate) fn render(bank:&StdBank,notes:&[StdNote],controls:&StdControls,cancellation:&AtomicBool,emit:&mut dyn FnMut(super::RenderProgress<'_>)->Result<()>)->Result<StdAudio>{
 cancel(cancellation)?;controls.absolute_pitch.validate_missing(true)?;
 ensure(controls.generated_pitch_shift.is_finite()&&controls.generated_pitch_shift.abs()<=48.,"R1 generated pitch shift exceeds -48 to 48 semitones")?;
 for curve in [&controls.pitch_deviation,&controls.tension,&controls.breathiness,&controls.gender]{curve.validate()?;}
 emit(super::RenderProgress::Stage(0))?;
 let score=Score::prepare(bank,notes,controls.request_origin_seconds)?;
 if score.inputs.iter().skip(2).all(|input|input.silence){
  let config=&bank.data.config;let duration=notes.last().unwrap().end_seconds-score.origin;
  ensure(duration.is_finite()&&duration>0.&&duration/config.hop as f64<=8192.,"R1 silent phrase exceeds the bounded frame renderer; split the part at a rest")?;
  let sample_rate=(config.nyquist*2.)as u32;
  cancel(cancellation)?;
  emit(super::RenderProgress::Prepared {origin:score.origin,hop:config.hop as f64,rate:sample_rate,
   phones:&[],failures:&score.note_failures,pitch:&[]})?;
  let mono=vec![0.;(duration*sample_rate as f64).ceil()as usize];
  for (i,samples) in mono.chunks(4096).enumerate(){cancel(cancellation)?;
   emit(super::RenderProgress::Pcm {origin:score.origin+(i*4096) as f64/sample_rate as f64,samples})?;}
  return Ok(StdAudio{sample_rate,start_seconds:score.origin,mono,pitch:Vec::new(),phonemes:Vec::new(),note_failures:score.note_failures});
 }
 let selected=selection::select(bank,&score,cancellation)?;let(guides,duration)=guides::create(bank,&score,&selected)?;
 let config=&bank.data.config;let count=(duration as f64/config.hop as f64)as usize;ensure((2..=8192).contains(&count),"R1 whole phrase exceeds the bounded frame renderer; split the part at a rest")?;
 emit(super::RenderProgress::Stage(1))?;
 let native=Native::load(&bank.paths.renderer_library)?;let axis=native.noise_axis(config)?;let pitch=Pitch::create(&score)?;
 let published_pitch=control_pitch(&score,&pitch,controls,config.hop,count,cancellation)?;
 let mut phonemes=Vec::new();
 for guide in &guides {
  let(record_index,_)=score.phones[guide.phone_index];let record=&score.records[record_index];
  if let Some(note_index)=score.inputs[record.input].owner.filter(|_|!score.inputs[record.input].silence) {
   phonemes.push(StdPhoneme {symbol:bank.phones[score.phone(guide.phone_index)].0.clone(),start_seconds:score.origin+guide.start as f64,
    duration_seconds:guide.first as f64,note_index,leading:guide.onset});
  }
 }
 emit(super::RenderProgress::Prepared {origin:score.origin,hop:config.hop as f64,rate:(config.nyquist*2.)as u32,
  phones:&phonemes,failures:&score.note_failures,pitch:&published_pitch})?;
 emit(super::RenderProgress::Stage(2))?;
 let mut canvas=vec![Frame::empty(config);count];let mut cache=HashMap::new();
 for guide in &guides{cancel(cancellation)?;
  let window_count=(guide.first/config.hop)as usize;if guide.silence_a&&guide.silence_b||window_count<=1{continue;}
  let positions=warp::create([guide.source_start/config.hop,guide.source_boundary/config.hop,guide.source_end/config.hop],
   [0.,guide.first/config.hop,(guide.first+guide.second)/config.hop])?;
  if !cache.contains_key(&guide.pair.first.entry){cache.insert(guide.pair.first.entry,bank.data.decode(guide.pair.first.entry,cancellation)?);}
  let source=&cache[&guide.pair.first.entry];let mut frames=f::sample(source,&positions)?;
  super::transform::apply(&mut frames,guide,bank,controls,&pitch,&axis,score.origin,native.phase_function(),cancellation)?;
  f::smooth(&mut frames);let start=(guide.start/config.hop)as isize;
  for(i,frame)in frames.iter().enumerate(){let at=start+i as isize;if at<0{continue;}if at>=canvas.len()as isize{break;}
   let(harmonic,noise)=weight(guide,window_count,i,config.hop);ensure((0.0..=1.0).contains(&harmonic)&&(0.0..=1.0).contains(&noise),"R1 unit window left its interpolation domain")?;
   let at=at as usize;canvas[at]=f::blend(&canvas[at],frame,harmonic,noise,true);}
 }
 cancel(cancellation)?;f::validate(config,&canvas)?;let mut spectra=Vec::with_capacity(canvas.len());
 for frame in canvas.iter_mut(){if frame.f0>0.{let spectrum=s::expand(frame,config)?;let used=frame.phases.len().min(config.harmonics).min((config.nyquist/frame.f0)as usize);frame.phases.truncate(used);spectra.push(Some(spectrum));}else{spectra.push(None);}}
 emit(super::RenderProgress::Stage(3))?;
 let rate=f64::from(config.nyquist*2.);
 let mut output=|first,samples:&[f32]|emit(super::RenderProgress::Pcm {origin:score.origin+first as f64/rate,samples});
 let mono=native.render(config,&canvas,&spectra,cancellation,&mut output)?;cancel(cancellation)?;
 Ok(StdAudio{sample_rate:(config.nyquist*2.)as u32,start_seconds:score.origin,mono,pitch:published_pitch,phonemes,note_failures:score.note_failures})
}
fn control_pitch(score:&Score,pitch:&Pitch,controls:&StdControls,hop:f32,count:usize,cancellation:&AtomicBool)->Result<Vec<(f64,f64)>> {
 let mut result=Vec::new();let mut input=2;let mut begin=score.note_start;
 if input>=score.inputs.len(){return Ok(result);}
 let mut end=begin+score.inputs[input].duration;
 for i in 0..count {
  if i%256==0{cancel(cancellation)?;}
  let local=i as f64*hop as f64;let global=score.origin+local;
  if global<begin{continue;}
  while global>=end {
   input+=1;if input>=score.inputs.len(){return Ok(result);}
   begin=end;end=begin+score.inputs[input].duration;
  }
  if score.inputs[input].silence{continue;}
  let absolute=controls.absolute_pitch.sample_absolute(global);let deviation=controls.pitch_deviation.sample(global,0.);
  ensure((absolute.is_nan()||(0.0..=127.0).contains(&absolute))&&deviation.abs()<=48.,"R1 published pitch control exceeds MIDI/deviation domain")?;
  let generated=if absolute.is_nan(){pitch.at(local as f32)*2_f32.powf(controls.generated_pitch_shift as f32/12.)}else{440_f32*2_f32.powf((absolute as f32-69.)/12.)};
  let frequency=generated*2_f32.powf(deviation as f32/12.);
  ensure(frequency.is_finite()&&frequency>0.,"R1 generated/control pitch publication is not positive finite")?;
  result.push((global,69.+12.*(frequency as f64/440.).log2()));
 }
 Ok(result)
}
fn weight(guide:&Guide,count:usize,index:usize,hop:f32)->(f32,f32){if index>=count{return(1.,1.);}
 let full=index as f32/(count-1)as f32;let span=guide.width/hop;
 let value=match guide.window{Window::Full=>full,Window::Left=>if(index as f32)<span{index as f32/span}else{1.},
  Window::Right=>if(index as f32)<count as f32-span{0.}else{(1.-((count-index)as f32/span)as f64)as f32},
  Window::SemiLeft=>if(index as f32)<span{(index*index)as f32/span/span}else{1.},
  Window::Mid=>{let time=index as f32*hop;let upper=count as f32*hop-guide.width;if time<guide.width{0.}else if time>=upper{1.}else{(time-guide.width)/(upper-guide.width)}}};
 if guide.window==Window::SemiLeft{(full,value)}else{(value,value)}
}
