// SPDX-License-Identifier: MIT
use super::{StdWord32,StdNumericError};
pub(crate) const COMPILED_LAYER1_PHASE_REQUEST_BINDING_PENDING:bool=true;
pub(crate) const EDGE_CAP:usize=12;
#[derive(Clone,Copy,Debug)]
pub(crate) struct Input<'a>{pub(crate) rd:u32,pub(crate) source_f0:u32,pub(crate) target:u32,pub(crate) nyquist:u32,pub(crate) original_phases:&'a[u32]}
#[derive(Clone,Copy,Debug)]
pub(crate) enum Value{Single(u32),Integer(i32),Length(usize),Decision(bool)}
#[derive(Clone,Copy,Debug)]
pub(crate) enum RequestError{Word32{stage:&'static str,error:StdNumericError},PhaseExtent{length:usize},NegativeSourceF0{bits:u32},RendererCount{count:i32,existing:i32},Capacity{stage:&'static str}}
#[derive(Clone,Debug)]
pub(crate) struct Edge{pub(crate) stage:&'static str,pub(crate) op:&'static str,pub(crate) a:Value,pub(crate) b:Option<Value>,pub(crate) result:Option<Value>,pub(crate) error:Option<RequestError>}
#[derive(Clone,Copy,Debug)]
pub(crate) struct Request<'a>{pub(crate) rd:u32,pub(crate) original_f0:u32,pub(crate) target:u32,pub(crate) count:i32,pub(crate) original_prefix:&'a[u32]}
#[derive(Clone,Copy,Debug)]
pub(crate) enum Outcome<'a>{UnvoicedBypass(&'a[u32]),Reuse(&'a[u32]),Request(Request<'a>)}
#[derive(Clone,Debug)]
pub(crate) struct Report<'a>{
 pub(crate) input:Input<'a>,pub(crate) edges:Vec<Edge>,pub(crate) completed_edges:usize,
 pub(crate) unvoiced:Option<bool>,pub(crate) source_f0_finite:Option<u32>,pub(crate) source_negative:Option<bool>,
 pub(crate) existing_length:Option<i32>,pub(crate) quotient:Option<u32>,pub(crate) truncated:Option<i32>,pub(crate) count:Option<i32>,pub(crate) extension_selected:Option<bool>,
 pub(crate) renderer_below_prefix:Option<bool>,pub(crate) renderer_above_limit:Option<bool>,
 pub(crate) outcome:Option<Outcome<'a>>,pub(crate) error:Option<RequestError>,pub(crate) complete:bool,
 pub(crate) compiled_layer1_binding_pending:bool,pub(crate) native_invoked:bool,
}
impl<'a>Report<'a>{
 fn begin(&mut self,stage:&'static str,op:&'static str,a:Value,b:Option<Value>)->Result<usize,RequestError>{
  if self.edges.len()>=EDGE_CAP{return Err(RequestError::Capacity{stage});}
  let at=self.edges.len();self.edges.push(Edge{stage,op,a,b,result:None,error:None});Ok(at)
 }
 fn done(&mut self,at:usize,v:Value){self.edges[at].result=Some(v);self.completed_edges+=1;}
 fn failed<T>(&mut self,at:usize,e:RequestError)->Result<T,RequestError>{self.edges[at].error=Some(e);Err(e)}
 fn calculate(&mut self)->Result<(),RequestError>{
  let i=self.input;
  let at=self.begin("source.unvoiced","zero32",Value::Single(i.source_f0),None)?;
  let zero=i.source_f0&0x7fff_ffff==0;self.done(at,Value::Decision(zero));self.unvoiced=Some(zero);
  if zero{self.outcome=Some(Outcome::UnvoicedBypass(i.original_phases));return Ok(());}
  let stage="profile.sourceF0";let at=self.begin(stage,"finite32",Value::Single(i.source_f0),None)?;
  match StdWord32::from_bits(i.source_f0){Ok(v)=>{self.done(at,Value::Single(v.bits()));self.source_f0_finite=Some(v.bits());},Err(error)=>return self.failed(at,RequestError::Word32{stage,error})}
  let at=self.begin("profile.sourceNegative","negative32",Value::Single(i.source_f0),None)?;
  let neg=i.source_f0>>31!=0;self.done(at,Value::Decision(neg));self.source_negative=Some(neg);
  if neg{return Err(RequestError::NegativeSourceF0{bits:i.source_f0});}
  let at=self.begin("source.phaseLength","lengthToI32",Value::Length(i.original_phases.len()),None)?;
  let existing=match i32::try_from(i.original_phases.len()){Ok(v)=>v,Err(_)=>return self.failed(at,RequestError::PhaseExtent{length:i.original_phases.len()})};
  self.done(at,Value::Integer(existing));self.existing_length=Some(existing);
  let stage="source.NyquistDivTarget";let at=self.begin(stage,"div32",Value::Single(i.nyquist),Some(Value::Single(i.target)))?;
  let quotient=match StdWord32::from_bits(i.nyquist).and_then(|x|StdWord32::from_bits(i.target).and_then(|y|x.div(y))){Ok(v)=>v,Err(error)=>return self.failed(at,RequestError::Word32{stage,error})};
  self.done(at,Value::Single(quotient.bits()));self.quotient=Some(quotient.bits());
  let stage="source.checkedCast";let at=self.begin(stage,"truncI32",Value::Single(quotient.bits()),None)?;
  let cast=match quotient.trunc_i32(){Ok(v)=>v,Err(error)=>return self.failed(at,RequestError::Word32{stage,error})};
  self.done(at,Value::Integer(cast));self.truncated=Some(cast);
  let at=self.begin("source.countMax","maxI32",Value::Integer(existing),Some(Value::Integer(cast)))?;
  let count=existing.max(cast);self.done(at,Value::Integer(count));self.count=Some(count);
  let at=self.begin("source.strictGreater","greaterI32",Value::Integer(count),Some(Value::Integer(existing)))?;
  let extension=count>existing;self.done(at,Value::Decision(extension));self.extension_selected=Some(extension);
  if !extension{self.outcome=Some(Outcome::Reuse(i.original_phases));return Ok(());}
  let at=self.begin("renderer.belowPrefix","lessI32",Value::Integer(count),Some(Value::Integer(existing)))?;
  let below=count<existing;self.done(at,Value::Decision(below));self.renderer_below_prefix=Some(below);
  if below{return Err(RequestError::RendererCount{count,existing});}
  let at=self.begin("renderer.above1600","greaterI32",Value::Integer(count),Some(Value::Integer(1600)))?;
  let above=count>1600;self.done(at,Value::Decision(above));self.renderer_above_limit=Some(above);
  if above{return Err(RequestError::RendererCount{count,existing});}
  self.outcome=Some(Outcome::Request(Request{rd:i.rd,original_f0:i.source_f0,target:i.target,count,original_prefix:i.original_phases}));
  Ok(())
 }
 pub(crate) fn create(input:Input<'a>)->Self{
  let mut r=Self{input,edges:Vec::new(),completed_edges:0,unvoiced:None,source_f0_finite:None,source_negative:None,existing_length:None,quotient:None,truncated:None,count:None,extension_selected:None,renderer_below_prefix:None,renderer_above_limit:None,outcome:None,error:None,complete:false,compiled_layer1_binding_pending:COMPILED_LAYER1_PHASE_REQUEST_BINDING_PENDING,native_invoked:false};
  match r.calculate(){Ok(())=>r.complete=true,Err(e)=>r.error=Some(e)}r
 }
}
