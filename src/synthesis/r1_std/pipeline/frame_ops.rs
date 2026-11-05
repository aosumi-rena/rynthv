use super::{Result,ensure,data::{Frame,Noise,Band,Config}};
pub(super) fn mix(a:f32,b:f32,r:f32)->f32{a+(b-a)*r}
pub(super) fn phase(mut a:f32,mut b:f32,r:f32)->f32{let difference=(b-a)*0.5;let turns=(difference as f64/std::f64::consts::PI).abs().round();let period=(turns+turns)*std::f64::consts::PI;if a>b{b=(b as f64+period)as f32;}else{a=(a as f64+period)as f32;}mix(a,b,r)}
fn angle(a:f32,b:f32,r:f32)->f32{mix(a.sin(),b.sin(),r).atan2(mix(a.cos(),b.cos(),r))}
pub(super) fn log2(value:f32)->f32{let bits=value.to_bits();let m=f32::from_bits((bits&0x7fffff)+0x3f000000);
 ((bits as f32*1.1920928955078125e-7-124.22551727294922)-1.4980303049087524*m)-1.7258800268173218/(m+0.35208871960639954)}
pub(super) fn fade(r:f32)->f32{((log2(r.max(1e-10))*0.6931471824645996)as f64*8.685889611636938)as f32}
fn power(v:f32)->f32{let x=(v/10.*1.4426950216293335).max(-126.);let fraction=(x-x.trunc())+if x<0.{1.}else{0.};
 f32::from_bits((((x+121.27405548095703)+27.728023529052734/(4.842525482177734-fraction)-fraction*1.4901291131973267)*8388608.)as i32 as u32)}
fn harmonics(a:&[f32],b:&[f32],r:f32,angle:bool)->Vec<f32>{let mut out=if a.len()>b.len(){a.to_vec()}else{b.to_vec()};for i in 0..a.len().min(b.len()){out[i]=if angle{phase(a[i],b[i],r)}else{mix(a[i],b[i],r)};}out}
pub(super) fn noise(a:&Noise,b:&Noise,r:f32,join:bool,av:bool,bv:bool)->Noise{
 let psd=a.psd.iter().zip(&b.psd).map(|(a,b)|log2(mix(power(*a),power(*b),r))*6.931471824645996).collect();
 let bands=a.bands.iter().zip(&b.bands).map(|(a,b)|{let dc=mix(a.dc,b.dc,r);let mut amplitudes=harmonics(&a.amplitudes,&b.amplitudes,r,false);
  if join&&((!av&&dc as f64>b.dc as f64*2.)||(!bv&&b.dc as f64>dc as f64*2.)){for v in amplitudes.iter_mut().take(a.amplitudes.len().min(b.amplitudes.len())){*v=0.;}}
  Band{dc,amplitudes,phases:harmonics(&a.phases,&b.phases,r,true)}}).collect();Noise{scale:a.scale,psd,bands}
}
pub(super) fn blend(a:&Frame,b:&Frame,r:f32,nr:f32,join:bool)->Frame{
 if r<0.01&&nr<0.01{return a.clone();}if r>0.99&&nr>0.99{return b.clone();}
 let mut out=a.clone();out.noise=noise(&a.noise,&b.noise,nr,join,a.f0>0.,b.f0>0.);
 if a.f0==0.&&b.f0==0.{out.f0=0.;out.rd=1.;return out;}
 if a.f0==0.||b.f0==0.{let voiced=if a.f0>0.{a}else{b};out.f0=voiced.f0;out.rd=voiced.rd;out.phases=voiced.phases.clone();let gain=fade(if a.f0>0.{(1.-r as f64)as f32}else{r});out.vt=voiced.vt.iter().map(|v|v+gain).collect();return out;}
 out.f0=mix(a.f0,b.f0,r);out.rd=mix(a.rd,b.rd,r);out.vt=harmonics(&a.vt,&b.vt,r,false);out.phases=harmonics(&a.phases,&b.phases,r,true);
 if join{for j in 0..a.phases.len().min(b.phases.len()){out.phases[j]=angle(a.phases[j],b.phases[j],r);}}
 out.contour=if join{0.}else{mix(a.contour,b.contour,r)};out
}
pub(super) fn sample(frames:&[Frame],positions:&[f32])->Result<Vec<Frame>>{
 ensure(frames.len()>=2&&!positions.is_empty(),"R1 source interpolation lacks neighbors")?;let mut previous=positions[0]as usize;let mut out=Vec::with_capacity(positions.len());
 for p in positions{let position=if *p>=frames.len()as f32-1.{(frames.len()as f64-1.01)as f32}else{p.max(0.)};let left=position as usize;ensure(left+1<frames.len(),"R1 source interpolation exceeds unit")?;
  let wave=frames.iter().take(position as usize+1).skip(previous.saturating_add(1)).find_map(|f|f.wave.clone());previous=left;
  let mut frame=blend(&frames[left],&frames[left+1],position-left as f32,position-left as f32,false);frame.wave=wave;out.push(frame);}
 Ok(out)
}
pub(super) fn smooth(frames:&mut[Frame]){let threshold=std::f32::consts::PI*0.15;
 for i in 1..frames.len().saturating_sub(1){if frames[i-1].f0==0.||frames[i].f0==0.||frames[i+1].f0==0.{continue;}
  let n=25.min(frames[i-1].phases.len()).min(frames[i].phases.len()).min(frames[i+1].phases.len());for j in 0..n{
   let(a,b,c)=(frames[i-1].phases[j],frames[i].phases[j],frames[i+1].phases[j]);let distance=|x:f32,y:f32|{let d=(x-y)as f64;(d+std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)-std::f64::consts::PI};
   if distance(a,b).abs()>threshold as f64&&distance(c,b).abs()>threshold as f64{frames[i].phases[j]=angle(a,c,0.5);}}}
}
pub(super) fn validate(c:&Config,frames:&[Frame])->Result<()>{ensure(frames.iter().all(|f|f.f0.is_finite()&&f.rd.is_finite()&&f.vt.iter().chain(&f.phases).chain(&f.noise.psd).all(|v|v.is_finite())&&f.noise.psd.len()==c.bins),"Nonfinite R1 processed frame")}
