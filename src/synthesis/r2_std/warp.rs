// SPDX-License-Identifier: MIT
use super::{Error, Result, directional::packed_exp, directional_model::finite};

struct Model {
    j: usize,
    k: usize,
    m: f32,
    q: f32,
    lambda: f64,
    b: Vec<f32>,
    alpha: Vec<f32>,
}
impl Model {
    fn new(curve: &[f32], count: usize, q: f64, lambda: f64) -> Self {
        let m = curve.len() as f32;
        let k = (curve.len() / 5).max(5);
        let scale = (2.0 / f64::from(m)).sqrt();
        let mut b = Vec::with_capacity(k);
        let mut alpha = Vec::with_capacity(k);
        for lane in 0..k {
            let a = if lane == 0 {
                f32::from_bits(0x3f3504f3)
            } else {
                1.0
            };
            alpha.push(a);
            let mut sum = 0.0f32;
            for (i, &value) in curve.iter().enumerate() {
                let angle =
                    (std::f64::consts::FRAC_PI_2 / f64::from(m) * (2 * i + 1) as f64) * lane as f64;
                sum = (f64::from(sum) + angle.cos() * f64::from(value.ln() * a)) as f32;
            }
            b.push((f64::from(sum) * scale) as f32);
        }
        Self {
            j: count.min(128),
            k,
            m,
            q: q as f32,
            lambda,
            b,
            alpha,
        }
    }
    fn reconstruct(&self, coords: &[f32]) -> Vec<f32> {
        let scale = (2.0 / f64::from(self.m)).sqrt() as f32;
        coords
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                let terms: Vec<_> = (0..self.k)
                    .map(|k| {
                        let pik = std::f32::consts::PI * k as f32;
                        let angle = c * pik + (0.5 * pik) / self.m;
                        (self.b[k] * angle.cos()) * self.alpha[k]
                    })
                    .collect();
                (aligned_sum(&terms, (i * self.k) & 3) * scale).exp()
            })
            .collect()
    }
}
struct State {
    a: Vec<f32>,
    sum: f32,
    p: Vec<f32>,
    z: Vec<f32>,
    w: Vec<f32>,
    c: Vec<f32>,
    r: Vec<f32>,
}
fn weights(x: &[f32]) -> Vec<f32> {
    let bulk = x.len() / 4 * 4;
    x.iter()
        .enumerate()
        .map(|(i, &v)| if i < bulk { packed_exp(v) } else { v.exp() })
        .collect()
}
fn common(m: &Model, x: &[f32]) -> State {
    let a = weights(x);
    let sum = reduce(&a);
    let u = (m.q / sum).ln();
    let p: Vec<_> = a.iter().map(|v| *v / sum).collect();
    let z = x.iter().map(|v| u + *v).collect();
    let w = p
        .iter()
        .map(|p| (((m.q * m.q) * p) * p + 1.0).sqrt())
        .collect();
    let mut c = Vec::with_capacity(x.len());
    let mut total = 0.0;
    for p in &p {
        total += p;
        c.push(total);
    }
    let r = m.reconstruct(&c);
    State {
        a,
        sum,
        p,
        z,
        w,
        c,
        r,
    }
}
fn objective(m: &Model, x: &[f32], s: &State) -> f32 {
    let square: Vec<_> = (0..x.len())
        .map(|i| {
            let d = if i == 0 { 0.0 } else { x[i] - x[i - 1] };
            d * d
        })
        .collect();
    let bulk = x.len() / 4 * 4;
    let weighted: Vec<_> = (0..x.len())
        .map(|i| {
            if i < bulk {
                ((s.z[i] * s.z[i]) * s.w[i]) * s.r[i]
            } else {
                (s.z[i] * s.r[i]) * (s.z[i] * s.w[i])
            }
        })
        .collect();
    (f64::from(reduce(&square)) * m.lambda + f64::from(reduce(&weighted))) as f32
}
fn gradient(m: &Model, x: &[f32], s: &State) -> Vec<f32> {
    let n = x.len();
    let mut a = vec![0.0; n * n];
    let mut t = vec![0.0; n * n];
    for j in 0..n {
        for i in 0..n {
            let at = i + n * j;
            a[at] = ((0.0 - s.a[i]) * s.p[j]) / s.sum;
            t[at] = (if i == j { 1.0 } else { 0.0 }) - s.p[j];
        }
    }
    for i in 0..n {
        a[i + n * i] = s.a[i] / s.sum + a[i + n * i];
    }
    let mut c = a.clone();
    for j in 0..n {
        for i in 1..n {
            c[i + n * j] = c[i - 1 + n * j] + c[i + n * j];
        }
    }
    let plus = m.reconstruct(&s.c.iter().map(|v| 0.001 + *v).collect::<Vec<_>>());
    let secant: Vec<_> = plus
        .iter()
        .zip(&s.r)
        .map(|(a, b)| ((a - b) * 1000.0).clamp(-5.0, 5.0))
        .collect();
    let rho: Vec<_> = s.z.iter().zip(&s.w).map(|(z, w)| (z * z) * w).collect();
    let v: Vec<_> =
        s.p.iter()
            .zip(&s.w)
            .map(|(p, w)| ((m.q * m.q) * p) / w)
            .collect();
    let mut km = vec![0.0; n * n];
    let mut g = vec![0.0; n * n];
    for j in 0..n {
        for i in 0..n {
            let at = i + n * j;
            km[at] = secant[i] * c[at];
            g[at] = a[at] * v[i];
        }
    }
    let first = multiply(&km, n, &rho);
    let mut h = vec![0.0; n * n];
    for j in 0..n {
        let lead = (4 - (n * j) % 4) % 4;
        let stop = lead + 4 * ((n - lead) / 4);
        let z2 = s.z[j] * s.z[j];
        for i in 0..n {
            let at = i + n * j;
            let packet = i >= lead && i < stop;
            let one = (s.z[i] * t[at]) * if packet { s.w[i] * 2.0 } else { 2.0 * s.w[i] };
            let two = if packet {
                z2 * g[at]
            } else {
                (s.z[j] * g[at]) * s.z[j]
            };
            h[at] = if i >= stop { two + one } else { one + two };
        }
    }
    let second = multiply(&h, n, &s.r);
    (0..n)
        .map(|i| {
            let lap = if i == 0 {
                x[0] - x[1]
            } else if i == n - 1 {
                x[i] - x[i - 1]
            } else {
                ((x[i] + x[i]) - x[i - 1]) - x[i + 1]
            };
            (second[i] + first[i]) + (m.lambda as f32 * lap) * 2.0
        })
        .collect()
}
fn multiply(w: &[f32], n: usize, x: &[f32]) -> Vec<f32> {
    let width = if n < 128 { n } else { 16 };
    let mut y = vec![0.0; n];
    for row in 0..n {
        for start in (0..n).step_by(width) {
            let mut sum = 0.0;
            for c in start..(start + width).min(n) {
                sum += w[c * n + row] * x[c];
            }
            y[row] = sum + y[row];
        }
    }
    y
}
pub(super) fn optimize(curve: &[f32], count: usize, q: f64, lambda: f64) -> Result<Vec<f32>> {
    if curve.len() < 5
        || count < 5
        || count > 4096
        || curve.len() > 8192
        || !q.is_finite()
        || q <= 0.0
        || !lambda.is_finite()
        || curve
            .iter()
            .any(|x| !x.is_finite() || *x < f32::MIN_POSITIVE)
    {
        return Err(Error::Format("source warp inputs"));
    }
    let m = Model::new(curve, count, q, lambda);
    let mut x = vec![0.0; m.j];
    let mut previous = 100000.0f32;
    for step in 0..100 {
        let s = common(&m, &x);
        let cost = objective(&m, &x, &s);
        let g = gradient(&m, &x, &s);
        finite(&g)?;
        finite(&[cost])?;
        if f64::from(cost) > f64::from(previous) - f64::from_bits(0x3f1a36e2eb1c432d) {
            break;
        }
        previous = cost;
        let size = if step <= 10 {
            0.05
        } else if step <= 20 {
            0.1
        } else if step <= 30 {
            0.2
        } else {
            0.4
        };
        for (x, g) in x.iter_mut().zip(g) {
            *x -= size * g;
        }
        finite(&x)?;
    }
    let a = weights(&x);
    let sum = reduce(&a);
    let mut result = Vec::with_capacity(m.j + 1);
    result.push(0.0);
    let mut cumulative = 0.0;
    for a in a {
        cumulative += a / sum;
        result.push(cumulative * m.q);
    }
    finite(&result)?;
    Ok(result)
}
pub(super) fn reduce(x: &[f32]) -> f32 {
    let bulk = x.len() / 4 * 4;
    let eight = x.len() / 8 * 8;
    if bulk == 0 {
        return x.iter().copied().reduce(|a, b| a + b).unwrap_or(0.0);
    }
    let mut a: [f32; 4] = x[..4].try_into().unwrap();
    if bulk > 4 {
        let mut b: [f32; 4] = x[4..8].try_into().unwrap();
        for i in (8..eight).step_by(8) {
            for lane in 0..4 {
                a[lane] += x[i + lane];
                b[lane] += x[i + 4 + lane];
            }
        }
        for lane in 0..4 {
            a[lane] += b[lane];
        }
        if bulk > eight {
            for lane in 0..4 {
                a[lane] += x[eight + lane];
            }
        }
    }
    let mut result = (a[2] + a[0]) + (a[3] + a[1]);
    for value in &x[bulk..] {
        result += value;
    }
    result
}
fn aligned_sum(x: &[f32], address: usize) -> f32 {
    let lead = x.len().min((4 - address) & 3);
    let stop = lead + 4 * ((x.len() - lead) / 4);
    if lead == stop {
        return x.iter().copied().reduce(|a, b| a + b).unwrap_or(0.0);
    }
    let mut result = reduce(&x[lead..stop]);
    for value in x[..lead].iter().chain(&x[stop..]) {
        result += value;
    }
    result
}
