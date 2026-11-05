mod phonology;

use super::dictionary::{PhoneTable, Phones, unknown};
use crate::synthesis::voice::Result;
use std::path::Path;

pub(super) struct Korean {
    phones: PhoneTable,
    legacy: bool,
}

impl Korean {
    pub fn load(directory: &Path, legacy:bool) -> Result<Self> {
        Ok(Self {
            phones: PhoneTable::load(&directory.join(if legacy {"korean-xsampa-dict.txt"}else{"korean-dict.txt"}), false)?,
            legacy,
        })
    }

    pub fn resolve(&self, lyrics: &str) -> Result<Vec<Phones>> {
        let mut output = Vec::new();
        for word in lyrics.split_whitespace() {
            let mut syllables = if word.is_ascii() {
                self.phones.segment(&word.to_lowercase())?
            } else {
                let parts = phonology::syllables(word).ok_or_else(|| unknown(word))?;
                let parts = phonology::rewrite(parts);
                parts
                    .into_iter()
                    .map(|part| self.phones.get(&part))
                    .collect::<Result<Vec<_>>>()?
            };
            // The dictionary distinguishes voiced onsets from unreleased codas;
            // a word-initial lax stop is the corresponding unvoiced onset.
            if !self.legacy && let Some(first) = syllables.first_mut().and_then(|s| s.first_mut()) {
                let initial = match first.as_str() {
                    "g" => Some("K"),
                    "d" => Some("T"),
                    "b" => Some("P"),
                    "j" => Some("J"),
                    _ => None,
                };
                if let Some(value) = initial {
                    *first = value.into();
                }
            }
            output.extend(syllables);
        }
        if output.is_empty() {
            return Err(unknown(lyrics));
        }
        Ok(output)
    }
}
