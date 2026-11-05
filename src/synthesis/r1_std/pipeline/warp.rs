use super::{Result,ensure};
pub(super) fn create(source:[f32;3],target:[f32;3])->Result<Vec<f32>>{
 ensure(source[0]>=0.&&source[0]<source[1]&&source[1]<=source[2]&&target[0]==0.&&target[1]>0.&&target[1]<=target[2],"Invalid R1 source warp anchors")?;
 let source_count=source[2].ceil()as usize;let target_count=target[2].ceil()as usize;ensure((2..=8192).contains(&source_count)&&(2..=8192).contains(&target_count),"R1 warp exceeds bounded frame grid")?;
 let mut weights=vec![0.01_f32;source_count];for x in source{weights[((x-source[0])as usize).min(source_count-1)]=1.;}let decay=f32::from_bits(0x3f21866c);
 for i in 1..source_count{weights[i]=weights[i].max(weights[i-1]*decay);}for i in(0..source_count-1).rev(){weights[i]=weights[i].max(weights[i+1]*decay);}
 let mut x=vec![0.;source_count];let mut y=vec![0.;source_count];
 for k in 0..2{let span_x=source[k+1]-source[k];let span_y=target[k+1]-target[k];let first=(source[k]-source[0])as usize;let end=((source[k+1]-source[0])as usize).min(source_count);if end<=first{continue;}
  let count=end-first;let step=span_x/count as f32;let sum=weights[first..end].iter().map(|w|1./w).sum::<f32>();let delta=(span_y-span_x)/sum;
  let mut inc=weights[first..end].iter().map(|w|step+delta/w).collect::<Vec<_>>();let mut fixed=vec![false;count];let(mut residual_x,mut residual_y,mut threshold)=(span_x,span_y,0.2_f32);
  for _ in 0..8192{let mut changed=false;let mut sum=0.;for j in 0..count{if fixed[j]{continue;}if inc[j]<threshold{inc[j]=threshold;fixed[j]=true;changed=true;residual_x-=step;residual_y-=threshold;}else{sum=(sum as f64+1./weights[first+j]as f64)as f32;}}
   if sum!=0.{let delta=(residual_y-residual_x)/sum;for j in 0..count{if !fixed[j]{inc[j]=step+delta/weights[first+j];}}}threshold*=0.9;if !changed{break;}}
  for j in 0..count{let at=first+j;x[at]=j as f32*step+source[k];if at+1<source_count{y[at+1]=y[at]+inc[j];}}
 }
 let mut populated=source_count;while populated>0&&x[populated-1]==0.{populated-=1;}ensure(populated>=2,"R1 warp has no inverse grid")?;
 let mut result=Vec::with_capacity(target_count);let mut lo=0;for j in 0..target_count{let query=j as f32*(target_count-1)as f32/(target_count-1)as f32;
  while lo+1<populated&&query>y[lo+1]{lo+=1;}let hi=(lo+1).min(populated-1);let numerator=(x[hi]-x[lo])*(query-y[lo]);result.push((x[lo]as f64+numerator as f64/((y[hi]-y[lo])as f64+1e-15))as f32);}
 ensure(result.iter().all(|v|v.is_finite()&&*v>=0.),"R1 inverse warp returned invalid positions")?;Ok(result)
}
