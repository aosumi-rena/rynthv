// SPDX-License-Identifier: MIT
use super::{Error, Result};

pub(super) struct Pulse {
    model: [f32; 5],
    state: [f32; 12],
}
impl Pulse {
    pub fn new(rd: f32, f0: f32) -> Result<Self> {
        if !rd.is_finite() || !f0.is_finite() || rd <= 0.0 || f0 <= 0.0 {
            return Err(Error::Format("LF positive finite Rd/F0"));
        }
        let r = f64::from(rd);
        let rap = if r < 0.21 {
            f32::from_bits(0x358637bd)
        } else if r < 2.7 {
            (r * 0.048 - 0.01) as f32
        } else {
            (0.323 / r) as f32
        };
        let (rk, rg) = if r < 2.7 {
            let rk = (r * 0.11800000000000001 + 0.22399999999999998) as f32;
            let rg = ((f64::from(rk) * 0.25)
                / ((r * 0.11) / (1.2 * f64::from(rk) + 0.5) - f64::from(rap)))
                as f32;
            (rk, rg)
        } else {
            let oq = (1.0 - 0.4608294930875576 / r) as f32;
            let rg = (5.96 / (7.96 - f64::from(oq + oq)) + 0.0093552) as f32;
            ((f64::from((rg + rg) * oq) - 1.0428) as f32, rg)
        };
        let tp = 0.5 / rg;
        let te = (rk + 1.0) * tp;
        let mut model = [1.0 / f0, te, tp, rap, 1.0];
        let original = model[0];
        let mut scale = 1.0;
        if original < 0.00125 {
            model[0] = f32::from_bits(0x3aa3d70a);
            scale = (0.00125 / f64::from(original)) as f32;
        }
        let mut state = [0.0; 12];
        let t0 = model[0];
        let tp = t0 * model[2];
        let te = t0 * model[1];
        let ta = t0 * model[3];
        let wg = (std::f64::consts::PI / f64::from(tp)) as f32;
        state[0] = t0;
        state[1] = te;
        state[2] = tp;
        state[3] = ta;
        state[4] = wg;
        state[11] = scale;
        (state[5], state[6]) = (wg * te).sin_cos();
        let denominator = f64::from(ta) + 1e-9;
        let initial = (2.0 / denominator) as f32;
        let epsilon = brent(initial, &state)?;
        state[7] = epsilon;
        let dt = te - t0;
        let ev = (dt * epsilon).exp();
        let e0 = (1.0 - ev) / ((epsilon * epsilon) * ta) + (dt * ev) / (ta * epsilon);
        state[8] = e0;
        let v23 = (((wg * wg) * state[5]) * e0) - state[6] * wg;
        let v24 = te * wg;
        let c12 = (2.0 * state[5]) * e0;
        let c13 = e0 * state[5];
        let mut alpha = 0.0f32;
        for _ in 0..8 {
            let e = f64::from(-(te * alpha)).exp();
            let t1 = (c13 * alpha + state[5]) * alpha;
            let num = f64::from(t1) + f64::from(wg) * e + f64::from(v23);
            let t2 = c12 * alpha + state[5];
            let den = f64::from(t2) - f64::from(v24) * e;
            alpha -= (num as f32) / (den as f32);
        }
        state[9] = alpha;
        state[10] =
            (-f64::from(model[4]) / (f64::from(te * alpha).exp() * f64::from(state[5]))) as f32;
        if state.iter().any(|v| !v.is_finite()) {
            return Err(Error::Format("LF solver nonfinite result"));
        }
        Ok(Self { model, state })
    }
    pub fn at(&self, hz: f32) -> Result<(f32, f32)> {
        self.evaluate(hz, true)
    }
    pub fn magnitude(&self, hz: f32) -> Result<f32> {
        Ok(self.evaluate(hz, false)?.0)
    }
    fn evaluate(&self, hz: f32, phase: bool) -> Result<(f32, f32)> {
        if !hz.is_finite() || hz < 0.0 {
            return Err(Error::Format("LF frequency"));
        }
        if hz == 0.0 {
            return Ok((0.0, 0.0));
        }
        let m = &self.model;
        let st = &self.state;
        let (g, t0, te, ta, wg, s, c, eps, alpha, gain, scale) = (
            m[4], st[0], st[1], st[3], st[4], st[5], st[6], st[7], st[9], st[10], st[11],
        );
        let tae = ta * eps;
        let gate = (1.0 - tae) * eps;
        let v48 = (te * alpha).exp();
        let v18 = wg * wg;
        let v15 = alpha * alpha;
        let v16 = 2.0 * alpha;
        let v49 = s * alpha - c * wg;
        let omega = (f64::from(hz) * std::f64::consts::TAU / f64::from(scale)) as f32;
        let no = -omega;
        let v23 = v16 * no;
        let o2 = no * no;
        let v24 = (v15 - o2) + v18;
        let inv = 1.0 / (v24 * v24 + v23 * v23);
        let v26 = (v24 * gain) * inv;
        let v27 = -((v23 * gain) * inv);
        let no_s = no * s;
        let no_te = no * te;
        let cw = fast_sin(no_te + f32::from_bits(0x3fc90fdb));
        let sw = fast_sin(no_te);
        let cv = cw * v48;
        let sv = sw * v48;
        let v33 = sv * v49 + cv * no_s;
        let v35 = (cv * v49 - sv * no_s) + wg;
        let v32 = tae * omega;
        let v34 = v32 * eps;
        let v38 = -(v32 * omega);
        let a = -(omega * te);
        let v39 = fast_sin(a + f32::from_bits(0x3fc90fdb));
        let v37 = fast_sin(a);
        let inv40 = 1.0 / (v38 * v38 + v34 * v34);
        let nr = v38 * v39 + v37 * v34;
        let ni = v38 * v37 - v39 * v34;
        let v41 = (nr * inv40) * g;
        let v42 = (ni * inv40) * g;
        let (v43, v44) = if gate >= 1.0 {
            let x2 = (te - t0) * omega;
            let cx = fast_sin(x2 + f32::from_bits(0x3fc90fdb));
            (-fast_sin(x2) * gate - v32, (1.0 - cx) * gate)
        } else {
            (-v32, 0.0)
        };
        let re = (v44 * v41 - v43 * v42) + (v35 * v26 - v33 * v27);
        let im = (v44 * v42 + v43 * v41) + (v35 * v27 + v33 * v26);
        let magnitude = (re * re + im * im).sqrt();
        if magnitude.is_nan() {
            return Ok((0.0, 0.0));
        }
        if !magnitude.is_finite() {
            return Err(Error::Format("LF spectrum nonfinite"));
        }
        Ok((magnitude, if phase { im.atan2(re) } else { 0.0 }))
    }
}

fn objective(state: &[f32; 12], x: f32) -> f32 {
    let d = (state[1] - state[0]) * x;
    let xa = x * state[3];
    ((1.0 - f64::from(d).exp()) - f64::from(xa)) as f32
}
fn brent(mut a: f32, state: &[f32; 12]) -> Result<f32> {
    let mut b = 1.0f32;
    let mut fb = objective(state, b);
    let mut fa = objective(state, a);
    if fa * fb >= 0.0 {
        return Ok((a + b) * 0.5);
    }
    if fb.abs() < fa.abs() {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut fa, &mut fb);
    }
    if fa.abs() <= 1e-8 {
        return Ok(a);
    }
    let mut mflag = true;
    let mut d = 0.0;
    let mut cp = b;
    let mut fc = fb;
    for _ in 0..4096 {
        if (b - a).abs() <= 1e-8 {
            return Ok(a);
        }
        let mut s = if fb == fc || fa == fc {
            a - ((a - b) * fa) / (fa - fb)
        } else {
            let t1 = ((fa * b) * fc) / ((fb - fc) * (fb - fa));
            let t2 = ((fb * a) * fc) / ((fa - fc) * (fa - fb));
            let t3 = ((cp * fb) * fa) / ((fc - fa) * (fc - fb));
            (t1 + t2) + t3
        };
        let bound = f64::from(a) * 0.25 + f64::from(b) * 0.75;
        let mut bisect = f64::from(s) < bound || s > a;
        if !bisect {
            let ds = (s - a).abs();
            let previous = if mflag {
                (a - cp).abs()
            } else {
                (cp - d).abs()
            };
            if ds >= previous * 0.5 || previous < 1e-8 {
                bisect = true;
            } else {
                mflag = false;
            }
        }
        if bisect {
            s = (a + b) * 0.5;
            if s == a || s == b {
                return Ok(a);
            }
            mflag = true;
        }
        let fs = objective(state, s);
        d = cp;
        cp = a;
        fc = fa;
        if fs * fb >= 0.0 {
            b = s;
            fb = fs;
        } else {
            a = s;
            fa = fs;
        }
        if fb.abs() < fa.abs() {
            std::mem::swap(&mut a, &mut b);
            std::mem::swap(&mut fa, &mut fb);
        }
        if fa.abs() <= 1e-8 {
            return Ok(a);
        }
    }
    Err(Error::Format("LF solver did not converge"))
}
pub(super) fn fast_sin(x: f32) -> f32 {
    let reduced = x * f32::from_bits(0x3e22f983);
    let k = reduced as i32 as f32;
    let v2 = (k + if x >= 0.0 { 0.5 } else { -0.5 }) * f32::from_bits(0x40c90fdb) - x;
    let sign = v2.to_bits() & 0x80000000;
    let v3 = v2 * f32::from_bits(0x3fa2f983) - (v2 * f32::from_bits(0x3ecf817b)) * v2.abs();
    let square = v3 * v3;
    let s =
        (square * f32::from_bits(sign ^ 0xbb533217) + f32::from_bits(sign | 0x3c77ce9a)) * square;
    let s = (s + f32::from_bits(sign | 0x3e5086d7)) * square;
    s + v3 * f32::from_bits(0x3f48d161)
}
