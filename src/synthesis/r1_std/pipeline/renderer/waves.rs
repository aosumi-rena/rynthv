use super::super::{data::{Config,Frame},ensure,Result};
pub(super) struct Waves {parts:Vec<(i64,Vec<f32>)>}
impl Waves {
    pub fn new(c:&Config,frames:&[Frame])->Result<Self> {
        let mut parts=Vec::new();let mut copied=0;
        for (i,frame) in frames.iter().enumerate() {if let Some(wave)=&frame.wave {
            let rate=c.nyquist*2.0;
            ensure(wave.rate.is_finite()&&wave.rate>0.0,"Invalid R1 stored-wave sample rate")?;
            let values=super::super::wave::resample(&wave.values,rate/wave.rate)?;
            copied+=values.len();ensure(copied<=16777216,"R1 stored-wave plan exceeds capacity")?;
            let start=(i as f32*(c.hop*rate)).round() as i64+(wave.offset*rate).round() as i64;
            parts.push((start,values));
        }}
        Ok(Self {parts})
    }
    pub fn mix(&self,periodic:&[f32],noise:&[f32],start:usize)->Vec<f32> {
        let mut adjusted=noise.to_vec();
        for (origin,values) in &self.parts {
            let begin=(start as i64-*origin).max(0) as usize;
            let end=((start+noise.len()) as i64-*origin).max(0).min(values.len() as i64) as usize;
            for j in begin..end {
                let mut weight=if j<64 {j as f32*0.015625}else{1.0};
                if j as isize>values.len() as isize-64 {weight=(values.len()-j) as f32*0.015625;}
                let at=(*origin+j as i64-start as i64) as usize;
                adjusted[at]=((values[j]*weight) as f64+adjusted[at] as f64*(1.0-weight as f64)) as f32;
            }
        }
        periodic.iter().zip(adjusted).map(|(p,n)|*p+n).collect()
    }
}
