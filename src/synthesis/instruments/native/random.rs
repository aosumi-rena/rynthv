use super::{Result, fail, tensor::Tensor};
pub const M: u64 = 0x5851f42d4c957f2d;
pub fn pcg(s: u64) -> u32 {
    (((s >> 45) ^ (s >> 27)) as u32).rotate_right((s >> 59) as u32)
}
fn advance(mut d: usize) -> (u64, u64) {
    let (mut m, mut i, mut cm, mut ci) = (1u64, 0u64, M, 1u64);
    while d != 0 {
        if d & 1 != 0 {
            m = m.wrapping_mul(cm);
            i = i.wrapping_mul(cm).wrapping_add(ci);
        }
        ci = ci.wrapping_mul(cm.wrapping_add(1));
        cm = cm.wrapping_mul(cm);
        d >>= 1;
    }
    (m, i)
}
pub struct Normal {
    pub state: u64,
    pub inc: u64,
    cached: Option<f32>,
    short: bool,
}
impl Normal {
    pub fn new(state: u64, inc: u64, short: bool) -> Self {
        Self {
            state,
            inc,
            cached: None,
            short,
        }
    }
    fn uniform(&mut self) -> f32 {
        let word = pcg(self.state);
        self.state = self.state.wrapping_mul(M).wrapping_add(self.inc);
        if self.short {
            ((word >> 8) as f32 * (1. / 16777216.)) * 2. - 1.
        } else {
            (word as f32 * (1. / 4294967296.)) * 2. - 1.
        }
    }
    pub fn next(&mut self) -> f32 {
        if let Some(v) = self.cached.take() {
            return v;
        }
        loop {
            let a = self.uniform();
            let b = self.uniform();
            if a == 0. || b == 0. {
                continue;
            }
            let r = a * a + b * b;
            if r >= 1. {
                continue;
            }
            let f = if r > 0.0001 {
                (-2. * r.ln() / r).sqrt()
            } else {
                let q = (a as f64).powi(2) + (b as f64).powi(2);
                (-2. * q.ln() / q).sqrt() as f32
            };
            self.cached = Some(b * f);
            return a * f;
        }
    }
}
pub fn normals(
    seeds: &Tensor,
    channels: usize,
    mean: f32,
    sigma: f32,
    reps: usize,
) -> Result<Tensor> {
    if seeds.shape.len() != 3 || ![1, 2, 4].contains(&seeds.shape[2]) || reps == 0 {
        return fail("Instrument noise seed shape");
    }
    let (b, t, w) = (seeds.shape[0], seeds.shape[1], seeds.shape[2]);
    let mut data = Vec::with_capacity(b * t * reps * channels);
    let (jm, ji) = advance(channels);
    for row in 0..b * t {
        let words: Vec<u32> = (0..w).map(|i| seeds.get(row * w + i).to_bits()).collect();
        let seed = words[0] as u64 | if w > 1 { (words[1] as u64) << 32 } else { 0 };
        let inc = if w == 4 {
            words[2] as u64 | ((words[3] as u64) << 32) | 1
        } else {
            0xda3e39cb94b95bdb
        };
        let mut rs = seed.wrapping_mul(M).wrapping_add(0x08f5dc87e5c07d8a);
        for _ in 0..reps {
            let mut rng = Normal::new(rs, inc, false);
            for _ in 0..channels {
                data.push((rng.next() * sigma) + mean);
            }
            rs = rs.wrapping_mul(jm).wrapping_add(inc.wrapping_mul(ji));
        }
    }
    Ok(Tensor::new(vec![b, t * reps, channels], data))
}
pub fn ambient(seed: u32, count: usize) -> Vec<f32> {
    let mut rng = Normal::new(seed as u64, 0xda3e39cb94b95bdb, true);
    let mut w = 0f32;
    (0..count)
        .map(|_| {
            w += rng.next();
            if w.abs() > 5. {
                w = 0.;
            }
            w + 40.
        })
        .collect()
}
