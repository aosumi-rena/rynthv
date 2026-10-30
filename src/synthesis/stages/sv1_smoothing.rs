mod math;

use std::collections::VecDeque;
use crate::synthesis::{inference::Sequence, voice::{Result, VoiceError}};

pub(crate) const LOOKAHEAD: usize = 2;

pub(crate) struct Smoother {
    rows: VecDeque<[f32; 72]>,
    previous: Option<[f32; 72]>,
    finished: bool,
}

impl Smoother {
    pub(crate) fn new() -> Self {
        Self { rows: VecDeque::new(), previous: None, finished: false }
    }

    pub(crate) fn process(&mut self, input: &Sequence, final_batch: bool) -> Result<Sequence> {
        input.validate()?;
        if self.finished || input.channels != 72 || input.values.iter().any(|v| !v.is_finite()) {
            return Err(VoiceError::new("Invalid DDS smoothing stream input"));
        }
        let mut values = Vec::with_capacity(input.values.len());
        for row in input.values.chunks_exact(72) {
            let row: [f32; 72] = row.try_into().unwrap();
            if self.rows.is_empty() {
                self.rows.extend([row, row]);
                self.previous = Some(row);
            }
            self.rows.push_back(row);
            if self.rows.len() < 5 { continue; }
            let mut maximum = -10.0_f32;
            let mut votes = 0.0_f32;
            for row in &self.rows {
                votes += if row[0] > 0.5 { 1.0 } else { 0.0 };
                for &value in &row[3..67] { if maximum < value { maximum = value; } }
            }
            let fraction = votes / 5.0;
            let u = ((maximum + 4.0) * 0.5).clamp(0.0, 1.0);
            let threshold = u + 2.0;
            let inverse = 1.0 / threshold;
            let previous = self.previous.as_ref().unwrap();
            let mut output = self.rows[2];
            for i in 3..67 {
                let delta = (output[i] - previous[i]) * inverse;
                if !delta.is_finite() { return Err(VoiceError::new("DDS smoothing overflow")); }
                output[i] = math::tanh(delta) * threshold + previous[i];
            }
            output[0] = if fraction > 0.5 { 1.0 } else { 0.0 };
            self.previous = Some(output);
            if fraction < f32::from_bits(0x3f7d70a4) {
                let adjustment = -0.3_f32 * (1.0 - fraction);
                math::add_curve(&mut output[3..67], adjustment);
            }
            if output.iter().any(|v| !v.is_finite()) {
                return Err(VoiceError::new("DDS smoothing produced non-finite output"));
            }
            values.extend_from_slice(&output);
            self.rows.pop_front();
        }
        self.finished = final_batch;
        Sequence::from_values(72, values)
    }
}
