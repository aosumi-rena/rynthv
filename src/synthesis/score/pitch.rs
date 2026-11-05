use crate::{
    model::pitch::PitchControl,
    synthesis::{
        plan::{GlobalBlick, OwnerInput, TempoMapping},
        voice::{Result, VoiceError},
    },
};

#[derive(Clone, Debug)]
pub struct PreparedPitchControls {
    controls: Vec<PitchControl>,
    tempo: TempoMapping,
    content_offset: GlobalBlick,
    pitch_offset: f32,
}

impl PreparedPitchControls {
    pub(crate) fn new(input: &OwnerInput) -> Result<Option<Self>> {
        let transpose=crate::synthesis::parameters::transpose(&input.voice)?;
        Ok((!input.pitch_controls.is_empty()).then(|| Self {
            controls: input.pitch_controls.clone(),
            tempo: input.tempo.clone(),
            content_offset: input.content_offset,
            pitch_offset: input.pitch_offset as f32+transpose,
        }))
    }

    pub fn sample(&self, origin: f64, step: f64, count: usize) -> Result<Vec<f32>> {
        if !origin.is_finite() || !step.is_finite() || step <= 0.0 {
            return Err(VoiceError::new("Invalid pitch control frame clock"));
        }
        let mut result = vec![f32::NAN; count];

        for control in &self.controls {
            let PitchControl::Curve(curve) = control else {
                continue;
            };
            let (begin, end) = curve
                .range()
                .ok_or_else(|| VoiceError::new("Invalid hand-drawn pitch range"))?;
            let first = ((self.seconds_at(begin)? - origin) / step)
                .floor()
                .clamp(0.0, count as f64) as usize;
            let stop = (((self.seconds_at(end)? - origin) / step).ceil() + 1.0)
                .clamp(0.0, count as f64) as usize;
            for (frame, value) in result.iter_mut().enumerate().take(stop).skip(first) {
                let global = self.tempo.position_at(origin + frame as f64 * step).0;
                let local = (i128::from(global) - i128::from(self.content_offset.0)) as f64;

                if local < begin as f64 || local >= end as f64 {
                    continue;
                }
                if let Some(pitch) = curve.sample(local) {
                    *value = self.absolute_pitch(pitch)?;
                }
            }
        }
        for control in &self.controls {
            match control {
                PitchControl::Point(point) => {
                    let frame = ((self.seconds_at(point.pos)? - origin) / step).round();
                    if frame >= 0.0 && frame < count as f64 {
                        let value = &mut result[frame as usize];
                        if value.is_nan() {
                            *value = self.absolute_pitch(point.pitch)?;
                        }
                    }
                }
                PitchControl::Curve(_) => {}
                PitchControl::Unsupported(_) => {
                    return Err(VoiceError::new(
                        "Unsupported pitch control reached synthesis",
                    ));
                }
            }
        }
        Ok(result)
    }

    fn seconds_at(&self, local: i64) -> Result<f64> {
        local
            .checked_add(self.content_offset.0)
            .map(|position| self.tempo.seconds_at(GlobalBlick(position)))
            .filter(|seconds| seconds.is_finite())
            .ok_or_else(|| VoiceError::new("Pitch control position overflows"))
    }

    fn absolute_pitch(&self, local: f64) -> Result<f32> {
        let pitch = local + f64::from(self.pitch_offset);
        if !pitch.is_finite() || !(0.0..=127.0).contains(&pitch) {
            return Err(VoiceError::new("Pitch control is outside MIDI range"));
        }
        Ok(pitch as f32)
    }
}
