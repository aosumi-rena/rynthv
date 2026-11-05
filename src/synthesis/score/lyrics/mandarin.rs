use std::{collections::HashMap, fs, path::Path};

use crate::synthesis::voice::{Result, VoiceError};

mod pinyin;

#[derive(Debug)]
pub struct MandarinLexicon {
    pinyin: pinyin::Pinyin,
    words: HashMap<String, Vec<Vec<String>>>,
    longest_word: usize,
}

impl MandarinLexicon {
    pub fn load(directory: impl AsRef<Path>, legacy:bool) -> Result<Self> {
        let directory = directory.as_ref();
        let pinyin = pinyin::Pinyin::load(&directory.join(if legacy {"mandarin-xsampa-dict.txt"}else{"mandarin-dict.txt"}))?;
        let word_path = directory.join("cedict.txt");
        let mut words: HashMap<String, Vec<Vec<String>>> = HashMap::new();
        let mut longest_word = 0;
        for (index, line) in fs::read_to_string(&word_path)?.lines().enumerate() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((spellings, pronunciation)) = line.split_once('[') else {
                return Err(malformed_word(&word_path, index));
            };
            let Some(pronunciation) = pronunciation.strip_suffix(']') else {
                return Err(malformed_word(&word_path, index));
            };
            let spellings: Vec<_> = spellings.split_whitespace().collect();
            let pronunciation: Vec<_> = pronunciation
                .split_whitespace()
                .map(|syllable| syllable.to_lowercase().replace("u:", "v").replace('ü', "v"))
                .collect();
            if spellings.len() != 2 || pronunciation.is_empty() {
                return Err(malformed_word(&word_path, index));
            }
            for spelling in spellings {
                longest_word = longest_word.max(spelling.chars().count());
                let alternatives = words.entry(spelling.to_owned()).or_default();
                if !alternatives.contains(&pronunciation) {
                    alternatives.push(pronunciation.clone());
                }
            }
        }
        if words.is_empty() {
            return Err(VoiceError::new("Mandarin word dictionary is empty"));
        }
        Ok(Self {
            pinyin,
            words,
            longest_word,
        })
    }

    pub fn resolve(&self, lyrics: &str) -> Result<Vec<Vec<String>>> {
        let lyrics = lyrics.trim();
        if lyrics.is_empty() {
            return Err(VoiceError::new(
                "Empty Mandarin lyrics require explicit phonemes",
            ));
        }
        if self.words.contains_key(lyrics) {
            return self.resolve_word(lyrics);
        }
        let mut result = Vec::new();
        for token in lyrics.split(|ch: char| ch.is_whitespace() || ch == '\'' || ch == '’') {
            if token.is_empty() {
                continue;
            }
            if self.words.contains_key(token) {
                result.extend(self.resolve_word(token)?);
            } else if token.chars().any(is_han) {
                result.extend(self.resolve_phrase(token)?);
            } else {
                result.extend(self.pinyin.resolve(token)?);
            }
        }
        if result.is_empty() {
            return Err(VoiceError::new("Mandarin lyrics contain no syllables"));
        }
        Ok(result)
    }

    fn resolve_word(&self, word: &str) -> Result<Vec<Vec<String>>> {
        let alternatives = self.words.get(word).ok_or_else(|| unknown(word))?;
        let mut phoneme_readings = Vec::new();
        for alternative in alternatives {
            let phones = alternative
                .iter()
                .map(|syllable| self.pinyin.syllable(syllable))
                .collect::<Result<Vec<_>>>()?;
            if !phoneme_readings.contains(&phones) {
                phoneme_readings.push(phones);
            }
        }
        if phoneme_readings.len() > 1 {
            let readings = alternatives
                .iter()
                .map(|entry| entry.join(" "))
                .collect::<Vec<_>>();
            return Err(VoiceError::new(format!(
                "Ambiguous Mandarin word {word:?}: {}; provide explicit phonemes or pinyin",
                readings.join(" / ")
            )));
        }
        phoneme_readings.pop().ok_or_else(|| unknown(word))
    }

    fn resolve_phrase(&self, phrase: &str) -> Result<Vec<Vec<String>>> {
        let mut boundaries: Vec<_> = phrase.char_indices().map(|(index, _)| index).collect();
        boundaries.push(phrase.len());
        let mut start = 0;
        let mut result = Vec::new();
        while start + 1 < boundaries.len() {
            let last = (start + self.longest_word).min(boundaries.len() - 1);
            let matched = ((start + 1)..=last).rev().find(|&end| {
                self.words
                    .contains_key(&phrase[boundaries[start]..boundaries[end]])
            });
            let Some(end) = matched else {
                return Err(unknown(&phrase[boundaries[start]..]));
            };
            result.extend(self.resolve_word(&phrase[boundaries[start]..boundaries[end]])?);
            start = end;
        }
        Ok(result)
    }
}

fn malformed_word(path: &Path, zero_based_line: usize) -> VoiceError {
    VoiceError::new(format!(
        "{}:{}: expected traditional simplified [pinyin syllables]",
        path.display(),
        zero_based_line + 1
    ))
}

fn unknown(text: &str) -> VoiceError {
    VoiceError::new(format!(
        "No installed Mandarin pronunciation for {text:?}; provide explicit phonemes or spaced pinyin"
    ))
}

fn is_han(ch: char) -> bool {
    matches!(ch, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
        | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
}
