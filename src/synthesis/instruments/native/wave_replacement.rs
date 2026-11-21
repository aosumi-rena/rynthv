use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use super::{
    Result,
    archive::Archive,
    fail, id, kernels,
    tensor::{Tensor, affine},
    u32at,
};
mod level;

pub(super) const HOP: usize = 240;
const BANDS: usize = 8;
const TAPS: usize = 16;
const OVERLAP: usize = 12;
const SUBFRAMES: usize = HOP / BANDS;
const HIDDEN: usize = 192;
const INPUT: usize = 50;
const GRU_BYTES: usize = 8 + (3 * HIDDEN * (INPUT + HIDDEN) + 6 * HIDDEN) * 4;

pub(in super::super) struct WaveModel {
    pub(super) archive: Arc<Archive>,
    pub(super) gate: Option<Gate>,
}

pub(in super::super) fn open_waveform(path: &Path) -> Result<WaveModel> {
    let (archive, unsigned) = Archive::open_waveform(path)?;
    let gate = if unsigned {
        Some(Gate::load(&archive)?)
    } else {
        None
    };
    let program = super::program::Program::new(
        archive.clone(),
        id(super::render::WAVE_PROGRAM),
        Arc::new(AtomicBool::new(false)),
    )?;
    program.validate_waveform(gate.is_some())?;
    Ok(WaveModel { archive, gate })
}

pub(super) struct Gate {
    gru: Vec<u8>,
    weights: Vec<f32>,
    bias: Vec<f32>,
    bank: Vec<f32>,
    units: [f32; 3],
    pub(super) level: Option<level::OutputLevel>,
}

impl Gate {
    fn load(archive: &Archive) -> Result<Self> {
        let version = archive.read(b"rynthv-wave-vers")?;
        let level = match version.as_slice() {
            [1, 0, 0, 0] => None,
            [2, 0, 0, 0] => Some(level::OutputLevel::load(archive)?),
            _ => return fail("Unsupported waveform override version"),
        };
        let bank = floats(archive, b"rynthv-wave-bank", BANDS * TAPS)?;
        validate_bank(&bank)?;
        let units: [f32; 3] = floats(archive, b"rynthv-wave-unit", 3)?.try_into().unwrap();
        if units.iter().any(|v| *v <= 0.0) {
            return fail("Invalid waveform interface gains");
        }
        Ok(Self {
            gru: load_gru(archive, b"rynthv-wave-gru_")?,
            bank,
            units,
            weights: floats(archive, b"rynthv-wave-head", 2 * HIDDEN)?,
            bias: floats(archive, b"rynthv-wave-bias", 2)?,
            level,
        })
    }

    pub(super) fn apply(
        &self,
        harmonic: &Tensor,
        noise: &Tensor,
        input: &Tensor,
        active: &[bool],
        cancel: &AtomicBool,
    ) -> Result<[Tensor; 2]> {
        let frames = input.shape.get(1).copied().ok_or("Waveform input rank")?;
        let samples = frames * HOP;
        if frames == 0
            || input.shape.len() != 3
            || input.shape[0] != 1
            || input.shape[2] <= 76
            || active.len() < frames
            || harmonic.shape != [1, samples, 1]
            || noise.shape != [1, samples / BANDS, BANDS]
        {
            return fail("Incompatible waveform override input");
        }
        let mut h = analyze(&harmonic.real_slice(), &self.bank);
        h.iter_mut().for_each(|v| *v *= self.units[0]);
        let noise = noise.real_slice();
        let features = packed_features(input, active, |frame| {
            let start = frame * HOP;
            (
                log_rms(&h[start + BANDS..][..HOP]),
                log_rms(&noise[start..][..HOP]),
            )
        });
        let hidden = kernels::gru(&self.gru, &features, cancel)?;
        let logits = affine(
            &hidden.real_slice(),
            frames,
            HIDDEN,
            &self.weights,
            2,
            Some(&self.bias),
        );
        let mut n = vec![0.0; h.len()];
        let mut previous = [0.0; 2];
        let mut first_gain = 0.0;
        for (frame, logits) in logits.chunks_exact(2).enumerate() {
            if frame % 64 == 0 && cancel.load(Ordering::Relaxed) {
                return fail("Instrument render cancelled");
            }
            let gain = [sigmoid(logits[0]), 0.35 * sigmoid(logits[1])];
            for sub in 0..SUBFRAMES {
                let t = (sub + 1) as f32 / SUBFRAMES as f32;
                let g0 = previous[0] + (gain[0] - previous[0]) * t;
                let g1 = previous[1] + (gain[1] - previous[1]) * t;
                if frame == 0 && sub == 0 {
                    first_gain = g0;
                }
                let start = (frame * SUBFRAMES + sub) * BANDS;
                for band in 0..BANDS {
                    h[start + BANDS + band] *= g0;
                    n[start + BANDS + band] = noise[start + band] * g1;
                }
            }
            previous = gain;
        }
        h[..BANDS].iter_mut().for_each(|v| *v *= first_gain);
        h[samples + BANDS..]
            .iter_mut()
            .for_each(|v| *v *= previous[0]);
        let signal = |bands: Vec<f32>, unit: f32| {
            let data: Vec<_> = synthesize(&bands, &self.bank)
                .into_iter()
                .map(|v| v * unit)
                .collect();
            Tensor::new(vec![1, samples, 1], data)
        };
        let source = [signal(h, self.units[1]), signal(n, self.units[2])];
        if source.iter().any(|tensor| !tensor.finite()) {
            return fail("Nonfinite waveform override output");
        }
        Ok(source)
    }
}

fn load_gru(archive: &Archive, id: &[u8; 16]) -> Result<Vec<u8>> {
    let gru = archive.read(id)?;
    if gru.len() != GRU_BYTES
        || u32at(&gru, 0) as usize != HIDDEN
        || u32at(&gru, 4) as usize != INPUT
        || !gru[8..]
            .chunks_exact(4)
            .all(|v| f32::from_bits(u32at(v, 0)).is_finite())
        || gru[8..8 + 3 * HIDDEN * INPUT * 4]
            .chunks_exact(INPUT * 4)
            .any(|row| row[8 * 4..].iter().any(|&v| v != 0))
    {
        return fail("Invalid waveform GRU parameters");
    }
    Ok(gru)
}

fn packed_features(
    input: &Tensor,
    active: &[bool],
    log_levels: impl Fn(usize) -> (f32, f32),
) -> Tensor {
    let frames = input.shape[1];
    let mut features = vec![0.0; frames * INPUT];
    let mut previous_pitch = 0.0;
    let mut previous_active: f32 = 0.0;
    for (frame, row) in features.chunks_exact_mut(INPUT).enumerate() {
        let hz = input.get(frame * input.shape[2] + 76);
        let pitch = (hz.clamp(20.0, 4600.0) / 440.0).log2();
        let (log_h, log_n) = log_levels(frame);
        let on = if active[frame] { 1.0 } else { 0.0 };
        let transition = on - previous_active;
        let delta = if frame == 0 {
            0.0
        } else {
            (pitch - previous_pitch).clamp(-1.0, 1.0)
        };
        row[..8].copy_from_slice(&[
            on,
            pitch / 4.0,
            log_h / 8.0,
            log_n / 8.0,
            transition.max(0.0),
            (-transition).max(0.0),
            delta,
            (log_h - log_n).clamp(-8.0, 8.0) / 8.0,
        ]);
        previous_active = on;
        previous_pitch = pitch;
    }
    Tensor::new(vec![1, frames, INPUT], features)
}

fn sigmoid(v: f32) -> f32 {
    1.0 / (1.0 + (-v.clamp(-60.0, 60.0)).exp())
}

fn log_rms(values: &[f32]) -> f32 {
    let power = values.iter().map(|&v| v * v).sum::<f32>() / values.len() as f32;
    power.sqrt().max(1.0e-5).ln()
}

fn floats(archive: &Archive, id: &[u8; 16], count: usize) -> Result<Vec<f32>> {
    let bytes = archive.read(id)?;
    if bytes.len() != count * 4 {
        return fail("Unexpected waveform coefficient count");
    }
    bytes
        .chunks_exact(4)
        .map(|word| {
            let value = f32::from_bits(u32at(word, 0));
            value
                .is_finite()
                .then_some(value)
                .ok_or_else(|| "Nonfinite waveform coefficient".into())
        })
        .collect()
}

fn analyze(samples: &[f32], bank: &[f32]) -> Vec<f32> {
    let mut result = Vec::with_capacity(samples.len() + 2 * BANDS);
    for row in 0..samples.len() / BANDS + 2 {
        for weights in bank.chunks_exact(TAPS) {
            result.push(
                weights
                    .iter()
                    .enumerate()
                    .map(|(tap, &weight)| {
                        let sample = (row * BANDS + tap)
                            .checked_sub(OVERLAP)
                            .and_then(|i| samples.get(i))
                            .copied()
                            .unwrap_or(0.0);
                        sample * weight
                    })
                    .sum(),
            );
        }
    }
    result
}

fn synthesize(rows: &[f32], bank: &[f32]) -> Vec<f32> {
    let mut output = vec![0.0; rows.len() + BANDS];
    for (row, gains) in rows.chunks_exact(BANDS).enumerate() {
        for tap in 0..TAPS {
            let sample: f32 = gains
                .iter()
                .enumerate()
                .map(|(band, &gain)| gain * bank[band * TAPS + tap])
                .sum();
            output[row * BANDS + tap] += sample;
        }
    }
    output[OVERLAP..output.len() - OVERLAP].to_vec()
}

fn validate_bank(bank: &[f32]) -> Result<()> {
    for phase in 0..BANDS {
        let mut impulse = [0.0; 4 * BANDS];
        impulse[BANDS + phase] = 1.0;
        let output = synthesize(&analyze(&impulse, bank), bank);
        if output
            .iter()
            .zip(impulse)
            .any(|(&a, b)| !a.is_finite() || (a - b).abs() > 2.0e-5)
        {
            return fail("Waveform filterbank cannot reconstruct its input");
        }
    }
    Ok(())
}
