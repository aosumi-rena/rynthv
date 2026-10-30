use super::super::Contract;
use super::{Frontend, PhoneInput, error};
use crate::synthesis::{
    inference::Sequence,
    stages::f0::TimedPhone,
    voice::{Result, VoiceError, resolve_explicit},
};

impl Frontend<'_> {
    pub fn linguistic(&self, phones: &[TimedPhone]) -> Result<PhoneInput> {
        let mut raw = Sequence::zeros(phones.len(), self.width)?;
        let mut language_ids = Vec::with_capacity(phones.len());
        let mut noise_ids = Vec::with_capacity(phones.len());
        let mut silence=Vec::with_capacity(phones.len());
        for (phone, row) in phones.iter().zip(raw.values.chunks_exact_mut(self.width)) {
            let resolved =
                resolve_explicit(&self.languages, self.unified, &phone.language, &phone.token)?;
            silence.push(resolved.source_class.unwrap_or("silence")=="silence");
            if resolved.unified_phone_index >= self.width - 1 {
                return Err(VoiceError::new(
                    "SV1 timbre phone exceeds its one-hot feature width",
                ));
            }
            row[resolved.unified_phone_index] = 1.0;
            let seconds = phone.frames as f32 * self.frame_seconds;
            row[self.width - 1] = (seconds + 0.05).ln();
            language_ids.push(resolved.language_index);

            let source = (!phone.language.is_empty())
                .then(|| {
                    self.languages
                        .iter()
                        .find(|table| table.name.contains(&phone.language))
                })
                .flatten()
                .copied()
                .unwrap_or(self.unified);
            let noise_id = source
                .lookup(&phone.token)
                .or_else(|| source.lookup("sil"))
                .map(|entry| {
                    i32::try_from(entry.stored_index).map_err(|_| {
                        VoiceError::new("Compatible acoustic phone ID exceeds signed index range")
                    })
                })
                .transpose()?
                .unwrap_or(-1);
            noise_ids.push(noise_id);
        }
        let mut normalized = Sequence::zeros(phones.len(), self.width)?;
        if self.contract == Contract::Native {
            self.linguistic.normalize_native(&raw.values, &mut normalized.values, true)
        } else {
            self.linguistic.normalize(&raw.values, &mut normalized.values, true)
        }.map_err(error)?;
        if self.contract == Contract::Native {

            for (row, id) in normalized
                .values
                .chunks_exact(self.width)
                .zip(&mut noise_ids)
            {
                let mut maximum = 0.0;
                *id = 0;
                for (index, &value) in row[..self.width - 1].iter().enumerate() {
                    if value > maximum {
                        maximum = value;
                        *id = i32::try_from(index).map_err(|_| {
                            VoiceError::new("SV1 acoustic phone ID exceeds signed index range")
                        })?;
                    }
                }
            }
        }
        Ok(PhoneInput {
            normalized,
            language_ids,
            noise_ids,
            silence,
        })
    }
}
