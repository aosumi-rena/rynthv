// SPDX-License-Identifier: MIT
use super::{Error, Result};

pub(super) fn expand(step: f32, compact: &[f32]) -> Result<Vec<f32>> {
    const N: usize = 1025;
    if !step.is_finite()
        || step <= 0.0
        || !(3..=4096).contains(&compact.len())
        || compact.iter().any(|x| !x.is_finite())
    {
        return Err(Error::Format("compact vocal tract domain"));
    }
    let mut output = vec![0.0; N];
    let mut tail = 0;
    for d in 1..compact.len() - 1 {
        let previous = compact[if d == 1 { 0 } else { d - 2 }];
        let current = compact[d - 1];
        let following = compact[d];
        let last = compact[d + 1];
        let hc = current * 0.5;
        let hf = following * 0.5;
        let a2 = previous * 0.5 - current + hf;
        let sixth_last = f64::from(last) / 6.0;
        let third_plus_half = f64::from(previous) / 3.0 + f64::from(hc);
        let a3 = ((f64::from(hc) - f64::from(previous) / 6.0 - f64::from(hf)) + sixth_last) as f32;
        let a1 = (f64::from(following) - third_plus_half - sixth_last) as f32;
        let scale = (N as f32) * (1.0 / 24000.0);
        let index = |x: f32| -> Result<usize> {
            if x.is_finite() && (0.0..2147483648.0).contains(&x) {
                Ok(x as usize)
            } else {
                Err(Error::Format("compact raster index"))
            }
        };
        let begin = if d == 1 {
            0
        } else {
            index((d as f32) * step * scale)?
        };
        let end = index(((d + 1) as f32) * step * scale)?.min(N);
        if begin > N || end < begin {
            return Err(Error::Format("compact raster interval"));
        }
        let ceiling = current.max(previous).max(following).max(last);
        let bulk = begin + (end - begin) / 8 * 8;
        for i in begin..end {
            let t = (((i as f32) * (1.0 / N as f32)) * 24000.0 - (d as f32) * step) * (1.0 / step);
            let square = t * t;
            let cube = square * t;
            let left = cube * a3 + square * a2;
            let linear = t * a1;
            let value = if i < bulk {
                (left + linear) + current
            } else {
                left + (linear + current)
            };
            if !value.is_finite() {
                return Err(Error::Format("compact raster nonfinite"));
            }
            output[i] = value.min(ceiling);
        }
        tail = end;
        if end == N {
            break;
        }
    }
    if tail == 0 {
        return Err(Error::Unsupported("empty compact raster prefix"));
    }
    for i in tail..N {
        output[i] = (output[i - 1] - 1.0).max(-100.0);
    }
    Ok(output)
}
