// SPDX-License-Identifier: MIT
use super::{StdWord32,StdWord64,StdNumericError,StdWideError,lf_from_rd};
pub(crate) const COMPILED_LAYER1_PHASE_INPUT_BINDING_PENDING:bool=true;
pub(crate) const FREQUENCY_EDGE_CAP:usize=8000;
#[derive(Clone,Copy,Debug)]pub(crate) struct Input<'a>{pub(crate) rd:u32,pub(crate) original_f0:u32,pub(crate) target:u32,pub(crate) count:u32,pub(crate) original_phases:&'a[u32]}
#[derive(Clone,Copy,Debug)]pub(crate) enum Value{Index(u32),Single(u32),Double(u64)}
#[derive(Clone,Copy,Debug)]pub(crate) enum AssemblyError{Count{count:u32},Prefix{length:usize},Word32{stage:&'static str,error:StdNumericError},Word64{stage:&'static str,error:StdWideError},Lf{error:lf_from_rd::LeafError},Capacity{stage:&'static str}}
#[derive(Clone,Debug)]pub(crate) struct Edge{pub(crate) index:Option<u32>,pub(crate) stage:&'static str,pub(crate) op:&'static str,pub(crate) a:Value,pub(crate) b:Option<Value>,pub(crate) width:u8,pub(crate) result:Option<Value>,pub(crate) error:Option<AssemblyError>}
#[derive(Clone,Debug)]pub(crate) struct Frequency{pub(crate) index:u32,pub(crate) index64:Option<u64>,pub(crate) plus_one:Option<u64>,pub(crate) target64:Option<u64>,pub(crate) product:Option<u64>,pub(crate) word:Option<u32>,pub(crate) edge_start:usize,pub(crate) edge_end:usize}
#[derive(Debug)]pub(crate) struct Report<'a>{pub(crate) input:Input<'a>,pub(crate) frequencies:Vec<Frequency>,pub(crate) frequency_words:Vec<u32>,pub(crate) edges:Vec<Edge>,pub(crate) completed_edges:usize,pub(crate) t0:Option<u32>,pub(crate) t0_edge:Option<Edge>,pub(crate) lf:Option<lf_from_rd::Report>,pub(crate) error:Option<AssemblyError>,pub(crate) complete:bool,pub(crate) compiled_layer1_binding_pending:bool,pub(crate) native_invoked:bool}
impl<'a>Report<'a>{
 fn op(&mut self,index:u32,stage:&'static str,op:&'static str,a:Value,b:Option<Value>,width:u8,f:impl FnOnce()->Result<Value,AssemblyError>)->Result<Value,AssemblyError>{
  if self.edges.len()>=FREQUENCY_EDGE_CAP{return Err(AssemblyError::Capacity{stage});}
  let at=self.edges.len();self.edges.push(Edge{index:Some(index),stage,op,a,b,width,result:None,error:None});
  match f(){Ok(v)=>{self.edges[at].result=Some(v);self.completed_edges+=1;Ok(v)},Err(e)=>{self.edges[at].error=Some(e);Err(e)}}
 }
 fn wide(&mut self,i:u32,stage:&'static str,op:&'static str,a:Value,b:Option<Value>,f:impl FnOnce()->Result<StdWord64,StdWideError>)->Result<u64,AssemblyError>{
  match self.op(i,stage,op,a,b,64,||f().map(|v|Value::Double(v.bits())).map_err(|error|AssemblyError::Word64{stage,error}))?{Value::Double(x)=>Ok(x),_=>unreachable!()}
 }
 fn calculate(&mut self)->Result<(),AssemblyError>{
  let input=self.input;if input.count<1||input.count>1600{return Err(AssemblyError::Count{count:input.count});}if input.original_phases.len()>512{return Err(AssemblyError::Prefix{length:input.original_phases.len()});}
  for i in 0..input.count{
   let start=self.edges.len();self.frequencies.push(Frequency{index:i,index64:None,plus_one:None,target64:None,product:None,word:None,edge_start:start,edge_end:start});
   let result=(||->Result<(),AssemblyError>{
    let index=self.wide(i,"frequency.index","exactIndexTo64",Value::Index(i),None,||StdWord32::from_i32(i as i32).map_err(StdWideError::Word32).and_then(StdWord64::widen))?;self.frequencies.last_mut().unwrap().index64=Some(index);
    let plus=self.wide(i,"frequency.addOne","add64",Value::Double(index),Some(Value::Double(0x3ff0000000000000)),||StdWord64::from_bits(index).and_then(|v|StdWord64::from_bits(0x3ff0000000000000).and_then(|one|v.add(one))))?;self.frequencies.last_mut().unwrap().plus_one=Some(plus);
    let target=self.wide(i,"frequency.target","widen32to64",Value::Single(input.target),None,||StdWord32::from_bits(input.target).map_err(StdWideError::Word32).and_then(StdWord64::widen))?;self.frequencies.last_mut().unwrap().target64=Some(target);
    let product=self.wide(i,"frequency.mul","mul64",Value::Double(plus),Some(Value::Double(target)),||StdWord64::from_bits(plus).and_then(|x|StdWord64::from_bits(target).and_then(|y|x.mul(y))))?;self.frequencies.last_mut().unwrap().product=Some(product);
    let stage="frequency.assign";let v=self.op(i,stage,"assign64to32",Value::Double(product),None,32,||StdWord64::from_bits(product).and_then(StdWord64::narrow).map(|x|Value::Single(x.bits())).map_err(|error|AssemblyError::Word64{stage,error}))?;
    let word=match v{Value::Single(x)=>x,_=>unreachable!()};self.frequencies.last_mut().unwrap().word=Some(word);self.frequency_words.push(word);Ok(())
   })();self.frequencies.last_mut().unwrap().edge_end=self.edges.len();result?;
  }
  let stage="source.T0.div32";self.t0_edge=Some(Edge{index:None,stage,op:"div32",a:Value::Single(0x3f800000),b:Some(Value::Single(input.original_f0)),width:32,result:None,error:None});
  let t0=match StdWord32::from_bits(0x3f800000).and_then(|one|StdWord32::from_bits(input.original_f0).and_then(|f|one.div(f))){Ok(v)=>v.bits(),Err(error)=>{let e=AssemblyError::Word32{stage,error};self.t0_edge.as_mut().unwrap().error=Some(e);return Err(e)}};
  self.t0_edge.as_mut().unwrap().result=Some(Value::Single(t0));self.t0=Some(t0);
  let lf=lf_from_rd::Report::create(lf_from_rd::Input{rd:input.rd,t0,ee:0x3f800000});let failure=lf.error;self.lf=Some(lf);if let Some(error)=failure{return Err(AssemblyError::Lf{error});}Ok(())
 }
 pub(crate) fn create(input:Input<'a>)->Self{
  let mut r=Self{input,frequencies:Vec::new(),frequency_words:Vec::new(),edges:Vec::new(),completed_edges:0,t0:None,t0_edge:None,lf:None,error:None,complete:false,compiled_layer1_binding_pending:COMPILED_LAYER1_PHASE_INPUT_BINDING_PENDING,native_invoked:false};
  match r.calculate(){Ok(())=>r.complete=true,Err(e)=>r.error=Some(e)}r
 }
}
