const DELAYS: [usize; 8] = [6373, 8927, 4619, 11019, 7287, 7825, 5354, 9642];
const DIFFUSERS: [usize; 8] = [977, 1172, 1517, 1312, 1099, 1406, 646, 918];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub pre_delay: f32,
    pub decay: f32,
    pub dry_wet: f32,
    pub damping_frequency: f32,
    pub directional_gain: f32,
}

struct Delay { values: Vec<f32>, cursor: usize }
impl Delay {
    fn new(length: usize) -> Self { Self { values: vec![0.0; length], cursor: 0 } }
    fn read(&self) -> f32 { self.values[self.cursor] }
    fn write(&mut self, value: f32) {
        self.values[self.cursor] = value;
        self.cursor += 1;
        if self.cursor == self.values.len() { self.cursor = 0; }
    }
}

pub struct Reverb {
    parameters: Parameters,
    delays: [Delay; 8],
    diffusers: [Delay; 8],
    low_state: [f32; 8],
    high_state: [f32; 8],
    coefficients: [[f32; 4]; 8],
    pre_delay: Vec<[f32; 2]>,
    pre_cursor: usize,
    delay_position: Option<f64>,
    gains: Option<[f32; 3]>,
}

impl Reverb {
    pub fn new(parameters: Parameters) -> Self {
        let mut result = Self {
            parameters, delays: DELAYS.map(Delay::new), diffusers: DIFFUSERS.map(Delay::new),
            low_state: [0.0; 8], high_state: [0.0; 8], coefficients: [[0.0; 4]; 8],
            pre_delay: vec![[0.0; 2]; 4801], pre_cursor: 0, delay_position: None, gains: None,
        };
        result.update_coefficients();
        result
    }

    pub fn set_parameters(&mut self, parameters: Parameters) {
        if parameters != self.parameters { self.parameters = parameters; self.update_coefficients(); }
    }

    fn update_coefficients(&mut self) {
        let p = self.parameters;
        let decay = f64::from(p.decay.max(0.001));
        let half_decay = f64::from(0.5 * p.decay.max(0.001));
        let cosine = if p.damping_frequency >= 23520.0 { 4.0 }
            else { f64::from(1.0 - (p.damping_frequency * 0.00013089970161672682_f32).cos()) * 2.0 };
        for (index, c) in self.coefficients.iter_mut().enumerate() {
            let exponent = DELAYS[index] as f64 * -0.00020762050593046015;
            let base = (exponent * (1.0 / decay)).exp2();
            let low = (exponent * (1.0 / decay)).exp2() * (1.0 / base) - 1.0;
            let high = (exponent * (1.0 / half_decay)).exp2() * (1.0 / base);
            let high2 = high * high;
            let t = (1.0 - high2) / (high2 * cosine);
            *c = [200.0 * 0.00013089970161672682_f32,
                (((t * 4.0 + 1.0).sqrt() - 1.0) / (t + t)) as f32, low as f32, base as f32];
        }
    }

    fn target_gains(&self) -> [f32; 3] {
        let gain_db = (self.parameters.dry_wet.clamp(0.0, 1.0).powf(0.1292831003665924) - 1.0) * 70.0;
        let amplitude = f64::from((gain_db * 0.16609640419483185_f32).exp2());
        let dry = ((1.0 - amplitude) * (amplitude + 1.0)) as f32;
        let normalization = 1.0 / f64::from(self.parameters.decay.max(0.001)).sqrt();
        let directional = (f64::from(self.parameters.directional_gain) * 0.16609640474436813).exp2() * normalization;
        [dry, normalization as f32, directional as f32]
    }

    pub fn process(&mut self, frames: &mut [[f32; 4]]) {
        let target = self.target_gains();
        let mut gains = self.gains.replace(target).unwrap_or(target);
        let step: [f32; 3] = std::array::from_fn(|i| ((f64::from(target[i]) - f64::from(gains[i])) / frames.len().max(1) as f64) as f32);
        let target_delay = (f64::from(self.parameters.pre_delay.clamp(0.0, 0.1)) * 48000.0).round().min(4800.0);
        for frame in frames {
            let current = self.delay_position.unwrap_or(target_delay);
            let difference = target_delay - current;
            let delay = if difference.abs() <= 0.01 { target_delay }
                else { current + (0.01 / difference.abs()).max(1e-5) * difference };
            self.delay_position = Some(delay);
            self.pre_delay[self.pre_cursor] = [frame[0], frame[1]];
            let read = (self.pre_cursor as f64 - delay + if (self.pre_cursor as f64) < delay { 4801.0 } else { 0.0 }).clamp(0.0, 4800.0);
            let first = read.floor() as usize;
            let fraction = (read - first as f64) as f32;
            let next = if first == 4800 { 0 } else { first + 1 };
            let delayed: [f32; 2] = std::array::from_fn(|i| {
                let a = self.pre_delay[first][i]; let b = self.pre_delay[next][i];
                ((b - a) * fraction + a) * 0.3
            });
            self.pre_cursor = (self.pre_cursor + 1) % 4801;
            let mut values = [0.0; 8];
            for (i, value) in values.iter_mut().enumerate() {
                let source = delayed[i / 4];
                let old = self.diffusers[i].read();
                let tail = self.delays[i].read();
                let coefficient = if i % 2 == 0 { -0.6 } else { 0.6 };
                let next = if i == 0 { (old * coefficient + tail) + source }
                    else { (if i % 4 < 2 { tail + source } else { tail - source }) + old * coefficient };
                self.diffusers[i].write(next);
                *value = if i % 2 == 0 { old + next * 0.6 } else { old - next * 0.6 };
            }
            let [a,b,c,d,e,f,g,h] = values;
            let p = [e+f,a+b]; let q = [e-f,a-b];
            let r = [g+h,c+d]; let s = [g-h,c-d];
            let u = [p[0]-r[0],p[1]-r[1]]; let v = [q[0]-s[0],q[1]-s[1]];
            let w = [q[0]+s[0],q[1]+s[1]]; let z = [r[0]+p[0],r[1]+p[1]];
            let mixed = [z[0]+z[1],w[1]+w[0],u[1]+u[0],v[1]+v[0],z[1]-z[0],w[1]-w[0],u[1]-u[0],v[1]-v[0]];
            for i in 0..3 { gains[i] += step[i]; }
            let wet = (gains[0] + 1.0) * (1.0 - gains[0]);
            let spatial = [mixed[0],mixed[1],mixed[4],mixed[2]];
            for i in 0..4 {
                let gain = wet * gains[if i == 0 { 1 } else { 2 }];
                frame[i] = gains[0] * frame[i] + spatial[i] * gain;
            }
            for (i, value) in mixed.into_iter().enumerate() {
                let value = value * 0.3535533845424652_f32;
                let c = self.coefficients[i];
                self.low_state[i] = (value - self.low_state[i]) * c[0] + self.low_state[i];
                let low = self.low_state[i] * c[2] + value;
                self.high_state[i] = (low - self.high_state[i]) * c[1] + self.high_state[i];
                self.delays[i].write(self.high_state[i] * c[3]);
            }
        }
    }
}
