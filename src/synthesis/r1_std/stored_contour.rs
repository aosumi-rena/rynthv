// SPDX-License-Identifier: MIT
use super::{StdNumericError, StdWord32};
use std::cmp::Ordering;
const N_CAP: usize = 128;
const EDGE_CAP: usize = 2048;
const COMPARE_CAP: usize = 400;
const FINITE_CAP: usize = 260;
const PROBE_CAP: usize = 257;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Guide {
    pub(crate) first: u32, pub(crate) second: u32,
    pub(crate) source_start: u32, pub(crate) source_end: u32, pub(crate) amount: u32,
}
#[derive(Clone, Debug)]
pub(crate) struct Input { pub(crate) guide: Guide, pub(crate) frame_count: usize, pub(crate) extras: [Option<u32>; N_CAP] }
#[derive(Clone, Debug)]
pub(crate) enum ContourError {
    FrameSpan { actual: usize }, Capacity { field: &'static str },
    Nonfinite { field: &'static str, index: Option<usize>, word: u32 },
    NegativeRatio { word: u32 },
    Numeric { stage: &'static str, index: Option<usize>, error: StdNumericError },
}
#[derive(Clone, Debug)]
pub(crate) struct Edge {
    pub(crate) row: Option<usize>, pub(crate) stage: &'static str, pub(crate) op: &'static str,
    pub(crate) a: u32, pub(crate) b: Option<u32>, pub(crate) result: Option<u32>,
}
#[derive(Clone, Debug)]
pub(crate) struct Compare {
    pub(crate) row: Option<usize>, pub(crate) stage: &'static str,
    pub(crate) a: u32, pub(crate) b: u32, pub(crate) ordering: Option<i8>,
}
#[derive(Clone, Debug)]
pub(crate) struct FiniteCheck { pub(crate) field: &'static str, pub(crate) index: Option<usize>, pub(crate) word: u32, pub(crate) finite: bool }
#[derive(Clone, Debug)]
pub(crate) enum KnotOrigin { AllocatedPlusZero, InputExtra { index: usize, was_null: bool }, LeftPad { source: usize }, RightPad { source: usize } }
#[derive(Clone, Debug)]
pub(crate) struct Probe {
    pub(crate) row: usize, pub(crate) left: usize, pub(crate) next: usize,
    pub(crate) within: bool, pub(crate) query: u32, pub(crate) x_next: Option<u32>,
    pub(crate) greater: Option<bool>, pub(crate) compare_index: Option<usize>,
}
#[derive(Clone, Debug)]
pub(crate) struct Query {
    pub(crate) row: usize, pub(crate) cursor_before: usize, pub(crate) cursor_after: Option<usize>,
    pub(crate) query: Option<u32>, pub(crate) left: Option<usize>, pub(crate) right: Option<usize>,
    pub(crate) initial_value: Option<u32>, pub(crate) conditional_compare: Option<usize>,
    pub(crate) interpolated: Option<bool>, pub(crate) final_value: Option<u32>, pub(crate) output: Option<u32>,
    pub(crate) edge_begin: usize, pub(crate) edge_end: Option<usize>,
    pub(crate) probe_begin: usize, pub(crate) probe_end: Option<usize>,
}
#[derive(Clone, Debug)]
pub(crate) struct Report {
    pub(crate) input: Input, pub(crate) n: usize,
    pub(crate) ratio: Option<u32>, pub(crate) coefficient: Option<u32>, pub(crate) coefficient_scaled: Option<bool>,
    pub(crate) x: Vec<u32>, pub(crate) y: Vec<u32>, pub(crate) origins: Vec<KnotOrigin>,
    pub(crate) filled_input_knots: usize, pub(crate) padding_complete: bool,
    pub(crate) output: Vec<u32>, pub(crate) completed_outputs: usize, pub(crate) cursor: usize,
    pub(crate) edges: Vec<Edge>, pub(crate) compares: Vec<Compare>, pub(crate) finite_checks: Vec<FiniteCheck>,
    pub(crate) probes: Vec<Probe>, pub(crate) queries: Vec<Query>,
    pub(crate) error: Option<ContourError>, pub(crate) complete: bool,
}
impl Report {
    fn finite(&mut self, field: &'static str, index: Option<usize>, word: u32) -> Result<(), ContourError> {
        if self.finite_checks.len() == FINITE_CAP { return Err(ContourError::Capacity { field: "finite_checks" }); }
        let finite=word&0x7f80_0000!=0x7f80_0000;
        self.finite_checks.push(FiniteCheck { field, index, word, finite });
        if finite { Ok(()) } else { Err(ContourError::Nonfinite { field, index, word }) }
    }
    fn edge<F>(&mut self,row:Option<usize>,stage:&'static str,op:&'static str,a:u32,b:Option<u32>,f:F)->Result<u32,ContourError>
    where F:FnOnce()->Result<StdWord32,StdNumericError> {
        if self.edges.len()==EDGE_CAP{return Err(ContourError::Capacity{field:"edges"});}
        let at=self.edges.len();self.edges.push(Edge{row,stage,op,a,b,result:None});
        let w=f().map_err(|error|ContourError::Numeric{stage,index:row,error})?.bits();
        self.edges[at].result=Some(w);Ok(w)
    }
    fn binary(&mut self,row:Option<usize>,stage:&'static str,op:&'static str,a:u32,b:u32)->Result<u32,ContourError>{
        self.edge(row,stage,op,a,Some(b),||{
            let x=StdWord32::from_bits(a)?;let y=StdWord32::from_bits(b)?;
            match op{"add"=>x.add(y),"sub"=>x.sub(y),"mul"=>x.mul(y),"div"=>x.div(y),_=>unreachable!("static SOURCE opcode")}
        })
    }
    fn integer(&mut self,row:Option<usize>,stage:&'static str,x:usize)->Result<u32,ContourError>{
        self.edge(row,stage,"from_i32",x as u32,None,||StdWord32::from_i32(x as i32))
    }
    fn compare(&mut self,row:Option<usize>,stage:&'static str,a:u32,b:u32)->Result<(usize,Ordering),ContourError>{
        if self.compares.len()==COMPARE_CAP{return Err(ContourError::Capacity{field:"compares"});}
        let at=self.compares.len();self.compares.push(Compare{row,stage,a,b,ordering:None});
        let x=StdWord32::from_bits(a).map_err(|error|ContourError::Numeric{stage,index:row,error})?;
        let y=StdWord32::from_bits(b).map_err(|error|ContourError::Numeric{stage,index:row,error})?;
        let ordering=x.numeric_cmp(y);self.compares[at].ordering=Some(match ordering{Ordering::Less=>-1,Ordering::Equal=>0,Ordering::Greater=>1});Ok((at,ordering))
    }
    fn calculate(&mut self)->Result<(),ContourError>{
        let n=self.n;
        if !(2..=N_CAP).contains(&n){return Err(ContourError::FrameSpan{actual:n});}
        let g=self.input.guide;
        let numerator=self.binary(None,"ratio.numerator","add",g.first,g.second)?;
        let denominator=self.binary(None,"ratio.denominator","sub",g.source_end,g.source_start)?;
        let ratio=self.binary(None,"ratio","div",numerator,denominator)?;self.ratio=Some(ratio);
        self.finite("ratio",None,ratio)?;
        let (_,negative)=self.compare(None,"ratio.nonnegative",ratio,0)?;
        if negative==Ordering::Less{return Err(ContourError::NegativeRatio{word:ratio});}
        self.finite("guide.amount",None,g.amount)?;
        let (_,less_one)=self.compare(None,"coefficient.branch",ratio,0x3f80_0000)?;
        let scaled=less_one==Ordering::Less;self.coefficient_scaled=Some(scaled);
        let coefficient=if scaled{self.binary(None,"coefficient","mul",g.amount,ratio)?}else{g.amount};self.coefficient=Some(coefficient);
        self.x=vec![0;n+2];self.y=vec![0;n+2];self.origins=vec![KnotOrigin::AllocatedPlusZero;n+2];
        for i in 0..n{
            let v=self.input.extras[i];self.y[i+1]=v.unwrap_or(0);
            self.origins[i+1]=KnotOrigin::InputExtra{index:i,was_null:v.is_none()};
            self.x[i+1]=self.integer(Some(i),"knots.x",i)?;
            self.filled_input_knots=i+1;
            self.finite("knots.y",Some(i),self.y[i+1])?;
        }
        self.y[0]=self.y[1];self.origins[0]=KnotOrigin::LeftPad{source:1};
        self.y[n+1]=self.y[n];self.origins[n+1]=KnotOrigin::RightPad{source:n};
        self.x[n+1]=self.integer(None,"knots.xLast",n)?;self.padding_complete=true;
        self.output=vec![0;n];
        for i in 0..n{
            if self.queries.len()==N_CAP{return Err(ContourError::Capacity{field:"queries"});}
            let at=self.queries.len();self.queries.push(Query{row:i,cursor_before:self.cursor,cursor_after:None,query:None,left:None,right:None,initial_value:None,conditional_compare:None,interpolated:None,final_value:None,output:None,edge_begin:self.edges.len(),edge_end:None,probe_begin:self.probes.len(),probe_end:None});
            let iw=self.integer(Some(i),"query.i",i)?;
            let factor=self.integer(Some(i),"query.nMinus1.mul",n-1)?;
            let product=self.binary(Some(i),"query.product","mul",iw,factor)?;
            let divisor=self.integer(Some(i),"query.nMinus1.div",n-1)?;
            let query=self.binary(Some(i),"query","div",product,divisor)?;self.queries[at].query=Some(query);
            loop{
                if self.probes.len()==PROBE_CAP{return Err(ContourError::Capacity{field:"probes"});}
                let next=self.cursor+1;let within=next<self.x.len();let pi=self.probes.len();
                self.probes.push(Probe{row:i,left:self.cursor,next,within,query,x_next:if within{Some(self.x[next])}else{None},greater:None,compare_index:None});
                if !within{break;}
                let (ci,o)=self.compare(Some(i),"scan.strictGreater",query,self.x[next])?;
                let greater=o==Ordering::Greater;self.probes[pi].greater=Some(greater);self.probes[pi].compare_index=Some(ci);
                if !greater{break;}self.cursor+=1;
            }
            self.queries[at].probe_end=Some(self.probes.len());self.queries[at].cursor_after=Some(self.cursor);
            let left=self.cursor;let right=if left==self.x.len()-1{left}else{left+1};
            self.queries[at].left=Some(left);self.queries[at].right=Some(right);
            let mut value=self.y[left];self.queries[at].initial_value=Some(value);
            let mut interpolate=false;
            if left!=right{
                let(ci,o)=self.compare(Some(i),"interpolation.strictGreaterX0",query,self.x[0])?;
                self.queries[at].conditional_compare=Some(ci);interpolate=o==Ordering::Greater;
            }
            self.queries[at].interpolated=Some(interpolate);
            if interpolate{
                let dx=self.binary(Some(i),"value.queryMinusXLeft","sub",query,self.x[left])?;
                let dy=self.binary(Some(i),"value.yRightMinusValue","sub",self.y[right],value)?;
                let prod=self.binary(Some(i),"value.numerator","mul",dx,dy)?;
                let den=self.binary(Some(i),"value.xRightMinusXLeft","sub",self.x[right],self.x[left])?;
                let quotient=self.binary(Some(i),"value.quotient","div",prod,den)?;
                value=self.binary(Some(i),"value.add","add",value,quotient)?;
            }
            self.queries[at].final_value=Some(value);
            let out=self.binary(Some(i),"output.coefficient","mul",value,coefficient)?;
            self.output[i]=out;self.queries[at].output=Some(out);
            self.finite("output",Some(i),out)?;self.completed_outputs=i+1;
            self.queries[at].edge_end=Some(self.edges.len());
        }
        Ok(())
    }
    pub(crate) fn create(input:Input)->Self{
        let n=input.frame_count;let mut r=Self{input,n,ratio:None,coefficient:None,coefficient_scaled:None,x:Vec::new(),y:Vec::new(),origins:Vec::new(),filled_input_knots:0,padding_complete:false,output:Vec::new(),completed_outputs:0,cursor:0,edges:Vec::new(),compares:Vec::new(),finite_checks:Vec::new(),probes:Vec::new(),queries:Vec::new(),error:None,complete:false};
        match r.calculate(){Ok(())=>r.complete=true,Err(e)=>r.error=Some(e)}r
    }
}
