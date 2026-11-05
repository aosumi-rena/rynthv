// SPDX-License-Identifier: MIT
use super::{StdWord32,StdWord64,StdNumericError,StdWideError};
use std::cmp::Ordering;
pub(crate) const COMPILED_LAYER1_LF_BINDING_PENDING:bool=true;
pub(crate) const SELECTED_SOURCE_TYPE_PROFILE:&str="FP_TYPE=float; double binary64 RN; float assignment binary32 RN; no contraction";
pub(crate) const EDGE_CAP:usize=192;
const C021:u64=0x3fcae147ae147ae1;
const CEPS:u64=0x3eb0c6f7a0b5ed8d;
const C27:u64=0x400599999999999a;
const CN1:u64=0xbff0000000000000;
const C48:u64=0x4013333333333333;
const C100:u64=0x4059000000000000;
const C0323:u64=0x3fd4ac083126e979;
const C1:u64=0x3ff0000000000000;
const C217:u64=0x40015c28f5c28f5c;
const C224:u64=0x4036666666666666;
const C118:u64=0x402799999999999a;
const C025:u64=0x3fd0000000000000;
const C011:u64=0x3fbc28f5c28f5c29;
const C05:u64=0x3fe0000000000000;
const C12:u64=0x3ff3333333333333;
const C0093552:u64=0x3f8328d1b0615c3b;
const C596:u64=0x4017d70a3d70a3d7;
const C796:u64=0x401fd70a3d70a3d7;
const C2:u64=0x4000000000000000;
const C10428:u64=0x3ff0af4f0d844d01;
#[derive(Clone,Copy,Debug)]
pub(crate) struct Input{pub(crate) rd:u32,pub(crate) t0:u32,pub(crate) ee:u32}
#[derive(Clone,Copy,Debug)]
pub(crate) struct Model{pub(crate) t0:u32,pub(crate) te:u32,pub(crate) tp:u32,pub(crate) ta:u32,pub(crate) ee:u32}
#[derive(Clone,Copy,Debug)]
pub(crate) enum Value{Single(u32),Double(u64),Decision(bool)}
#[derive(Clone,Copy,Debug)]
pub(crate) enum LeafError{Word32{stage:&'static str,error:StdNumericError},Word64{stage:&'static str,error:StdWideError},Capacity{stage:&'static str}}
#[derive(Clone,Copy,Debug)]
pub(crate) enum RapBranch{Below021,Middle,Upper}
#[derive(Clone,Copy,Debug)]
pub(crate) enum MainBranch{Below27,AtOrAbove27}
#[derive(Clone,Debug)]
pub(crate) struct Edge{pub(crate) stage:&'static str,pub(crate) op:&'static str,pub(crate) a:Value,pub(crate) b:Option<Value>,pub(crate) output_width:u8,pub(crate) result:Option<Value>,pub(crate) error:Option<LeafError>}
#[derive(Clone,Debug)]
pub(crate) struct Report{
 pub(crate) input:Input,pub(crate) compiled_layer1_binding_pending:bool,pub(crate) source_type_profile:&'static str,
 pub(crate) rap_branch:Option<RapBranch>,pub(crate) main_branch:Option<MainBranch>,
 pub(crate) rap:Option<u32>,pub(crate) oqupp:Option<u32>,pub(crate) rkp:Option<u32>,pub(crate) rgp:Option<u32>,
 pub(crate) tp:Option<u32>,pub(crate) te:Option<u32>,pub(crate) ta:Option<u32>,pub(crate) t0:Option<u32>,pub(crate) ee:Option<u32>,
 pub(crate) output:Option<Model>,pub(crate) edges:Vec<Edge>,pub(crate) completed_edges:usize,pub(crate) error:Option<LeafError>,pub(crate) complete:bool,
}
#[derive(Clone,Copy)]enum Binary{Add,Sub,Mul,Div}
impl Report{
 fn begin(&mut self,stage:&'static str,op:&'static str,a:Value,b:Option<Value>,width:u8)->Result<usize,LeafError>{
  if self.edges.len()>=EDGE_CAP{return Err(LeafError::Capacity{stage});}
  let at=self.edges.len();self.edges.push(Edge{stage,op,a,b,output_width:width,result:None,error:None});Ok(at)
 }
 fn finish64(&mut self,at:usize,stage:&'static str,r:Result<StdWord64,StdWideError>)->Result<u64,LeafError>{
  match r{Ok(v)=>{let b=v.bits();self.edges[at].result=Some(Value::Double(b));self.completed_edges+=1;Ok(b)},Err(error)=>{let e=LeafError::Word64{stage,error};self.edges[at].error=Some(e);Err(e)}}
 }
 fn single(&mut self,stage:&'static str,op:&'static str,v:u32)->Result<u32,LeafError>{
  let at=self.begin(stage,op,Value::Single(v),None,32)?;
  match StdWord32::from_bits(v){Ok(w)=>{self.edges[at].result=Some(Value::Single(w.bits()));self.completed_edges+=1;Ok(w.bits())},Err(error)=>{let e=LeafError::Word32{stage,error};self.edges[at].error=Some(e);Err(e)}}
 }
 fn widen(&mut self,stage:&'static str,v:u32)->Result<u64,LeafError>{
  let at=self.begin(stage,"widen32to64",Value::Single(v),None,64)?;
  let r=StdWord32::from_bits(v).map_err(StdWideError::from).and_then(StdWord64::widen);self.finish64(at,stage,r)
 }
 fn literal(&mut self,stage:&'static str,v:u64)->Result<u64,LeafError>{let at=self.begin(stage,"literal64",Value::Double(v),None,64)?;self.finish64(at,stage,StdWord64::from_bits(v))}
 fn narrow(&mut self,stage:&'static str,v:u64)->Result<u32,LeafError>{
  let at=self.begin(stage,"assign64to32",Value::Double(v),None,32)?;
  match StdWord64::from_bits(v).and_then(StdWord64::narrow){Ok(w)=>{self.edges[at].result=Some(Value::Single(w.bits()));self.completed_edges+=1;Ok(w.bits())},Err(error)=>{let e=LeafError::Word64{stage,error};self.edges[at].error=Some(e);Err(e)}}
 }
 fn binary(&mut self,stage:&'static str,op:Binary,a:u64,b:u64)->Result<u64,LeafError>{
  let label=match op{Binary::Add=>"add64",Binary::Sub=>"sub64",Binary::Mul=>"mul64",Binary::Div=>"div64"};
  let at=self.begin(stage,label,Value::Double(a),Some(Value::Double(b)),64)?;
  let r=StdWord64::from_bits(a).and_then(|x|StdWord64::from_bits(b).and_then(|y|match op{Binary::Add=>x.add(y),Binary::Sub=>x.sub(y),Binary::Mul=>x.mul(y),Binary::Div=>x.div(y)}));self.finish64(at,stage,r)
 }
 fn less(&mut self,stage:&'static str,a:u64,b:u64)->Result<bool,LeafError>{
  let at=self.begin(stage,"less64",Value::Double(a),Some(Value::Double(b)),1)?;
  let r=StdWord64::from_bits(a).and_then(|x|StdWord64::from_bits(b).map(|y|(x.bits(),y.bits())));
  match r{Err(error)=>{let e=LeafError::Word64{stage,error};self.edges[at].error=Some(e);Err(e)},Ok((x,y))=>{
   let sign=1_u64<<63;let ax=x&!sign;let ay=y&!sign;
   let cmp=if ax==0&&ay==0{Ordering::Equal}else if x>>63!=y>>63{if x>>63!=0{Ordering::Less}else{Ordering::Greater}}else if x>>63!=0{y.cmp(&x)}else{x.cmp(&y)};
   let out=cmp==Ordering::Less;self.edges[at].result=Some(Value::Decision(out));self.completed_edges+=1;Ok(out)
  }}
 }
 fn calculate(&mut self)->Result<(),LeafError>{
  use Binary::{Add,Sub,Mul,Div};let i=self.input;
  self.single("profile.rd","finite32",i.rd)?;self.single("profile.T0","finite32",i.t0)?;self.single("profile.Ee","finite32",i.ee)?;
  let rd=self.widen("Rap.test021.rd",i.rd)?;
  let rap64=if self.less("Rap.test021",rd,C021)?{
   self.rap_branch=Some(RapBranch::Below021);self.literal("Rap.literal1e-6",CEPS)?
  }else{
   let rd=self.widen("Rap.test27.rd",i.rd)?;
   if self.less("Rap.test27",rd,C27)?{
    self.rap_branch=Some(RapBranch::Middle);
    let rd=self.widen("Rap.middle.rd",i.rd)?;let v=self.binary("Rap.middle.mul",Mul,C48,rd)?;
    let v=self.binary("Rap.middle.add",Add,CN1,v)?;self.binary("Rap.middle.div",Div,v,C100)?
   }else{
    self.rap_branch=Some(RapBranch::Upper);let rd=self.widen("Rap.upper.rd",i.rd)?;self.binary("Rap.upper.div",Div,C0323,rd)?
   }
  };
  let rap=self.narrow("Rap.assign",rap64)?;self.rap=Some(rap);
  let rd=self.widen("OQupp.rd",i.rd)?;let v=self.binary("OQupp.mul",Mul,C217,rd)?;
  let v=self.binary("OQupp.div",Div,C1,v)?;let v=self.binary("OQupp.sub",Sub,C1,v)?;
  let oq=self.narrow("OQupp.assign",v)?;self.oqupp=Some(oq);
  let rd=self.widen("main.test27.rd",i.rd)?;
  let (rkp,rgp)=if self.less("main.test27",rd,C27)?{
   self.main_branch=Some(MainBranch::Below27);
   let rd=self.widen("Rkp.low.rd",i.rd)?;let v=self.binary("Rkp.low.mul",Mul,C118,rd)?;
   let v=self.binary("Rkp.low.add",Add,C224,v)?;let v=self.binary("Rkp.low.div",Div,v,C100)?;
   let k=self.narrow("Rkp.low.assign",v)?;self.rkp=Some(k);
   let kw=self.widen("Rgp.low.numerator.Rkp",k)?;let num=self.binary("Rgp.low.numerator.mul",Mul,C025,kw)?;
   let rd=self.widen("Rgp.low.denominator.rd",i.rd)?;let v=self.binary("Rgp.low.denominator.mul011",Mul,C011,rd)?;
   let kw=self.widen("Rgp.low.denominator.Rkp",k)?;let w=self.binary("Rgp.low.denominator.mul12",Mul,C12,kw)?;
   let w=self.binary("Rgp.low.denominator.add05",Add,C05,w)?;let v=self.binary("Rgp.low.denominator.div",Div,v,w)?;
   let rw=self.widen("Rgp.low.denominator.Rap",rap)?;let den=self.binary("Rgp.low.denominator.sub",Sub,v,rw)?;
   let v=self.binary("Rgp.low.div",Div,num,den)?;let g=self.narrow("Rgp.low.assign",v)?;self.rgp=Some(g);(k,g)
  }else{
   self.main_branch=Some(MainBranch::AtOrAbove27);
   let ow=self.widen("Rgp.high.OQupp",oq)?;let v=self.binary("Rgp.high.mul2",Mul,C2,ow)?;
   let v=self.binary("Rgp.high.sub796",Sub,C796,v)?;let v=self.binary("Rgp.high.div",Div,C596,v)?;
   let v=self.binary("Rgp.high.add",Add,C0093552,v)?;let g=self.narrow("Rgp.high.assign",v)?;self.rgp=Some(g);
   let gw=self.widen("Rkp.high.Rgp",g)?;let v=self.binary("Rkp.high.mul2",Mul,C2,gw)?;
   let ow=self.widen("Rkp.high.OQupp",oq)?;let v=self.binary("Rkp.high.mulOQupp",Mul,v,ow)?;
   let v=self.binary("Rkp.high.sub",Sub,v,C10428)?;let k=self.narrow("Rkp.high.assign",v)?;self.rkp=Some(k);(k,g)
  };
  let gw=self.widen("tp.Rgp",rgp)?;let v=self.binary("tp.mul2",Mul,C2,gw)?;
  let v=self.binary("tp.div",Div,C1,v)?;let tp=self.narrow("tp.assign",v)?;self.tp=Some(tp);
  let tw=self.widen("te.tp",tp)?;let kw=self.widen("te.Rkp",rkp)?;let v=self.binary("te.add1",Add,kw,C1)?;
  let v=self.binary("te.mul",Mul,tw,v)?;let te=self.narrow("te.assign",v)?;self.te=Some(te);
  let ta=self.single("ta.copyRap","copy32",rap)?;self.ta=Some(ta);
  let t0=self.single("T0.copyInput","copy32",i.t0)?;self.t0=Some(t0);
  let ee=self.single("Ee.copyInput","copy32",i.ee)?;self.ee=Some(ee);
  self.output=Some(Model{t0,te,tp,ta,ee});Ok(())
 }
 pub(crate) fn create(input:Input)->Self{
  let mut r=Self{input,compiled_layer1_binding_pending:COMPILED_LAYER1_LF_BINDING_PENDING,source_type_profile:SELECTED_SOURCE_TYPE_PROFILE,rap_branch:None,main_branch:None,rap:None,oqupp:None,rkp:None,rgp:None,tp:None,te:None,ta:None,t0:None,ee:None,output:None,edges:Vec::new(),completed_edges:0,error:None,complete:false};
  match r.calculate(){Ok(())=>r.complete=true,Err(e)=>r.error=Some(e)}r
 }
}
