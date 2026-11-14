mod spans;

use super::{Controls, native_bound};
use crate::synthesis::{
    audio::{
        LiveAudio, RenderedAudio,
        live::{LIVE_BLOCK_FRAMES, LIVE_CHANNELS, LIVE_SAMPLE_RATE},
    },
    plan::{OwnerId, RenderPlan},
};
use spans::ReadySpans;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    sync::Arc,
};

struct Owner {
    coverage: Range<usize>,
    controls: Controls,
    ready: ReadySpans,
    completed: Option<Arc<RenderedAudio>>,
}

pub(crate) struct LiveMixer {
    source: Option<Arc<LiveAudio>>,
    owners: BTreeMap<OwnerId, Owner>,
}

impl LiveMixer {
    pub(crate) fn new(plan: &RenderPlan, solo_tracks: &BTreeSet<u64>) -> Result<Self, String> {
        let mut owners = BTreeMap::new();
        let mut total_frames = 0;
        for (id, request) in &plan.requests {
            let coverage = request.coverage;
            if coverage.sample_rate == 0 || coverage.sample_end.0 < coverage.sample_start.0 {
                return Err(format!("音符组 {} 的音频覆盖范围无效", id.0));
            }
            let coverage = native_bound(coverage.sample_start.0, coverage.sample_rate)?
                ..native_bound(coverage.sample_end.0, coverage.sample_rate)?;
            total_frames = total_frames.max(coverage.end);
            let output = plan
                .output_controls
                .get(id)
                .ok_or_else(|| format!("音符组 {} 缺少混音控制", id.0))?;
            let controls = Controls::read(&output.mixer)
                .map_err(|error| format!("音轨 {}：{error}", output.track_id))?;
            if controls.solo != solo_tracks.contains(&output.track_id) {
                return Err(format!(
                    "音轨 {} 的独奏状态与项目混音快照不一致",
                    output.track_id
                ));
            }
            if coverage.is_empty()
                || output.rocoflex
                || output.reference_muted
                || controls.mute
                || (!solo_tracks.is_empty() && !controls.solo)
            {
                continue;
            }
            owners.insert(
                *id,
                Owner {
                    coverage,
                    controls,
                    ready: ReadySpans::default(),
                    completed: None,
                },
            );
        }
        let source = (total_frames > 0).then(|| Arc::new(LiveAudio::new(total_frames)));
        let mixer = Self { source, owners };

        mixer.publish_blocks(0..total_frames)?;
        Ok(mixer)
    }

    pub(crate) fn source(&self) -> Option<Arc<LiveAudio>> {
        self.source.clone()
    }

    pub(crate) fn publish_for(
        &mut self,
        ids: &[OwnerId],
        audio: Arc<RenderedAudio>,
    ) -> Result<(), String> {
        let mut dirty: Option<Range<usize>> = None;
        for id in ids {
            let Some(owner) = self.owners.get_mut(id) else {
                continue;
            };
            let range = owner
                .ready
                .insert_audio(Arc::clone(&audio), &owner.coverage)?;
            if !range.is_empty() {
                owner.completed = None;
                dirty = Some(match dirty {
                    Some(previous) => previous.start.min(range.start)..previous.end.max(range.end),
                    None => range,
                });
            }
        }
        match dirty {
            Some(range) => self.publish_blocks(range),
            None => Ok(()),
        }
    }

    pub(crate) fn complete(
        &mut self,
        id: OwnerId,
        audio: Arc<RenderedAudio>,
    ) -> Result<(), String> {
        let Some(owner) = self.owners.get_mut(&id) else {
            return Ok(());
        };
        if owner.completed.as_ref().is_some_and(|previous| Arc::ptr_eq(previous, &audio)) {
            return Ok(());
        }
        let range = owner.coverage.clone();
        let mut ready = ReadySpans::default();
        ready.insert_silence(range.clone());
        ready.insert_audio(Arc::clone(&audio), &range)?;
        owner.ready = ready;
        owner.completed = None;
        self.publish_blocks(range)?;
        if let Some(owner) = self.owners.get_mut(&id) {
            owner.completed = Some(audio);
        }
        Ok(())
    }

    pub(crate) fn prepared(&mut self, id: OwnerId, model: Range<i64>) -> Result<(), String> {
        let Some(owner) = self.owners.get_mut(&id) else {
            return Ok(());
        };
        if model.end < model.start {
            return Err(format!("音符组 {} 的模型采样范围无效", id.0));
        }
        let start = usize::try_from(model.start.max(0)).map_err(|_| "模型开始位置超出地址范围")?;
        let end = usize::try_from(model.end.max(0)).map_err(|_| "模型结束位置超出地址范围")?;
        let before = owner.coverage.start..owner.coverage.end.min(start);
        let after = owner.coverage.start.max(end)..owner.coverage.end;

        if !before.is_empty() || !after.is_empty() {
            owner.completed = None;
        }
        owner.ready.insert_silence(before.clone());
        owner.ready.insert_silence(after.clone());
        self.publish_blocks(before)?;
        self.publish_blocks(after)
    }

    pub(crate) fn needs(&self, id: OwnerId, seconds: f64) -> bool {
        let Some(owner) = self.owners.get(&id) else {
            return false;
        };
        let frame = frame_at(seconds);
        let start = frame / LIVE_BLOCK_FRAMES * LIVE_BLOCK_FRAMES;
        let range = intersection(
            &(start..start.saturating_add(LIVE_BLOCK_FRAMES)),
            &owner.coverage,
        );
        !range.is_empty() && owner.ready.first_missing(range).is_some()
    }

    pub(crate) fn next_needed(&self, id: OwnerId, seconds: f64) -> Option<f64> {
        let owner = self.owners.get(&id)?;
        let frame = frame_at(seconds);
        let start = (frame / LIVE_BLOCK_FRAMES * LIVE_BLOCK_FRAMES)
            .clamp(owner.coverage.start, owner.coverage.end);
        owner
            .ready
            .first_missing(start..owner.coverage.end)
            .or_else(|| owner.ready.first_missing(owner.coverage.start..start))
            .map(|frame| frame as f64 / f64::from(LIVE_SAMPLE_RATE))
    }

    fn publish_blocks(&self, dirty: Range<usize>) -> Result<(), String> {
        let Some(source) = &self.source else {
            return Ok(());
        };
        if dirty.is_empty() {
            return Ok(());
        }
        let end = dirty.end.min(source.total_frames());
        for index in dirty.start / LIVE_BLOCK_FRAMES..end.div_ceil(LIVE_BLOCK_FRAMES) {
            let start = index * LIVE_BLOCK_FRAMES;
            let frames = source.block_frames(index).ok_or("混音块超出项目范围")?;
            let range = start..start + frames;
            let relevant = self
                .owners
                .values()
                .filter(|owner| !intersection(&owner.coverage, &range).is_empty())
                .collect::<Vec<_>>();
            if relevant.iter().any(|owner| {
                owner
                    .ready
                    .first_missing(intersection(&owner.coverage, &range))
                    .is_some()
            }) {
                continue;
            }
            if relevant
                .iter()
                .all(|owner| !owner.ready.has_audio(range.clone()))
            {
                source.silence(index).map_err(|error| error.to_string())?;
                continue;
            }
            let mut samples = vec![0.0; frames * LIVE_CHANNELS];
            for owner in relevant {
                owner.ready.mix_into(&range, &owner.controls, &mut samples);
            }
            source
                .publish(index, samples.into())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

fn intersection(left: &Range<usize>, right: &Range<usize>) -> Range<usize> {
    left.start.max(right.start)..left.end.min(right.end)
}

fn frame_at(seconds: f64) -> usize {
    if seconds.is_finite() {
        (seconds.max(0.0) * f64::from(LIVE_SAMPLE_RATE)).floor() as usize
    } else {
        0
    }
}
