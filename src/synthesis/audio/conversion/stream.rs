//! Same sinc operands, phase and summation order as batch conversion. Hold real
//! right context; only EOF supplies zero boundary samples. No per-chunk resets.
use super::kernels::{self,RADIUS,PHASES};
use std::{collections::VecDeque,sync::Arc};
pub struct Mono {
    source:u32,target:u32,buffer:VecDeque<f32>,base:usize,received:usize,next:usize,
    kernels:Option<Arc<Vec<Vec<f64>>>>,
}
impl Mono {
    pub fn new(source:u32,target:u32)->Self {Self {source,target,buffer:VecDeque::new(),base:0,
        received:0,next:0,kernels:(source!=target).then(||kernels::get(source,target))}}
    pub fn push(&mut self,values:&[f32]) {self.buffer.extend(values);self.received+=values.len();}
    pub fn next_chunk(&mut self,finished:bool)->Option<(usize,Vec<f32>)> {
        let final_count=(self.received as u128*self.target as u128).div_ceil(self.source as u128) as usize;
        let first=self.next;let mut result=Vec::with_capacity(4096);
        while self.next<final_count && result.len()<4096 {
            if self.source==self.target {
                result.push(self.buffer[self.next-self.base]);
            } else {
                let position=self.next as f64*f64::from(self.source)/f64::from(self.target);
                let center=position.floor() as i64;
                if !finished && center+RADIUS>=self.received as i64 {break;}
                let phase=(position.fract()*PHASES as f64).round() as usize;
                let mut value=0.0;
                for (tap,weight) in (-RADIUS+1..=RADIUS).zip(&self.kernels.as_ref().unwrap()[phase]) {
                    let source=center+tap;
                    if source>=0 && (source as usize)<self.received {
                        value+=f64::from(self.buffer[source as usize-self.base])*weight;
                    }
                }
                result.push(value as f32);
            }
            self.next+=1;
        }
        let keep=if self.source==self.target {self.next}else{
            ((self.next as f64*f64::from(self.source)/f64::from(self.target)).floor() as usize).saturating_sub(RADIUS as usize)
        }.min(self.received);
        while self.base<keep {self.buffer.pop_front();self.base+=1;}
        (!result.is_empty()).then_some((first,result))
    }
}
