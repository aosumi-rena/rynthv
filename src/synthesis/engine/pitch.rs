mod assembly;
mod growl;
pub(super) use growl::apply as apply_growl;
pub(super) use assembly::assemble as assemble_note_curves;

use crate::synthesis::{
    score::PreparedPhrase,
    stages::f0::{PitchResult, frontend::frame_count},
    voice::{Result, VoiceError},
};

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct DdsPitchClock {
    pub origin_seconds: f64,
    pub frame_seconds: f64,
    pub count: usize,
}

pub(super) type HostPitchSampler<'a> = dyn FnMut(DdsPitchClock) -> Result<Vec<Option<f32>>> + 'a;

pub(super) fn apply_host_dds_pitch(
    phrase: &PreparedPhrase,
    pitch: &mut PitchResult,
    sampler: &mut HostPitchSampler<'_>,
) -> Result<()> {
    let clock = DdsPitchClock {
        origin_seconds: phrase.start_seconds,
        frame_seconds: pitch.frame_seconds,
        count: pitch.pitch.len(),
    };
    if !clock.origin_seconds.is_finite() || !clock.frame_seconds.is_finite()
        || clock.frame_seconds <= 0.0 || clock.count == 0 || clock.count > 1_000_000
        || !(clock.origin_seconds + (clock.count - 1) as f64 * clock.frame_seconds).is_finite()
    {
        return Err(VoiceError::new("Invalid or oversized host DDS pitch clock"));
    }
    let values = sampler(clock)?;
    apply_host_values(&mut pitch.pitch, &values)
}

fn apply_host_values(pitch: &mut [f32], values: &[Option<f32>]) -> Result<()> {
    if values.len() != pitch.len() || values.iter().flatten().any(|value| !value.is_finite()) {
        return Err(VoiceError::new("Host pitch requires one finite value or null per model frame"));
    }
    let values: Vec<_> = values.iter().map(|value| value.unwrap_or(f32::NAN)).collect();
    apply_dds_values(pitch, &values)
}

fn apply_dds_values(pitch: &mut [f32], controls: &[f32]) -> Result<()> {
    if pitch.len() != controls.len() {
        return Err(VoiceError::new("DDS pitch control count differs from its model clock"));
    }
    for (value, &control) in pitch.iter_mut().zip(controls) {
        if control.is_finite() { *value = control; }
    }
    Ok(())
}

pub(super) fn sv2_controls(phrase: &PreparedPhrase) -> Result<Option<Vec<f32>>> {
    let Some(controls) = &phrase.pitch_controls else {
        return Ok(None);
    };
    let count = phrase.notes.iter().try_fold(0_usize, |count, note| {
        count
            .checked_add(frame_count(note)?)
            .ok_or_else(|| VoiceError::new("Pitch control frame count overflow"))
    })?;
    controls
        .sample(
            phrase.start_seconds,
            crate::synthesis::score::FRAME_SECONDS,
            count,
        )
        .map(Some)
}

pub(super) fn expressiveness(
    phrase: &PreparedPhrase,
    step: f64,
    count: usize,
) -> Result<Option<Vec<f32>>> {
    phrase
        .performance_controls
        .as_ref()
        .map(|controls| controls.sample(phrase.start_seconds, step, count))
        .transpose()
}

pub(super) fn apply_deviation(phrase:&PreparedPhrase,pitch:&mut PitchResult) {
    let Some(curves)=phrase.automation.as_ref().filter(|c|c.has("pitchDelta")) else{return};
    for (frame,value) in pitch.pitch.iter_mut().enumerate() {
        let cents=curves.value("pitchDelta",phrase.start_seconds+frame as f64*pitch.frame_seconds,0.0);
        if cents!=0.0 {*value+=cents/100.0;}
    }
}

pub(super) fn apply_dds_curves(phrase: &PreparedPhrase, pitch: &mut PitchResult) -> Result<()> {
    let Some(controls) = &phrase.pitch_controls else {
        return Ok(());
    };
    let controls = controls.sample(phrase.start_seconds, pitch.frame_seconds, pitch.pitch.len())?;
    apply_dds_values(&mut pitch.pitch, &controls)
}

#[cfg(test)]
mod host_pitch_tests {
    use super::*;
    #[test]
    fn null_values_preserve_prediction_bits_and_finite_values_replace_exactly() {
        let mut pitch = [60.25, -0.0, 61.0];
        let before = pitch.map(f32::to_bits);
        apply_host_values(&mut pitch, &[None, None, None]).unwrap();
        assert_eq!(pitch.map(f32::to_bits), before);
        apply_host_values(&mut pitch, &[Some(62.0), None, Some(-0.0)]).unwrap();
        assert_eq!(pitch.map(f32::to_bits), [62.0f32.to_bits(), (-0.0f32).to_bits(), (-0.0f32).to_bits()]);
    }
    #[test]
    fn invalid_count_or_value_cannot_partially_mutate_pitch() {
        let mut pitch = [60.0, 61.0]; let before = pitch;
        assert!(apply_host_values(&mut pitch, &[Some(62.0)]).is_err());
        assert_eq!(pitch, before);
        assert!(apply_host_values(&mut pitch, &[Some(62.0), Some(f32::INFINITY)]).is_err());
        assert_eq!(pitch, before);
    }
}
