use crate::synthesis::instruments::rooms::flxr::Program;
use rustfft::num_complex::Complex;
use std::f64::consts::{PI,TAU,SQRT_2};

pub(super) type Sos=[f64;6];
pub(super) struct Equalizer {
    q:f64,
    centers:Vec<f64>,
    ends:[f64;2],
    frequencies:Vec<f64>,
    correction:Vec<f64>,
    iterations:usize,
    rate:f64,
}
impl Equalizer {
    pub fn source_prefix(&self)->usize {1+self.centers.iter().filter(|v|**v<=1000.).count()}
    pub fn read(p:&Program,base:u32,width:usize,rate:f64)->Result<Self,String> {
        let q=p.values(base+1,&[])?[0];
        let centers=p.values(base+2,&[width-2])?;
        let ends=p.values(base+3,&[2])?.try_into().unwrap();
        let frequencies=p.values(base+4,&[width])?;
        let correction=p.values(base+5,&[width,width])?;
        let iterations=usize::try_from(p.integers(base+6,&[])?[0]).map_err(|_|"Invalid Hall equalizer iterations")?;
        if q<=0. || iterations>16 || centers.iter().chain(&ends).any(|f|*f<=0.||*f>=rate/2.) {
            return Err("Invalid Hall equalizer specification".into());
        }
        Ok(Self{q,centers,ends,frequencies,correction,iterations,rate})
    }
    fn adjust(&self,input:&[f64],output:&mut[f64]) {
        for (row,value) in self.correction.chunks_exact(input.len()).zip(output) {
            *value+=row.iter().zip(input).fold(0.,|sum,(&a,&b)|sum+a*b);
        }
    }
    pub fn design(&self,target:&[f64])->Result<Vec<Sos>,String> {
        if target.len()!=self.frequencies.len() || target.iter().any(|v|!v.is_finite()) {
            return Err("Invalid Hall equalizer controls".into());
        }
        let mut values=vec![0.;target.len()];self.adjust(target,&mut values);
        for _ in 0..self.iterations {
            let sections=self.sections(&values);
            let residual:Vec<_>=target.iter().zip(&self.frequencies).map(|(&goal,&frequency)| {
                let phase=frequency*(-TAU)/self.rate;let c=phase.cos();let s=phase.sin();
                let mut response=Complex::new(1.,0.);
                for section in &sections {
                    let poly=|row:&[f64]| Complex::new(
                        ((row[2]*c)*c-(row[2]*s)*s)+(row[1]*c+row[0]),
                        row[1]*s+((row[2]*c)*s+(row[2]*s)*c));
                    response*=poly(&section[..3])/poly(&section[3..]);
                }
                goal-20.*response.norm().log10()
            }).collect();
            self.adjust(&residual,&mut values);
        }
        let sections=self.sections(&values);
        if sections.iter().flatten().any(|v|!v.is_finite()) { return Err("Nonfinite Hall equalizer".into()); }
        Ok(sections)
    }
    fn sections(&self,values:&[f64])->Vec<Sos> {
        let mut result=Vec::with_capacity(values.len());
        let g=10f64.powf(values[0]/20.);let a=g.sqrt();let t=(self.ends[0]*PI/self.rate).tan();
        let gamma=(SQRT_2*t)*g.powf(0.25);let t2=t*t;
        result.push([a*((a*t2+gamma)+1.),a*((2.*a)*t2-2.),a*((a*t2-gamma)+1.),
            (a+gamma)+t2,2.*t*t-2.*a,(a-gamma)+t2]);
        for (&gain,&frequency) in values[1..values.len()-1].iter().zip(&self.centers) {
            let g=10f64.powf(gain/20.);let a=g.sqrt();let theta=frequency*TAU/self.rate;
            let beta=(theta/self.q*0.5).tan();let b=-2.*a*theta.cos();
            result.push([a+g*beta,b,a-g*beta,a+beta,b,a-beta]);
        }
        let g=10f64.powf(values[values.len()-1]/20.);let a=g.sqrt();let t=(self.ends[1]*PI/self.rate).tan();
        let gamma=(t*SQRT_2)*g.powf(0.25);let t2=t*t;
        result.push([g*((a+gamma)+t2),g*(2.*t*t-2.*a),g*((a-gamma)+t2),
            a*((a*t2+gamma)+1.),a*((2.*a)*t2-2.),a*((a*t2-gamma)+1.)]);
        result
    }
}
