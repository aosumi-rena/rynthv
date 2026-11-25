use super::model::Model;
use rustfft::{FftPlanner,num_complex::Complex};

impl Model {
    pub fn split(&self,direction:usize,half:usize)->Result<[Vec<f32>;2],String> {
        if half>128 { return Err("Invalid Hall source filter extent".into()); }
        let gain=self.p.values(0x315,&[4])?[direction];
        let cutoff=self.p.values(0x316,&[4])?[direction];
        let order=self.p.values(0x317,&[])?[0];
        let offset=self.p.values(0x505,&[])?[0];
        if !(0. ..=1.).contains(&gain) || cutoff<=0. || order<=0. { return Err("Invalid Hall source separation".into()); }
        let size=65536;let mut planner=FftPlanner::<f64>::new();let inverse=planner.plan_fft_inverse(size);
        let mut output=[Vec::new(),Vec::new()];
        for (common,row) in output.iter_mut().enumerate() {
            let mut spectrum=vec![Complex::new(0.,0.);size];
            for i in 0..=size/2 {
                let frequency=(i as f64*self.rate)*(1./65536.);
                let power=gain/(1.+(frequency/cutoff).powf(order));
                let amplitude=if common==1 {power.sqrt()} else {(1.-power).sqrt()};
                let phase=(((i as f64*self.rate)*(1./65536.))*(-std::f64::consts::TAU))/self.rate;
                let phase=(128.+offset)*phase;
                let value=Complex::new(phase.cos()*amplitude,phase.sin()*amplitude);
                spectrum[i]=value;
                if i>0 && i<size/2 { spectrum[size-i]=value.conj(); }
            }
            inverse.process(&mut spectrum);
            *row=spectrum[128-half..=128+half].iter().map(|v|(v.re/65536.) as f32).collect();
        }
        Ok(output)
    }
}
