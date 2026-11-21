use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

use super::{
    Archive, HIDDEN, HOP, Result, Tensor, affine, fail, floats, kernels, load_gru, log_rms,
    packed_features,
};

const PORTS: [u32; 2] = [931, 941];
const CHANNELS: usize = 4;
const MAX_GAIN_DB: f32 = 18.0;

pub(in crate::synthesis::instruments::native) struct OutputLevel {
    gru: Vec<u8>,
    weight: Vec<f32>,
    bias: Vec<f32>,
}

impl OutputLevel {
    pub(super) fn load(archive: &Archive) -> Result<Self> {
        Ok(Self {
            gru: load_gru(archive, b"rynthv-levl-gru_")?,
            weight: floats(archive, b"rynthv-levl-head", HIDDEN)?,
            bias: floats(archive, b"rynthv-levl-bias", 1)?,
        })
    }

    pub(in crate::synthesis::instruments::native) fn apply(
        &self,
        outputs: &mut HashMap<u32, Tensor>,
        input: &Tensor,
        active: &[bool],
        stable: usize,
        cancel: &AtomicBool,
    ) -> Result<usize> {
        let frames = input.shape[1];
        let samples = frames * HOP;
        let rms_width = HOP * CHANNELS;
        let features = {
            let first = outputs
                .get(&PORTS[0])
                .ok_or("Instrument primary source absent")?;
            let second = outputs
                .get(&PORTS[1])
                .ok_or("Instrument noise source absent")?;
            if first.shape != [1, samples, CHANNELS] || second.shape != first.shape {
                return fail("Incompatible waveform output level dimensions");
            }
            let harmonic = first.real_slice();
            let noise = second.real_slice();
            packed_features(input, active, |frame| {
                let start = frame * rms_width;
                (
                    log_rms(&harmonic[start..][..rms_width]),
                    log_rms(&noise[start..][..rms_width]),
                )
            })
        };
        let hidden = kernels::gru(&self.gru, &features, cancel)?;
        let mut gains = affine(
            &hidden.real_slice(),
            frames,
            HIDDEN,
            &self.weight,
            1,
            Some(&self.bias),
        );
        for value in &mut gains {
            *value = 10.0f32.powf(MAX_GAIN_DB * value.tanh() / 20.0);
        }
        if gains.iter().any(|gain| !gain.is_finite()) {
            return fail("Nonfinite waveform output gain");
        }
        for port in PORTS {
            let output = outputs.get_mut(&port).ok_or("Missing waveform output")?;
            let mut values = output.dense();
            for (frame, block) in values.chunks_exact_mut(rms_width).enumerate() {
                if frame % 64 == 0 && cancel.load(Ordering::Relaxed) {
                    return fail("Instrument render cancelled");
                }
                let gain = f64::from(gains[frame]);
                let next = f64::from(gains.get(frame + 1).copied().unwrap_or(gains[frame]));
                for (sample, channels) in block.chunks_exact_mut(CHANNELS).enumerate() {
                    let value = (gain + (next - gain) * sample as f64 / HOP as f64) as f32;
                    channels.iter_mut().for_each(|sample| *sample *= value);
                }
            }
            if values.iter().any(|value| !value.is_finite()) {
                return fail("Nonfinite waveform output");
            }
            *output = Tensor::new(output.shape.clone(), values);
        }
        // Gain interpolation needs the next complete frame of both outputs.
        Ok((stable.min(samples) / HOP).saturating_sub(1) * HOP)
    }
}
