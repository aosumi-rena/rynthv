use super::{
    Result, fail,
    tensor::{Tensor, affine},
};

pub fn run(q: &Tensor, k: &Tensor, v: &Tensor, window: usize, penalty: f32) -> Result<Tensor> {
    if q.shape.len() != 4
        || k.shape.len() != 4
        || v.shape != k.shape
        || q.shape[2..] != k.shape[2..]
    {
        return fail("Instrument attention shape");
    }
    let (b, t, h, c) = (q.shape[0], q.shape[1], q.shape[2], q.shape[3]);
    let s = k.shape[1];
    let scale = 1. / (c as f64).sqrt() as f32;
    let mut out = vec![0.; b * t * h * c];
    for bi in 0..b {
        for hi in 0..h {
            let mut key = vec![0.; s * c];
            for j in 0..s {
                for ch in 0..c {
                    key[j * c + ch] = k.data[k.at(&[bi, j, hi, ch])];
                }
            }
            for first in (0..t).step_by(64) {
                let end = (first + 64).min(t);
                let left = if window == 0 {
                    0
                } else {
                    first.saturating_sub(window / 2).min(s)
                };
                let right = if window == 0 {
                    s
                } else {
                    (end + window / 2).min(s)
                };
                if left >= right {
                    return fail("Instrument attention empty window");
                }
                let width = right - left;
                let mut query = vec![0.; (end - first) * c];
                for i in first..end {
                    for ch in 0..c {
                        query[(i - first) * c + ch] = q.data[q.at(&[bi, i, hi, ch])];
                    }
                }
                let mut logits = affine(
                    &query,
                    end - first,
                    c,
                    &key[left * c..right * c],
                    width,
                    None,
                );
                for i in first..end {
                    let row = &mut logits[(i - first) * width..(i - first + 1) * width];
                    for (offset, l) in row.iter_mut().enumerate() {
                        let j = left + offset;
                        *l *= scale;
                        if window != 0 && i.abs_diff(j) > window / 2 {
                            *l = f32::NEG_INFINITY;
                        }
                        if penalty != 0. {
                            *l -= i.abs_diff(j) as f32 * penalty;
                        }
                    }
                    let max = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                    let mut total = 0.;
                    for l in row.iter_mut() {
                        *l = (*l - max).exp();
                        total += *l;
                    }
                    for l in row.iter_mut() {
                        *l /= total;
                    }
                    for ch in 0..c {
                        let mut sum = 0.;
                        for (offset, p) in row.iter().enumerate() {
                            sum += p * v.data[v.at(&[bi, left + offset, hi, ch])];
                        }
                        out[((bi * t + i) * h + hi) * c + ch] = sum;
                    }
                }
            }
        }
    }
    Ok(Tensor::new(q.shape.clone(), out))
}
