use super::{AudioError, ChannelLayout, RenderedAudio};
mod kernels;
pub(crate) mod stream;

pub(crate) fn convert(
    audio: &RenderedAudio,
    rate: u32,
    layout: ChannelLayout,
) -> Result<RenderedAudio, AudioError> {
    convert_with_checkpoint(audio, rate, layout, || Ok(()))
}

/// Converts PCM, returning immediately if a checkpoint fails. Partial output
/// remains private to this call and is discarded on failure.
pub(crate) fn convert_with_checkpoint(
    audio: &RenderedAudio,
    rate: u32,
    layout: ChannelLayout,
    mut checkpoint: impl FnMut() -> Result<(), AudioError>,
) -> Result<RenderedAudio, AudioError> {
    checkpoint()?;
    let channels = layout.channels();
    let frames =
        (audio.frames() as u128 * u128::from(rate)).div_ceil(u128::from(audio.sample_rate()));
    let count = usize::try_from(frames)
        .ok()
        .and_then(|n| n.checked_mul(channels))
        .ok_or_else(|| AudioError::Wave("导出音频长度溢出".into()))?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(count)
        .map_err(|e| AudioError::Wave(e.to_string()))?;
    let source_channels = audio.layout().channels();
    let sample = |frame: usize, channel: usize| {
        let input = &audio.samples()[frame * source_channels..][..source_channels];
        if channels == 1 && source_channels == 2 {
            (f64::from(input[0]) + f64::from(input[1])) * 0.5
        } else {
            f64::from(input[channel.min(source_channels - 1)])
        }
    };
    if rate == audio.sample_rate() {
        for frame in 0..audio.frames() {
            if frame % 1024 == 0 {
                checkpoint()?;
            }
            for channel in 0..channels {
                samples.push(sample(frame, channel) as f32);
            }
        }
    } else {
        use kernels::{RADIUS, PHASES};
        let kernels=kernels::get(audio.sample_rate(),rate);
        for frame in 0..count / channels {
            if frame % 1024 == 0 {
                checkpoint()?;
            }
            let position = frame as f64 * f64::from(audio.sample_rate()) / f64::from(rate);
            let base = position.floor() as i64;
            let phase = (position.fract() * PHASES as f64).round() as usize;
            for channel in 0..channels {
                let mut value = 0.0;
                for (tap, weight) in (-RADIUS + 1..=RADIUS).zip(&kernels[phase]) {
                    let source = base + tap;
                    if source >= 0 && (source as usize) < audio.frames() {
                        value += sample(source as usize, channel) * weight;
                    }
                }
                samples.push(value as f32);
            }
        }
    }
    checkpoint()?;
    RenderedAudio::new(rate, audio.start_seconds(), layout, samples)
}
