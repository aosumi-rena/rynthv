use super::{Result,ensure,score::Score};
#[derive(Clone,Copy)]
struct Segment {kind:u8,f:f64,start:f64,end:f64}
pub(super) struct Pitch {rows:Vec<(f32,f32)>}
impl Pitch {
 pub fn create(score:&Score)->Result<Self>{let notes=&score.inputs[2..];let mut segments=Vec::new();let mut cursor=0.;
  for(i,n)in notes.iter().enumerate(){if n.silence{segments.push(Segment{kind:0,f:0.,start:cursor,end:cursor+n.duration});cursor+=n.duration;continue;}
   let f=440.*2_f64.powf((n.pitch as f64-69.)/12.);let attack=i==0||notes[i-1].silence;let release=i+1==notes.len()||notes[i+1].silence;
   let left=0.07_f64.min(n.duration/2.);let right=0.07_f64.min(n.duration-left);let middle=n.duration-left-right;
   segments.push(Segment{kind:if attack{1}else{4},f,start:cursor,end:cursor+left});cursor+=left;
   if middle>0.{segments.push(Segment{kind:2,f,start:cursor,end:cursor+middle});cursor+=middle;}
   segments.push(Segment{kind:if release{5}else{3},f,start:cursor,end:cursor+right});cursor+=right;}
  segments.push(Segment{kind:0,f:0.,start:cursor,end:cursor+0.1});let mut knots:Vec<(f64,f64)>=Vec::new();
  for(i,s)in segments.iter().enumerate(){let next=segments.get(i+1);let mut add=|time:f64,f:f64|->Result<()>{ensure(time>=0.&&time.is_finite()&&knots.last().is_none_or(|p|p.0<time),"R1 pitch knots are not strictly ordered")?;knots.push((time,f));Ok(())};
   match s.kind{
    0=>{for t in[s.start,s.start+0.001,s.start+0.002,s.end-0.002,s.end-0.001]{add(t,s.f)?;}},
    1=>{add(s.start,s.f)?;add(s.start+0.001,s.f)?;if next.is_none_or(|p|p.kind==0){for t in[s.start+0.002,s.end-0.002,s.end-0.001]{add(t,s.f)?;}}
     else if next.is_some_and(|p|matches!(p.kind,2|3|5)){add(s.start+0.002,next.unwrap().f)?;}},
    2=>{add(s.start,s.f)?;if next.is_none_or(|p|p.kind==0){for t in[s.start+0.001,s.start+0.002,s.end-0.002,s.end-0.001]{add(t,s.f)?;}}},
    3=>{let n=next.ok_or_else(||super::StdError::Invalid("R1 pitch transition lacks successor".into()))?;add(s.start,s.f)?;add(s.start+(s.end-s.start)*0.25,n.f)?;},
    4=>{let n=next.ok_or_else(||super::StdError::Invalid("R1 pitch transition lacks successor".into()))?;add(s.start,s.f)?;add(s.start+(s.end-s.start)*0.75,n.f)?;},
    5=>{add(s.start,s.f)?;add(s.end-0.002,s.f)?;add(s.end-0.001,s.f)?;},_=>unreachable!()}
  }
  ensure(knots.len()>=4&&cursor<=40.,"R1 constructor pitch extent exceeds capacity")?;let hop=0.005;let first=(score.note_start/hop)as usize;let residual=score.note_start-first as f64*hop;
  let size=(knots.last().unwrap().0/hop+1.)as usize;ensure(size<=8192,"R1 pitch cache exceeds capacity")?;let mut base=vec![0.;size];
  for w in knots.windows(4){let(t0,t1,t2,t3)=(w[0].0,w[1].0,w[2].0,w[3].0);let weight=w[0].1;
   let(h01,h12,h23)=(t1-t0,t2-t1,t3-t2);let left=weight/(t2-t0);let right=weight/(t3-t1);let(c0,c1,c2,c3)=(left/h01,left/h12,right/h12,right/h23);
   let(a,b,c,d)=((t0/hop)as usize,(t1/hop)as usize,(t2/hop)as usize,(t3/hop)as usize);
   if h01>0.002{for(j,out)in base.iter_mut().enumerate().take(b).skip(a){let delta=j as f64*hop-residual-t0;*out+=(delta*c0)*delta;}}
   if h12>0.002{for(j,out)in base.iter_mut().enumerate().take(c).skip(b){let t=j as f64*hop-residual;*out+=((t-t0)*c1)*(t2-t)+((t3-t)*c2)*(t-t1);}}
   if h23>0.002{for(j,out)in base.iter_mut().enumerate().take(d).skip(c){let delta=j as f64*hop-residual-t3;*out+=(delta*c3)*delta;}}
  }
  let count=((score.note_start+cursor)/hop)as usize-first;let mut vibrato=vec![0.;size];let mut start=0.;
  for n in notes{let end=start+n.duration;let begin=start+0.25;if !n.silence&&end-start>0.25{for(i,out)in vibrato.iter_mut().enumerate().take((end/hop)as usize).skip((begin/hop)as usize){let t=i as f64*hop-residual-begin;let remaining=end-begin-t;
     let envelope=1_f64.min(remaining/0.15).min(t/0.15);let factor=((2.*t*3.14159*5.5).sin()*envelope)*0.025+1.;*out=factor.ln();}}start=end;}
  let mut rows=Vec::new();for i in 0..count{let f=base.get(i).copied().unwrap_or(0.)as f32;let v=vibrato.get(i).copied().unwrap_or(0.)as f32;let frequency=(v as f64).exp()as f32*f;
   ensure(frequency.is_finite(),"R1 pitch producer returned nonfinite frequency")?;if frequency>0.{rows.push((((first+i)as f64*hop-score.origin)as f32,frequency));}}
  ensure(!rows.is_empty(),"R1 pitch has no positive interpolation rows")?;Ok(Self{rows})
 }
 pub fn at(&self,time:f32)->f32{let i=self.rows.partition_point(|p|p.0<time);if i==0{return self.rows[0].1;}if i==self.rows.len(){return self.rows[i-1].1;}
  let(a,x)=self.rows[i-1];let(b,y)=self.rows[i];x+(time-a)*(y-x)/(b-a)}
}
