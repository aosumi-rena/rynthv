use rynthv::synthesis::{audio::RenderedAudio, rendered::RenderedClip};
use std::sync::Arc;

/// The offline tool publishes the same phrase clips as the editor. Real gaps
/// have zero PCM; no resynthesis, crossfade, gain or phrase-clock rounding fit.
pub fn join(clips: &[Arc<RenderedClip>]) -> Result<RenderedAudio, String> {
    let first = clips.first().ok_or("No STD audio")?;
    let rate = first.audio.sample_rate();
    let origin = first.audio.start_seconds();
    let layout = first.audio.layout();
    let channels = layout.channels();
    let origin_frame = (origin * f64::from(rate)).round();
    let mut samples: Vec<f32> = Vec::new();
    for clip in clips {
        let audio = &clip.audio;
        if audio.sample_rate() != rate || audio.layout() != layout {
            return Err("STD phrase output formats disagree".into());
        }
        // Match the live mixer: round absolute clocks independently, rather
        // than round a fractional difference from the first clip's origin.
        let frame = (audio.start_seconds() * f64::from(rate)).round() - origin_frame;
        if !frame.is_finite() || frame < 0.0 || frame > 32.0 * 1024.0 * 1024.0 {
            return Err("STD phrase output clock exceeds capacity".into());
        }
        let start = frame as usize * channels;
        let end = start.checked_add(audio.samples().len()).filter(|end| *end <= 32 * 1024 * 1024)
            .ok_or("STD whole-part audio exceeds capacity")?;
        let previous_end = samples.len();
        samples.resize(previous_end.max(end), 0.0);
        // Resampling uses ceil(output length), so adjacent fractional source
        // clocks can share an output sample. The editor mixer sums that sample.
        let overlap_end = previous_end.min(end).max(start);
        for (target, value) in samples[start..overlap_end].iter_mut().zip(audio.samples()) {
            *target += *value;
        }
        samples[overlap_end..end].copy_from_slice(&audio.samples()[overlap_end-start..]);
    }
    RenderedAudio::new(rate, origin, layout, samples).map_err(|error| error.to_string())
}
