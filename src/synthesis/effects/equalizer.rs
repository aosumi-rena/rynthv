
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind { LowShelf, Peak }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub kind: Kind,
    pub frequency: f32,
    pub gain: f32,
    pub q: f32,
}

impl Parameters {
    pub fn quality(self) -> f32 {
        let root = (f64::from(self.gain.max(-60.0)) * 0.16609640474436813).exp2().sqrt();
        match self.kind {
            Kind::Peak => f64::from(self.q) * root,
            Kind::LowShelf => 1.0 / (((root + 1.0 / root) * (1.0 / f64::from(self.q) - 1.0) + 2.0).max(1e-5)).sqrt(),
        }.clamp(0.01, 100.0) as f32
    }

    pub fn coefficients(self, sample_rate: u32) -> [f32; 6] {
        let normalized = (self.frequency / (sample_rate as f32 * 0.5)).clamp(0.001, 0.999);
        let mut g = (std::f32::consts::FRAC_PI_2 * normalized).tan();
        let amplitude = if self.gain > -60.0 { (0.16609640419483185_f32 * self.gain).exp2() } else { 0.0 };
        let k = 1.0 / self.quality();
        let (m0, m1, m2) = match self.kind {
            Kind::Peak => (1.0, (amplitude - 1.0) * k, 0.0),
            Kind::LowShelf => {
                g /= amplitude.max(1e-5).sqrt().sqrt();
                let amplitude = amplitude.min(100.0);
                (1.0, (amplitude.sqrt() - 1.0) * k, amplitude - 1.0)
            }
        };
        let a1 = 1.0 / ((k + g) * g + 1.0);
        let a2 = a1 * g;
        [a1, a2, a2 * g, m0, m1, m2]
    }
}

pub struct Filter {
    parameters: Parameters,
    sample_rate: u32,
    previous: Option<[f32; 6]>,
    integrator1: [f32; 4],
    integrator2: [f32; 4],
}

impl Filter {
    pub fn new(parameters: Parameters, sample_rate: u32) -> Self {
        Self { parameters, sample_rate, previous: None, integrator1: [0.0; 4], integrator2: [0.0; 4] }
    }

    pub fn set_parameters(&mut self, parameters: Parameters) { self.parameters = parameters; }

    pub fn process(&mut self, frames: &mut [[f32; 4]]) {
        let target = self.parameters.coefficients(self.sample_rate);
        let previous = self.previous.replace(target).unwrap_or(target);
        let reciprocal = 1.0 / frames.len().max(1) as f32;
        for (index, frame) in frames.iter_mut().enumerate() {
            let position = (index + 1) as f32;
            let c: [f32; 6] = std::array::from_fn(|i| ((target[i] - previous[i]) * position) * reciprocal + previous[i]);
            for (lane, sample) in frame.iter_mut().enumerate() {
                let input = *sample;
                let v3 = input - self.integrator2[lane];
                let v1 = self.integrator1[lane] * c[0] + v3 * c[1];
                let v2 = (self.integrator1[lane] * c[1] + self.integrator2[lane]) + v3 * c[2];
                self.integrator1[lane] = v1 + v1 - self.integrator1[lane];
                self.integrator2[lane] = v2 + v2 - self.integrator2[lane];
                *sample = (v1 * c[4] + input * c[3]) + v2 * c[5];
            }
        }
    }
}
