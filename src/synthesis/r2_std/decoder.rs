// SPDX-License-Identifier: MIT
use super::{
    AcousticFrame, Error, Result, StdBank,
    lf::Pulse,
    transforms::{dct_negative, dct_positive},
};

#[derive(Clone)]
pub(crate) struct DecodedFrame {
    pub source_pitch: f64,
    pub f0_hz: f32,
    pub rd: f32,
    pub vocal_tract_db: Vec<f32>,
    pub harmonic_phase: Vec<f32>,
    pub noise_psd_db: [f32; 256],
    pub noise_bands: [f32; 4],
    pub noise_envelopes: [super::frames::Envelope; 4],
    pub noise_wave: Option<super::frames::NoiseWave>,
}
pub(crate) struct SourceDecoder {
    pub nyquist: f32,
    linear: Vec<f32>,
    mel: Vec<f32>,
    noise_axis: Vec<f32>,
    ap_axis: Vec<f32>,
    lip_divisors: Vec<f32>,
}
fn lip_divisors(nyquist: f32) -> Vec<f32> {
    let step = nyquist / 1025.0;
    let scale = (f64::from(1.5 * f32::from_bits(0x3cda740e)) * 0.0009362055475993844) as f32;
    let rr = f32::from_bits(0x3fb87318);
    let rr2 = f32::from_bits(0x4004e5a6);
    (0..1025).map(|i| {
        let omega = (((i + 1) as f64) * f64::from(step) * std::f64::consts::TAU) as f32;
        let u = omega * scale;
        let den = u * u + rr2;
        let inv = 1.0 / den;
        let v = u * rr;
        let a = ((((u * u) * rr2) * u) * inv) * u;
        let b = ((v * rr2) * inv) * v;
        ((a + b) / den).sqrt()
    }).collect()
}
impl SourceDecoder {
    pub fn new(bank: &StdBank) -> Result<Self> {
        let nyquist = if bank.profile.encoder_only {
            22050.0
        } else {
            24000.0
        };
        for (key, value) in [
            ("voc_order_spec", 64.0),
            ("voc_order_bap", 5.0),
            ("voc_nspec", 1025.0),
            ("voc_npsd", 256.0),
            ("voc_fnyq", f64::from(nyquist)),
            ("voc_nchannel", 4.0),
            ("voc_maxnhar_e", 5.0),
            ("voc_lipradius", 1.5),
        ] {
            if bank.configuration.numbers(key)? != [value] {
                return Err(Error::Unsupported("LCOD physical decoder geometry"));
            }
        }
        let linear = (0..2048)
            .map(|i| ((nyquist + nyquist) * (i as f32)) / 2048.0)
            .collect();
        let ceiling =
            fast_log(nyquist * f32::from_bits(0x3abb3ee7) + 1.0) * f32::from_bits(0x4442f299);
        let span = ceiling - f32::from_bits(0x429b06e3);
        let mel = (0..1025)
            .map(|i| {
                let arg = (((i as f32) * span) / 1025.0) * f32::from_bits(0x3a690453)
                    + f32::from_bits(0x3d8d1be3);
                ((arg * f32::from_bits(0x3fb8aa3b)).exp2() - 1.0) * 700.0
            })
            .collect();
        Ok(Self {
            nyquist,
            linear,
            mel,
            noise_axis: uniform(256, nyquist),
            ap_axis: uniform(6, nyquist),
            lip_divisors: lip_divisors(nyquist),
        })
    }
    pub fn decode(&self, frame: &AcousticFrame) -> Result<DecodedFrame> {
        let row = &frame.0;
        if row.iter().any(|v| !v.is_finite()) {
            return Err(Error::Format("nonfinite LCOD72"));
        }
        let voiced = row[0] > 0.5;
        let f0 = row[1].max(20.0);
        if voiced && f0 > self.nyquist {
            return Err(Error::Unsupported("voiced source F0 exceeds Nyquist"));
        }
        let rd = f64::from(row[2]).clamp(0.02, 3.0) as f32;
        let input: Vec<_> = row[3..67]
            .iter()
            .map(|x| ((f64::from(*x * 0.5) * 1024.0) * 2.0 / 64.0) as f32)
            .collect();
        let first = dct_negative(&input)?;
        let mut input = vec![0.0; 1024];
        input[..64].copy_from_slice(&first);
        input[0] *= 0.5;
        let mut envelope = dct_positive(&input)?;
        for value in &mut envelope {
            *value = (f64::from(*value) * (2.0 / 1024.0)) as f32;
        }
        envelope.push(envelope[1023]);
        let spectrum = interpolate(&self.mel, &envelope, &self.linear[..1025])?;
        let mut ap = vec![if voiced { 0.0 } else { 1.0 }];
        ap.extend_from_slice(&row[67..]);
        let mut adjusted = interpolate(&self.ap_axis, &ap, &self.linear[..1025])?;
        let mut noise = vec![0.0; 1025];
        let mut periodic = vec![0.0; 1025];
        for i in 0..1025 {
            if voiced {
                let hz = (i as f32) * self.nyquist / 1025.0;
                if hz < 500.0 {
                    adjusted[i] = f32::from_bits(0x3a83126f);
                } else if hz < 2000.0 {
                    adjusted[i] = (((f64::from(adjusted[i]) - 0.001) * f64::from(hz - 500.0))
                        * (1.0 / 1500.0)
                        + 0.001) as f32;
                }
            }
            let power = (spectrum[i] * f32::from_bits(0x4038aa3b)).exp2();
            noise[i] = power * adjusted[i];
            if voiced {
                periodic[i] =
                    ((((1.0 - adjusted[i]) * power) * f0) * f32::from_bits(0x38be37c6)).sqrt();
            }
        }
        let noise = interpolate(&self.linear[..1025], &noise, &self.noise_axis)?;
        let noise_psd_db: Vec<_> = noise
            .into_iter()
            .map(|v| {
                (f64::from(fast_log(v) * f32::from_bits(0x3f317218)) * 4.342944805818469) as f32
            })
            .collect();
        let mut vocal_tract_db = Vec::new();
        let mut harmonic_phase = Vec::new();
        if voiced {
            let pulse = Pulse::new(rd, f0)?;
            let fundamental = pulse.magnitude(f0)?;
            let mut post_lip = periodic;
            for (value, divisor) in post_lip.iter_mut().zip(&self.lip_divisors) {
                *value /= divisor;
            }
            vocal_tract_db = vec![0.0; 1025];
            let factor = (1.0 / f0) * fundamental;
            for i in 1..1025 {
                let lf = pulse.magnitude(self.linear[i])?;
                let value = ((self.linear[i] * post_lip[i]) * factor) / lf;
                vocal_tract_db[i] = (f64::from(fast_log(value) * f32::from_bits(0x3f317218))
                    * 8.685889611636938) as f32;
            }
            vocal_tract_db[0] = vocal_tract_db[1];
            let count = (self.nyquist / f0) as usize;
            for hz in uniform(count + 1, (count as f32) * f0).into_iter().skip(1) {
                harmonic_phase.push(pulse.at(hz)?.1);
            }
        }
        if vocal_tract_db
            .iter()
            .chain(&harmonic_phase)
            .chain(&noise_psd_db)
            .any(|v| !v.is_finite())
        {
            return Err(Error::Format("nonfinite physical source decoder output"));
        }
        Ok(DecodedFrame {
            source_pitch: 0.0,
            noise_wave: None,
            f0_hz: if voiced { f0 } else { 0.0 },
            rd,
            vocal_tract_db,
            harmonic_phase,
            noise_psd_db: noise_psd_db.try_into().ok().unwrap(),
            noise_bands: [f32::from_bits(0x3727c5ac); 4],
            noise_envelopes: std::array::from_fn(|_| super::frames::Envelope {
                amplitude: vec![0.0; 5],
                phase: vec![0.0; 5],
            }),
        })
    }
}

pub(super) fn uniform(count: usize, top: f32) -> Vec<f32> {
    let den = (count - 1) as f32;
    (0..count)
        .map(|i| ((i as f32) * top) * (1.0 / den))
        .collect()
}
pub(super) fn interpolate(x: &[f32], y: &[f32], queries: &[f32]) -> Result<Vec<f32>> {
    if x.len() != y.len()
        || x.len() < 2
        || x.windows(2).any(|v| !v[0].is_finite() || v[1] <= v[0])
        || queries.windows(2).any(|v| v[1] < v[0])
    {
        return Err(Error::Format("spectral interpolation axes"));
    }
    let mut j = 0;
    Ok(queries
        .iter()
        .map(|&q| {
            while j < x.len() - 2 && q > x[j + 1] {
                j += 1;
            }
            if q < x[0] {
                y[0]
            } else if q > x[x.len() - 1] {
                y[y.len() - 1]
            } else {
                ((y[j + 1] - y[j]) * (q - x[j])) / (x[j + 1] - x[j]) + y[j]
            }
        })
        .collect())
}
pub(super) fn fast_log(x: f32) -> f32 {
    let word = x.to_bits();
    let m = f32::from_bits((word & 0x7fffff) | 0x3f000000);
    let y = (word as f32) * f32::from_bits(0x34000000) - f32::from_bits(0x42f87377);
    let y = y - m * f32::from_bits(0x3fbfbf75);
    y - f32::from_bits(0x3fdce9a3) / (m + f32::from_bits(0x3eb444f9))
}
