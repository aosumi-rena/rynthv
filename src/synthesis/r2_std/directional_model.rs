// SPDX-License-Identifier: MIT
use super::{Error, Result, archive::Reader};
use std::collections::VecDeque;

pub(super) struct Matrix {
    rows: usize,
    columns: usize,
    weights: Vec<f32>,
    bias: Vec<f32>,
}
impl Matrix {
    fn read(r: &mut Reader<'_>, rows: usize, columns: usize, bias: usize) -> Result<Self> {
        Ok(Self {
            rows,
            columns,
            weights: floats(r, rows * columns)?,
            bias: floats(r, bias)?,
        })
    }
    pub fn multiply(&self, x: &[f32], stem: bool) -> Vec<f32> {
        debug_assert_eq!(x.len(), self.columns);
        let width = if stem || self.columns >= 128 {
            16
        } else {
            self.columns
        };
        let mut y = vec![0.0; self.rows];
        for start in (0..self.columns).step_by(width) {
            let mut part = vec![0.0; self.rows];
            for c in start..(start + width).min(self.columns) {
                for row in 0..self.rows {
                    part[row] += self.weights[c * self.rows + row] * x[c];
                }
            }
            for row in 0..self.rows {
                y[row] += part[row];
            }
        }
        y
    }
    fn add_bias(&self, y: &mut [f32]) {
        for (y, b) in y.iter_mut().zip(&self.bias) {
            *y += b;
        }
    }
    pub fn affine(&self, x: &[f32]) -> Vec<f32> {
        let mut y = self.multiply(x, false);
        self.add_bias(&mut y);
        y
    }
}
pub(super) struct Stem {
    taps: Vec<Matrix>,
    bias: Vec<f32>,
    history: VecDeque<Vec<f32>>,
}
impl Stem {
    fn read(r: &mut Reader<'_>, rows: usize, columns: usize, taps: usize) -> Result<Self> {
        expect(r, rows)?;
        expect(r, columns)?;
        expect(r, taps)?;
        let bias = shape(r, true)?;
        if bias != rows {
            return Err(Error::Format("directional stem bias"));
        }
        let matrices = (0..taps)
            .map(|_| Matrix::read(r, rows, columns, 0))
            .collect::<Result<_>>()?;
        Ok(Self {
            taps: matrices,
            bias: floats(r, bias)?,
            history: (0..taps - 1).map(|_| vec![0.0; columns]).collect(),
        })
    }
    pub fn push(&mut self, x: Vec<f32>) -> Result<Vec<f32>> {
        let mut y = self.bias.clone();
        for (t, m) in self.taps.iter().enumerate() {
            let term = m.multiply(
                if t < self.history.len() {
                    &self.history[t]
                } else {
                    &x
                },
                true,
            );
            for (a, b) in y.iter_mut().zip(term) {
                *a += b;
            }
        }
        finite(&y)?;
        self.history.pop_front();
        self.history.push_back(x);
        relu(&mut y);
        Ok(y)
    }
}
pub(super) struct Pair {
    first: Matrix,
    second: Matrix,
    history: VecDeque<Vec<f32>>,
}
impl Pair {
    fn read(
        r: &mut Reader<'_>,
        rows: usize,
        columns: usize,
        history: usize,
        reverse: bool,
    ) -> Result<Self> {
        expect(r, rows)?;
        expect(r, columns)?;
        expect(r, history)?;
        let bias = shape(r, true)?;
        if bias != 0 && bias != rows {
            return Err(Error::Format("directional pair bias"));
        }
        let a = Matrix::read(r, rows, columns, bias)?;
        let b = Matrix::read(r, rows, columns, bias)?;
        let (first, second) = if reverse { (b, a) } else { (a, b) };
        Ok(Self {
            first,
            second,
            history: (0..history).map(|_| vec![0.0; columns]).collect(),
        })
    }
    pub fn push(&mut self, x: &[f32]) -> Vec<f32> {
        let mut y = self.first.affine(x);
        let past = self.second.multiply(&self.history[0], false);
        for (a, b) in y.iter_mut().zip(past) {
            *a += b;
        }
        self.second.add_bias(&mut y);
        self.history.pop_front();
        self.history.push_back(x.to_vec());
        y
    }
}
pub(super) struct Block {
    pub pair: Pair,
    pub condition: Matrix,
    pub skip: Matrix,
    pub residual: Matrix,
}
pub(super) struct Model {
    pub lower: Vec<f32>,
    pub range: Vec<f32>,
    pub embedding: Vec<f32>,
    pub stems: Vec<Stem>,
    pub work: Pair,
    pub blocks: Vec<Block>,
    pub seed: Matrix,
    pub heads: [Matrix; 2],
}
impl Model {
    pub fn read(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        for n in [177, 192, 256, 70, 64, 32, 9, 1, 2, 4, 1, 2, 4, 1, 2, 4] {
            expect(&mut r, n)?;
        }
        let lower = floats(&mut r, 72)?;
        let upper = floats(&mut r, 72)?;
        let range: Vec<_> = upper.iter().zip(&lower).map(|(a, b)| a - b).collect();
        if range.iter().any(|x| !x.is_finite() || *x == 0.0) {
            return Err(Error::Format("directional physical domain"));
        }
        let embedding = floats(&mut r, 32)?;
        expect(&mut r, 3)?;
        let stems = vec![
            Stem::read(&mut r, 192, 177, 5)?,
            Stem::read(&mut r, 192, 192, 3)?,
            Stem::read(&mut r, 64, 192, 5)?,
        ];
        let work = Pair::read(&mut r, 192, 70, 1, true)?;
        let mut blocks = Vec::new();
        for history in [2, 4, 1, 2, 4, 1, 2, 4] {
            blocks.push(Block {
                pair: Pair::read(&mut r, 384, 192, history, false)?,
                condition: affine(&mut r, 384, 96)?,
                skip: affine(&mut r, 256, 192)?,
                residual: affine(&mut r, 192, 192)?,
            });
        }
        let seed = affine(&mut r, 256, 96)?;
        let heads = [affine(&mut r, 256, 256)?, affine(&mut r, 280, 256)?];
        r.end()?;
        Ok(Self {
            lower,
            range,
            embedding,
            stems,
            work,
            blocks,
            seed,
            heads,
        })
    }
}
fn affine(r: &mut Reader<'_>, rows: usize, columns: usize) -> Result<Matrix> {
    expect(r, rows)?;
    expect(r, columns)?;
    let bias = shape(r, true)?;
    if bias != 0 && bias != rows {
        return Err(Error::Format("directional affine bias"));
    }
    Matrix::read(r, rows, columns, bias)
}
fn shape(r: &mut Reader<'_>, zero: bool) -> Result<usize> {
    let x = r.f32()?;
    if !x.is_finite() || x < if zero { 0.0 } else { 1.0 } || x >= 2147483648.0 || x.trunc() != x {
        return Err(Error::Format("directional model shape"));
    }
    Ok(x as usize)
}
fn expect(r: &mut Reader<'_>, n: usize) -> Result<()> {
    if shape(r, false)? != n {
        return Err(Error::Unsupported("directional model geometry"));
    }
    Ok(())
}
pub(super) fn floats(r: &mut Reader<'_>, n: usize) -> Result<Vec<f32>> {
    if n > r.remaining() / 4 {
        return Err(Error::Format("truncated directional model"));
    }
    let x = (0..n).map(|_| r.f32()).collect::<Result<Vec<_>>>()?;
    finite(&x)?;
    Ok(x)
}
pub(super) fn finite(x: &[f32]) -> Result<()> {
    if x.iter().any(|x| !x.is_finite()) {
        Err(Error::Format("nonfinite directional inference"))
    } else {
        Ok(())
    }
}
pub(super) fn relu(x: &mut [f32]) {
    for x in x {
        *x = if *x > 0.0 { *x } else { 0.0 };
    }
}
