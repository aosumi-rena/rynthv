// SPDX-License-Identifier: MIT
use super::{Error, Result, directional::packed_exp, warp::reduce};
fn positive(x: &[f32]) -> Vec<f32> {
    x.iter()
        .enumerate()
        .map(|(i, &v)| {
            if i < x.len() / 4 * 4 {
                packed_exp(v)
            } else {
                v.exp()
            }
        })
        .collect()
}
fn evaluate(d: &[f32], lambda: f64, x: &[f32]) -> Result<(f32, Vec<f32>)> {
    let n = x.len();
    let a = positive(x);
    let sum = reduce(&a);
    if !sum.is_finite() || sum <= 0.0 {
        return Err(Error::Format("overlap positive normalization"));
    }
    let p: Vec<_> = a.iter().map(|v| *v / sum).collect();
    let mut c = 0.0;
    let mut square = Vec::with_capacity(n);
    let mut weighted = Vec::with_capacity(n);
    let mut suffix = Vec::with_capacity(n);
    for i in 0..n {
        c += p[i];
        let value = f64::from(c);
        let one = 1.0 - value;
        let h = (value * value).min(one * one) as f32;
        let derivative = if value >= 0.5 {
            ((value - 1.0) + value) - 1.0
        } else {
            value + value
        };
        square.push(p[i] * p[i]);
        weighted.push(h * d[i]);
        suffix.push(derivative as f32 * d[i]);
    }
    let objective =
        ((n as f64 * lambda) * f64::from(reduce(&square)) + f64::from(reduce(&weighted))) as f32;
    for i in (0..n - 1).rev() {
        suffix[i] += suffix[i + 1];
    }
    let k = ((lambda + lambda) * n as f64) as f32;
    let b: Vec<_> = (0..n).map(|i| k * p[i] + suffix[i]).collect();
    let mut gradient = vec![0.0; n];
    for j in 0..n {
        let mut acc = [0.0; 4];
        for i in 0..n / 4 * 4 {
            let mut m = (-a[i] * p[j]) / sum;
            if i == j {
                m = a[i] / sum + m;
            }
            acc[i % 4] += m * b[i];
        }
        let mut value = (acc[2] + acc[0]) + (acc[3] + acc[1]);
        for i in n / 4 * 4..n {
            let mut m = (-a[i] * p[j]) / sum;
            if i == j {
                m = a[i] / sum + m;
            }
            value += m * b[i];
        }
        gradient[j] = value;
    }
    if !objective.is_finite() || gradient.iter().any(|v| !v.is_finite()) {
        return Err(Error::Format("overlap finite optimizer"));
    }
    Ok((objective, gradient))
}
fn ramp(slope: i32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| ((i as f64 * (slope + slope) as f64) * (1.0 / n as f64) - slope as f64) as f32)
        .collect()
}
pub(super) fn compute(curve: &[f32], lambda: f64) -> Result<Vec<f32>> {
    let n = curve.len();
    if n == 0 || n > 256 || !lambda.is_finite() || curve.iter().any(|v| !v.is_finite() || *v < 0.0)
    {
        return Err(Error::Format("overlap fade inputs"));
    }
    if curve.iter().copied().fold(0.0f32, f32::max) < f32::from_bits(0x38d1b717) {
        return Ok((0..n).map(|i| i as f32 / n as f32).collect());
    }
    let mut best = 1e10;
    let mut selected = 0;
    for slope in -2..=2 {
        let mut x = ramp(slope, n);
        let mut objective = 0.0;
        for _ in 0..10 {
            let (value, g) = evaluate(curve, lambda, &x)?;
            objective = value;
            for (x, g) in x.iter_mut().zip(g) {
                *x -= 5.0 * g;
            }
        }
        if best > f64::from(objective) {
            selected = slope;
        }
        best = best.min(f64::from(objective));
    }
    let mut x = ramp(selected, n);
    let mut previous = f32::from_bits(0x501502f9);
    for _ in 0..50 {
        let (value, g) = evaluate(curve, lambda, &x)?;
        if f64::from(value) > f64::from(previous) - f64::from_bits(0x3f1a36e2eb1c432d) {
            break;
        }
        previous = value;
        for (x, g) in x.iter_mut().zip(g) {
            *x -= 5.0 * g;
        }
    }
    let a = positive(&x);
    let sum = reduce(&a);
    if !sum.is_finite() || sum <= 0.0 {
        return Err(Error::Format("overlap output normalization"));
    }
    let mut result = vec![0.0];
    let mut cumulative = 0.0;
    for a in a {
        cumulative += a / sum;
        result.push(cumulative);
    }
    Ok(result)
}
