use super::{
    Result, archive::Archive, id, program::Program, render::WAVE_PROGRAM,
    tensor::Tensor,
};
use std::sync::{Arc, atomic::AtomicBool};

pub struct WaveStream {
    archive: Arc<Archive>,
    pub(super) program: Option<Program>,
    input: Tensor,
    seeds: Tensor,
    pub pitch: Vec<f32>,
    pub controls: Vec<super::super::EffectiveNote>,
    next: usize,
    published: usize,
}

impl WaveStream {
    pub fn new(archive: Arc<Archive>, input: Tensor, seeds: Tensor, pitch: Vec<f32>) -> Self {
        Self {
            archive, program: None, input, seeds, pitch, controls: Vec::new(),
            next: 256, published: 0,
        }
    }
    pub fn ready(&self) -> usize {
        self.published
    }
    pub fn advance(
        &mut self,
        cancel: Arc<AtomicBool>,
        emit: &mut impl FnMut(usize, Vec<f32>) -> Result<()>,
    ) -> Result<bool> {
        let frames = self.input.shape[1];
        let count = self.next.min(frames);
        let finished = count == frames;
        let prefix = |t: &Tensor| {
            let mut shape = t.shape.clone();
            shape[1] = count;
            t.view(shape, t.strides.clone(), 0)
        };
        if self.program.is_none(){
            self.program = Some(Program::new(self.archive.clone(), id(WAVE_PROGRAM), cancel.clone())?);
        }
        let program=self.program.as_mut().unwrap();
        program.set_cancel(cancel);
        let inputs = vec![
            (900, prefix(&self.input)?),
            (910, prefix(&self.seeds)?),
        ];
        let (out, safe) = program.run_stable(inputs)?;
        program.release_inputs();
        let primary = out.get(&931).ok_or("Instrument primary source absent")?;
        let noise = out.get(&941).ok_or("Instrument noise source absent")?;
        if primary.shape != noise.shape || primary.shape.len() != 3
            || primary.shape[0] != 1 || primary.shape[2] != 4 {
            return Err("Invalid instrument source stream".into());
        }
        let until = if finished { primary.shape[1] } else { safe };
        if until < self.published || until > primary.shape[1] {
            return Err("Invalid instrument source stream length".into());
        }
        let mut sources = Vec::with_capacity((until - self.published) * 8);
        for frame in self.published..until {
            for tensor in [primary, noise] {
                sources.extend((0..4).map(|c| tensor.get(frame * 4 + c)));
            }
        }
        if sources.iter().any(|x| !x.is_finite()) { return Err("Nonfinite instrument source".into()); }
        if !sources.is_empty() { emit(self.published, sources)?; }
        self.published = until;
        self.next = if self.next==256 {2048} else {self.next.saturating_mul(4)};
        if frames.saturating_sub(self.next) < self.next / 2 {
            self.next=frames;
        }
        Ok(finished)
    }
}
