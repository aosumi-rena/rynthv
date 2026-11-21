use super::{
    Result, WaveModel, archive::Archive, id, program::Program, render::WAVE_PROGRAM,
    score::Note, tensor::Tensor,
};
use std::sync::{Arc, atomic::AtomicBool};

pub struct WaveStream {
    main: std::path::PathBuf,
    pub(super) program: Option<Program>,
    input: Tensor,
    seeds: Tensor,
    pub pitch: Vec<f32>,
    pub controls: Vec<super::super::EffectiveNote>,
    next: usize,
    published: usize,
    waveform: Option<Arc<WaveModel>>,
    active: Vec<bool>,
}
impl WaveStream {
    pub fn new(
        main: std::path::PathBuf,
        input: Tensor,
        seeds: Tensor,
        pitch: Vec<f32>,
        waveform: Option<Arc<WaveModel>>,
        notes: &[Note],
    ) -> Result<Self> {
        let mut active = Vec::new();
        if waveform.as_ref().is_some_and(|model| model.gate.is_some()) {
            active.resize(input.shape[1], false);
            for note in notes {
                active.get_mut(note.start..note.start + note.length)
                    .ok_or("Waveform note is outside the input clock")?.fill(true);
            }
        }
        Ok(Self {
            main,
            program: None,
            input,
            seeds,
            pitch,
            controls:Vec::new(),
            next: 256,
            published: 0,
            waveform,
            active,
        })
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
            let archive = match &self.waveform {
                Some(waveform) => Arc::clone(&waveform.archive),
                None => Archive::open(&self.main)?,
            };
            let program = Program::new(archive, id(WAVE_PROGRAM), cancel.clone())?;
            if let Some(model) = &self.waveform {
                program.validate_waveform(model.gate.is_some())?;
            }
            self.program = Some(program);
        }
        let program=self.program.as_mut().unwrap();
        program.set_cancel(cancel);
        let inputs = vec![
            (900, prefix(&self.input)?),
            (910, prefix(&self.seeds)?),
        ];
        let (out, safe) = match self.waveform.as_ref().and_then(|model| model.gate.as_ref()) {
            Some(gate) => program.run_waveform(inputs, gate, &self.active)?,
            None => program.run_stable(inputs)?,
        };
        program.release_inputs();
        let primary = out.get(&931).ok_or("Instrument primary source absent")?;
        let noise = out.get(&941).ok_or("Instrument noise source absent")?;
        let until = if finished { primary.shape[1] } else { safe };
        if primary.shape != noise.shape || primary.shape.len() != 3
            || primary.shape[0] != 1 || primary.shape[2] != 4 || until < self.published
            || until > primary.shape[1] {
            return Err("Invalid instrument source stream".into());
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
