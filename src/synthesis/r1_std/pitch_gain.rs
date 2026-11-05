// SPDX-License-Identifier: MIT
use super::{log2,StdNumericError,StdWideError,StdWord32,StdWord64};
const LN2_SOURCE_WORD:u32=0x3f31_7218;
const DB_SOURCE_WORD:u64=0x4021_5f2c_ec55_4b28;
const EDGE_CAP:usize=11;
pub(crate) const COMPILED_PITCHGAIN_BINDING_PENDING:bool=true;

#[derive(Clone,Copy,Debug)]
pub(crate) struct Input{pub(crate) original_f0:u32,pub(crate) target_f0:u32,pub(crate) spectrum_amplitude:u32}
#[derive(Clone,Copy,Debug)]
pub(crate) enum Word{Single(u32),Double(u64)}
#[derive(Clone,Copy,Debug)]
pub(crate) enum GainError{
    Word32{stage:&'static str,error:StdNumericError},
    Word64{stage:&'static str,error:StdWideError},
    Capacity{field:&'static str},
}
#[derive(Clone,Debug)]
pub(crate) struct Edge{
    pub(crate) stage:&'static str,pub(crate) op:&'static str,
    pub(crate) a:Word,pub(crate) b:Option<Word>,pub(crate) output_width:u8,
    pub(crate) result:Option<Word>,pub(crate) error:Option<GainError>,
}
#[derive(Clone,Debug)]
pub(crate) struct Report{
    pub(crate) input:Input,pub(crate) compiled_binding_pending:bool,
    pub(crate) ln2_source_word:u32,pub(crate) db_source_word:u64,
    pub(crate) first_log:Option<u32>,pub(crate) first_term:Option<u32>,
    pub(crate) ratio:Option<u32>,pub(crate) second_log:Option<u32>,pub(crate) second_term:Option<u32>,
    pub(crate) first_wide:Option<u64>,pub(crate) first_product:Option<u64>,
    pub(crate) second_wide:Option<u64>,pub(crate) second_product:Option<u64>,
    pub(crate) sum:Option<u64>,pub(crate) output:Option<u32>,
    pub(crate) edges:Vec<Edge>,pub(crate) completed_edges:usize,
    pub(crate) error:Option<GainError>,pub(crate) complete:bool,
}
impl Report{
    fn begin(&mut self,stage:&'static str,op:&'static str,a:Word,b:Option<Word>,output_width:u8)->Result<usize,GainError>{
        if self.edges.len()==EDGE_CAP{return Err(GainError::Capacity{field:"edges"});}
        let at=self.edges.len();self.edges.push(Edge{stage,op,a,b,output_width,result:None,error:None});Ok(at)
    }
    fn single<F>(&mut self,stage:&'static str,op:&'static str,a:Word,b:Option<Word>,f:F)->Result<u32,GainError>
    where F:FnOnce()->Result<StdWord32,StdNumericError>{
        let at=self.begin(stage,op,a,b,32)?;
        match f(){
            Ok(v)=>{let w=v.bits();self.edges[at].result=Some(Word::Single(w));self.completed_edges+=1;Ok(w)},
            Err(error)=>{let e=GainError::Word32{stage,error};self.edges[at].error=Some(e);Err(e)},
        }
    }
    fn double<F>(&mut self,stage:&'static str,op:&'static str,a:Word,b:Option<Word>,f:F)->Result<u64,GainError>
    where F:FnOnce()->Result<StdWord64,StdWideError>{
        let at=self.begin(stage,op,a,b,64)?;
        match f(){
            Ok(v)=>{let w=v.bits();self.edges[at].result=Some(Word::Double(w));self.completed_edges+=1;Ok(w)},
            Err(error)=>{let e=GainError::Word64{stage,error};self.edges[at].error=Some(e);Err(e)},
        }
    }
    fn narrow(&mut self,value:u64)->Result<u32,GainError>{
        let stage="return.narrow";let at=self.begin(stage,"narrow64to32",Word::Double(value),None,32)?;
        match StdWord64::from_bits(value).and_then(StdWord64::narrow){
            Ok(v)=>{let w=v.bits();self.edges[at].result=Some(Word::Single(w));self.completed_edges+=1;Ok(w)},
            Err(error)=>{let e=GainError::Word64{stage,error};self.edges[at].error=Some(e);Err(e)},
        }
    }
    fn calculate(&mut self)->Result<(),GainError>{
        let input=self.input;
        let first_log=self.single("first.log2","call_log2",Word::Single(input.spectrum_amplitude),None,||log2(StdWord32::from_bits(input.spectrum_amplitude)?))?;
        self.first_log=Some(first_log);
        let first=self.single("first.term","mul32",Word::Single(first_log),Some(Word::Single(LN2_SOURCE_WORD)),||StdWord32::from_bits(first_log)?.mul(StdWord32::from_bits(LN2_SOURCE_WORD)?))?;
        self.first_term=Some(first);
        let ratio=self.single("second.ratio","div32",Word::Single(input.original_f0),Some(Word::Single(input.target_f0)),||StdWord32::from_bits(input.original_f0)?.div(StdWord32::from_bits(input.target_f0)?))?;
        self.ratio=Some(ratio);
        let second_log=self.single("second.log2","call_log2",Word::Single(ratio),None,||log2(StdWord32::from_bits(ratio)?))?;
        self.second_log=Some(second_log);
        let second=self.single("second.term","mul32",Word::Single(second_log),Some(Word::Single(LN2_SOURCE_WORD)),||StdWord32::from_bits(second_log)?.mul(StdWord32::from_bits(LN2_SOURCE_WORD)?))?;
        self.second_term=Some(second);
        let first_wide=self.double("first.widen","widen32to64",Word::Single(first),None,||StdWord64::widen(StdWord32::from_bits(first)?))?;
        self.first_wide=Some(first_wide);
        let first_product=self.double("first.product","mul64",Word::Double(first_wide),Some(Word::Double(DB_SOURCE_WORD)),||StdWord64::from_bits(first_wide)?.mul(StdWord64::from_bits(DB_SOURCE_WORD)?))?;
        self.first_product=Some(first_product);
        let second_wide=self.double("second.widen","widen32to64",Word::Single(second),None,||StdWord64::widen(StdWord32::from_bits(second)?))?;
        self.second_wide=Some(second_wide);
        let second_product=self.double("second.product","mul64",Word::Double(second_wide),Some(Word::Double(DB_SOURCE_WORD)),||StdWord64::from_bits(second_wide)?.mul(StdWord64::from_bits(DB_SOURCE_WORD)?))?;
        self.second_product=Some(second_product);
        let sum=self.double("return.sum","add64",Word::Double(first_product),Some(Word::Double(second_product)),||StdWord64::from_bits(first_product)?.add(StdWord64::from_bits(second_product)?))?;
        self.sum=Some(sum);
        let out=self.narrow(sum)?;self.output=Some(out);Ok(())
    }
    pub(crate) fn create(input:Input)->Self{
        let mut r=Self{input,compiled_binding_pending:COMPILED_PITCHGAIN_BINDING_PENDING,ln2_source_word:LN2_SOURCE_WORD,db_source_word:DB_SOURCE_WORD,first_log:None,first_term:None,ratio:None,second_log:None,second_term:None,first_wide:None,first_product:None,second_wide:None,second_product:None,sum:None,output:None,edges:Vec::new(),completed_edges:0,error:None,complete:false};
        match r.calculate(){Ok(())=>r.complete=true,Err(e)=>r.error=Some(e)}r
    }
}
