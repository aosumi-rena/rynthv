use super::{Configuration,Processor,floating_point,room::Room};

pub(crate) fn encode_center(value:f32)->[f32;4] {
    #[cfg(target_arch="x86_64")]
    let inverse=unsafe {
        use std::arch::x86_64::{_mm_set_ss,_mm_cvtss_f32,_mm_rsqrt_ss};
        _mm_cvtss_f32(_mm_rsqrt_ss(_mm_set_ss(1.0)))
    };
    #[cfg(not(target_arch="x86_64"))]
    let inverse=1.0_f32;
    let factor=(inverse * -0.5)*(inverse*inverse-3.0);
    [value,0.0,value*factor,0.0]
}

pub struct VoiceProcessor {
    room: Option<(Room,Room)>,
    primary: Processor,
    aspiration: Processor,
    threshold: Option<f32>,
}
impl VoiceProcessor {
    pub fn new(configuration: &Configuration) -> Self {
        let mut aspiration=configuration.clone();aspiration.reverb=None;
        Self {room:configuration.room.map(|p|(Room::new(p,true),Room::new(p,false))),
            primary:Processor::new(configuration),aspiration:Processor::new(&aspiration),
            threshold:configuration.compressor.map(|p|p.threshold)}
    }
    pub fn process_encoded(&mut self, primary:&mut [[f32;4]], aspiration:&mut [[f32;4]]) {
        assert_eq!(primary.len(),aspiration.len());
        let _environment=floating_point::Guard::enter();
        if let Some((room,air))=&mut self.room {
            room.process(primary);air.process(aspiration);
        }
        if let Some(threshold)=self.threshold {
            let difference=self.aspiration.input_level()-self.primary.input_level();
            self.primary.set_threshold(threshold);
            self.aspiration.set_threshold(difference+threshold);
        }
        self.primary.process(primary);
        self.aspiration.process(aspiration);
    }

    pub fn decode(&self,primary:[f32;4],aspiration:[f32;4])->[f32;2] {
        let decode=|v:[f32;4]| {
            let z=if self.room.is_some(){v[3]*0.5}else{v[3]*0.0};
            let second=z+v[2]*0.0;
            [second+(v[1]*(-1.0)+v[0]),second+(v[1]+v[0])]
        };
        let p=decode(primary);let a=decode(aspiration);[p[0]+a[0],p[1]+a[1]]
    }
}
