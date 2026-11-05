// SPDX-License-Identifier: MIT
use super::{StdWord32, StdNumericError, StdWord64, StdWideError};
use super::interval_math::{Ledger, MathError};

#[derive(Clone, Debug)]
pub(crate) enum AxisError { Domain, Capacity, Word32(StdNumericError), Word64(StdWideError), Math(MathError) }
impl From<StdNumericError> for AxisError { fn from(e: StdNumericError) -> Self { Self::Word32(e) } }
impl From<StdWideError> for AxisError { fn from(e: StdWideError) -> Self { Self::Word64(e) } }
impl From<MathError> for AxisError { fn from(e: MathError) -> Self { Self::Math(e) } }
#[derive(Clone, Debug)]
pub(crate) struct Edge {
    pub(crate) index: Option<u32>, pub(crate) source: &'static str, pub(crate) op: &'static str,
    pub(crate) width: u32, pub(crate) a: u64, pub(crate) b: Option<u64>, pub(crate) result: Option<u64>,
}
#[derive(Default, Debug)]
pub(crate) struct Axis {
    pub(crate) count: u32, pub(crate) nyquist: u32, pub(crate) warp: u32,
    pub(crate) wmin: Option<u32>, pub(crate) wmax: Option<u32>,
    pub(crate) edges: Vec<Edge>, pub(crate) math: Ledger, pub(crate) output: Vec<u32>,
    pub(crate) error: Option<AxisError>, pub(crate) complete: bool,
}
impl Axis {
    fn edge<F>(&mut self, i: Option<u32>, source: &'static str, op: &'static str, width: u32, a: u64, b: Option<u64>, f: F) -> Result<u64, AxisError>
    where F: FnOnce(&mut Ledger) -> Result<u64, AxisError> {
        if self.edges.len() >= 4096 { return Err(AxisError::Capacity); }
        let n = self.edges.len(); self.edges.push(Edge { index: i, source, op, width, a, b, result: None });
        let r = f(&mut self.math)?; self.edges[n].result = Some(r); Ok(r)
    }
    fn b32(&mut self, i: Option<u32>, s: &'static str, op: &'static str, a: u32, b: u32) -> Result<u32, AxisError> {
        self.edge(i,s,op,32,a as u64,Some(b as u64), |_| {
            let x=StdWord32::from_bits(a)?; let y=StdWord32::from_bits(b)?;
            let v=match op { "add"=>x.add(y)?,"sub"=>x.sub(y)?,"mul"=>x.mul(y)?,"div"=>x.div(y)?,_=>return Err(AxisError::Domain) };
            Ok(v.bits() as u64)
        }).map(|x| x as u32)
    }
    fn b64(&mut self, i: Option<u32>, s: &'static str, op: &'static str, a: u64, b: u64) -> Result<u64, AxisError> {
        self.edge(i,s,op,64,a,Some(b), |_| {
            let x=StdWord64::from_bits(a)?; let y=StdWord64::from_bits(b)?;
            Ok(match op { "add"=>x.add(y)?,"sub"=>x.sub(y)?,"mul"=>x.mul(y)?,"div"=>x.div(y)?,_=>return Err(AxisError::Domain) }.bits())
        })
    }
    fn widen(&mut self, i: Option<u32>, s: &'static str, a: u32) -> Result<u64, AxisError> {
        self.edge(i,s,"widen32to64",64,a as u64,None, |_| Ok(StdWord64::widen(StdWord32::from_bits(a)?)?.bits()))
    }
    fn narrow(&mut self, i: Option<u32>, s: &'static str, a: u64) -> Result<u32, AxisError> {
        self.edge(i,s,"narrow64to32",32,a,None, |_| Ok(StdWord64::from_bits(a)?.narrow()?.bits() as u64)).map(|x| x as u32)
    }
    fn integer(&mut self, i: Option<u32>, s: &'static str, a: u32) -> Result<u32, AxisError> {
        self.edge(i,s,"from_i32",32,a as u64,None, |_| Ok(StdWord32::from_i32(a as i32)?.bits() as u64)).map(|x| x as u32)
    }
    fn transcendental(&mut self, i: Option<u32>, s: &'static str, op: &'static str, a: u64) -> Result<u64, AxisError> {
        self.edge(i,s,op,64,a,None, |math| Ok(math.evaluate(op,a)?))
    }
    fn bound(&mut self, input: u32, stem: &'static str) -> Result<u32, AxisError> {
        let r=self.b32(None,stem,"div",input,self.warp)?;
        let w=self.widen(None,stem,r)?;
        let a=self.b64(None,stem,"add",0x3ff0_0000_0000_0000,w)?;
        let l=self.transcendental(None,stem,"ln",a)?;
        let m=self.b64(None,stem,"mul",0x40b3_8800_0000_0000,l)?;
        self.narrow(None,stem,m)
    }
    fn calculate(&mut self) -> Result<(), AxisError> {
        if !(4..=128).contains(&self.count) || !(0x45fa_0000..=0x473b_8000).contains(&self.nyquist) || !(0x45fa_0000..=0x473b_8000).contains(&self.warp) { return Err(AxisError::Domain); }
        let lo=self.bound(0,"dsputils.llsm_warp_frequency/wmin")?; self.wmin=Some(lo);
        let hi=self.bound(self.nyquist,"dsputils.llsm_warp_frequency/wmax")?; self.wmax=Some(hi);
        for i in 0..self.count {
            let at=Some(i); let s="dsputils.llsm_warp_frequency/freq[i]";
            let iw=self.integer(at,s,i)?;
            let nw=self.integer(at,s,self.count)?;
            let q=self.b32(at,s,"div",iw,nw)?;
            let range=self.b32(at,s,"sub",hi,lo)?;
            let p=self.b32(at,s,"mul",q,range)?;
            let a=self.b32(at,s,"add",p,lo)?;
            let aw=self.widen(at,s,a)?;
            let scaled=self.b64(at,s,"div",aw,0x40b3_8800_0000_0000)?;
            let e=self.transcendental(at,s,"exp",scaled)?;
            let minus=self.b64(at,s,"sub",e,0x3ff0_0000_0000_0000)?;
            let warp=self.widen(at,s,self.warp)?;
            let result=self.b64(at,s,"mul",warp,minus)?;
            let result=self.narrow(at,s,result)?;
            if self.output.len()>=128 { return Err(AxisError::Capacity); }
            self.output.push(result);
        }
        Ok(())
    }
    pub(crate) fn observe(count:u32,nyquist:u32,warp:u32)->Self {
        let mut r=Self{count,nyquist,warp,..Self::default()};
        match r.calculate(){Ok(())=>r.complete=true,Err(e)=>r.error=Some(e)} r
    }
}
