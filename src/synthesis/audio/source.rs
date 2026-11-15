use super::{LiveAudio, RenderedAudio, metronome::Metronome};
use std::{ops::Range, sync::{Arc, RwLock}};

#[derive(Clone)]
pub struct AudioLayer {
    pub track_id: u64,
    pub audio: Arc<RenderedAudio>,

    pub start_frame: i64,
    pub source_start: usize,
    pub frames: usize,
    pub gains: [f32; 2],
}

impl AudioLayer {
    pub fn end_frame(&self) -> usize {
        (i128::from(self.start_frame) + self.frames as i128).clamp(0, usize::MAX as i128) as usize
    }

    pub(crate) fn sample(&self, position: f64) -> [f32; 2] {
        if position < self.start_frame as f64 || position >= self.end_frame() as f64 {
            return [0.0; 2];
        }
        let local = position - self.start_frame as f64;
        let a = self.source_start + local.floor() as usize;
        let b = (a + 1).min(self.source_start + self.frames - 1);
        let channels = self.audio.layout().channels();
        let fraction = local.fract() as f32;
        std::array::from_fn(|c| {
            let channel = c.min(channels - 1);
            let first = self.audio.samples()[a * channels + channel];
            let last = self.audio.samples()[b * channels + channel];
            (first + (last - first) * fraction) * self.gains[c]
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct MorphLayer {
    data: Arc<RwLock<MorphAudio>>,
}

#[derive(Default)]
struct MorphAudio {
    pending: Range<usize>,
    parts: Vec<AudioLayer>,
}

impl MorphLayer {
    pub(crate) fn reset(&self, pending: Range<usize>) {
        if let Ok(mut data) = self.data.write() {
            data.parts.clear();
            data.pending = pending;
        }
    }

    pub(crate) fn publish(&self, parts: impl IntoIterator<Item = AudioLayer>, ready: usize) {
        if let Ok(mut data) = self.data.write() {
            data.parts.extend(parts);
            data.pending.start = ready.min(data.pending.end);
        }
    }

    pub(crate) fn gains(&self, gains: [f32; 2]) {
        if let Ok(mut data) = self.data.write() {
            for part in &mut data.parts { part.gains = gains; }
        }
    }

    fn end_frame(&self) -> usize {
        self.data.read().ok().map_or(0, |data| {
            data.pending.end.max(data.parts.last().map_or(0, AudioLayer::end_frame))
        })
    }

    pub(super) fn sample(&self, position: f64) -> Option<[f32; 2]> {
        let data = self.data.try_read().ok()?;
        let frame = position.floor() as i64;
        if data.pending.contains(&(frame as usize)) { return None; }
        let index = data.parts.partition_point(|part| part.end_frame() as i64 <= frame);
        let Some(part) = data.parts.get(index).filter(|part| part.start_frame <= frame) else { return Some([0.0; 2]); };
        let fraction = position.fract() as f32;
        if fraction > 0.0 && frame + 1 == part.end_frame() as i64 {
            if data.pending.contains(&((frame + 1) as usize)) { return None; }
            if let Some(next) = data.parts.get(index + 1).filter(|next| next.start_frame == frame + 1) {
                let channels=part.audio.layout().channels();let next_channels=next.audio.layout().channels();
                return Some(std::array::from_fn(|c|{
                    let a=part.audio.samples()[(part.source_start+part.frames-1)*channels+c.min(channels-1)];
                    let b=next.audio.samples()[next.source_start*next_channels+c.min(next_channels-1)];
                    (a+(b-a)*fraction)*part.gains[c]
                }));
            }
        }
        Some(part.sample(position))
    }
}

pub struct PlaybackSource {
    pub(crate) metronome: Option<Arc<Metronome>>,
    pub(crate) vocals: Vec<Arc<LiveAudio>>,
    pub(crate) layers: Vec<AudioLayer>,
    pub(crate) morphs: Vec<MorphLayer>,
    pub(crate) pending: Vec<Range<usize>>,
    total_frames: usize,
}

impl PlaybackSource {
    pub fn new(
        vocals: Vec<Arc<LiveAudio>>,
        layers: Vec<AudioLayer>,
        pending: Vec<Range<usize>>,
        end: usize,
    ) -> Self {
        let total_frames = end
            .max(vocals.iter().map(|v| v.total_frames()).max().unwrap_or(0))
            .max(layers.iter().map(AudioLayer::end_frame).max().unwrap_or(0));
        Self {
            metronome: None,
            vocals,
            layers,
            morphs: Vec::new(),
            pending,
            total_frames,
        }
    }

    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    pub(crate) fn with_morphs(mut self, morphs: Vec<MorphLayer>) -> Self {
        self.total_frames = self.total_frames.max(morphs.iter().map(MorphLayer::end_frame).max().unwrap_or(0));
        self.morphs = morphs;
        self
    }

    pub fn with_metronome(mut self, metronome: Arc<Metronome>) -> Self {
        self.metronome = Some(metronome);
        self
    }
}
