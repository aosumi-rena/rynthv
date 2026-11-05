use super::{Result,ensure,data::{Config,Frame,Timbre},frame_ops::mix};
pub(super) fn uniform(values:&[f32],end:f32,queries:&[f32])->Vec<f32>{queries.iter().map(|q|{let p=(*q/end*values.len()as f32).max(0.);let i=p as usize;if i+1>=values.len(){*values.last().unwrap()}else{mix(values[i],values[i+1],p-i as f32)}}).collect()}
pub(super) fn expand(frame:&Frame,c:&Config)->Result<Vec<f32>>{let controls=&frame.vt;let spacing=frame.f0/3.;ensure(controls.len()>=3&&spacing>0.,"R1 voiced frame has no spectral controls")?;
 let mut out=vec![0.;1025];let mut filled=0;let bin=|k:usize|((k as f32*spacing/c.nyquist*1025.)as usize).min(1025);
 for k in 1..controls.len()-1{let(p0,p1,p2,p3)=(if k<2{controls[0]}else{controls[k-2]},controls[k-1],controls[k],controls[k+1]);
  let linear=(((p1 as f64-p2 as f64)*0.5)+(-1./6.*p0 as f64+p3 as f64/6.))as f32;
  let square=((p0 as f64+p2 as f64)*0.5-p1 as f64)as f32;let cube=((-p0 as f64/3.-p1 as f64*0.5+p2 as f64)-p3 as f64/6.)as f32;
  let ceiling=p0.max(p1).max(p2).max(p3);let origin=k as f32*spacing;let begin=if k==1{0}else{bin(k)};let end=bin(k+1);
  for(i,o)in out.iter_mut().enumerate().take(end).skip(begin){let frequency=i as f32/1025.*c.nyquist;let t=(frequency-origin)/spacing;let t2=t*t;*o=(((cube*t2+linear)*t)+(t2*square+p1)).min(ceiling);}
  filled=end;if end==1025{break;}}
 ensure(filled>0,"R1 spectral controls represent no output bins")?;let last=out[filled-1];out[filled..].fill(last);Ok(out)
}
pub(super) fn pitch_warp(spectrum:&[f32],original:f32,target:f32,nyquist:f32)->Result<Vec<f32>>{
 let end=original.max(target)+500.;ensure(end<nyquist,"R1 pitch-warp anchors exceed Nyquist")?;let source=[0.,original,end,nyquist];let dest=[0.,target,end,nyquist];let mut out=vec![0.;spectrum.len()];let mut filled=0;
 for k in 0..3{let begin=source[k]/nyquist*spectrum.len()as f32;let span=(source[k+1]-source[k])/nyquist*spectrum.len()as f32;let dspan=dest[k+1]-dest[k];let fit=(spectrum.len()as f32-1.-begin)/span*dspan+dest[k];
  let first=(dest[k]/nyquist*spectrum.len()as f32)as usize;let last=spectrum.len().min((dest[k+1]/nyquist*spectrum.len()as f32)as usize).min((fit/nyquist*spectrum.len()as f32)as usize);
  for(j,o)in out.iter_mut().enumerate().take(last).skip(first){let f=j as f32*nyquist/spectrum.len()as f32;let position=(f-dest[k])/dspan*span+begin;let at=position as usize;ensure(at+1<spectrum.len(),"R1 pitch warp lacks interpolation neighbor")?;*o=mix(spectrum[at],spectrum[at+1],position-at as f32);}filled=last;}
 ensure(filled>0,"R1 pitch warp represents no bins")?;let last=out[filled-1];out[filled..].fill(last);Ok(out)
}
fn at(x:&[f32],y:&[f32],q:f32)->f32{if q<=x[0]{return y[0];}let i=x.partition_point(|v|*v<q).min(x.len()-1);if q>=x[i]{return y[i];}y[i-1]+(q-x[i-1])*(y[i]-y[i-1])/(x[i]-x[i-1])}
pub(super) fn warp_expression(spectrum:&[f32],f0:f32,scale:f32,c:&Config)->Result<Vec<f32>>{if scale==1.{return Ok(spectrum.to_vec());}
 let grid=(0..1025).map(|i|i as f32*(c.nyquist*2.*0.00048828125)).collect::<Vec<_>>();let anchor=(4000.*scale).clamp(100.,7000.);
 let source=[0.,f0,anchor,8000.,grid[1024]];let target=[0.,f0,4000.,8000.,grid[1024]];ensure(source.windows(2).all(|w|w[0]<w[1])&&target.windows(2).all(|w|w[0]<w[1]),"R1 expression warp anchors are not increasing")?;
 let query=grid.iter().map(|q|at(&source,&target,*q)).collect::<Vec<_>>();Ok(uniform(spectrum,2.*grid[1024]-grid[1023],&query))}
pub(super) fn timbre(t:&Timbre,original:f32,target:f32,rd:f32,spectrum:&mut[f32],noise:&mut[f32],axis:&[f32],expression_scale:f32,c:&Config)->Result<f32>{
 let class=t.classes.iter().min_by(|a,b|(a.f0.ln()-original.ln()).abs().total_cmp(&(b.f0.ln()-original.ln()).abs())).unwrap();
 let p=class.positions.at(target).max(0.).min(class.targets.len()as f32-1.001);let index=p as usize;let fraction=p-index as f32;let a=&class.targets[index];let b=&class.targets[index+1];
 let scale=t.base.scale*mix(a.scale,b.scale,fraction)*expression_scale;let base_rd=t.base.rd.at(rd);let out_rd=mix(a.rd.at(base_rd),b.rd.at(base_rd),fraction);
 for(i,v)in spectrum.iter_mut().enumerate(){let f=i as f32*(c.nyquist/1024.);*v=((*v+t.base.spectrum.at(f))+a.spectrum.at(f)*(1.-fraction))+b.spectrum.at(f)*fraction;}
 for(i,v)in noise.iter_mut().enumerate(){let f=axis[i];*v=((*v+t.base.noise.at(f))+a.noise.at(f)*(1.-fraction))+b.noise.at(f)*fraction;}
 ensure(scale.is_finite()&&scale>0.&&out_rd>0.,"R1 timbre transform exceeds finite positive domain")?;if scale!=1.{spectrum.copy_from_slice(&warp_expression(spectrum,target,scale,c)?);}Ok(out_rd)
}
