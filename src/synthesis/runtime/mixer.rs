pub(super) mod live;
mod recordings;
pub(super) use live::LiveMixer;
pub(super) use recordings::{RecordingMix, recordings};

use crate::{
    model::Extra,
    synthesis::{
        audio::{AudioLayer, ChannelLayout, RenderedAudio},
        plan::{Coverage, OwnerId},
        render_cache::PublishedOwner,
    },
};
use std::{collections::{BTreeMap, BTreeSet}, ops::Range, sync::Arc};

const NATIVE_SAMPLE_RATE: u32 = 48_000;

pub(super) fn append_instruments(mix:&mut RecordingMix,project:&crate::model::Project,instruments:&crate::synthesis::instruments::Instruments,solo:&BTreeSet<u64>)->Result<(),String>{
    for (track_id,output) in &instruments.mixer.tracks{
        let Some(track)=project.tracks.iter().find(|t|t.id==*track_id) else{continue};
        if let Some(error)=&output.error {return Err(error.clone());}
        let controls=Controls::read_instrument(&track.mixer)?;
        if controls.mute||(!solo.is_empty()&&!solo.contains(&track.id)){continue}
        for audio in &output.parts {
        let frames=audio.frames();let start=(audio.start_seconds()*48000.0).round() as i64;
        let (left,right)=controls.gains(audio.layout());
        let layer=AudioLayer{track_id:track.id,audio:Arc::clone(audio),start_frame:start,source_start:0,frames,gains:[left as f32,right as f32]};
        mix.end=mix.end.max(layer.end_frame());mix.layers.push(layer);
        }
        if let Some(range)=&output.pending {mix.pending.push(range.clone());mix.end=mix.end.max(range.end);}
    }
    Ok(())
}

struct Controls {
    mute: bool,
    solo: bool,
    gain: f64,
    pan: f64,
}

struct Span<'a> {
    audio: &'a RenderedAudio,
    output_start: usize,
    source_start: usize,
    frames: usize,
    left: f64,
    right: f64,
}

pub(super) fn mix(
    owners: &[PublishedOwner],
    solo_tracks: &BTreeSet<u64>,
) -> Result<Option<Arc<RenderedAudio>>, String> {
    mix_with_recordings(owners, solo_tracks, &[])
}

pub(super) fn mix_with_recordings(
    owners: &[PublishedOwner],
    solo_tracks: &BTreeSet<u64>,
    recordings: &[AudioLayer],
) -> Result<Option<Arc<RenderedAudio>>, String> {
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(owners.len())
        .map_err(|error| format!("无法分配混音片段：{error}"))?;
    let mut output_frames = 0;
    for owner in owners {
        if owner.controls.rocoflex || has_effects(&owner.controls.mixer) {
            continue;
        }
        if owner.audio.sample_rate() != NATIVE_SAMPLE_RATE {
            return Err(format!(
                "音符组 {} 的 PCM 采样率为 {} Hz，混音要求所有音符组使用 48000 Hz",
                owner.owner.0,
                owner.audio.sample_rate()
            ));
        }
        let controls = Controls::read(&owner.controls.mixer)
            .map_err(|error| format!("音轨 {}：{error}", owner.controls.track_id))?;
        if controls.solo != solo_tracks.contains(&owner.controls.track_id) {
            return Err(format!(
                "音轨 {} 的独奏状态与项目混音快照不一致",
                owner.controls.track_id
            ));
        }
        if owner.controls.reference_muted
            || controls.mute
            || (!solo_tracks.is_empty() && !controls.solo)
        {
            continue;
        }
        let Some(span) = prepare_span(owner, controls)? else {
            continue;
        };
        output_frames = output_frames.max(
            span.output_start
                .checked_add(span.frames)
                .ok_or("混音结束位置溢出")?,
        );
        spans.push(span);
    }
    for layer in recordings {
        let skipped = if layer.start_frame < 0 {
            layer.start_frame.unsigned_abs().min(layer.frames as u64) as usize
        } else { 0 };
        let source_start = layer.source_start + skipped;
        let frames = layer.frames - skipped;
        if frames == 0 { continue; }
        output_frames = output_frames.max(layer.end_frame());
        spans.push(Span {
            audio: &layer.audio,
            output_start: layer.start_frame.max(0) as usize,
            source_start,
            frames,
            left: f64::from(layer.gains[0]),
            right: f64::from(layer.gains[1]),
        });
    }
    if spans.is_empty() {
        return Ok(None);
    }
    let sample_count = output_frames.checked_mul(2).ok_or("混音样本数量溢出")?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(sample_count)
        .map_err(|error| format!("无法分配立体声混音缓冲区：{error}"))?;
    samples.resize(sample_count, 0.0_f32);
    for span in spans {
        let channels = span.audio.layout().channels();
        let source = &span.audio.samples()
            [span.source_start * channels..(span.source_start + span.frames) * channels];
        let destination =
            &mut samples[span.output_start * 2..(span.output_start + span.frames) * 2];
        for (input, output) in source
            .chunks_exact(channels)
            .zip(destination.as_chunks_mut::<2>().0)
        {
            let right = input[channels - 1];
            output[0] = (f64::from(output[0]) + f64::from(input[0]) * span.left) as f32;
            output[1] = (f64::from(output[1]) + f64::from(right) * span.right) as f32;
        }
    }
    RenderedAudio::new(NATIVE_SAMPLE_RATE, 0.0, ChannelLayout::Stereo, samples)
        .map(Arc::new)
        .map(Some)
        .map_err(|error| format!("混音输出无效：{error}"))
}

pub(super) fn has_effects(mixer:&Extra)->bool {
    mixer.get("fxParams").and_then(|v|crate::synthesis::effects::Configuration::read(v).ok()).is_some_and(|v|v.enabled())
}

fn prepare_span(owner: &PublishedOwner, controls: Controls) -> Result<Option<Span<'_>>, String> {
    let coverage = owner.coverage;
    if coverage.sample_rate == 0 || coverage.sample_end.0 < coverage.sample_start.0 {
        return Err(format!("音符组 {} 的音频覆盖范围无效", owner.owner.0));
    }

    let first = native_bound(coverage.sample_start.0, coverage.sample_rate)?;
    let last = native_bound(coverage.sample_end.0, coverage.sample_rate)?;
    let origin = (owner.audio.start_seconds() * f64::from(NATIVE_SAMPLE_RATE)).round();

    if !origin.is_finite() || origin < 0.0 || origin >= isize::MAX as f64 {
        return Err(format!(
            "音符组 {} 的 PCM 开始位置超出地址范围",
            owner.owner.0
        ));
    }
    let origin = origin as usize;
    let end = origin
        .checked_add(owner.audio.frames())
        .ok_or("音符组 PCM 结束位置溢出")?;
    let output_start = first.max(origin);
    let output_end = last.min(end);
    if output_end <= output_start {
        return Ok(None);
    }
    let (left, right) = controls.gains(owner.audio.layout());
    Ok(Some(Span {
        audio: &owner.audio,
        output_start,
        source_start: output_start - origin,
        frames: output_end - output_start,
        left,
        right,
    }))
}

pub(super) fn morph_range(coverage: Coverage) -> Result<Range<usize>, String> {
    if coverage.sample_rate == 0 || coverage.sample_end.0 < coverage.sample_start.0 {
        return Err("Invalid Rocoflex source coverage".into());
    }
    Ok(native_bound(coverage.sample_start.0, coverage.sample_rate)?
        ..native_bound(coverage.sample_end.0, coverage.sample_rate)?)
}

pub(super) struct MorphSpan {
    pub end: usize,
    pub start: usize,
    offset: usize,
    audio: Arc<RenderedAudio>,
}

impl MorphSpan {
    pub fn clipped(&self, start: usize, end: usize) -> Self {
        Self { start, end, offset: self.offset + start - self.start, audio: Arc::clone(&self.audio) }
    }
}

pub(super) fn morph_span(audio: &Arc<RenderedAudio>, coverage: Range<usize>) -> Option<MorphSpan> {
    if audio.sample_rate() != NATIVE_SAMPLE_RATE { return None; }
    let origin = (audio.start_seconds() * f64::from(NATIVE_SAMPLE_RATE)).round() as usize;
    let end = origin.checked_add(audio.frames())?.min(coverage.end);
    let start = origin.max(coverage.start);
    (start < end).then(|| MorphSpan { start, end, offset: start - origin, audio: Arc::clone(audio) })
}

pub(super) fn mix_morph(spans: &BTreeMap<(OwnerId, usize), MorphSpan>, range: Range<usize>, samples: &mut [f32]) -> Result<(), String> {
    for span in spans.values() {
        let first = span.start.max(range.start);
        let end = span.end.min(range.end);
        if end <= first { continue; }
        let channels = span.audio.layout().channels();
        let source = span.offset + first - span.start;
        let input = &span.audio.samples()[source * channels..(source + end - first) * channels];
        for (input, output) in input.chunks_exact(channels).zip(&mut samples[first - range.start..end - range.start]) {
            *output += if channels == 1 { input[0] } else { (input[0] + input[1]) * 0.5 };
        }
    }
    Ok(())
}

pub(super) fn morph_gains(mixer: &Extra, solo: &BTreeSet<u64>) -> Option<[f32; 2]> {
    let controls = Controls::read(mixer).ok()?;
    if controls.mute || !solo.is_empty() && !controls.solo { return None; }
    let (left, right) = controls.gains(ChannelLayout::Mono);
    Some([left as f32, right as f32])
}

pub(super) fn morph_layer(track_id: u64, gains: [f32; 2], audio: Arc<RenderedAudio>) -> AudioLayer {
    AudioLayer {
        track_id, start_frame: (audio.start_seconds() * f64::from(NATIVE_SAMPLE_RATE)).round() as i64,
        source_start: 0, frames: audio.frames(), audio, gains,
    }
}

impl Controls {
    fn gains(&self, layout: ChannelLayout) -> (f64, f64) {
        let (left, right) = match layout {
            ChannelLayout::Mono => {
                let angle = (self.pan + 1.0) * std::f64::consts::FRAC_PI_4;
                (angle.cos(), angle.sin())
            }
            ChannelLayout::Stereo if self.pan < 0.0 => {
                (1.0, (self.pan * std::f64::consts::FRAC_PI_2).cos())
            }
            ChannelLayout::Stereo => ((self.pan * std::f64::consts::FRAC_PI_2).cos(), 1.0),
        };
        (self.gain * left, self.gain * right)
    }
}

fn native_bound(project_sample: i64, project_rate: u32) -> Result<usize, String> {
    let numerator = i128::from(project_sample.max(0)) * i128::from(NATIVE_SAMPLE_RATE);
    let denominator = i128::from(project_rate);
    let frame = (numerator + denominator - 1) / denominator;
    usize::try_from(frame).map_err(|_| "音符组覆盖范围超出地址范围".into())
}

impl Controls {
    fn read_instrument(extra: &Extra) -> Result<Self, String> {
        crate::synthesis::mixing_options::validate_instrument_mixer(extra)?;
        Self::values(extra)
    }
    fn read(extra: &Extra) -> Result<Self, String> {
        crate::synthesis::mixing_options::validate_mixer(extra)?;
        Self::values(extra)
    }
    fn values(extra: &Extra) -> Result<Self, String> {
        let mute = boolean(extra, "mute")?;
        let solo = boolean(extra, "solo")?;
        let decibels = number(extra, "gainDecibel")?;
        let pan = number(extra, "pan")?;
        if !(-1.0..=1.0).contains(&pan) {
            return Err("声像 pan 必须在 -1 到 1 之间".into());
        }
        let gain = 10.0_f64.powf(decibels / 20.0);
        if !gain.is_finite() {
            return Err("gainDecibel 产生了非有限增益".into());
        }
        Ok(Self {
            mute,
            solo,
            gain,
            pan,
        })
    }
}

fn boolean(extra: &Extra, field: &str) -> Result<bool, String> {
    extra
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| format!("混音字段 {field} 缺失或不是布尔值"))
}

fn number(extra: &Extra, field: &str) -> Result<f64, String> {
    extra
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("混音字段 {field} 缺失或不是有限数值"))
}
