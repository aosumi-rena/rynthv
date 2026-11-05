// SPDX-License-Identifier: MIT
use std::cmp::Ordering;
use super::word32::{StdWord32 as W,StdNumericError};
use super::word64::{StdWord64 as D,StdWideError};
use super::frame::{self,RawFrame,SourceFrame,Value,Origin,Noise,Band,Transition};
pub(crate) const READ_CAP:usize=40960;
pub(crate) const DELTA_CAP:usize=32768;
pub(crate) const QUANT_CAP:usize=12288;
#[derive(Clone,Debug)]
pub(crate) enum DecodeError {Extent{field:&'static str,actual:u64,limit:u64,offset:usize},Shape{field:&'static str,actual:u64,offset:usize},Nonfinite{read:usize,word:u32},Numeric(StdWideError),TimbreUnsupported{flags:u32,offset:usize},NoiseNullNormalization{frame:usize},Normalize(frame::FrameError)}
impl From<StdWideError> for DecodeError{fn from(e:StdWideError)->Self{Self::Numeric(e)}}
impl From<StdNumericError> for DecodeError{fn from(e:StdNumericError)->Self{Self::Numeric(e.into())}}
#[derive(Clone,Debug,Default)]
pub(crate) struct Configuration {pub version:u32,pub units:u32,pub reserved_a:Vec<u32>,pub flags:u32,pub reserved_b:Vec<u32>,pub hop:u32,pub harmonic_capacity:u32,pub envelope_capacity:u32,pub bins:u32,pub warp:u32,pub nyquist:u32,pub channels:u32,pub edges:Vec<u32>,pub lip:u32,pub magnitude_size:u32,pub directory:Option<usize>,pub complete:bool}
#[derive(Clone,Debug,Default)]
pub(crate) struct Unit {pub relative:u32,pub position:usize,pub frame_relatives:Vec<u32>,pub positions:Vec<usize>,pub end:Option<usize>}
#[derive(Clone,Debug,Default)]
pub(crate) struct DeltaBlock {pub frame:usize,pub field:u32,pub start:usize,pub count:usize,pub header_reads:Vec<usize>,pub step:Option<u32>,pub first:Option<u32>,pub begin:usize,pub end:usize,pub complete:bool}
#[derive(Clone,Debug)]
pub(crate) struct DeltaEdge {pub read:usize,pub product:Option<u32>,pub inner:Option<u32>,pub result:Option<u32>}
#[derive(Clone,Debug)]
pub(crate) struct QuantEdge {pub read:usize,pub divided:Option<u64>,pub scaled:Option<u64>,pub result:Option<u32>}
#[derive(Clone,Debug,Default)]
pub(crate) struct Decoded {pub index:usize,pub start:usize,pub end:usize,pub consumed:usize,pub transition_flag:Option<u8>,pub extra_flag:Option<u8>,pub harmonics:Option<usize>,pub spectral:Option<u8>,pub noise_null:Option<bool>,pub envelope_counts:Vec<u8>,pub transition_count:Option<usize>,pub transition_type:Option<i32>,pub value_budget:usize,pub raw:Option<RawFrame>,pub word_reads:Vec<usize>,pub layer1_complete:bool,pub complete:bool}
#[derive(Clone,Debug,Default)]
pub(crate) struct DecodeWork {pub data_bytes:usize,pub selected_unit:usize,pub cursor:usize,pub selected_end:Option<usize>,pub configuration:Configuration,pub units:Vec<Unit>,pub frames:Vec<Decoded>,pub reads:Vec<[u32;6]>,pub deltas:Vec<DeltaEdge>,pub quantized:Vec<QuantEdge>,pub delta_blocks:Vec<DeltaBlock>,pub decoded_values:u64,pub field_context:[u32;3],pub complete:bool,pub error:Option<DecodeError>}
struct Reader<'a>{data:&'a[u8],at:usize,end:usize,w:&'a mut DecodeWork}
impl Reader<'_>{
 fn extent(&self,field:&'static str,actual:u64,limit:u64)->DecodeError{DecodeError::Extent{field,actual,limit,offset:self.at}}
 fn need(&self,n:usize)->Result<(),DecodeError>{if self.at>self.end||n>self.end-self.at||self.end>self.data.len(){Err(self.extent("bounded-read",n as u64,self.end.saturating_sub(self.at)as u64))}else{Ok(())}}
 fn scalar(&mut self,n:usize,field:u32,index:usize)->Result<(u32,usize),DecodeError>{
  self.w.field_context=[self.w.field_context[0],field,index as u32];self.w.cursor=self.at;
  if self.w.reads.len()>=READ_CAP{return Err(self.extent("scalar-records",(self.w.reads.len()+1)as u64,READ_CAP as u64));}self.need(n)?;
  let mut value=0u32;for i in 0..n{value|=(self.data[self.at+i]as u32)<<(8*i);}let ri=self.w.reads.len();self.w.reads.push([self.at as u32,n as u32,self.w.field_context[0],field,index as u32,value]);self.at+=n;self.w.cursor=self.at;Ok((value,ri))
 }
 fn word(&mut self,field:u32,index:usize)->Result<u32,DecodeError>{Ok(self.scalar(4,field,index)?.0)}
 fn byte(&mut self,field:u32,index:usize)->Result<u8,DecodeError>{Ok(self.scalar(1,field,index)?.0 as u8)}
 fn float(&mut self,field:u32,index:usize)->Result<(u32,usize),DecodeError>{let(v,r)=self.scalar(4,field,index)?;if v&0x7f800000==0x7f800000{return Err(DecodeError::Nonfinite{read:r,word:v});}Ok((v,r))}
 fn positive(&self,x:u32,zero:bool,field:&'static str)->Result<(),DecodeError>{let v=W::from_bits(x)?;let order=v.numeric_cmp(W::from_bits(0)?);if order==Ordering::Less||(!zero&&order==Ordering::Equal){return Err(DecodeError::Shape{field,actual:x as u64,offset:self.at});}Ok(())}
 fn limit(&self,n:usize,maximum:usize,field:&'static str)->Result<(),DecodeError>{if n>maximum{Err(self.extent(field,n as u64,maximum as u64))}else{Ok(())}}
 fn source_values(&mut self,n:usize)->Result<(),DecodeError>{let sum=self.w.decoded_values.checked_add(n as u64).ok_or(self.extent("source-decoded-overflow",u64::MAX,16777216))?;self.w.decoded_values=sum;if sum>16777216{return Err(self.extent("source-decoded-values",sum,16777216));}if sum>64512{return Err(self.extent("own-decoded-values",sum,64512));}Ok(())}
 fn frame_values(&mut self,i:usize,n:usize)->Result<(),DecodeError>{let sum=self.w.frames[i].value_budget.checked_add(n).ok_or(self.extent("frame-values-overflow",u64::MAX,504))?;self.w.frames[i].value_budget=sum;self.limit(sum,504,"frame-float-words")}
 fn value(&self,bits:u32,read:usize)->Result<Value,DecodeError>{Ok(Value{word:W::from_bits(bits)?,origin:Origin{role:0,a_frame:self.w.field_context[0]as i32,a_offset:read as i32,b_frame:-1,b_offset:-1}})}
 fn delta(&mut self,count:usize,field:u32)->Result<Vec<Value>,DecodeError>{
  if count==0{return Err(DecodeError::Shape{field:"empty-delta",actual:0,offset:self.at});}self.limit(count,384,"delta-count")?;
  let b=self.w.delta_blocks.len();self.w.delta_blocks.push(DeltaBlock{frame:self.w.field_context[0]as usize,field,start:self.at,count,begin:self.w.deltas.len(),..DeltaBlock::default()});
  let result=(||{let(bias,br)=self.float(field,0)?;self.w.delta_blocks[b].header_reads.push(br);let(scale,sr)=self.float(field,1)?;self.w.delta_blocks[b].header_reads.push(sr);let(first,fr)=self.float(field,2)?;self.w.delta_blocks[b].header_reads.push(fr);self.w.delta_blocks[b].first=Some(first);
   let mut values=Vec::with_capacity(count);values.push(self.value(first,fr)?);let step=W::from_bits(0x3c000000)?.mul(W::from_bits(scale)?)?;self.w.delta_blocks[b].step=Some(step.bits());
   for i in 1..count{
    if self.w.deltas.len()>=DELTA_CAP{return Err(self.extent("delta-records",(self.w.deltas.len()+1)as u64,DELTA_CAP as u64));}
    let(raw,ri)=self.scalar(1,field,i+2)?;let code=if raw>=128{raw as i32-256}else{raw as i32};let e=self.w.deltas.len();self.w.deltas.push(DeltaEdge{read:ri,product:None,inner:None,result:None});
    let product=W::from_i32(code)?.mul(step)?;self.w.deltas[e].product=Some(product.bits());let inner=values[i-1].word.add(product)?;self.w.deltas[e].inner=Some(inner.bits());let out=inner.add(W::from_bits(bias)?)?;self.w.deltas[e].result=Some(out.bits());values.push(self.value(out.bits(),ri)?);
   }self.w.delta_blocks[b].complete=true;Ok(values)
  })();self.w.delta_blocks[b].end=self.w.deltas.len();result
 }
 fn quantize(&mut self,width:usize,signed:bool,divisor:i32,scale:u64,field:u32,index:usize)->Result<Value,DecodeError>{
  if self.w.quantized.len()>=QUANT_CAP{return Err(self.extent("quantize-records",(self.w.quantized.len()+1)as u64,QUANT_CAP as u64));}
  let(raw,ri)=self.scalar(width,field,index)?;let code=if signed{let shift=32-width*8;(raw<<shift)as i32>>shift}else{raw as i32};let q=self.w.quantized.len();self.w.quantized.push(QuantEdge{read:ri,divided:None,scaled:None,result:None});
  let integer=D::widen(W::from_i32(code)?)?;let denominator=D::widen(W::from_i32(divisor)?)?;let divided=integer.div(denominator)?;self.w.quantized[q].divided=Some(divided.bits());let scaled=divided.mul(D::from_bits(scale)?)?;self.w.quantized[q].scaled=Some(scaled.bits());let word=scaled.narrow()?;self.w.quantized[q].result=Some(word.bits());self.value(word.bits(),ri)
 }
 fn header(&mut self)->Result<(),DecodeError>{
  self.w.field_context=[128,0,0];let magic=self.word(1,0)?;if magic!=0x20643573{return Err(DecodeError::Shape{field:"s5d-magic",actual:magic as u64,offset:0});}
  let version=self.word(2,0)?;self.w.configuration.version=version;if !(1..=9).contains(&version){return Err(DecodeError::Shape{field:"version",actual:version as u64,offset:4});}
  for i in 0..6{let v=self.word(3,i)?;self.w.configuration.reserved_a.push(v);}
  let units=self.word(4,0)?;self.w.configuration.units=units;self.limit(units as usize,1000000,"source-unit-count")?;self.limit(units as usize,16,"own-unit-count")?;
  self.w.configuration.flags=self.word(5,0)?;for i in 0..8{let v=self.word(6,i)?;self.w.configuration.reserved_b.push(v);}
  self.w.configuration.hop=self.word(7,0)?;self.w.configuration.harmonic_capacity=self.word(8,0)?;self.w.configuration.envelope_capacity=self.word(9,0)?;self.w.configuration.bins=self.word(10,0)?;self.w.configuration.warp=self.word(11,0)?;self.w.configuration.nyquist=self.word(12,0)?;self.w.configuration.channels=self.word(13,0)?;
  let c=&self.w.configuration;self.positive(c.hop,false,"hop")?;self.positive(c.warp,true,"warp")?;self.positive(c.nyquist,false,"nyquist")?;
  self.limit(c.harmonic_capacity as usize,65536,"source-harmonic-capacity")?;self.limit(c.envelope_capacity as usize,65536,"source-envelope-capacity")?;self.limit(c.bins as usize,65536,"source-noise-bins")?;
  if !(1..=1024).contains(&c.channels){return Err(DecodeError::Shape{field:"source-channels",actual:c.channels as u64,offset:self.at});}
  self.limit(c.harmonic_capacity as usize,128,"own-harmonic-capacity")?;self.limit(c.envelope_capacity as usize,128,"own-envelope-capacity")?;self.limit(c.bins as usize,64,"own-noise-bins")?;
  if c.channels!=2{return Err(DecodeError::Shape{field:"own-canonical-channels",actual:c.channels as u64,offset:self.at});}
  let mut previous=0;for i in 0..self.w.configuration.channels as usize-1{let edge=self.float(14,i)?.0;if W::from_bits(edge)?.numeric_cmp(W::from_bits(previous)?)!=Ordering::Greater||W::from_bits(edge)?.numeric_cmp(W::from_bits(self.w.configuration.nyquist)?)!=Ordering::Less{return Err(DecodeError::Shape{field:"edges",actual:edge as u64,offset:self.at});}self.w.configuration.edges.push(edge);previous=edge;}
  self.w.configuration.lip=self.float(15,0)?.0;self.positive(self.w.configuration.lip,false,"lip-radius")?;self.w.configuration.magnitude_size=1025;
  if self.w.configuration.flags&255!=0{return Err(DecodeError::TimbreUnsupported{flags:self.w.configuration.flags,offset:self.at});}
  self.w.configuration.directory=Some(self.at);self.w.configuration.complete=true;Ok(())
 }
 fn directory(&mut self,selected:usize)->Result<(),DecodeError>{
  let base=self.at;let count=self.w.configuration.units as usize;let mut relatives=Vec::new();for i in 0..count{relatives.push(self.word(16,i)?);}
  let mut total_frames=0;
  for(i,relative)in relatives.into_iter().enumerate(){let position=base.checked_add(relative as usize).ok_or(self.extent("unit-offset-overflow",u64::MAX,self.data.len()as u64))?;if position<base+4*count||position>self.data.len().saturating_sub(4){return Err(self.extent("unit-offset",position as u64,self.data.len()as u64));}
   self.w.units.push(Unit{relative,position,..Unit::default()});self.at=position;let frames=self.word(17,i)?as usize;self.limit(frames,1000000,"source-frame-count")?;self.limit(frames,128,"own-frame-count")?;total_frames+=frames;self.limit(total_frames,128,"directory-total-frames")?;
   for j in 0..frames{let relative=self.word(18,j)?;self.w.units[i].frame_relatives.push(relative);}
   for j in 0..frames{let at=position.checked_add(self.w.units[i].frame_relatives[j]as usize).ok_or(self.extent("frame-offset-overflow",u64::MAX,self.data.len()as u64))?;if at<position+4+4*frames||at>=self.data.len(){return Err(self.extent("frame-offset",at as u64,self.data.len()as u64));}self.w.units[i].positions.push(at);}
  }
  if selected>=self.w.units.len(){return Err(self.extent("selected-unit",selected as u64,self.w.units.len()as u64));}
  for i in 1..self.w.units.len(){if self.w.units[i].position<=self.w.units[i-1].position{return Err(self.extent("strict-unit-order",self.w.units[i].position as u64,self.w.units[i-1].position as u64));}}
  for i in 0..self.w.units.len(){self.w.units[i].end=Some(if i+1<self.w.units.len(){self.w.units[i+1].position}else{self.data.len()});}
  Ok(())
 }
 fn frame(&mut self,index:usize,start:usize,end:usize)->Result<(),DecodeError>{
  self.at=start;self.end=end;self.w.field_context=[index as u32,0,0];self.w.frames.push(Decoded{index,start,end,..Decoded::default()});
  let result:Result<(),DecodeError>=(||{
   let tf=self.byte(20,0)?;self.w.frames[index].transition_flag=Some(tf);let ef=self.byte(21,0)?;self.w.frames[index].extra_flag=Some(ef);if tf>1||ef>1{return Err(DecodeError::Shape{field:"binary-frame-flags",actual:((tf as u64)<<8)|ef as u64,offset:self.at});}
   let(f0,fr)=self.float(22,0)?;let(rd,rr)=self.float(23,0)?;let h=self.scalar(2,24,0)?.0 as usize;self.w.frames[index].harmonics=Some(h);self.limit(h,self.w.configuration.harmonic_capacity as usize,"harmonics")?;self.source_values(4*h)?;self.frame_values(index,2+4*h)?;
   let vt=if h==0{Vec::new()}else{self.delta(3*h,25)?};let mut phases=Vec::new();for i in 0..h{phases.push(self.quantize(2,true,32767,0x400921fb54442d18,26,i)?);}
   let spectral=self.byte(27,0)?;self.w.frames[index].spectral=Some(spectral);if spectral>1{return Err(DecodeError::Shape{field:"binary-spectral",actual:spectral as u64,offset:self.at});}self.w.frames[index].noise_null=Some(spectral==1);
   let noise=if spectral==1{None}else{
    let channels=self.w.configuration.channels as usize;let mut counts=Vec::new();for i in 0..channels{let n=self.byte(28,i)?;self.w.frames[index].envelope_counts.push(n);self.limit(n as usize,self.w.configuration.envelope_capacity as usize,"envelope-count")?;counts.push(n as usize);}
    let bins=self.w.configuration.bins as usize;let extent=bins+channels+2*counts.iter().sum::<usize>();self.source_values(extent)?;self.frame_values(index,1+extent)?;
    let(scale,sr)=self.float(29,0)?;let psd=self.delta(bins,30)?;let scale64=D::widen(W::from_bits(scale)?)?.bits();let mut dc=Vec::new();for i in 0..channels{dc.push(self.quantize(2,false,65535,scale64,31,i)?);}
    let mut bands=Vec::new();for i in 0..channels{let mut amplitudes=Vec::new();let mut phases=Vec::new();for j in 0..counts[i]{amplitudes.push(self.quantize(2,false,65535,scale64,32+i as u32*2,j)?);}for j in 0..counts[i]{phases.push(self.quantize(1,true,127,0x400921fb54442d18,33+i as u32*2,j)?);}bands.push(Band{dc:dc[i],amplitudes,phases});}
    Some(Noise{scale:self.value(scale,sr)?,psd,bands})
   };
   let transition=if tf==0{None}else{
    let count=self.word(40,0)?as usize;self.w.frames[index].transition_count=Some(count);let(rate,rate_read)=self.float(41,0)?;let kind=self.word(42,0)?as i32;self.w.frames[index].transition_type=Some(kind);let(scale,scale_read)=self.float(43,0)?;self.source_values(count)?;
    if count>self.end.saturating_sub(self.at)/2{return Err(self.extent("transition-frame-extent",count as u64,(self.end.saturating_sub(self.at)/2)as u64));}self.limit(count,64,"own-transition-values")?;self.frame_values(index,2+count)?;
    let scale64=D::widen(W::from_bits(scale)?)?.bits();let mut values=Vec::new();for i in 0..count{values.push(self.quantize(2,true,32767,scale64,44,i)?);}Some(Transition{rate:self.value(rate,rate_read)?,kind,scale:self.value(scale,scale_read)?,values})
   };
   let extra=if ef==0{None}else{self.frame_values(index,1)?;let(v,r)=self.float(45,0)?;Some(self.value(v,r)?)};
   if self.at!=end{return Err(self.extent("exact-frame-EOF",self.at as u64,end as u64));}
   self.w.frames[index].layer1_complete=true;
   let noise=noise.ok_or(DecodeError::NoiseNullNormalization{frame:index})?;
   let semantic=SourceFrame{f0:self.value(f0,fr)?,rd:self.value(rd,rr)?,vt,phases,noise,transition,extra};let normalized=frame::assemble(&semantic).map_err(DecodeError::Normalize)?;
   self.w.frames[index].word_reads=normalized.links.iter().map(|v|v.a_offset as usize).collect();self.w.frames[index].raw=Some(normalized.raw);self.w.frames[index].complete=true;Ok(())
  })();self.w.frames[index].consumed=self.at;result
 }
 fn unit(&mut self,selected:usize)->Result<(),DecodeError>{
  let u=&self.w.units[selected];let end=u.end.unwrap_or(self.data.len());let start=u.position;let positions=u.positions.clone();self.w.selected_end=Some(end);self.limit(end-start,64*1024*1024,"source-unit-bytes")?;
  let mut expected=start+4+4*positions.len();for i in 0..positions.len(){let next=if i+1<positions.len(){positions[i+1]}else{end};if positions[i]!=expected||next<=positions[i]||next>end{return Err(self.extent("strict-frame-range",positions[i]as u64,expected as u64));}self.frame(i,positions[i],next)?;expected=next;}
  if expected!=end{return Err(self.extent("exact-unit-EOF",expected as u64,end as u64));}Ok(())
 }
}
pub(crate) fn decode(data:&[u8],selected:usize)->DecodeWork{
 let mut w=DecodeWork{data_bytes:data.len(),selected_unit:selected,field_context:[128,0,0],..DecodeWork::default()};
 if data.len()>4194304{w.error=Some(DecodeError::Extent{field:"encoded-data",actual:data.len()as u64,limit:4194304,offset:0});return w;}
 let result={let mut r=Reader{data,at:0,end:data.len(),w:&mut w};(move||{r.header()?;r.directory(selected)?;r.unit(selected)})()};
 match result{Ok(())=>w.complete=true,Err(e)=>w.error=Some(e)}w
}
