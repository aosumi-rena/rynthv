use super::super::{Error,Result};
use super::model::{Conv,Linear,Matrix,Node};
use std::sync::atomic::{AtomicBool,Ordering};

struct Wave {gate:Conv,skip:Conv,residual:Conv}
pub(super) struct Network {
    trunk:Vec<Conv>,wave:Vec<Wave>,feedback:Linear,input:Linear,
    feature_first:Linear,feature_last:Linear,policy:Linear,top:Linear,
    gru:Vec<Matrix>,bias:Vec<Vec<f32>>,
}
pub(super) struct Normal {state:u32,cache:Option<f32>}
impl Normal {
    pub fn new(seed:u32)->Self{Self{state:(seed%2147483647).max(1),cache:None}}
    fn uniform(&mut self)->f32{self.state=((16807u64*u64::from(self.state))%2147483647)as u32;(self.state as f32-1.0)/2147483648.0}
    fn draw(&mut self)->f32 {
        if let Some(x)=self.cache.take(){return x;}
        loop{let x=self.uniform()*2.0-1.0;let y=self.uniform()*2.0-1.0;let r=y*y+x*x;
            if r>=1.0||x==0.0||y==0.0{continue;}
            let factor=(-2.0*r.ln()/r).sqrt();self.cache=Some(y*factor);return x*factor;}
    }
}
fn checkpoint(cancel:&AtomicBool)->Result<()>{if cancel.load(Ordering::Relaxed){Err(Error::Cancelled)}else{Ok(())}}
fn reciprocal(x:f32)->f32 {
    #[cfg(target_arch="x86_64")]
    unsafe {use std::arch::x86_64::*;_mm_cvtss_f32(_mm_rcp_ss(_mm_set_ss(x)))}
    #[cfg(not(target_arch="x86_64"))]
    {1.0/x}
}
fn activation(x:f32,tanh:bool)->f32 {
    let(c2,c1,c0)=if tanh{(0x3e060807,0x3f198f1d,0x3f7bfbbd)}else{(0x3c060568,0x3e194af5,0x3efbfc65)};
    let mut p=f32::from_bits(c2).mul_add(x*x,f32::from_bits(c1));
    p=p.mul_add(x.abs(),f32::from_bits(c0));p=p.mul_add(x.abs(),1.0);
    let q=if if tanh{x>=0.0}else{x<0.0}{p}else{reciprocal(p)};let square=q*q;
    if tanh{(square-1.0)/(square+1.0)}else{reciprocal(square+1.0)}
}
fn activate(x:&mut[f32],tanh:bool){for x in x{*x=activation(*x,tanh);}}
impl Network {
    pub fn read(core:&Node)->Result<Self>{
        core.require("_f0g4v1",6)?;let t=&core.children[1];t.require("modm0",5)?;
        let trunk=[0,2,4].into_iter().map(|i|Conv::read(&t.children[i])).collect::<Result<Vec<_>>>()?;
        let w=&core.children[4];w.require("_ncwnv0",3)?;
        for group in &w.children{group.require("cmpg1",4)?;}
        let wave=(0..4).map(|i|{
            let gate=&w.children[0].children[i];gate.require("_gnc1v0",1)?;
            Ok(Wave{gate:Conv::read(&gate.children[0])?,skip:Conv::read(&w.children[1].children[i])?,residual:Conv::read(&w.children[2].children[i])?})
        }).collect::<Result<_>>()?;
        let r=&core.children[2];r.require("_wgv0",4)?;
        let p=&r.children[2];p.require("_fpolv0",2)?;p.children[0].require("modm0",3)?;
        let g=&r.children[3];g.require("modl3",12)?;
        let gru=(0..6).map(|i|Matrix::read(&g.children[2*i])).collect::<Result<_>>()?;
        let bias=(0..6).map(|i|g.children[2*i+1].vector()).collect::<Result<Vec<_>>>()?;
        if bias.iter().any(|b|b.len()!=256){return Err(Error::Format("F0 GRU state width"));}
        let gaussian=&p.children[1];gaussian.require("_gaolv0",1)?;
        core.children[5].require("_gaolv0",1)?;
        Ok(Self{trunk,wave,feedback:Linear::read(&r.children[0])?,input:Linear::read(&r.children[1])?,
            feature_first:Linear::read(&p.children[0].children[0])?,feature_last:Linear::read(&p.children[0].children[2])?,
            policy:Linear::read(&gaussian.children[0])?,top:Linear::read(&core.children[5].children[0])?,gru,bias})
    }
    pub fn forward(&self,rows:&[Vec<f32>],counts:&[usize],style:&[f32],normal:&mut Normal,cancel:&AtomicBool)->Result<Vec<f32>>{
        if rows.len()!=counts.len()||style.len()!=8||rows.iter().any(|r|r.len()!=6){return Err(Error::Format("F0 formatted input shape"));}
        let mut x:Vec<Vec<f32>>=rows.iter().map(|r|r.iter().chain(style).copied().collect()).collect();
        for(i,layer)in self.trunk.iter().enumerate(){checkpoint(cancel)?;x=layer.sequence(&x,cancel)?;if i<2{for row in &mut x{for v in row{*v=v.max(0.0);}}}}
        let total=counts.iter().sum::<usize>();if total>100_000{return Err(Error::Unsupported("F0 phrase exceeds 100000 frames"));}
        let mut expanded=Vec::with_capacity(total);let mut baseline=Vec::with_capacity(total);
        for(i,&count)in counts.iter().enumerate(){for j in 0..count{
            let t=if count==1{0.5}else{j as f32/(count-1)as f32};let q=1.0-t;
            let mut row=x[i].clone();row.extend([t,q,(q*q+q*q)+(t*t+t*t)-1.0]);expanded.push(row);baseline.push(rows[i][4]);
        }}
        let mut shared=vec![vec![0.0;64];total];x=expanded;
        for layer in &self.wave{
            checkpoint(cancel)?;let gate=layer.gate.sequence(&x,cancel)?;let mut next=Vec::with_capacity(total);
            for(i,(mut z,source))in gate.into_iter().zip(&x).enumerate(){
                if i%64==0{checkpoint(cancel)?;}
                if z.len()!=256{return Err(Error::Format("F0 wave gate width"));}
                let mut glu=vec![0.0;128];for c in 0..128{glu[c]=activation(z[c],true)*activation(z[c+128],false);}z.clear();
                let skip=layer.skip.point(&glu)?;if skip.len()!=64{return Err(Error::Format("F0 skip width"));}
                for(c,v)in skip.into_iter().enumerate(){shared[i][c]+=v;}
                let residual=layer.residual.point(source)?;if residual.len()!=128{return Err(Error::Format("F0 wave residual width"));}
                for(c,v)in residual.into_iter().enumerate(){glu[c]+=v;}next.push(glu);
            }x=next;
        }
        let mut hidden=vec![0.0;256];let mut feedback=0.0;let mut output=Vec::with_capacity(total);
        let smoothing=1.0-(0.5f32*0.5*std::f32::consts::PI*0.5).cos();
        for(i,row)in shared.iter().enumerate(){
            if i%32==0{checkpoint(cancel)?;}
            let mut drive=self.feedback.apply(&[feedback])?;self.input.matrix.accumulate(row,&mut drive,true)?;
            let mut r=(0..256).map(|c|self.bias[0][c]+self.bias[3][c]).collect::<Vec<_>>();
            self.gru[0].accumulate(&drive,&mut r,true)?;self.gru[3].accumulate(&hidden,&mut r,true)?;activate(&mut r,false);
            let mut z=(0..256).map(|c|self.bias[1][c]+self.bias[4][c]).collect::<Vec<_>>();
            self.gru[1].accumulate(&drive,&mut z,true)?;self.gru[4].accumulate(&hidden,&mut z,true)?;activate(&mut z,false);
            let mut candidate=self.bias[5].clone();self.gru[5].accumulate(&hidden,&mut candidate,true)?;
            for c in 0..256{candidate[c]*=r[c];}self.gru[2].accumulate(&drive,&mut candidate,true)?;
            for c in 0..256{candidate[c]+=self.bias[2][c];}activate(&mut candidate,true);
            for c in 0..256{hidden[c]=hidden[c]*z[c]+candidate[c]*(1.0-z[c]);}
            let mut features=self.feature_first.apply(&hidden)?;activate(&mut features,true);
            let reduced=self.feature_last.apply(&features)?;let pair=self.policy.apply(&reduced)?;
            if pair.len()!=2{return Err(Error::Format("F0 Gaussian head"));}
            let mut policy=normal.draw().clamp(-3.0,3.0)*pair[1].exp()+pair[0];
            for(width,alpha)in [(smoothing*0.25,0.1),(smoothing*0.5,0.2),(smoothing+smoothing,0.4)]{
                policy+=(-width-policy).max(0.0)*alpha;policy-=(policy-width).max(0.0)*alpha;
            }
            feedback=policy;let top=self.top.apply(row)?;output.push(baseline[i]+(top[0]+policy));
        }
        if output.iter().any(|v|!v.is_finite()){return Err(Error::Format("F0 nonfinite network output"));}Ok(output)
    }
}
