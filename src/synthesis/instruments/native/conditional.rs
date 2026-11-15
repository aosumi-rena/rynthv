use super::{
    Result, fail, fourier,
    tensor::{Tensor, affine},
    u16at, u32at,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::borrow::Cow;
use rayon::prelude::*;
fn accumulate(a:&mut [f32],b:&[f32]) {
    if a.len()>=131072 {
        a.par_iter_mut().zip(b.par_iter()).for_each(|(a,b)|*a+=b);
    } else {for(a,b)in a.iter_mut().zip(b){*a+=b;}}
}
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
    dtype: u8,
}
impl Reader<'_> {
    fn words(&mut self, n: usize) -> Result<Vec<usize>> {
        self.at = self.at.div_ceil(4) * 4;
        if self.at + n * 4 > self.b.len() {
            return fail("Conditional parameter bounds");
        }
        let out = (0..n)
            .map(|i| u32at(self.b, self.at + i * 4) as usize)
            .collect();
        self.at += n * 4;
        Ok(out)
    }
    fn array(&mut self, n: usize, dtype: Option<u8>) -> Result<Vec<f32>> {
        self.at = self.at.div_ceil(4) * 4;
        let dtype = dtype.unwrap_or(self.dtype);
        let size = match dtype {
            1 => 2,
            2 => 4,
            _ => return fail("Conditional tensor dtype"),
        };
        if self.at + n * size > self.b.len() {
            return fail("Conditional tensor bounds");
        }
        let out = self.b[self.at..self.at + n * size]
            .chunks_exact(size)
            .map(|b| {
                if dtype == 1 {
                    half::f16::from_bits(u16at(b, 0)).to_f32()
                } else {
                    f32::from_bits(u32at(b, 0))
                }
            })
            .collect();
        self.at += n * size;
        Ok(out)
    }
    fn linear(&mut self, o: usize, i: usize) -> Result<Linear> {
        Ok(Linear {
            w: self.array(o * i, None)?,
            b: self.array(o, Some(2))?,
            input: i,
            output: o,
        })
    }
}
struct Linear {
    w: Vec<f32>,
    b: Vec<f32>,
    input: usize,
    output: usize,
}
impl Linear {
    fn run(&self, x: &[f32], rows: usize) -> Vec<f32> {
        affine(x, rows, self.input, &self.w, self.output, Some(&self.b))
    }
}
struct Layer {
    causal: bool,
    k: usize,
    d: usize,
    input: usize,
    norm: Option<Vec<f32>>,
    epsilon: f32,
    w: Vec<f32>,
    b: Vec<f32>,
    condition: Option<Linear>,
    pruned: Option<Linear>,
    skip: Linear,
    residual: Option<Linear>,
}
pub struct Conditional {
    h: usize,
    o: usize,
    c: usize,
    normalized: bool,
    edge: bool,
    direct: Option<Linear>,
    layers: Vec<Layer>,
}
impl Conditional {
    pub(super) fn stable_frames(&self,input:usize,condition:usize)->usize {
        let mut state=input;let mut output=condition;
        for layer in &self.layers {
            let left=if layer.causal{(layer.k-1)*layer.d}else{layer.k/2*layer.d};
            state=state.saturating_sub((layer.k-1)*layer.d-left).min(condition);
            output=output.min(state);
        }
        output
    }
    pub fn parse(
        b: &[u8],
        dtype: u8,
        normalized: bool,
        prunable: bool,
        edge: bool,
    ) -> Result<Self> {
        let mut r = Reader { b, at: 0, dtype };
        let v = r.words(5)?;
        let (h, i, o, c, count) = (v[0], v[1], v[2], v[3], v[4]);
        if [h, i, o, count].iter().any(|v| *v == 0 || *v >= 32768) || c >= 32768 {
            return fail("Conditional model dimensions");
        }
        let pruning = if prunable { r.words(1)?[0] } else { 0 };
        let direct = if c != 0 { Some(r.linear(h, c)?) } else { None };
        let mut layers = Vec::new();
        for index in 0..count {
            let v = r.words(4)?;
            if v[3] != 1 || v[1] == 0 || v[2] == 0 {
                return fail("Conditional kernel layout");
            }
            let (causal, k, d) = (v[0] != 0, v[1], v[2]);
            let input = if index == 0 { i } else { o };
            let norm = if normalized { r.words(2)? } else { vec![0, 0] };
            let epsilon = f32::from_bits(norm[1] as u32);
            let norm = if norm[0] != 0 {
                Some(r.array(input, Some(2))?)
            } else {
                None
            };
            let w = r.array(k * 2 * o * input, None)?;
            let bias = r.array(2 * o, Some(2))?;
            let condition = if c != 0 {
                Some(r.linear(2 * o, c)?)
            } else {
                None
            };
            let pruned = if pruning != 0 && index + 1 < count && (index + 1) % pruning == 0 {
                Some(r.linear((o & !1) + 2, c + (o & !1) + 2)?)
            } else {
                None
            };
            let skip = r.linear(h, o)?;
            let residual = if index + 1 < count {
                Some(r.linear(o, input)?)
            } else {
                None
            };
            layers.push(Layer {
                causal,
                k,
                d,
                input,
                norm,
                epsilon,
                w,
                b: bias,
                condition,
                pruned,
                skip,
                residual,
            });
        }
        if r.at != b.len() {
            return fail("Conditional model trailing data");
        }
        Ok(Self {
            h,
            o,
            c,
            normalized,
            edge,
            direct,
            layers,
        })
    }
    pub fn run(&self, x: &Tensor, condition: &Tensor, cancel: &AtomicBool) -> Result<Tensor> {
        if x.shape.len() != 3
            || condition.shape.len() != 3
            || x.shape[..2] != condition.shape[..2]
            || condition.shape[2] != self.c
        {
            return fail("Conditional input shape");
        }
        let (batch, frames) = (x.shape[0], x.shape[1]);
        let rows = batch * frames;
        let cd = condition.real_slice();
        let mut y = self
            .direct
            .as_ref()
            .map_or_else(|| vec![0.; rows * self.h], |p| p.run(&cd, rows));
        let mut state = x.dense();
        for layer in &self.layers {
            if cancel.load(Ordering::Relaxed) {
                return fail("Instrument render cancelled");
            }
            if state.len() != rows * layer.input {
                return fail("Conditional layer width");
            }
            let original = state;
            let x = if let Some(w) = &layer.norm {
                let mut x=original.clone();
                for row in x.chunks_mut(layer.input) {
                    let mut q = 0.;
                    for v in row.iter() {
                        q += *v * *v;
                    }
                    let scale = (q / layer.input as f32 + layer.epsilon).sqrt();
                    for (v, w) in row.iter_mut().zip(w) {
                        *v = (*v / scale) * *w;
                    }
                }
                Cow::Owned(x)
            } else {Cow::Borrowed(original.as_slice())};
            let pad = if layer.causal {
                (layer.k - 1) * layer.d
            } else {
                (layer.k / 2) * layer.d
            };
            let mut gate = vec![0.; rows * 2 * self.o];
            let padded=if batch==1 && layer.k>1 {
                let mut p=vec![0.;(frames+(layer.k-1)*layer.d)*layer.input];
                p[pad*layer.input..(pad+frames)*layer.input].copy_from_slice(&x);
                if self.edge {
                    for row in p[..pad*layer.input].chunks_exact_mut(layer.input) {
                        row.copy_from_slice(&x[..layer.input]);
                    }
                    for row in p[(pad+frames)*layer.input..].chunks_exact_mut(layer.input) {
                        row.copy_from_slice(&x[(frames-1)*layer.input..]);
                    }
                }
                Some(p)
            } else {None};
            for j in 0..layer.k {
                if batch==1 {
                    let q=padded.as_ref().map_or(x.as_ref(),|p|
                        &p[j*layer.d*layer.input..(j*layer.d+frames)*layer.input]);
                    let weights=&layer.w[j*2*self.o*layer.input..(j+1)*2*self.o*layer.input];
                    accumulate(&mut gate,&affine(q,rows,layer.input,weights,2*self.o,None));
                    continue;
                }
                let mut q = vec![0.; rows * layer.input];
                for b in 0..batch {
                    for t in 0..frames {
                        let s = t as isize + (j * layer.d) as isize - pad as isize;
                        if s < 0 || s >= frames as isize {
                            if !self.edge {
                                continue;
                            }
                        }
                        let s = s.clamp(0, frames as isize - 1) as usize;
                        q[(b * frames + t) * layer.input..(b * frames + t + 1) * layer.input]
                            .copy_from_slice(
                                &x[(b * frames + s) * layer.input
                                    ..(b * frames + s + 1) * layer.input],
                            );
                    }
                }
                let weights =
                    &layer.w[j * 2 * self.o * layer.input..(j + 1) * 2 * self.o * layer.input];
                let z = affine(&q, rows, layer.input, weights, 2 * self.o, None);
                accumulate(&mut gate,&z);
            }
            for row in gate.chunks_mut(2 * self.o) {
                for (v, b) in row.iter_mut().zip(&layer.b) {
                    *v += b;
                }
            }
            if let Some(p) = &layer.condition {
                let z = p.run(&cd, rows);
                accumulate(&mut gate,&z);
            }
            let mut g = vec![0.; rows * self.o];
            let activate=|(dest,row):(&mut [f32],&[f32])| {
                for ch in 0..self.o {
                    let l = row[ch];
                    let r = row[self.o + ch];
                    dest[ch] = if self.normalized {
                        (l * r) / (1. + (-r).exp())
                    } else {
                        l.tanh() / (1. + (-r).exp())
                    };
                }
            };
            if g.len()>=131072 {
                g.par_chunks_exact_mut(self.o).zip(gate.par_chunks_exact(2*self.o)).for_each(activate);
            } else {g.chunks_exact_mut(self.o).zip(gate.chunks_exact(2*self.o)).for_each(activate);}
            if let Some(p) = &layer.pruned {
                let width = self.o & !1;
                let mut trimmed = Vec::with_capacity(rows * width);
                for row in g.chunks(self.o) {
                    trimmed.extend(&row[..width]);
                }
                let (re, im) = fourier::forward_rows(&trimmed, width);
                let bins = width / 2 + 1;
                let mut packed = Vec::with_capacity(rows * (width + 2 + self.c));
                for row in 0..rows {
                    for j in 0..bins {
                        packed.push(re[row * bins + j]);
                        packed.push(im[row * bins + j]);
                    }
                    packed.extend(&cd[row * self.c..(row + 1) * self.c]);
                }
                let coefficients = p.run(&packed, rows);
                let mut product_re = vec![0.; re.len()];
                let mut product_im = product_re.clone();
                for row in 0..rows {
                    for j in 0..bins {
                        let i = row * bins + j;
                        let a = coefficients[row * (width + 2) + 2 * j];
                        let b = coefficients[row * (width + 2) + 2 * j + 1];
                        product_re[i] = re[i] * a - im[i] * b;
                        product_im[i] = re[i] * b + im[i] * a;
                    }
                }
                let correction = fourier::inverse_rows(&product_re, &product_im, width);
                for row in 0..rows {
                    for j in 0..width {
                        g[row * self.o + j] += correction[row * width + j];
                    }
                }
            }
            let skip = layer.skip.run(&g, rows);
            accumulate(&mut y,&skip);
            state = if let Some(p) = &layer.residual {
                let r = p.run(&original, rows);
                g.iter().zip(r).map(|(g, r)| g + r).collect()
            } else {
                g
            };
        }
        Ok(Tensor::new(vec![batch, frames, self.h], y))
    }
}
