// SPDX-License-Identifier: MIT
use super::{Error, Result, overlap::Row};
struct Distribution {
    mean: [f32; 64],
    variance: [f32; 64],
}
fn statistics(rows: &[[f32; 64]]) -> Distribution {
    let epsilon = f32::from_bits(0x3a83126f);
    let mut mean = [0.0; 64];
    let mut variance = [epsilon; 64];
    for row in rows {
        for j in 0..64 {
            let v = row[j].max(-6.0);
            mean[j] = v + mean[j];
            variance[j] = v * v + variance[j];
        }
    }
    let inverse = 1.0 / rows.len().max(1) as f32;
    for j in 0..64 {
        mean[j] *= inverse;
        variance[j] = (inverse * variance[j] - mean[j] * mean[j]).max(epsilon);
    }
    Distribution { mean, variance }
}
fn likelihood(stats: &Distribution, row: &[f32; 64]) -> f32 {
    let mut sum = 0.0;
    for j in 0..64 {
        let product = stats.variance[j] * f32::from_bits(0x40c90fdb);
        let basis =
            f32::from_bits(0x42affadd) - product.to_bits() as f32 * f32::from_bits(0x33b17218);
        let diff = row[j].max(-6.0) - stats.mean[j];
        sum += basis - ((diff * diff) / stats.variance[j]) * 0.5;
    }
    let z = ((sum / 64.0) * f32::from_bits(0x3fb8aa3b)).max(f32::from_bits(0xc2fc0000));
    let t = z * f32::from_bits(0x4b000000) + f32::from_bits(0x4e7de2a9);
    f32::from_bits(t as i64 as u32)
}
pub(super) struct Correction {
    delta: [f32; 64],
    old: Vec<f64>,
    next: Vec<f64>,
}
impl Correction {
    pub fn compute(old: &mut [[f32; 64]], next: &mut [[f32; 64]]) -> Result<Self> {
        if old.is_empty() || old.len() > next.len() {
            return Err(Error::Format("overlap correction tail/prefix shape"));
        }
        let a = statistics(old);
        let b = statistics(&next[next.len() - old.len()..]);
        let context = statistics(&[]);
        let delta = std::array::from_fn(|j| (b.mean[j] - a.mean[j]) * 0.5);
        let weight = |stats: &Distribution, row: &[f32; 64], previous: f64| -> Result<f64> {
            let main = f64::from(likelihood(stats, row));
            let other = f64::from(likelihood(&context, row));
            let sum = main + other;
            if !sum.is_finite() || sum <= 0.0 {
                return Err(Error::Format("overlap correction likelihood"));
            }
            Ok((main / sum).max(previous - 0.1))
        };
        let mut previous = 1.0;
        let mut ow = vec![0.0; old.len()];
        let mut nw = vec![0.0; next.len()];
        for i in (0..old.len()).rev() {
            let mut w = weight(&a, &old[i], previous)?;
            if i < 10 {
                w = w.min(i as f64 * 0.1);
            }
            ow[i] = w;
            previous = w;
        }
        previous = 1.0;
        for i in 0..next.len() {
            let w = weight(&b, &next[i], previous)?;
            nw[i] = -w;
            previous = w;
        }
        let result = Self {
            delta,
            old: ow,
            next: nw,
        };
        for (i, row) in old.iter_mut().enumerate() {
            result.metadata(row, i, true);
        }
        for (i, row) in next.iter_mut().enumerate() {
            result.metadata(row, i, false);
        }
        Ok(result)
    }
    fn metadata(&self, row: &mut [f32; 64], i: usize, old: bool) {
        let weight = if old { self.old[i] } else { self.next[i] };
        for j in 0..64 {
            row[j] += (f64::from(self.delta[j]) * weight) as f32;
        }
    }
    pub fn apply(&self, row: &mut Row, i: usize, old: bool, nyquist: f32) -> Result<()> {
        let weight = if old { self.old[i] } else { self.next[i] };
        let delta: [f32; 64] = std::array::from_fn(|j| (f64::from(self.delta[j]) * weight) as f32);
        for j in 0..64 {
            row.1[j] += delta[j];
        }
        let scaled: Vec<_> = delta
            .iter()
            .map(|&v| (f64::from(v) * 4.342162396873643) as f32)
            .collect();
        let low = super::decoder::fast_log(50.0 * f32::from_bits(0x3abb3ee7) + 1.0);
        let high = super::decoder::fast_log(nyquist * f32::from_bits(0x3abb3ee7) + 1.0);
        let axis: Vec<_> = (0..64)
            .map(|i| ((i as f32 * (1.0 / 64.0) * (high - low) + low).exp2() - 1.0) * 700.0)
            .collect();
        let add = |values: &mut [f32]| -> Result<()> {
            if values.is_empty() {
                return Ok(());
            }
            let queries = super::decoder::uniform(values.len(), nyquist);
            let additions = super::decoder::interpolate(&axis, &scaled, &queries)?;
            for (v, d) in values.iter_mut().zip(additions) {
                *v += d;
            }
            Ok(())
        };
        add(&mut row.0.vocal_tract_db)?;
        add(&mut row.0.noise_psd_db)?;
        Ok(())
    }
}
