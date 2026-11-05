// SPDX-License-Identifier: MIT
use super::{StdNumericError,StdWord32};
use std::cmp::Ordering;
const ROW_CAP:usize=128;
const MINIMUM_WORD:u32=0x41f0_0000;
const PREFIX_EDGE_CAP:usize=2;
const ROW_EDGE_CAP:usize=10;

#[derive(Clone,Copy,Debug)]
pub(crate) struct PrefixInput{pub(crate) guide_start:u32,pub(crate) hop:u32}
#[derive(Clone,Copy,Debug)]
pub(crate) enum PrefixError{Numeric{stage:&'static str,error:StdNumericError},Capacity}
#[derive(Clone,Copy,Debug)]
pub(crate) enum PrefixValue{Word32(u32),SignedI32(i32)}
#[derive(Clone,Debug)]
pub(crate) struct PrefixEdge{
 pub(crate) stage:&'static str,pub(crate) op:&'static str,pub(crate) a:u32,pub(crate) b:Option<u32>,
 pub(crate) result:Option<PrefixValue>,pub(crate) error:Option<PrefixError>,
}
#[derive(Clone,Debug)]
pub(crate) struct UnitPrefix{
 pub(crate) input:PrefixInput,pub(crate) quotient:Option<u32>,pub(crate) pitch_start:Option<i32>,
 pub(crate) edges:Vec<PrefixEdge>,pub(crate) completed_operations:usize,
 pub(crate) complete:bool,pub(crate) error:Option<PrefixError>,
}
impl UnitPrefix{
 fn begin(&mut self,stage:&'static str,op:&'static str,a:u32,b:Option<u32>)->Result<usize,PrefixError>{
  if self.edges.len()==PREFIX_EDGE_CAP{return Err(PrefixError::Capacity);}
  let at=self.edges.len();self.edges.push(PrefixEdge{stage,op,a,b,result:None,error:None});Ok(at)
 }
 fn calculate(&mut self)->Result<(),PrefixError>{
  let input=self.input;let stage="pitch_start.div";let at=self.begin(stage,"div32",input.guide_start,Some(input.hop))?;
  let q=match StdWord32::from_bits(input.guide_start).and_then(|a|StdWord32::from_bits(input.hop).and_then(|b|a.div(b))){
   Ok(v)=>{self.edges[at].result=Some(PrefixValue::Word32(v.bits()));self.completed_operations+=1;v.bits()},
   Err(error)=>{let e=PrefixError::Numeric{stage,error};self.edges[at].error=Some(e);return Err(e)},
  };self.quotient=Some(q);
  let stage="pitch_start.checked_i32";let at=self.begin(stage,"checked_trunc_i32",q,None)?;
  let p=match StdWord32::from_bits(q).and_then(StdWord32::trunc_i32){
   Ok(v)=>{self.edges[at].result=Some(PrefixValue::SignedI32(v));self.completed_operations+=1;v},
   Err(error)=>{let e=PrefixError::Numeric{stage,error};self.edges[at].error=Some(e);return Err(e)},
  };self.pitch_start=Some(p);Ok(())
 }
 pub(crate) fn create(input:PrefixInput)->Self{
  let mut r=Self{input,quotient:None,pitch_start:None,edges:Vec::new(),completed_operations:0,complete:false,error:None};
  match r.calculate(){Ok(())=>r.complete=true,Err(e)=>r.error=Some(e)}r
 }
}

#[derive(Clone,Copy,Debug)]
pub(crate) struct RowInput<'a>{
 pub(crate) frequency_words:&'a[u32],pub(crate) editor_rows_present:bool,pub(crate) has_host_edits:bool,
 pub(crate) frame_index:usize,pub(crate) contour_word:u32,pub(crate) source_f0:u32,
 pub(crate) spectrum_amplitude:u32,
}
#[derive(Clone,Copy,Debug)]
pub(crate) struct ScalarInput{pub(crate) original_f0:u32,pub(crate) target_f0:u32,pub(crate) spectrum_amplitude:u32}
#[derive(Clone,Copy,Debug)]
pub(crate) enum PreludeError{
 PrefixUnavailable{error:Option<PrefixError>},UnsupportedEditorSampling,
 Shape{field:&'static str,actual:usize,limit:usize},
 Numeric{stage:&'static str,error:StdNumericError},
 IndexArithmetic{pitch_start:i32,frame_index:usize},NegativeIndex{index:i32},
 NegativeSourceF0{word:u32},Capacity{field:&'static str},
}
#[derive(Clone,Copy,Debug)]
pub(crate) enum Value{Word32(u32),SignedI32(i32),Index(usize),Bool(bool),Scalar(ScalarInput)}
#[derive(Clone,Debug)]
pub(crate) struct RowEdge{
 pub(crate) stage:&'static str,pub(crate) op:&'static str,pub(crate) a:Value,pub(crate) b:Option<Value>,
 pub(crate) result:Option<Value>,pub(crate) error:Option<PreludeError>,
}
#[derive(Clone,Debug)]
pub(crate) struct RowReport<'a>{
 pub(crate) prefix:&'a UnitPrefix,pub(crate) input:RowInput<'a>,
 pub(crate) prefix_accepted:bool,pub(crate) profile_accepted:bool,
 pub(crate) index_sum:Option<i32>,pub(crate) last_frequency_index:Option<i32>,pub(crate) selected_index:Option<usize>,
 pub(crate) expression_word:Option<u32>,pub(crate) added_word:Option<u32>,pub(crate) minimum_source_word:u32,
 pub(crate) target_word:Option<u32>,pub(crate) target_finite:Option<bool>,pub(crate) source_finite:Option<bool>,
 pub(crate) source_negative:Option<bool>,pub(crate) source_zero:Option<bool>,pub(crate) unvoiced_skip:Option<bool>,
 pub(crate) explicit_amplitude_copied:bool,pub(crate) scalar_input:Option<ScalarInput>,
 pub(crate) edges:Vec<RowEdge>,pub(crate) completed_operations:usize,pub(crate) complete:bool,pub(crate) error:Option<PreludeError>,
}
impl<'a> RowReport<'a>{
 fn begin(&mut self,stage:&'static str,op:&'static str,a:Value,b:Option<Value>)->Result<usize,PreludeError>{
  if self.edges.len()==ROW_EDGE_CAP{return Err(PreludeError::Capacity{field:"row_edges"});}
  let at=self.edges.len();self.edges.push(RowEdge{stage,op,a,b,result:None,error:None});Ok(at)
 }
 fn done(&mut self,at:usize,value:Value){self.edges[at].result=Some(value);self.completed_operations+=1;}
 fn failed(&mut self,at:usize,error:PreludeError)->PreludeError{self.edges[at].error=Some(error);error}
 fn calculate(&mut self)->Result<(),PreludeError>{
  let input=self.input;
  if !self.prefix.complete{return Err(PreludeError::PrefixUnavailable{error:self.prefix.error});}
  let Some(pitch_start)=self.prefix.pitch_start else{return Err(PreludeError::PrefixUnavailable{error:self.prefix.error})};
  self.prefix_accepted=true;
  if input.frame_index>=ROW_CAP{return Err(PreludeError::Shape{field:"frame_index",actual:input.frame_index,limit:ROW_CAP-1});}
  if input.frequency_words.is_empty()||input.frequency_words.len()>ROW_CAP{return Err(PreludeError::Shape{field:"frequency_count",actual:input.frequency_words.len(),limit:ROW_CAP});}
  if input.editor_rows_present&&!input.has_host_edits{return Err(PreludeError::UnsupportedEditorSampling);}
  self.profile_accepted=true;
  let at=self.begin("row.index_add","checked_index_add",Value::SignedI32(pitch_start),Some(Value::Index(input.frame_index)))?;
  let i=match i32::try_from(input.frame_index){Ok(v)=>v,Err(_)=>return Err(self.failed(at,PreludeError::IndexArithmetic{pitch_start,frame_index:input.frame_index}))};
  let sum=match pitch_start.checked_add(i){Some(v)=>v,None=>return Err(self.failed(at,PreludeError::IndexArithmetic{pitch_start,frame_index:input.frame_index}))};
  self.index_sum=Some(sum);self.done(at,Value::SignedI32(sum));
  let last=(input.frequency_words.len()-1)as i32;self.last_frequency_index=Some(last);
  let at=self.begin("row.index_min","min_i32",Value::SignedI32(sum),Some(Value::SignedI32(last)))?;
  let selected=sum.min(last);self.done(at,Value::SignedI32(selected));
  if selected<0{return Err(PreludeError::NegativeIndex{index:selected});}
  let selected=selected as usize;self.selected_index=Some(selected);
  let at=self.begin("row.expression","copy_frequency",Value::Index(selected),None)?;
  let expression=input.frequency_words[selected];self.expression_word=Some(expression);self.done(at,Value::Word32(expression));
  let stage="target.add_contour";let at=self.begin(stage,"add32",Value::Word32(expression),Some(Value::Word32(input.contour_word)))?;
  let added=match StdWord32::from_bits(expression).and_then(|a|StdWord32::from_bits(input.contour_word).and_then(|b|a.add(b))){
   Ok(v)=>v,Err(error)=>return Err(self.failed(at,PreludeError::Numeric{stage,error})),
  };self.added_word=Some(added.bits());self.done(at,Value::Word32(added.bits()));
  let stage="target.max_thirty";let at=self.begin(stage,"maximum32",Value::Word32(MINIMUM_WORD),Some(Value::Word32(added.bits())))?;
  let minimum=match StdWord32::from_bits(MINIMUM_WORD){Ok(v)=>v,Err(error)=>return Err(self.failed(at,PreludeError::Numeric{stage,error}))};
  let target=minimum.maximum(added);self.target_word=Some(target.bits());self.done(at,Value::Word32(target.bits()));
  let at=self.begin("target.finite","finite32_bits",Value::Word32(target.bits()),None)?;
  let finite=(target.bits()&0x7f80_0000)!=0x7f80_0000;self.target_finite=Some(finite);self.done(at,Value::Bool(finite));
  let stage="source.finite_profile";let at=self.begin(stage,"finite_word32",Value::Word32(input.source_f0),None)?;
  let source=match StdWord32::from_bits(input.source_f0){Ok(v)=>v,Err(error)=>{self.source_finite=Some(false);return Err(self.failed(at,PreludeError::Numeric{stage,error}))}};
  self.source_finite=Some(true);self.done(at,Value::Bool(true));
  let zero=StdWord32::from_bits(0).map_err(|error|PreludeError::Numeric{stage:"source.zero_literal",error})?;
  let at=self.begin("source.negative","less32",Value::Word32(input.source_f0),Some(Value::Word32(0)))?;
  let negative=source.numeric_cmp(zero)==Ordering::Less;self.source_negative=Some(negative);self.done(at,Value::Bool(negative));
  if negative{return Err(PreludeError::NegativeSourceF0{word:input.source_f0});}
  let at=self.begin("source.zero","equal32",Value::Word32(input.source_f0),Some(Value::Word32(0)))?;
  let is_zero=source.numeric_cmp(zero)==Ordering::Equal;self.source_zero=Some(is_zero);self.unvoiced_skip=Some(is_zero);self.done(at,Value::Bool(is_zero));
  if is_zero{return Ok(());}
  let at=self.begin("row.scalar_input","copy_scalar_tuple",Value::Word32(input.source_f0),Some(Value::Word32(target.bits())))?;
  let scalar=ScalarInput{original_f0:input.source_f0,target_f0:target.bits(),spectrum_amplitude:input.spectrum_amplitude};
  self.explicit_amplitude_copied=true;self.scalar_input=Some(scalar);self.done(at,Value::Scalar(scalar));Ok(())
 }
 pub(crate) fn create(prefix:&'a UnitPrefix,input:RowInput<'a>)->Self{
  let mut r=Self{prefix,input,prefix_accepted:false,profile_accepted:false,index_sum:None,last_frequency_index:None,selected_index:None,expression_word:None,added_word:None,minimum_source_word:MINIMUM_WORD,target_word:None,target_finite:None,source_finite:None,source_negative:None,source_zero:None,unvoiced_skip:None,explicit_amplitude_copied:false,scalar_input:None,edges:Vec::new(),completed_operations:0,complete:false,error:None};
  match r.calculate(){Ok(())=>r.complete=true,Err(e)=>r.error=Some(e)}r
 }
}
