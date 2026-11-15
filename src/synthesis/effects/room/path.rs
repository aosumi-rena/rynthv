const RING:usize=1455;
const AIR:[f32;4]=[-0.001,-0.005,-0.03,-0.231];

pub(super) struct Path {
    ring:Vec<[f32;4]>,cursor:usize,delay:usize,gain:[f32;4],direction:[f32;3],
}
impl Path {
    pub fn new(position:[f32;3],db:[f32;4],invert:bool)->Self {
        let x=position[0]-(-1.0);let y=position[1];let z=position[2]-2.0;
        let distance=((x*x+y*y)+z*z).sqrt().max(1e-5);
        let inverse=1.0/distance;
        let direction=[y*inverse,z*inverse,x*inverse];
        let seconds=((distance as f64*0.0029154518950437317)as f32).min(30.0);
        let delay=((seconds as f64*48000.0).round()as usize).min(RING-1);
        let attenuation=distance.log10()*(-20.0);
        let gain=std::array::from_fn(|i|{
            let value=((attenuation+db[i])+(distance+(-1.0))*AIR[i])*0.16609640419483185_f32;
            let gain=value.exp2();let gain=if gain<0.001 {0.0}else{gain};
            if invert {-gain}else{gain}
        });
        Self {ring:vec![[0.0;4];RING],cursor:0,delay,gain,direction}
    }
    pub fn add(&mut self,input:[f32;4],output:&mut [f32;4]) {
        self.ring[self.cursor]=input;
        let at=(self.cursor+RING-self.delay)%RING;
        let sample=self.ring[at];self.cursor=(self.cursor+1)%RING;
        for (i,value) in sample.into_iter().enumerate() {
            let value=value*self.gain[i];output[0]+=value;
            for c in 0..3 {output[c+1]=value*self.direction[c]+output[c+1];}
        }
    }
}
