#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters { pub attack: f32, pub ratio: f32, pub threshold: f32 }

pub struct Compressor {
    parameters: Parameters,
    sample_rate: u32,
    envelope: f64,
    detector_decay: f64,
    gain: Option<f32>,
}

impl Compressor {
    pub fn new(parameters: Parameters, sample_rate: u32) -> Self {
        Self { parameters, sample_rate, envelope: 0.0,
            detector_decay: (-1.0 / (f64::from(sample_rate) * 0.05)).exp().min(0.999999), gain: None }
    }

    pub fn set_parameters(&mut self, parameters: Parameters) { self.parameters = parameters; }
    pub fn set_threshold(&mut self, threshold: f32) { self.parameters.threshold = threshold; }
    pub fn input_level(&self) -> f32 { (self.envelope.max(1e-5).log10() * 10.0) as f32 }

    pub fn process(&mut self, frames: &mut [[f32; 4]]) {
        let attack = (-1.0 / (self.sample_rate as f32 * self.parameters.attack.max(0.005))).exp();
        let release = (-1.0 / (self.sample_rate as f32 * 0.005)).exp();
        let slope = 1.0 - 1.0 / self.parameters.ratio.max(1.0);
        for frame in frames {
            let energy = frame.iter().fold(0.0_f32, |peak, &x| peak.max(x * x));
            self.envelope = (self.envelope - f64::from(energy)) * self.detector_decay + f64::from(energy);
            let over = (self.envelope.max(1e-8).log10() * 10.0) as f32 - self.parameters.threshold;
            let reduction = if over > 0.0 { over * slope } else { 0.0 };
            let target = (reduction * -0.16609640419483185_f32).exp2();
            let gain = self.gain.map_or(target, |previous| {
                let decay = if target >= previous { release } else { attack };
                (previous - target) * decay + target
            });
            self.gain = Some(gain);
            for x in frame { *x = gain * *x; }
        }
    }
}
