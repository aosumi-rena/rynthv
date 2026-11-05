use super::Contract;

use crate::synthesis::{
    inference::noise::seeds::SeedSource,
    stages::{
        f0::{TimedPhone, expression::expression_controls, frontend::FrontendNote},
        sv1_noise::Normal,
    },
    voice::{Result, VoiceError},
};

pub(super) struct Controls {
    pub positions: Vec<[f32; 3]>,
    pub noise: Vec<f32>,
}

#[derive(Clone, Copy)]
pub(super) struct Defaults {

    pub temperature: f32,

    pub flagged_temperature_scale: f32,

    pub position_scale: f32,
}

pub(super) fn prepare(
    notes: &[FrontendNote],
    note_frames: &[usize],
    phones: &[TimedPhone],
    phone_frames: &[usize],
    defaults: Defaults,
    contract: Contract,
    pitch_seeds: &[u32],
) -> Result<Controls> {
    if notes.len() != note_frames.len()
        || notes.len() != pitch_seeds.len()
        || phones.len() != phone_frames.len()
        || !defaults.temperature.is_finite()
        || !defaults.flagged_temperature_scale.is_finite()
        || !defaults.position_scale.is_finite()
        || notes.iter().any(|note| note.kind > match contract {
            Contract::Compatible => 2,
            Contract::Native { .. } => 1,
        })
    {
        return Err(VoiceError::new("Invalid version 1 F0 default controls"));
    }
    let total = note_frames
        .iter()
        .try_fold(0_usize, |sum, &count| sum.checked_add(count))
        .filter(|&total| total <= i32::MAX as usize)
        .ok_or_else(|| VoiceError::new("Version 1 F0 frame count overflow"))?;
    let mut temperature = Vec::with_capacity(total);
    let mut position_a = vec![f32::from_bits(0x3f7e_b852); total];
    let mut position_c = Vec::with_capacity(total);
    for (note, &count) in notes.iter().zip(note_frames) {
        let (note_temperature, value) = match contract {
            Contract::Compatible => {
                let controls = expression_controls(
                    note.kind,
                    note.performance.expression_x,
                    note.performance.expression_y,
                );
                let mut temperature = if note.flag {
                    f64::from(defaults.temperature) * f64::from(defaults.flagged_temperature_scale)
                } else {
                    f64::from(defaults.temperature)
                };
                if let Some(controls) = controls {
                    temperature *= f64::from(controls.expression + controls.expression);
                }
                (
                    temperature as f32,
                    controls.map_or(0.0, |controls| controls.position_c),
                )
            }
            Contract::Native { enhancement } => {

                (
                    defaults.temperature,
                    if note.kind == 0 { enhancement } else { 0.0 },
                )
            }
        };
        temperature.extend(std::iter::repeat_n(note_temperature, count));
        position_c.extend(std::iter::repeat_n(value, count));
    }
    smooth(&mut temperature);
    smooth(&mut position_a);
    smooth(&mut position_c);
    let positions = temperature
        .iter()
        .zip(position_a)
        .zip(position_c)
        .map(|((&temperature, a), c)| {
            let position_b = match contract {
                Contract::Compatible => {

                    let scale = if f64::from(temperature) > 0.8 {
                        ((1.2 - f64::from(temperature)) / 0.4).max(0.0)
                    } else {
                        1.0
                    };
                    (f64::from(defaults.position_scale) * scale) as f32
                }
                Contract::Native { .. } => {

                    if temperature > 0.8 {
                        defaults.position_scale * (temperature * -2.5 + 3.0).max(0.0)
                    } else {
                        defaults.position_scale
                    }
                }
            };
            [a, position_b, c]
        })
        .collect();
    let tokens: Vec<&str> = phones.iter().map(|phone|
        crate::synthesis::voice::resource_phone_symbol(&phone.token).unwrap_or(&phone.token)).collect();
    let mut phone_seeds = SeedSource::context(&tokens, phone_frames, "magic", 0)
        .map_err(|error| VoiceError::new(error.to_string()))?;
    let mut note_seeds = SeedSource::new(pitch_seeds, note_frames)
        .map_err(|error| VoiceError::new(error.to_string()))?;
    let mut normal = Normal::new(123);
    let mut last_phone = 0;
    let mut last_note = 0;
    let noise = temperature
        .into_iter()
        .map(|temperature| {
            let phone = phone_seeds.next_event();
            let note = note_seeds.next_event();
            if let Some(seed) = phone {
                last_phone = seed;
            }
            if let Some(seed) = note {
                last_note = seed;
            }
            if phone.is_some() || note.is_some() {
                normal.reseed(last_phone ^ last_note);
            }
            let amplitude = if temperature < 0.5 {
                temperature * 2.0
            } else {
                1.0
            };
            normal.next() * amplitude
        })
        .collect();
    Ok(Controls { positions, noise })
}

pub(super) fn vibrato(
    notes: &[FrontendNote],
    note_frames: &[usize],
    values: Option<&[f32]>,
) -> Result<Vec<f32>> {
    let total: usize = note_frames.iter().sum();
    let values = match values {
        Some(values) if values.len() == total => values.to_vec(),
        Some(_) => {
            return Err(VoiceError::new(
                "DDS vibrato controls differ from its note frame count",
            ));
        }
        None => notes
            .iter()
            .zip(note_frames)
            .flat_map(|(note, &count)| std::iter::repeat_n(note.performance.vibrato as f32, count))
            .collect(),
    };
    if values.iter().any(|value| !value.is_finite()) {
        return Err(VoiceError::new("DDS vibrato controls must be finite"));
    }

    Ok(values
        .into_iter()
        .map(|value| value.clamp(0.0, 2.0) - 1.0)
        .collect())
}

fn smooth(values: &mut [f32]) {
    let previous_weight = 1.0_f32 - 0.95;
    for index in 1..values.len() {
        values[index] = previous_weight * values[index - 1] + 0.95 * values[index];
    }
    for index in (0..values.len().saturating_sub(1)).rev() {
        values[index] = previous_weight * values[index + 1] + 0.95 * values[index];
    }
}
