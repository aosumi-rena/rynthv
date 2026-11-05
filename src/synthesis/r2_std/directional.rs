// SPDX-License-Identifier: MIT
use super::{
    Error, Result, StdBank,
    archive::Reader,
    directional_model::{Model, finite, floats, relu},
};
use std::collections::{HashMap, VecDeque};

pub(super) struct Directions {
    pub forward: Network,
    pub backward: Network,
}
impl Directions {
    pub fn open(bank: &StdBank) -> Result<Option<Self>> {
        if bank.profile.encoder_only {
            return Ok(None);
        }
        let names = bank.configuration.strings("unitsynth")?;
        let mut groups = [HashMap::new(), HashMap::new()];
        for row in bank.configuration.string_rows("unitsynth_embd") {
            if row.len() < 3 || row[0].is_empty() {
                return Err(Error::Format("directional embedding row"));
            }
            if groups[0].contains_key(&row[0]) {
                continue;
            }
            for direction in 0..2 {
                let mut r = Reader::new(bank.archive.named(&row[direction + 1])?);
                let embedding = floats(&mut r, 32)?;
                r.end()?;
                groups[direction].insert(row[0].clone(), embedding);
            }
        }
        if groups[0].is_empty() {
            return Err(Error::Format("directional embedding groups absent"));
        }
        let [forward_groups, backward_groups] = groups;
        Ok(Some(Self {
            forward: Network::new(Model::read(bank.archive.named(&names[0])?)?, forward_groups),
            backward: Network::new(
                Model::read(bank.archive.named(&names[1])?)?,
                backward_groups,
            ),
        }))
    }
}
pub(super) struct Network {
    model: Model,
    groups: HashMap<Vec<u8>, Vec<f32>>,
    ar: [f32; 70],
    raw_history: VecDeque<[f32; 2]>,
    faulted: bool,
}
impl Network {
    fn new(model: Model, groups: HashMap<Vec<u8>, Vec<f32>>) -> Self {
        Self {
            model,
            groups,
            ar: [0.0; 70],
            raw_history: VecDeque::new(),
            faulted: false,
        }
    }
    pub fn select_group(&mut self, group: &[u8]) {
        if let Some(value) = self.groups.get(group) {
            self.model.embedding.clone_from(value);
        }
    }
    pub fn push(
        &mut self,
        raw: &[f32; 177],
        initialize: Option<&[f32; 72]>,
        rng: &mut u32,
    ) -> Result<[f32; 72]> {
        if self.faulted {
            return Err(Error::Format("directional owner faulted"));
        }
        self.faulted = true;
        let result = self.push_inner(raw, initialize, rng);
        if result.is_ok() {
            self.faulted = false;
        }
        result
    }
    fn push_inner(
        &mut self,
        raw: &[f32; 177],
        initialize: Option<&[f32; 72]>,
        rng: &mut u32,
    ) -> Result<[f32; 72]> {
        finite(raw)?;
        if let Some(x) = initialize {
            finite(x)?;
            for j in 0..70 {
                let delta = x[j + 2] - self.model.lower[j + 2];
                self.ar[j] = (delta + delta) / self.model.range[j + 2] - 1.0;
            }
            finite(&self.ar)?;
        }
        self.raw_history.push_back([raw[175], raw[176]]);
        if self.raw_history.len() > 5 {
            self.raw_history.pop_front();
        }
        let mut x = raw.to_vec();
        for j in 0..2 {
            let delta = x[175 + j] - self.model.lower[j];
            x[175 + j] = (delta + delta) / self.model.range[j] - 1.0;
        }
        x[176] = x[176].clamp(-0.95, 0.95);
        for stem in &mut self.model.stems {
            x = stem.push(x)?;
        }
        x.extend_from_slice(&self.model.embedding);
        let condition = x;
        let mut skip = self.model.seed.affine(&condition);
        let mut h = self.model.work.push(&self.ar);
        finite(&condition)?;
        finite(&skip)?;
        finite(&h)?;
        for block in &mut self.model.blocks {
            let mut pair = block.pair.push(&h);
            let projected = block.condition.affine(&condition);
            finite(&pair)?;
            finite(&projected)?;
            for (a, b) in pair.iter_mut().zip(projected) {
                *a += b;
            }
            finite(&pair)?;
            let gated: Vec<_> = (0..192)
                .map(|j| pair[j].tanh() / ((-pair[192 + j]).exp() + 1.0))
                .collect();
            let sp = block.skip.affine(&gated);
            let rp = block.residual.affine(&gated);
            for (a, b) in skip.iter_mut().zip(sp) {
                *a += b;
            }
            for (a, b) in h.iter_mut().zip(rp) {
                *a += b;
            }
            finite(&h)?;
            finite(&skip)?;
        }
        for head in &self.model.heads {
            finite(&skip)?;
            relu(&mut skip);
            skip = head.affine(&skip);
        }
        finite(&skip)?;
        let mut bands = [0.0; 280];
        for band in 0..4 {
            for lane in 0..70 {
                let i = band * 70 + lane;
                let exp = if lane < 68 {
                    packed_exp(-skip[i])
                } else {
                    (-skip[i]).exp()
                };
                let denominator = 1.0 + exp;
                bands[i] = if band == 1 {
                    let inner = 4.0 / denominator;
                    f32::from_bits(0x3c008081)
                        * if lane < 68 {
                            packed_exp(inner)
                        } else {
                            inner.exp()
                        }
                } else {
                    2.0 / denominator - if band == 3 { 0.0 } else { 1.0 }
                };
            }
        }
        self.ar = sample(&bands, rng)?;
        let mut physical = [0.0; 72];
        physical[..2].copy_from_slice(&self.raw_history[0]);
        for j in 0..70 {
            physical[j + 2] =
                self.model.range[j + 2] * ((self.ar[j] + 1.0) * 0.5) + self.model.lower[j + 2];
        }
        finite(&physical)?;
        Ok(physical)
    }
}
fn sample(b: &[f32; 280], rng: &mut u32) -> Result<[f32; 70]> {
    finite(b)?;
    let mut params = Vec::with_capacity(70);
    for j in 0..70 {
        let (b0, b1, b2, b3) = (b[j], b[70 + j], b[140 + j], b[210 + j]);
        let mut mixture = [0.0; 4];
        let mut mu = [0.0; 4];
        let mut sigma = [0.0; 4];
        let mut total = 0.0;
        for k in 0..4 {
            mixture[k] = ((f64::from(b3) * (4.0 / 7.0)).powi(k as i32)
                * f64::from(b2).powi(2 * k as i32)) as f32;
            total += mixture[k];
            mu[k] = (((f64::from(b2.abs()) * 1.1 - 1.0) * k as f64).exp() * f64::from(b1)) as f32;
            sigma[k] = (f64::from(k as f32 * mu[k]) * 1.6 * f64::from(b2) + f64::from(b0)) as f32;
        }
        if !total.is_finite() || total <= 0.0 {
            return Err(Error::Format("directional mixture mass"));
        }
        let inverse = 1.0 / total;
        for value in &mut mixture {
            *value *= inverse;
        }
        let mean = (mixture[3] * sigma[3] + mixture[2] * sigma[2])
            + (mixture[1] * sigma[1] + mixture[0] * sigma[0]);
        finite(&mu)?;
        finite(&sigma)?;
        params.push((mixture, mu, sigma, mean));
    }
    let mut ar = [0.0; 70];
    for (j, (mixture, mu, sigma, mean)) in params.into_iter().enumerate() {
        let u = uniform(rng);
        let mut cumulative = mixture[0];
        let selected = if cumulative > u {
            0
        } else {
            cumulative += mixture[1];
            if cumulative > u {
                1
            } else {
                cumulative += mixture[2];
                if cumulative > u { 2 } else { 3 }
            }
        };
        let scale = f32::from_bits(if j < 6 {
            0x3d4ccccd
        } else if j < 10 {
            0x3e4ccccd
        } else if j < 20 {
            0x3e99999a
        } else if j < 40 {
            0x3ecccccd
        } else if j < 65 {
            0x3f333333
        } else {
            0x3c23d70a
        });
        let term = (mean - sigma[selected]) * scale + sigma[selected];
        let amplitude = mu[selected] * scale;
        let radius = uniform(rng).max(f32::from_bits(0x3a83126f));
        let angle = uniform(rng);
        let gaussian = ((f64::from(radius.ln() * -2.0)).sqrt()
            * (f64::from(angle) * std::f64::consts::TAU).cos()) as f32;
        ar[j] = gaussian * amplitude + term;
    }
    finite(&ar)?;
    Ok(ar)
}
fn uniform(rng: &mut u32) -> f32 {
    *rng = rng.wrapping_mul(214013).wrapping_add(2531011);
    ((*rng >> 16) & 32767) as f32 * f32::from_bits(0x38000100)
}
pub(super) fn packed_exp(x: f32) -> f32 {
    let clipped = x.clamp(f32::from_bits(0xc2b0c0a5), f32::from_bits(0x42b0c0a6));
    let fx = clipped * f32::from_bits(0x3fb8aa3b) + 0.5;
    let n = fx.floor();
    let g = (clipped - f32::from_bits(0x3f318000) * n) - f32::from_bits(0xb95e8083) * n;
    let mut p = f32::from_bits(0x39506967);
    for bits in [0x3ab743ce, 0x3c088908, 0x3d2aa9c1, 0x3e2aaaaa, 0x3f000000] {
        p = p * g + f32::from_bits(bits);
    }
    p = (p * (g * g) + g) + 1.0;
    let scale = f32::from_bits(((127.0 + n) as i32 as u32) << 23);
    x.max(p * scale)
}
