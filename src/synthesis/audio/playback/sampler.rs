use crate::synthesis::audio::PlaybackSource;
use crate::synthesis::audio::live::{LIVE_BLOCK_FRAMES, LiveAudio, LiveBlock};
use crate::synthesis::audio::metronome::MetronomeSampler;
use std::sync::RwLockReadGuard;

type Block<'a> = (usize, RwLockReadGuard<'a, LiveBlock>);

pub(super) struct Sampler<'a> {
    source: &'a LiveAudio,
    current: Option<Block<'a>>,
    next: Option<Block<'a>>,
}

impl<'a> Sampler<'a> {
    pub fn new(source: &'a LiveAudio) -> Self {
        Self {
            source,
            current: None,
            next: None,
        }
    }

    pub fn sample(&mut self, position: f64) -> Option<[f32; 2]> {
        let left = position.floor() as usize;
        if left >= self.source.total_frames() {
            return Some([0.0; 2]);
        }
        let index = left / LIVE_BLOCK_FRAMES;
        if self
            .current
            .as_ref()
            .is_none_or(|(block, _)| *block != index)
        {
            self.current = match self.next.take() {
                Some(next) if next.0 == index => Some(next),
                _ => Some((index, self.source.block(index)?)),
            };
        }
        let a = frame(&self.current.as_ref()?.1, left % LIVE_BLOCK_FRAMES)?;
        let fraction = position.fract();
        if fraction == 0.0 || left + 1 == self.source.total_frames() {
            return Some(a);
        }
        let right = left + 1;
        let b = if right / LIVE_BLOCK_FRAMES == index {
            frame(&self.current.as_ref()?.1, right % LIVE_BLOCK_FRAMES)?
        } else {
            if self.next.is_none() {
                self.next = Some((index + 1, self.source.block(index + 1)?));
            }
            frame(&self.next.as_ref()?.1, 0)?
        };
        Some(std::array::from_fn(|channel| {
            (f64::from(a[channel]) * (1.0 - fraction) + f64::from(b[channel]) * fraction) as f32
        }))
    }
}

pub(super) struct MixSampler<'a> {
    source: &'a PlaybackSource,
    vocals: smallvec::SmallVec<[Sampler<'a>; 8]>,
    metronome: Option<MetronomeSampler<'a>>,
}

impl<'a> MixSampler<'a> {
    pub fn new(source: &'a PlaybackSource) -> Self {
        Self {
            source,
            vocals: source.vocals.iter().map(|v| Sampler::new(v)).collect(),
            metronome: source.metronome.as_deref().map(MetronomeSampler::new),
        }
    }

    pub fn sample(&mut self, position: f64) -> Option<[f32; 2]> {
        let frame = position.floor() as usize;
        if self.source.pending.iter().any(|r| r.contains(&frame)) {
            return None;
        }
        let mut output = [0.0; 2];
        for vocals in &mut self.vocals {
            let sample = vocals.sample(position)?;
            output[0] += sample[0];
            output[1] += sample[1];
        }
        for layer in &self.source.layers {
            let sample = layer.sample(position);
            output[0] += sample[0];
            output[1] += sample[1];
        }
        for layer in &self.source.morphs {
            let sample = layer.sample(position)?;
            output[0] += sample[0];
            output[1] += sample[1];
        }
        if let Some(metronome) = &mut self.metronome {
            let click = metronome.sample(position);
            output[0] += click;
            output[1] += click;
        }
        Some(output)
    }
}

fn frame(block: &LiveBlock, frame: usize) -> Option<[f32; 2]> {
    match block {
        LiveBlock::Pending => None,
        LiveBlock::Silence => Some([0.0; 2]),
        LiveBlock::Pcm(samples) => Some([samples[frame * 2], samples[frame * 2 + 1]]),
    }
}
