use super::{Result, archive::Archive, metadata, u32at};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
pub const IR: usize = 24000;
const FFT: usize = 65536;
const BLOCK: usize = FFT - IR + 1;
pub struct Microphones {
    filters: Vec<Vec<Complex<f64>>>,
    tails: Vec<Vec<f64>>,
    input: Vec<f32>,
    forward: Arc<dyn Fft<f64>>,
    inverse: Arc<dyn Fft<f64>>,
    source_frames: usize,
    pub emitted: usize,
}
impl Microphones {
    pub fn from_path(path: &std::path::Path, selected: &[super::super::microphones::Mix]) -> Result<Self> {
        let archive=Archive::open(path)?;
        Self::new(&archive, selected)
    }
    pub fn new(a: &Archive, selected: &[super::super::microphones::Mix]) -> Result<Self> {
        let (root, _) = metadata::description(a)?;
        let catalog = metadata::microphones(&root)?;
        let mut selected = selected.to_vec();
        selected.sort_by(|a,b|a.name.cmp(&b.name));
        if selected.windows(2).any(|p| p[0].name == p[1].name) {
            return Err("Duplicate instrument microphone".into());
        }
        let mut responses = vec![vec![0f32; IR]; 8];
        for channel in selected {
            let name=&channel.name;
            let left=(1.-channel.pan.max(0.))*channel.gain;
            let right=(1.+channel.pan.min(0.))*channel.gain;
            let raw = a.read(
                catalog
                    .get(name)
                    .ok_or_else(|| format!("Instrument microphone {name} absent"))?,
            )?;
            if raw.is_empty() || raw.len() % 16 != 0 {
                return Err("Invalid instrument microphone response".into());
            }
            let width = raw.len() / 16;
            for c in 0..4 {
                for j in 0..IR.min(width) {
                    let s = (j + width / 2 + width - 64 % width) % width;
                    let v = f32::from_bits(u32at(&raw, (c * width + s) * 4));
                    if !v.is_finite() {
                        return Err("Nonfinite instrument microphone response".into());
                    }
                    responses[c * 2][j] += v*left;
                    responses[c * 2 + 1][j] += v*right;
                }
            }
        }
        let mut planner = FftPlanner::<f64>::new();
        let forward = planner.plan_fft_forward(FFT);
        let inverse = planner.plan_fft_inverse(FFT);
        let filters = responses
            .into_iter()
            .map(|r| {
                let mut v = vec![Complex::new(0., 0.); FFT];
                for (x, r) in v.iter_mut().zip(r) {
                    x.re = r as f64;
                }
                forward.process(&mut v);
                v
            })
            .collect();
        Ok(Self {
            filters,
            tails: vec![vec![0.; IR - 1]; 8],
            input: Vec::new(),
            forward,
            inverse,
            source_frames: 0,
            emitted: 0,
        })
    }
    pub fn feed_mixed(
        &mut self,
        input: &[[f32; 4]],
        final_block: bool,
        cancel: &AtomicBool,
        emit: &mut impl FnMut(usize, Vec<f32>) -> Result<()>,
    ) -> Result<()> {
        self.input.extend(input.iter().flatten().copied());
        self.source_frames += input.len();
        while self.input.len() >= BLOCK * 4 {
            self.block(BLOCK, false, cancel, emit)?;
        }
        if final_block {
            self.block(self.input.len() / 4, true, cancel, emit)?;
        }
        Ok(())
    }
    fn block(
        &mut self,
        n: usize,
        last: bool,
        cancel: &AtomicBool,
        emit: &mut impl FnMut(usize, Vec<f32>) -> Result<()>,
    ) -> Result<()> {
        let count = if last { n + IR - 1 } else { n };
        let mut pcm = vec![0f32; count * 2];
        for c in 0..4 {
            if cancel.load(Ordering::Relaxed) {
                return Err("Instrument render cancelled".into());
            }
            let mut source = vec![Complex::new(0., 0.); FFT];
            for j in 0..n {
                source[j].re = self.input[j * 4 + c] as f64;
            }
            self.forward.process(&mut source);
            for side in 0..2 {
                let index = c * 2 + side;
                let mut spectrum: Vec<_> = source
                    .iter()
                    .zip(&self.filters[index])
                    .map(|(a, b)| a * b)
                    .collect();
                self.inverse.process(&mut spectrum);
                let mut convolution: Vec<_> = spectrum
                    .iter()
                    .take(n + IR - 1)
                    .map(|v| v.re / FFT as f64)
                    .collect();
                for (value, tail) in convolution.iter_mut().zip(&self.tails[index]) {
                    *value += tail;
                }
                for j in 0..count { pcm[j * 2 + side] += convolution[j] as f32; }
                self.tails[index].copy_from_slice(&convolution[n..n + IR - 1]);
            }
        }
        if pcm.iter().any(|x| !x.is_finite()) {
            return Err("Nonfinite instrument PCM".into());
        }
        emit(self.emitted, pcm)?;
        self.emitted += count;
        self.input.drain(..n * 4);
        Ok(())
    }
}
