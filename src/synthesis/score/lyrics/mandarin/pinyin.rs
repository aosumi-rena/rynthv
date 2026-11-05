use crate::synthesis::voice::{Result, VoiceError};
use std::{collections::HashMap, path::Path};

#[derive(Debug)]
pub(super) struct Pinyin {
    entries: HashMap<String, Vec<String>>,
    max_key_bytes: usize,
}

impl Pinyin {
    pub fn load(path: &Path) -> Result<Self> {
        let mut entries = HashMap::new();
        let mut max_key_bytes = 0;
        for (index, line) in super::super::dictionary::read(path)?.lines().enumerate() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split_whitespace();
            let key = fields.next().expect("nonempty line");
            let phones: Vec<String> = fields.map(str::to_owned).collect();
            if phones.is_empty() {
                return Err(VoiceError::new(format!(
                    "{}:{}: Mandarin syllable has no phonemes",
                    path.display(),
                    index + 1
                )));
            }
            max_key_bytes = max_key_bytes.max(key.len());
            if let Some(previous) = entries.insert(key.to_owned(), phones.clone())
                && previous != phones
            {
                return Err(VoiceError::new(format!(
                    "Conflicting Mandarin phoneme entries for {key:?}"
                )));
            }
        }
        if entries.is_empty() {
            return Err(VoiceError::new("Mandarin syllable dictionary is empty"));
        }
        Ok(Self {
            entries,
            max_key_bytes,
        })
    }

    pub fn syllable(&self, text: &str) -> Result<Vec<String>> {
        let mut key = normalize(text).ok_or_else(|| super::unknown(text))?;
        if key.ends_with(['0', '1', '2', '3', '4', '5']) {
            key.pop();
        }
        self.entries
            .get(&key)
            .cloned()
            .ok_or_else(|| super::unknown(text))
    }

    pub fn resolve(&self, text: &str) -> Result<Vec<Vec<String>>> {
        let key = normalize(text).ok_or_else(|| super::unknown(text))?;
        let bytes = key.as_bytes();
        let mut next = vec![None; key.len() + 1];
        next[key.len()] = Some((key.len(), key.len()));
        for start in (0..key.len()).rev() {
            if !bytes[start].is_ascii_lowercase() {
                continue;
            }
            let limit = key.len().min(start + self.max_key_bytes);

            for end in ((start + 1)..=limit).rev() {
                if !self.entries.contains_key(&key[start..end]) {
                    continue;
                }
                let after = if bytes
                    .get(end)
                    .is_some_and(|byte| matches!(byte, b'0'..=b'5'))
                {
                    end + 1
                } else {
                    end
                };
                if next[after].is_some() {
                    next[start] = Some((end, after));
                    break;
                }
            }
        }
        let mut start = 0;
        let mut result = Vec::new();
        while start < key.len() {
            let (end, after) = next[start].ok_or_else(|| super::unknown(text))?;
            result.push(self.entries[&key[start..end]].clone());
            start = after;
        }
        Ok(result)
    }
}

fn normalize(text: &str) -> Option<String> {
    let value = text.to_lowercase().replace("u:", "v");
    let mut key = String::with_capacity(value.len());
    for ch in value.chars() {
        key.push(match ch {
            'ā' | 'á' | 'ǎ' | 'à' => 'a',
            'ē' | 'é' | 'ě' | 'è' => 'e',
            'ī' | 'í' | 'ǐ' | 'ì' => 'i',
            'ō' | 'ó' | 'ǒ' | 'ò' => 'o',
            'ū' | 'ú' | 'ǔ' | 'ù' => 'u',
            'ü' | 'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => 'v',
            'a'..='z' | '0'..='5' => ch,
            _ => return None,
        });
    }
    (!key.is_empty()).then_some(key)
}
