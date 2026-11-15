mod buffer;
mod clock;
mod plan;
mod segment;

pub use plan::SegmentPlan;
pub use segment::Segment;

use super::super::PcmComponents;
use crate::synthesis::{
    audio::{ChannelLayout, RenderedAudio},
    rendered::patch::{PREVIEW_BLOCK_SAMPLES, PREVIEW_SAMPLE_RATE},
    voice::{Result, VoiceError},
};
use clock::Clock;
use std::{ops::Range, sync::Arc};

const BLOCK: i64 = PREVIEW_BLOCK_SAMPLES as i64;
const LOOKAHEAD: i64 = 1_024;
const RATE: f64 = PREVIEW_SAMPLE_RATE as f64;

pub struct PcmStore {
    clock: Clock,
    first_block: i64,
    ready: Vec<bool>,
    samples: [Vec<f32>; 2],
}

impl PcmStore {
    pub(super) fn complete(components: &PcmComponents) -> Result<Self> {
        if components.primary.len() != components.secondary.len() {
            return Err(VoiceError::new(
                "Complete PCM components have different lengths",
            ));
        }
        let mut store = Self::new(components.start_seconds, components.primary.len())?;
        store.samples = [components.primary.to_vec(), components.secondary.to_vec()];
        store.ready.fill(true);
        Ok(store)
    }

    pub fn new(start_seconds: f64, total_samples: usize) -> Result<Self> {
        let clock = Clock::new(start_seconds, total_samples)?;
        let first_block = clock.origin.div_euclid(BLOCK);
        let last_block = (clock.end - 1).div_euclid(BLOCK);
        let count = usize::try_from(last_block - first_block + 1)
            .map_err(|_| VoiceError::new("Progressive PCM block count overflow"))?;
        Ok(Self {
            clock,
            first_block,
            ready: vec![false; count],
            samples: std::array::from_fn(|_| vec![0.0; total_samples]),
        })
    }

    pub fn from_existing(base: &PcmComponents, dirty: Range<usize>) -> Result<Self> {
        if base.primary.len() != base.secondary.len() || dirty.start >= dirty.end {
            return Err(VoiceError::new("Invalid progressive PCM baseline"));
        }
        let mut store = Self::new(base.start_seconds, base.primary.len())?;
        store.samples = [base.primary.to_vec(), base.secondary.to_vec()];
        store.ready.fill(true);
        let first = i64::try_from(dirty.start)
            .map_err(|_| VoiceError::new("Progressive dirty block overflow"))?;
        let stop = i64::try_from(dirty.end)
            .map_err(|_| VoiceError::new("Progressive dirty block overflow"))?;
        for (index, ready) in store.ready.iter_mut().enumerate() {
            let block = store.first_block + index as i64;
            if (first..stop).contains(&block) {
                *ready = false;
            }
        }
        Ok(store)
    }

    pub fn is_complete(&self) -> bool {
        self.ready.iter().all(|ready| *ready)
    }

    pub fn model_samples(&self) -> Range<i64> {
        self.clock.origin..self.clock.end
    }

    pub fn needs(&self, seconds: f64) -> bool {
        if !seconds.is_finite() {
            return false;
        }
        let sample = (seconds * RATE).floor() as i64;
        sample >= self.clock.origin
            && sample < self.clock.end
            && !self.block_ready(sample.div_euclid(BLOCK))
    }

    pub fn segment(&self, plan: SegmentPlan) -> Segment {
        Segment::new(plan)
    }

    pub fn ready_audio(&self) -> Result<Vec<RenderedAudio>> {
        let mut output = Vec::new();
        let mut first = 0;
        while first < self.ready.len() {
            if !self.ready[first] {
                first += 1;
                continue;
            }
            let mut stop = first + 1;
            while stop < self.ready.len() && self.ready[stop] {
                stop += 1;
            }
            let begin = ((self.first_block + first as i64) * BLOCK).max(self.clock.origin);
            let end = ((self.first_block + stop as i64) * BLOCK).min(self.clock.end);
            if let Some(audio) = self.audio(begin..end)? {
                output.push(audio);
            }
            first = stop;
        }
        Ok(output)
    }

    pub fn into_components(self) -> Result<PcmComponents> {
        if !self.is_complete() {
            return Err(VoiceError::new(
                "Cannot finish PCM with unpublished model blocks",
            ));
        }
        let [primary, secondary] = self.samples;
        Ok(PcmComponents {
            start_seconds: self.clock.origin as f64 / RATE,
            primary: Arc::from(primary),
            secondary: Arc::from(secondary),
        })
    }

    fn block_ready(&self, block: i64) -> bool {
        usize::try_from(block - self.first_block)
            .ok()
            .and_then(|index| self.ready.get(index))
            .copied()
            .unwrap_or(false)
    }

    fn mark_ready(&mut self, range: Range<i64>) -> Result<()> {
        let first = range.start.div_euclid(BLOCK);
        let last = (range.end - 1).div_euclid(BLOCK);
        for block in first..=last {
            let complete_begin = (block * BLOCK).max(self.clock.origin);
            let complete_end = ((block + 1) * BLOCK).min(self.clock.end);
            if range.start > complete_begin || range.end < complete_end {
                return Err(VoiceError::new(
                    "PCM publication contains an incomplete block",
                ));
            }
            self.ready[(block - self.first_block) as usize] = true;
        }
        Ok(())
    }

    fn write(&mut self, range: Range<i64>, source: [&[f32]; 2]) -> Result<Option<RenderedAudio>> {
        let destination = self.clock.range(range.clone())?;
        if source
            .iter()
            .any(|channel| channel.len() != destination.len())
        {
            return Err(VoiceError::new(
                "Progressive PCM publication length mismatch",
            ));
        }
        for (output, input) in self.samples.iter_mut().zip(source) {
            output[destination.clone()].copy_from_slice(input);
        }
        self.mark_ready(range.clone())?;
        self.audio(range)
    }

    fn audio(&self, range: Range<i64>) -> Result<Option<RenderedAudio>> {
        let begin = range.start.max(0);
        if begin >= range.end {
            return Ok(None);
        }
        let source = self.clock.range(begin..range.end)?;
        let samples = self.samples[0][source.clone()]
            .iter()
            .zip(&self.samples[1][source.clone()])
            .map(|(&a, &b)| a + b)
            .collect();
        RenderedAudio::new(
            PREVIEW_SAMPLE_RATE,
            begin as f64 / RATE,
            ChannelLayout::Mono,
            samples,
        )
        .and_then(|audio|audio.with_components(self.samples[0][source.clone()].into(),self.samples[1][source].into(),0))
        .map(Some)
        .map_err(|error| VoiceError::new(error.to_string()))
    }
}
