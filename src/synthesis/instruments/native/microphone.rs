use super::{Result, archive::Archive, room_response};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use rayon::prelude::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
pub struct Microphones {
    width: usize,
    fft: usize,
    block_size: usize,
    filters: Vec<Vec<Complex<f64>>>,
    tails: Vec<Vec<f64>>,
    input: Vec<f32>,
    forward: Arc<dyn Fft<f64>>,
    inverse: Arc<dyn Fft<f64>>,
    source_frames: usize,
    pub emitted: usize,
}
impl Microphones {
    pub fn from_path(path: &std::path::Path, selected: &[super::super::microphones::Mix],
        accept_any_v4_signature: bool) -> Result<Self> {
        let archive=Archive::open(path, accept_any_v4_signature)?;
        Self::new(&archive, selected)
    }
    pub fn from_room(path: &std::path::Path, selected: &[super::super::microphones::Mix],
        room: Option<&str>, pack: Option<&std::path::Path>, accept_any_v4_signature: bool) -> Result<Self> {
        let archive=Archive::open(path, accept_any_v4_signature)?;
        Self::from_response(room_response::prepare(&archive, selected, room, pack)?)
    }
    pub fn new(a: &Archive, selected: &[super::super::microphones::Mix]) -> Result<Self> {
        Self::from_response(room_response::prepare(a, selected, None, None)?)
    }
    fn from_response(response: room_response::Response) -> Result<Self> {
        let width=response.width;
        let fft=(2*width-1).next_power_of_two().max(65536);
        let mut planner = FftPlanner::<f64>::new();
        let forward = planner.plan_fft_forward(fft);
        let inverse = planner.plan_fft_inverse(fft);
        let filters = response.channels
            .into_iter()
            .map(|r| {
                let mut v = vec![Complex::new(0., 0.); fft];
                for (x, r) in v.iter_mut().zip(r) {
                    x.re = r as f64;
                }
                forward.process(&mut v);
                v
            })
            .collect();
        Ok(Self {
            width,
            fft,
            block_size: fft-width+1,
            filters,
            tails: vec![vec![0.; width - 1]; 8],
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
        while self.input.len() >= self.block_size * 4 {
            self.block(self.block_size, false, cancel, emit)?;
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
        let width=self.width;
        let fft=self.fft;
        let count = if last { n + width - 1 } else { n };
        let mut pcm = vec![0f32; count * 2];
        let input = &self.input;
        let filters = &self.filters;
        let forward = &self.forward;
        let inverse = &self.inverse;
        let channels = self.tails.par_chunks_mut(2).enumerate().map(|(c, tails)| {
            if cancel.load(Ordering::Relaxed) {
                return Err(String::from("Instrument render cancelled"));
            }
            let mut source = vec![Complex::new(0., 0.); fft];
            for j in 0..n {
                source[j].re = input[j * 4 + c] as f64;
            }
            forward.process(&mut source);
            let mut sides = [Vec::new(), Vec::new()];
            for side in 0..2 {
                let index = c * 2 + side;
                let mut spectrum: Vec<_> = source
                    .iter()
                    .zip(&filters[index])
                    .map(|(a, b)| a * b)
                    .collect();
                inverse.process(&mut spectrum);
                let mut convolution: Vec<_> = spectrum
                    .iter()
                    .take(n + width - 1)
                    .map(|v| v.re / fft as f64)
                    .collect();
                for (value, tail) in convolution.iter_mut().zip(&tails[side]) {
                    *value += tail;
                }
                tails[side].copy_from_slice(&convolution[n..n + width - 1]);
                convolution.truncate(count);
                sides[side] = convolution;
            }
            Ok(sides)
        }).collect::<Result<Vec<_>>>()?;
        for sides in channels {
            for side in 0..2 {
                for j in 0..count { pcm[j * 2 + side] += sides[side][j] as f32; }
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
