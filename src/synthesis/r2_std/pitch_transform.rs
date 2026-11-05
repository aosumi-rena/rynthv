// SPDX-License-Identifier: MIT
use super::{DecodedFrame, Error, Result, StdBank, archive::Reader};
use std::collections::HashMap;
struct Component {
    lower: f32,
    upper: f32,
    weights: Vec<f32>,
    means: Vec<f32>,
    variance: Vec<f32>,
    normalizer: Vec<f32>,
    scalar: Vec<f32>,
    vt: Vec<f32>,
    nm: Vec<f32>,
}
struct Model {
    base: Component,
    learned: Vec<Component>,
    entries: Vec<(Option<usize>, f32)>,
}
struct Delta {
    vt: Vec<f32>,
    nm: Vec<f32>,
    rd: f32,
}
pub(super) struct PitchTransform(HashMap<Vec<u8>, Model>, f32);
impl PitchTransform {
    pub fn open(bank: &StdBank) -> Result<Self> {
        let declarations = bank.configuration.optional_strings("pt_model")?;
        if declarations.len() % 2 != 0 {
            return Err(Error::Format("directory pitch-model pairs"));
        }
        let mut models = HashMap::new();
        for pair in declarations.chunks_exact(2) {
            if pair.iter().any(|v| v.is_empty() || v.contains(&0)) || models.contains_key(&pair[0])
            {
                return Err(Error::Format("directory pitch-model identity"));
            }
            let mut reader = Reader::new(bank.archive.named(&pair[1])?);
            let count = count(&mut reader)?;
            if !matches!(count, 0 | 2) || reader.f32()? != 1.0 {
                return Err(Error::Unsupported(
                    "zero/two learned pitch model components",
                ));
            }
            let base = Component::read(&mut reader, false)?;
            let mut learned = Vec::new();
            let mut anchor = 0.0;
            for _ in 0..count {
                let component = Component::read(&mut reader, true)?;
                if anchor == 0.0 {
                    anchor = component.lower;
                }
                learned.push(component);
            }
            reader.end()?;
            let mut entries: Vec<_> = learned
                .iter()
                .enumerate()
                .map(|(i, c)| (Some(i), c.upper))
                .collect();
            if count != 0 {
                let mut at = 0;
                while at < entries.len() && anchor > entries[at].1 {
                    at += 1;
                }
                entries.insert(at, (None, anchor));
            }
            if entries.windows(2).any(|v| v[1].1 < v[0].1) {
                return Err(Error::Format("pitch selector order"));
            }
            models.insert(
                pair[0].clone(),
                Model {
                    base,
                    learned,
                    entries,
                },
            );
        }
        Ok(Self(
            models,
            if bank.profile.encoder_only {
                22050.0
            } else {
                24000.0
            },
        ))
    }
    pub fn apply(
        &self,
        directory: &[u8],
        sp: &mut [f32; 64],
        pitch_hz: f32,
        frame: &mut DecodedFrame,
    ) -> Result<()> {
        let Some(model) = self.0.get(directory) else {
            return Ok(());
        };
        if sp.iter().any(|v| !v.is_finite()) || !pitch_hz.is_finite() {
            return Err(Error::Format("pitch transform input"));
        }
        let delta = model.evaluate(sp, pitch_hz)?;
        let source = if frame.vocal_tract_db.is_empty() {
            &delta.nm
        } else {
            &delta.vt
        };
        let low = super::decoder::fast_log(50.0 * f32::from_bits(0x3abb3ee7) + 1.0);
        let high = super::decoder::fast_log(self.1 * f32::from_bits(0x3abb3ee7) + 1.0);
        for i in 0..64 {
            let hz = ((((i as f32) * (1.0 / 64.0)) * (high - low) + low).exp2() - 1.0) * 700.0;
            sp[i] += (f64::from(super::spectrum::dense(source, self.1, hz))
                * f64::from_bits(0x3fbd7a786c22680a)) as f32;
        }
        let old_psd=frame.noise_wave.as_ref().map(|_|frame.noise_psd_db);
        for i in 0..256 {
            frame.noise_psd_db[i] += delta.nm[i];
        }
        if let (Some(key),Some(old))=(&mut frame.noise_wave,old_psd) {
            key.adjust_psd(&old,&frame.noise_psd_db)?;
        }
        if !frame.vocal_tract_db.is_empty() {
            let angle = f64::from((frame.rd - 1.5) * f32::from_bits(0x3eaaaaab))
                * f64::from_bits(0x40069e9565708efc);
            let input = angle.tan() as f32 + delta.rd;
            frame.rd = (f64::from(input).atan() * f64::from_bits(0x3ff0f9fdb0d2d1c3) + 1.5) as f32;
            frame.rd = frame.rd.clamp(f32::from_bits(0x3ca3d70a), 3.0);
            for (value, delta) in frame.vocal_tract_db.iter_mut().zip(delta.vt) {
                *value += delta;
            }
        }
        if !frame.rd.is_finite()
            || frame
                .vocal_tract_db
                .iter()
                .chain(&frame.noise_psd_db)
                .any(|x| !x.is_finite())
        {
            return Err(Error::Format("pitch transform publication"));
        }
        Ok(())
    }
}
impl Model {
    fn evaluate(&self, sp: &[f32; 64], pitch: f32) -> Result<Delta> {
        if self.learned.is_empty() {
            return Ok(Delta {
                vt: self.base.vt.clone(),
                nm: self.base.nm.clone(),
                rd: self.base.scalar[0],
            });
        }
        let mut upper = 0;
        while upper < self.entries.len() && pitch > self.entries[upper].1 {
            upper += 1;
        }
        let lo = upper.saturating_sub(1);
        let hi = upper.min(self.entries.len() - 1);
        let entry = |index: usize| -> Result<Delta> {
            match self.entries[index].0 {
                None => Ok(Delta {
                    vt: vec![0.0; 1025],
                    nm: vec![0.0; 256],
                    rd: 0.0,
                }),
                Some(i) => self.learned[i].evaluate(sp),
            }
        };
        let left = entry(lo)?;
        let right = entry(hi)?;
        let weight = (f64::from(pitch - self.entries[lo].1)
            / (f64::from(self.entries[hi].1 - self.entries[lo].1)
                + f64::from_bits(0x3ee4f8b588e368f1))) as f32;
        let weight = weight.clamp(0.0, 1.0);
        let other = 1.0 - weight;
        Ok(Delta {
            vt: (0..1025)
                .map(|i| self.base.vt[i] + (weight * right.vt[i] + other * left.vt[i]))
                .collect(),
            nm: (0..256)
                .map(|i| self.base.nm[i] + (weight * right.nm[i] + other * left.nm[i]))
                .collect(),
            rd: self.base.scalar[0] + (right.rd * weight + left.rd * other),
        })
    }
}
impl Component {
    fn read(reader: &mut Reader<'_>, learned: bool) -> Result<Self> {
        if learned && (count(reader)? != 8 || count(reader)? != 64) {
            return Err(Error::Unsupported("pitch density 8x64"));
        }
        let weights = if learned {
            array(reader, 8, true)?
        } else {
            Vec::new()
        };
        let means = if learned {
            array(reader, 512, false)?
        } else {
            Vec::new()
        };
        let variance = if learned {
            array(reader, 512, true)?
        } else {
            Vec::new()
        };
        if count(reader)? != 1025 || count(reader)? != 256 {
            return Err(Error::Unsupported("pitch physical output geometry"));
        }
        let lower = reader.f32()?;
        let upper = reader.f32()?;
        if !lower.is_finite() || !upper.is_finite() {
            return Err(Error::Format("pitch component bounds"));
        }
        let n = if learned { 8 } else { 1 };
        let scalar = array(reader, n, false)?;
        let vt = array(reader, n * 1025, false)?;
        let nm = array(reader, n * 256, false)?;
        let mut normalizer = vec![0.0; if learned { 8 } else { 0 }];
        for k in 0..normalizer.len() {
            for j in 0..64 {
                let value = f64::from(variance[k * 64 + j]);
                normalizer[k] = (f64::from(normalizer[k])
                    - ((value + value) * std::f64::consts::PI).ln())
                    as f32;
            }
        }
        Ok(Self {
            lower,
            upper,
            weights,
            means,
            variance,
            normalizer,
            scalar,
            vt,
            nm,
        })
    }
    fn evaluate(&self, sp: &[f32; 64]) -> Result<Delta> {
        let mut logits = [0.0f32; 8];
        for k in 0..8 {
            let mut total = 0.0f32;
            for j in 0..64 {
                let d = sp[j] - self.means[k * 64 + j];
                let term = d * d / self.variance[k * 64 + j];
                total = if j % 4 == 0 {
                    term + total
                } else {
                    total + term
                };
            }
            let density = (f64::from(self.normalizer[k]) * 0.5 - f64::from(total) * 0.5) as f32;
            logits[k] = (f64::from(density) * f64::from(1.0f32 / 64.0)
                + f64::from(self.weights[k].ln())) as f32;
        }
        let mut maximum = f32::from_bits(0xccbebc20);
        for &v in &logits {
            if v > maximum {
                maximum = v;
            }
        }
        let mut gamma = logits.map(|x| (x - maximum).exp());
        let mut sum = 0.0;
        for v in gamma {
            sum += v;
        }
        if !sum.is_finite() || sum <= 0.0 {
            return Err(Error::Format("pitch GMM responsibility sum"));
        }
        let inverse = 1.0 / sum;
        for x in &mut gamma {
            *x = inverse * *x;
        }
        let mut result = Delta {
            vt: vec![0.0; 1025],
            nm: vec![0.0; 256],
            rd: 0.0,
        };
        for k in 0..8 {
            let w = gamma[k];
            if f64::from(w) < f64::from_bits(0x3f50624dd2f1a9fc) {
                continue;
            }
            for i in 0..256 {
                result.nm[i] = w * self.nm[k * 256 + i] + result.nm[i];
            }
            for i in 0..1025 {
                result.vt[i] = self.vt[k * 1025 + i] * w + result.vt[i];
            }
            result.rd = self.scalar[k] * w + result.rd;
        }
        if !result.rd.is_finite() || result.vt.iter().chain(&result.nm).any(|x| !x.is_finite()) {
            return Err(Error::Format("nonfinite pitch GMM output"));
        }
        Ok(result)
    }
}
fn count(reader: &mut Reader<'_>) -> Result<usize> {
    let x = reader.f32()?;
    if x.is_finite() && (0.0..2147483648.0).contains(&x) && (x as usize) as f32 == x {
        Ok(x as usize)
    } else {
        Err(Error::Format("integral model extent"))
    }
}
fn array(reader: &mut Reader<'_>, count: usize, positive: bool) -> Result<Vec<f32>> {
    if count > reader.remaining() / 4 {
        return Err(Error::Format("truncated pitch model"));
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let x = reader.f32()?;
        if !x.is_finite() || positive && (!x.is_normal() || x <= 0.0) {
            return Err(Error::Format("pitch model numeric domain"));
        }
        out.push(x);
    }
    Ok(out)
}
