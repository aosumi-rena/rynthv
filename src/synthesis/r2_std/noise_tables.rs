// SPDX-License-Identifier: MIT
use super::{
    Error, Result, StdBank,
    decoder::fast_log,
    directional_model::finite,
    lf::fast_sin,
    noise_filter_tables::{HIGH_A, HIGH_B, LOW_A, LOW_B},
};
pub(super) struct NoiseTables {
    pub bands: [Vec<f32>; 4],
}
impl NoiseTables {
    pub fn construct(bank: &StdBank, rng: &mut u32) -> Result<Self> {
        let edges = bank.configuration.numbers("voc_chanfreq")?;
        if edges.len() != 3
            || edges.iter().any(|x| !x.is_finite() || *x <= 0.0)
            || edges.windows(2).any(|v| v[1] <= v[0])
        {
            return Err(Error::Format("P3 noise channel edges"));
        }
        let mut bands = std::array::from_fn(|_| Vec::new());
        for _epoch in 0..2 {
            for c in 0..4 {
                let lower = if c == 0 { 0.0 } else { edges[c - 1] as f32 } / 44100.0;
                let upper = if c == 3 { 22050.0 } else { edges[c] as f32 } / 44100.0;
                bands[c] = generate(44100, lower, upper, rng)?;
            }
        }
        Ok(Self { bands })
    }
}
fn next(rng: &mut u32) -> f32 {
    *rng = rng.wrapping_mul(214013).wrapping_add(2531011);
    ((*rng >> 16) & 32767) as f32
}
fn generate(count: usize, lower: f32, upper: f32, rng: &mut u32) -> Result<Vec<f32>> {
    let q = count.min(20000);
    let work = q + 128;
    let m = work.min(20000);
    let mut white = vec![0.0; work];
    for value in &mut white[..m] {
        let u = next(rng) * f32::from_bits(0x38000100);
        let angle = next(rng) * f32::from_bits(0x3949116d) + f32::from_bits(0x3fc90fdb);
        let radius = fast_log(u) * f32::from_bits(0xbfb17218);
        if radius < 0.0 || !radius.is_finite() {
            return Err(Error::Format("C-noise radius"));
        }
        *value = (f64::from(fast_sin(angle)) * f64::from(radius).sqrt()) as f32;
    }
    for i in m..work {
        white[i] = white[(i - m) % m];
    }
    let low = lower.max(0.0);
    let high = upper.min(0.5);
    let filtered = if low != 0.0 && high < 0.5 {
        one_sided(&one_sided(&white, low, false), high, true)
    } else {
        one_sided(&white, if low == 0.0 { high } else { low }, low == 0.0)
    };
    let mut out = vec![0.0; count];
    out[..q.min(count)].copy_from_slice(&filtered[..q.min(count)]);
    if count > q {
        let mut head = q;
        'extend: loop {
            for i in 0..128 {
                out[head - 128 + i] = normalize(
                    filtered[i],
                    out[head - 128 + i],
                    i,
                    1.0 / 128.0,
                    1.0 / 16384.0,
                );
            }
            for i in 0..q - 128 {
                if head + i >= count {
                    break 'extend;
                }
                out[head + i] = filtered[128 + i];
            }
            head += q - 128;
        }
    }
    let tail = out[count - 32..].to_vec();
    for i in 0..32 {
        out[i] = normalize(tail[i], out[i], i, 1.0 / 32.0, 1.0 / 1024.0);
    }
    finite(&out)?;
    Ok(out)
}
fn normalize(a: f32, b: f32, index: usize, inverse: f32, inverse_square: f32) -> f32 {
    let z = index as f32;
    let r = z * inverse;
    let d = (z * z) * inverse_square - r;
    let denominator = (d + d) + 1.0;
    (f64::from(r * a + (1.0 - r) * b) / f64::from(denominator).sqrt()) as f32
}
fn one_sided(x: &[f32], cutoff: f32, low: bool) -> Vec<f32> {
    let row = ((f64::from(cutoff) * f64::from_bits(0x4059000009600004) - 1.0)
        .round()
        .max(0.0) as usize)
        .min(47);
    let (a, b) = if low {
        (&LOW_A, &LOW_B)
    } else {
        (&HIGH_A, &HIGH_B)
    };
    let a: [f32; 5] = std::array::from_fn(|i| f32::from_bits(a[row * 5 + i]));
    let b: [f32; 5] = std::array::from_fn(|i| f32::from_bits(b[row * 5 + i]));
    let y = filter(x, &a, &b);
    let reversed: Vec<_> = y[..x.len()].iter().rev().copied().collect();
    let z = filter(&reversed, &a, &b);
    z[..x.len()].iter().rev().copied().collect()
}
fn filter(x: &[f32], a: &[f32; 5], b: &[f32; 5]) -> Vec<f32> {
    let n = x.len();
    let mut y = vec![0.0; n + 4];
    if n <= 4 {
        return y;
    }
    let end = if n >= 8 { 4 + ((n - 4) / 4) * 4 } else { 4 };
    for i in 4..n {
        let packet = i < end && (i - 4) % 4 != 0;
        let left = a[4] * y[i - 4] + a[3] * y[i - 3];
        let right = a[2] * y[i - 2] + a[1] * y[i - 1];
        let mut value = y[i] - (left + right);
        value += b[4] * x[i - 4];
        value += b[3] * x[i - 3];
        let (p0, p1, p2) = (b[0] * x[i], b[1] * x[i - 1], b[2] * x[i - 2]);
        y[i] = value
            + if packet {
                (p1 + p2) + p0
            } else {
                (p0 + p2) + p1
            };
    }
    y
}
