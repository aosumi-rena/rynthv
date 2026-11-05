// SPDX-License-Identifier: MIT
use std::cmp::Ordering;
use super::word32::StdWord32 as W;
use super::word64::{StdWord64 as D,StdWideError};
use super::sse_reciprocal::{seed_word,SeedError};
pub(crate) const OP_CAP:usize=24576;
pub(crate) const SEED_CAP:usize=4096;
pub(crate) const ROUND_CAP:usize=128;
#[derive(Clone,Debug)]
pub(crate) enum WarpError {Numeric(StdWideError),Seed(SeedError),Domain{field:&'static str,word:u32},Extent{field:&'static str,actual:usize,limit:usize},InverseGrid,NegativeResult{index:usize,word:u32}}
impl From<StdWideError> for WarpError{fn from(x:StdWideError)->Self{Self::Numeric(x)}}
impl From<super::word32::StdNumericError> for WarpError{fn from(x:super::word32::StdNumericError)->Self{Self::Numeric(x.into())}}
#[derive(Clone,Debug)]
pub(crate) struct Op {pub context:[i32;5],pub kind:u8,pub width:u8,pub a:u64,pub b:u64,pub result:Option<u64>}
#[derive(Clone,Debug)]
pub(crate) struct Seed {pub context:[i32;5],pub value:u32,pub seed:Option<u32>,pub values:[Option<u32>;4],pub begin:usize,pub end:usize,pub invoked:bool,pub x86_64:bool,pub sse_checked:bool}
#[derive(Clone,Debug,Default)]
pub(crate) struct SumState {pub first:usize,pub count:usize,pub blocks:usize,pub pairs:usize,pub accum:[u32;8],pub a:u32,pub b:u32,pub sum:u32,pub begin:usize,pub end:usize,pub complete:bool}
#[derive(Clone,Debug,Default)]
pub(crate) struct Round {pub segment:usize,pub number:usize,pub begin:usize,pub end:usize,pub before_inc:Vec<u32>,pub before_fixed:Vec<i32>,pub after_inc:Vec<u32>,pub after_fixed:Vec<i32>,pub residual_before:[u32;2],pub residual_after:[u32;2],pub threshold:u32,pub minimum:Option<u32>,pub sum:u32,pub delta:Option<u32>,pub threshold_next:Option<u32>,pub changed:bool,pub complete:bool}
#[derive(Clone,Debug,Default)]
pub(crate) struct Segment {pub index:usize,pub span_x:Option<u32>,pub span_y:Option<u32>,pub first:Option<usize>,pub end:Option<usize>,pub n:Option<usize>,pub empty:bool,pub step:Option<u32>,pub sum:Option<u32>,pub delta:Option<u32>,pub inc_residue:Option<usize>,pub mask_residue:Option<usize>,pub initial_head:Option<usize>,pub head:Option<usize>,pub fallback:bool,pub vector_end:Option<usize>,pub four_end:Option<usize>,pub mask_head:Option<usize>,pub mask_end:Option<usize>,pub increments:Vec<u32>,pub fixed:Vec<i32>,pub initial_completed:usize,pub grid_completed:usize,pub inc_allocated:bool,pub mask_allocated:bool,pub inc_released:bool,pub mask_released:bool,pub complete:bool}
#[derive(Clone,Debug,Default)]
pub(crate) struct Inverse {pub index:usize,pub begin:usize,pub end:usize,pub query:Option<u32>,pub lo_before:usize,pub lo_after:usize,pub hi:Option<usize>,pub left:Option<usize>,pub numerator:Option<u32>,pub denominator:Option<u64>,pub result:Option<u32>}
#[derive(Clone,Debug,Default)]
pub(crate) struct WarpWork {pub inputs:[u32;6],pub source:[Option<u32>;3],pub target:[Option<u32>;3],pub source_count:Option<usize>,pub target_count:Option<usize>,pub anchors:Vec<usize>,pub weights_initial:Vec<u32>,pub weights_anchored:Vec<u32>,pub weights_forward:Vec<u32>,pub weights_backward:Vec<u32>,pub forward_completed:usize,pub backward_completed:usize,pub segments:Vec<Segment>,pub sums:Vec<SumState>,pub rounds:Vec<Round>,pub ops:Vec<Op>,pub seeds:Vec<Seed>,pub x_grid:Vec<u32>,pub y_grid:Vec<u32>,pub populated:Option<usize>,pub inverse:Vec<Inverse>,pub originals:Vec<u32>,pub context:[i32;5],pub complete:bool,pub error:Option<WarpError>}
impl WarpWork {
 fn slot(&mut self,kind:u8,width:u8,a:u64,b:u64)->Result<usize,WarpError>{if self.ops.len()>=OP_CAP{return Err(WarpError::Extent{field:"operations",actual:self.ops.len()+1,limit:OP_CAP});}let i=self.ops.len();self.ops.push(Op{context:self.context,kind,width,a,b,result:None});Ok(i)}
 fn op(&mut self,kind:u8,a:u32,b:u32)->Result<u32,WarpError>{let i=self.slot(kind,32,a as u64,b as u64)?;let a=W::from_bits(a)?;let b=W::from_bits(b)?;let r=match kind{1=>a.add(b),2=>a.sub(b),3=>a.mul(b),4=>a.div(b),5=>Ok(a.maximum(b)),_=>unreachable!()}?.bits();self.ops[i].result=Some(r as u64);Ok(r)}
 fn wide(&mut self,x:u32)->Result<u64,WarpError>{let i=self.slot(6,64,x as u64,0)?;let r=D::widen(W::from_bits(x)?)?.bits();self.ops[i].result=Some(r);Ok(r)}
 fn narrow(&mut self,x:u64)->Result<u32,WarpError>{let i=self.slot(7,32,x,0)?;let r=D::from_bits(x)?.narrow()?.bits();self.ops[i].result=Some(r as u64);Ok(r)}
 fn double(&mut self,kind:u8,a:u64,b:u64)->Result<u64,WarpError>{let i=self.slot(kind,64,a,b)?;let a=D::from_bits(a)?;let b=D::from_bits(b)?;let r=match kind{8=>a.add(b),9=>a.div(b),_=>unreachable!()}?.bits();self.ops[i].result=Some(r);Ok(r)}
 fn integer(&mut self,x:usize)->Result<u32,WarpError>{let i=self.slot(10,32,x as u64,0)?;if x>8192{return Err(WarpError::Extent{field:"integer-to-single",actual:x,limit:8192});}let r=W::from_i32(x as i32)?.bits();self.ops[i].result=Some(r as u64);Ok(r)}
 fn cmp(&mut self,a:u32,b:u32)->Result<Ordering,WarpError>{let i=self.slot(11,32,a as u64,b as u64)?;let r=W::from_bits(a)?.numeric_cmp(W::from_bits(b)?);self.ops[i].result=Some(match r{Ordering::Less=>0,Ordering::Equal=>1,Ordering::Greater=>2});Ok(r)}
 fn bound(&mut self,x:u32)->Result<usize,WarpError>{if self.cmp(x,0)?==Ordering::Less||self.cmp(x,0x46000000)?==Ordering::Greater{return Err(WarpError::Domain{field:"source-Bound-0..8192",word:x});}let i=self.slot(12,32,x as u64,0)?;let r=W::from_bits(x)?.trunc_i32()?;self.ops[i].result=Some(r as u64);Ok(r as usize)}
 fn ceil_count(&mut self,x:u32)->Result<usize,WarpError>{let floor=self.bound(x)?;let f=self.integer(floor)?;let n=floor+usize::from(self.cmp(x,f)?==Ordering::Greater);if !(2..=128).contains(&n){return Err(WarpError::Extent{field:"ceil-count",actual:n,limit:128});}Ok(n)}
 fn reciprocal_accum(&mut self,a:u32,value:u32)->Result<u32,WarpError>{let aw=self.wide(a)?;let vw=self.wide(value)?;let q=self.double(9,0x3ff0000000000000,vw)?;let s=self.double(8,aw,q)?;self.narrow(s)}
 fn refined(&mut self,value:u32)->Result<u32,WarpError>{
  if self.seeds.len()>=SEED_CAP{return Err(WarpError::Extent{field:"seeds",actual:self.seeds.len()+1,limit:SEED_CAP});}
  if self.ops.len()+4>OP_CAP{return Err(WarpError::Extent{field:"refinement-operations",actual:self.ops.len()+4,limit:OP_CAP});}
  let s=self.seeds.len();self.seeds.push(Seed{context:self.context,value,seed:None,values:[None;4],begin:self.ops.len(),end:self.ops.len(),invoked:false,x86_64:false,sse_checked:false});
  let own=match seed_word(value){Ok(own)=>own,Err(error)=>{if let SeedError::NonfiniteSeed{seed,..}=error{self.seeds[s].seed=Some(seed);self.seeds[s].invoked=true;self.seeds[s].x86_64=true;self.seeds[s].sse_checked=true;}return Err(WarpError::Seed(error));}};self.seeds[s].seed=Some(own.seed);self.seeds[s].invoked=own.intrinsic_invocations==1;self.seeds[s].x86_64=own.x86_64;self.seeds[s].sse_checked=own.sse_checked;
  let result=(||{let a=self.op(1,own.seed,own.seed)?;self.seeds[s].values[0]=Some(a);let b=self.op(3,value,own.seed)?;self.seeds[s].values[1]=Some(b);let c=self.op(3,b,own.seed)?;self.seeds[s].values[2]=Some(c);let d=self.op(2,a,c)?;self.seeds[s].values[3]=Some(d);Ok(d)})();self.seeds[s].end=self.ops.len();result
 }
 fn reciprocal_sum(&mut self,values:&[u32],first:usize,count:usize)->Result<u32,WarpError>{
  let s=self.sums.len();let blocks=count/8*8;let pairs=(count-blocks)/2*2;self.sums.push(SumState{first,count,blocks,pairs,begin:self.ops.len(),..SumState::default()});
  let result=(||{
   for j in (0..blocks).step_by(8){for lane in 0..8{self.context[3]=(j+lane)as i32;self.context[4]=lane as i32;let r=self.reciprocal_accum(self.sums[s].accum[lane],values[first+j+lane])?;self.sums[s].accum[lane]=r;}}
   self.context[3]=-1;self.context[4]=-1;let a=self.sums[s].accum;
   let p02=self.op(1,a[0],a[2])?;let p46=self.op(1,a[4],a[6])?;let even=self.op(1,p02,p46)?;
   let p13=self.op(1,a[1],a[3])?;let p57=self.op(1,a[5],a[7])?;let odd=self.op(1,p13,p57)?;let sum=self.op(1,even,odd)?;
   self.sums[s].sum=sum;self.sums[s].a=sum;
   for j in (0..pairs).step_by(2){self.context[3]=(blocks+j)as i32;self.context[4]=0;let a=self.reciprocal_accum(self.sums[s].a,values[first+blocks+j])?;self.sums[s].a=a;self.context[3]=(blocks+j+1)as i32;self.context[4]=1;let b=self.reciprocal_accum(self.sums[s].b,values[first+blocks+j+1])?;self.sums[s].b=b;}
   let mut sum=self.op(1,self.sums[s].a,self.sums[s].b)?;self.sums[s].sum=sum;
   for j in blocks+pairs..count{self.context[3]=j as i32;self.context[4]=-1;sum=self.reciprocal_accum(sum,values[first+j])?;self.sums[s].sum=sum;}
   self.sums[s].complete=true;Ok(sum)
  })();self.sums[s].end=self.ops.len();result
 }
}
fn head(residue:usize,count:usize)->usize{if count<8{return count;}let h=if residue==0{0}else{(16-residue)/4};if h+8<=count{h}else{count}}
fn segment(w:&mut WarpWork,k:usize,weights:&[u32],source:[u32;3],target:[u32;3],source_count:usize)->Result<(),WarpError>{
 w.context=[4,k as i32,-1,-1,-1];w.segments.push(Segment{index:k,..Segment::default()});
 let sx=w.op(2,source[k+1],source[k])?;w.segments[k].span_x=Some(sx);let sy=w.op(2,target[k+1],target[k])?;w.segments[k].span_y=Some(sy);
 let f=w.op(2,source[k],source[0])?;let first=w.bound(f)?;w.segments[k].first=Some(first);let e=w.op(2,source[k+1],source[0])?;let end=w.bound(e)?.min(source_count);w.segments[k].end=Some(end);
 let n=end.checked_sub(first).ok_or(WarpError::Extent{field:"segment-end-before-first",actual:first,limit:end})?;w.segments[k].n=Some(n);
 if n==0{w.segments[k].empty=true;w.segments[k].complete=true;return Ok(());}
 if n>128||first+n>weights.len(){return Err(WarpError::Extent{field:"segment-grid",actual:first+n,limit:weights.len()});}
 let nw=w.integer(n)?;let step=w.op(4,sx,nw)?;w.segments[k].step=Some(step);w.context[0]=5;let sum=w.reciprocal_sum(weights,first,n)?;w.segments[k].sum=Some(sum);let difference=w.op(2,sy,sx)?;let delta=w.op(4,difference,sum)?;w.segments[k].delta=Some(delta);
 let mut inc=vec![0u32;n];let mut fixed=vec![0i32;n];let ir=inc.as_ptr()as usize&15;let mr=fixed.as_ptr()as usize&15;
 let initial_head=head(ir,n);let ih=if initial_head==n{0}else{initial_head};let ve=ih+(n-ih)/8*8;let fe=ve+(n-ve)/4*4;let mh=head(mr,n);let me=mh+(n-mh)/8*8;
 {let s=&mut w.segments[k];s.inc_allocated=true;s.mask_allocated=true;s.inc_residue=Some(ir);s.mask_residue=Some(mr);s.initial_head=Some(initial_head);s.head=Some(ih);s.fallback=initial_head==n;s.vector_end=Some(ve);s.four_end=Some(fe);s.mask_head=Some(mh);s.mask_end=Some(me);}
 let result=(||{
  w.context=[6,k as i32,-1,-1,-1];
  for j in 0..n{w.context[3]=j as i32;let term=if j>=ih&&j<fe{let r=w.refined(weights[first+j])?;w.op(3,r,delta)?}else{w.op(4,delta,weights[first+j])?};inc[j]=w.op(1,step,term)?;w.segments[k].initial_completed=j+1;}
  let(mut rx,mut ry,mut threshold)=(sx,sy,0x3e4ccccd);
  for round in 1..=65 {
   if round>64||w.rounds.len()>=ROUND_CAP{return Err(WarpError::Extent{field:"positivity-round",actual:round,limit:64});}
   w.context=[7,k as i32,round as i32,-1,-1];let ri=w.rounds.len();w.rounds.push(Round{segment:k,number:round,begin:w.ops.len(),before_inc:inc.clone(),before_fixed:fixed.clone(),residual_before:[rx,ry],threshold,..Round::default()});
   let mut sum=0;let mut changed=false;
   let round_result:Result<(),WarpError>=(||{
    let minimum=w.op(3,0x3f800000,threshold)?;w.rounds[ri].minimum=Some(minimum);
    for j in 0..n{w.context[3]=j as i32;if fixed[j]==0{if w.cmp(minimum,inc[j])?==Ordering::Greater{inc[j]=minimum;fixed[j]=1;changed=true;rx=w.op(2,rx,step)?;ry=w.op(2,ry,minimum)?;}else{sum=w.reciprocal_accum(sum,weights[first+j])?;w.rounds[ri].sum=sum;}}}
    let difference=w.op(2,ry,rx)?;let delta=w.op(4,difference,sum)?;w.rounds[ri].delta=Some(delta);
    w.context[0]=8;
    for j in 0..n{w.context[3]=j as i32;if fixed[j]==0{let term=if j>=mh&&j<me{let r=w.refined(weights[first+j])?;w.op(3,r,delta)?}else{w.op(4,delta,weights[first+j])?};inc[j]=w.op(1,step,term)?;}}
    w.context[3]=-1;threshold=w.op(3,threshold,0x3f666666)?;w.rounds[ri].threshold_next=Some(threshold);w.rounds[ri].complete=true;Ok(())
   })();
   {let r=&mut w.rounds[ri];r.after_inc=inc.clone();r.after_fixed=fixed.clone();r.residual_after=[rx,ry];r.changed=changed;r.end=w.ops.len();}round_result?;
   if !changed{break;}
  }
  w.context=[9,k as i32,-1,-1,-1];
  for j in 0..n{w.context[3]=j as i32;let at=first+j;let jw=w.integer(j)?;let product=w.op(3,jw,step)?;w.x_grid[at]=w.op(1,product,source[k])?;if at+1<source_count{w.y_grid[at+1]=w.op(1,w.y_grid[at],inc[j])?;}w.segments[k].grid_completed=j+1;}
  w.segments[k].complete=true;Ok(())
 })();
 w.segments[k].increments=inc.clone();w.segments[k].fixed=fixed.clone();drop(fixed);w.segments[k].mask_released=true;drop(inc);w.segments[k].inc_released=true;result
}
fn create(w:&mut WarpWork)->Result<(),WarpError>{
 w.context=[1,-1,-1,-1,-1];let g=w.inputs;let mut source=[0;3];let mut target=[0;3];
 for i in 0..3{w.context[3]=i as i32;source[i]=w.op(4,g[i],g[5])?;w.source[i]=Some(source[i]);}
 w.context[3]=3;target[0]=w.op(4,0,g[5])?;w.target[0]=Some(target[0]);w.context[3]=4;target[1]=w.op(4,g[3],g[5])?;w.target[1]=Some(target[1]);w.context[3]=5;let total=w.op(1,g[3],g[4])?;target[2]=w.op(4,total,g[5])?;w.target[2]=Some(target[2]);
 w.context=[2,-1,-1,-1,-1];for x in source.into_iter().chain(target){if w.cmp(x,0)?==Ordering::Less{return Err(WarpError::Domain{field:"negative-anchor",word:x});}}
 if w.cmp(source[0],source[1])?!=Ordering::Less||w.cmp(source[1],source[2])?==Ordering::Greater||w.cmp(target[0],0)?!=Ordering::Equal||w.cmp(target[0],target[1])?!=Ordering::Less||w.cmp(target[1],target[2])?==Ordering::Greater{return Err(WarpError::Domain{field:"ordered-anchors",word:0});}
 let sc=w.ceil_count(source[2])?;w.source_count=Some(sc);let tc=w.ceil_count(target[2])?;w.target_count=Some(tc);
 let mut weights=vec![0x3c23d70a;sc];w.weights_initial=weights.clone();w.context=[3,-1,-1,-1,-1];
 for (i,x)in source.iter().enumerate(){w.context[3]=i as i32;let d=w.op(2,*x,source[0])?;let at=w.bound(d)?.min(sc-1);weights[at]=0x3f800000;w.anchors.push(at);w.weights_anchored=weights.clone();}
 w.weights_forward=weights.clone();for i in 1..sc{w.context[3]=i as i32;let p=w.op(3,weights[i-1],0x3f21866c)?;weights[i]=w.op(5,weights[i],p)?;w.weights_forward[i]=weights[i];w.forward_completed+=1;}
 w.weights_backward=weights.clone();for i in (0..sc-1).rev(){w.context[3]=i as i32;let p=w.op(3,weights[i+1],0x3f21866c)?;weights[i]=w.op(5,weights[i],p)?;w.weights_backward[i]=weights[i];w.backward_completed+=1;}
 w.x_grid=vec![0;sc];w.y_grid=vec![0;sc];w.y_grid[0]=target[0];for k in 0..2{segment(w,k,&weights,source,target,sc)?;}
 let mut populated=sc;while populated>0&&w.cmp(w.x_grid[populated-1],0)?==Ordering::Equal{populated-=1;}w.populated=Some(populated);if populated<2{return Err(WarpError::InverseGrid);}
 let mut lo=0;w.context=[10,-1,-1,-1,-1];
 for j in 0..tc{
  w.context[3]=j as i32;let ii=w.inverse.len();w.inverse.push(Inverse{index:j,begin:w.ops.len(),lo_before:lo,..Inverse::default()});
  let result:Result<(),WarpError>=(||{let jw=w.integer(j)?;let tw=w.integer(tc-1)?;let p=w.op(3,jw,tw)?;let query=w.op(4,p,tw)?;w.inverse[ii].query=Some(query);
   while lo+1<populated&&w.cmp(query,w.y_grid[lo+1])?==Ordering::Greater{lo+=1;}w.inverse[ii].lo_after=lo;let hi=if lo==populated-1{lo}else{lo+1};let left=if lo==0{0}else{(hi-1).max(lo)};w.inverse[ii].hi=Some(hi);w.inverse[ii].left=Some(left);
   let dx=w.op(2,w.x_grid[hi],w.x_grid[left])?;let dy=w.op(2,query,w.y_grid[left])?;let numerator=w.op(3,dx,dy)?;w.inverse[ii].numerator=Some(numerator);
   let ydiff=w.op(2,w.y_grid[hi],w.y_grid[left])?;let yd=w.wide(ydiff)?;let denominator=w.double(8,yd,0x3cd203af9ee75616)?;w.inverse[ii].denominator=Some(denominator);
   let xl=w.wide(w.x_grid[left])?;let nd=w.wide(numerator)?;let q=w.double(9,nd,denominator)?;let a=w.double(8,xl,q)?;let output=w.narrow(a)?;w.inverse[ii].result=Some(output);w.originals.push(output);Ok(())
  })();w.inverse[ii].end=w.ops.len();result?;
 }
 for i in 0..w.originals.len(){if w.cmp(w.originals[i],0)?==Ordering::Less{return Err(WarpError::NegativeResult{index:i,word:w.originals[i]});}}
 w.complete=true;Ok(())
}
pub(crate) fn derive_mapping(inputs:[u32;6])->WarpWork{let mut w=WarpWork{inputs,..WarpWork::default()};if let Err(e)=create(&mut w){w.error=Some(e);}w}
