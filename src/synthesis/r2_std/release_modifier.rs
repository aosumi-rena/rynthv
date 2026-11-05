// SPDX-License-Identifier: MIT
use super::{ControlSample, DecodedFrame, Error, Result, StdBank, release_spectrum as spectral};

pub(super) struct ReleaseModifier {
    below: f64,
    formant: f64,
    voiced_eq: Vec<f64>,
    noise_eq: Vec<f64>,
    nyquist: f32,
}
impl ReleaseModifier {
    pub fn open(bank: &StdBank, nyquist: f32) -> Result<Self> {
        let c = &bank.configuration;
        if c.contains("svc_model") {
            return Err(Error::Unsupported("bank SVC model"));
        }
        let mut below = c
            .optional_first_number("voc_adjust_rd_below")?
            .unwrap_or(0.0);
        let above = c
            .optional_first_number("voc_adjust_rd_above")?
            .unwrap_or(0.0);
        let formant = c
            .optional_first_number("voc_default_formant_scale")?
            .unwrap_or(1.0);
        let mut thresholds = Vec::new();
        for row in c.string_rows("pitch_map") {
            if row.len() != 2 {
                return Err(Error::Format("modifier pitch-map pair"));
            }
            let text = &row[1];
            if !(text.len() == 2 || text.len() == 3)
                || !(b'A'..=b'G').contains(&text[0])
                || !text[text.len() - 1].is_ascii_digit()
                || text.len() == 3 && !matches!(text[1], b'#' | b'b')
            {
                return Err(Error::Unsupported("modifier canonical pitch-map spelling"));
            }
            let accidental = if text.len() == 2 {
                0
            } else if text[1] == b'#' {
                1
            } else {
                -1
            };
            thresholds.push(
                12 * (i32::from(text[text.len() - 1] - b'0') + 1)
                    + [9, 11, 0, 2, 4, 5, 7][(text[0] - b'A') as usize]
                    + accidental,
            );
        }
        if below == 0.0 {
            if let Some(pitch) = thresholds.into_iter().min() {
                let exponent = (pitch - 69) as f32 * f32::from_bits(0x3daaaaab);
                below = f64::from(exponent.exp2() * 440.0);
            }
        }
        if !below.is_finite()
            || below < 0.0
            || !above.is_finite()
            || above < 0.0
            || !formant.is_finite()
            || formant <= 0.0
        {
            return Err(Error::Format("release bank settings"));
        }
        let eq = |key| -> Result<Vec<f64>> {
            let values = if c.contains(key) {
                c.numbers(key)?.to_vec()
            } else {
                Vec::new()
            };
            if values.len() > 4096 || values.len() % 2 != 0 || values.iter().any(|v| !v.is_finite())
            {
                return Err(Error::Format("release EQ frequency/gain pairs"));
            }
            Ok(values)
        };
        Ok(Self {
            below,
            formant,
            voiced_eq: eq("pf_eq_voiced")?,
            noise_eq: eq("pf_eq_unvoiced")?,
            nyquist,
        })
    }
    pub fn apply(&self, frame: &mut DecodedFrame, next: f32, control: ControlSample) -> Result<()> {
        let old = frame.f0_hz;
        let voiced = old > 0.0;
        let lanes = transform(control);
        let factor = (if voiced {
            self.formant * lanes[3]
        } else {
            lanes[3]
        }) as f32;
        if voiced {
            if next <= 0.0
                || old >= self.nyquist - 500.0
                || next >= self.nyquist - 500.0
                || frame.vocal_tract_db.len() != 1025
            {
                return Err(Error::Unsupported("release voiced pitch geometry"));
            }
            let prior_envelope = spectral::envelope(frame.rd, old, 1025, self.nyquist)?;
            frame.vocal_tract_db = spectral::remap(&frame.vocal_tract_db, old, next, self.nyquist)?;
            frame.noise_psd_db = spectral::remap(&frame.noise_psd_db, old, next, self.nyquist)?
                .try_into()
                .unwrap();
            if factor != 1.0 {
                frame.vocal_tract_db =
                    spectral::formant(&frame.vocal_tract_db, next, factor, self.nyquist)?;
                frame.noise_psd_db =
                    spectral::formant(&frame.noise_psd_db, next, factor, self.nyquist)?
                        .try_into()
                        .unwrap();
            }
            let mut rd = f64::from(frame.rd);
            if f64::from(next) < self.below {
                rd += (1.0 - f64::from(next) / self.below) * (3.0 - rd) * 0.5;
            }
            let prior = rd as f32;
            let shape = lanes[0] as f32;
            let distance = if shape > 0.0 {
                3.0 - f64::from(prior)
            } else {
                f64::from(prior) - 0.1
            };
            frame.rd = (f64::from(shape) * distance + f64::from(prior)) as f32;
            frame.f0_hz = next;
            let envelope = spectral::envelope(frame.rd, next, 1025, self.nyquist)?;
            let start = ((5000.0 / self.nyquist) * 1025.0) as usize;
            for i in start..1025 {
                let hz = i as f32 * self.nyquist / 1025.0;
                let weight = if hz < 8000.0 {
                    (hz - 5000.0) * f32::from_bits(0x39aec33e)
                } else {
                    1.0
                };
                if prior_envelope[i] < envelope[i] {
                    frame.vocal_tract_db[i] += (prior_envelope[i] - envelope[i]) * weight;
                }
            }
        } else if factor != 1.0 {
            if !frame.vocal_tract_db.is_empty() {
                frame.vocal_tract_db =
                    spectral::formant(&frame.vocal_tract_db, 0.0, factor, self.nyquist)?;
            }
            frame.noise_psd_db = spectral::formant(&frame.noise_psd_db, 0.0, factor, self.nyquist)?
                .try_into()
                .unwrap();
        }
        if !frame.vocal_tract_db.is_empty() {
            gain(&mut frame.vocal_tract_db, lanes[1]);
            spectral::equalize(&mut frame.vocal_tract_db, &self.voiced_eq, self.nyquist);
        }
        gain(&mut frame.noise_psd_db, lanes[2]);
        if let Some(key)=&mut frame.noise_wave {
            if lanes[2]!=1.0 { key.scale(lanes[2])?; }
        }
        let old_psd=frame.noise_wave.as_ref().map(|_|frame.noise_psd_db);
        spectral::equalize(&mut frame.noise_psd_db, &self.noise_eq, self.nyquist);
        if !self.noise_eq.is_empty() {
            if let (Some(key),Some(old))=(&mut frame.noise_wave,old_psd) {
                key.adjust_psd(&old,&frame.noise_psd_db)?;
            }
        }
        if !frame.rd.is_finite()
            || frame
                .vocal_tract_db
                .iter()
                .chain(&frame.noise_psd_db)
                .any(|v| !v.is_finite())
        {
            return Err(Error::Format("release finite spectral output"));
        }
        Ok(())
    }
}
fn gain(values: &mut [f32], gain: f64) {
    if gain == 1.0 {
        return;
    }
    let offset = if gain < f64::from_bits(0x3f50624dd2f1a9fc) {
        -100.0
    } else {
        (gain.ln() * f64::from_bits(0x40215e5fd1c1f91c)) as f32
    };
    for value in values {
        *value += offset;
    }
}
fn transform(c: ControlSample) -> [f64; 4] {
    let mut loudness = c.loudness_db as f32;
    if loudness < -36.0 {
        let q = (loudness + 48.0).max(0.0) / 12.0 + f32::from_bits(0x33d6bf95);
        loudness += q.log10() * 20.0;
    }
    let (t, b, v, g) = (
        c.tension as f32,
        c.breathiness as f32,
        c.voicing as f32,
        c.gender as f32,
    );
    let a = (f64::from(loudness) - f64::from(t) * 6.0 - f64::from(b.max(0.0)) * 5.5) as f32;
    let noise = (f64::from(b) * 7.5 + f64::from(loudness)) as f32;
    let shape = -(f64::from(loudness) / 12.0 * 0.1
        + f64::from(t) * if t > 0.0 { 0.5 } else { 0.8 })
    .clamp(-1.0, 1.0);
    [
        shape,
        10.0f64.powf(f64::from(a) / 20.0) * f64::from(v.max(0.0)),
        10.0f64.powf(f64::from(noise) / 20.0),
        (-f64::from(g) * 0.5).exp(),
    ]
}
