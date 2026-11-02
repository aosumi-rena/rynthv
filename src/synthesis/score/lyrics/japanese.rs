use super::dictionary::{PhoneTable, Phones, WordReadings, read, unknown};
use crate::synthesis::voice::{Result, VoiceError};
use std::{collections::HashMap, path::Path};

pub(super) struct Japanese {
    phones: PhoneTable,
    kana: HashMap<char, String>,
    words: WordReadings,
}

impl Japanese {
    pub(super) fn load(directory: &Path) -> Result<Self> {
        let phones = romaji(directory)?;
        let mut kana = HashMap::new();
        for name in [
            "japanese-hira2romaji-dict.txt",
            "japanese-kata2romaji-dict.txt",
            "japanese-sute2romaji-dict.txt",
        ] {
            for line in read(&directory.join(name))?.lines() {
                let mut fields = line.split_whitespace();
                if let (Some(key), Some(value)) = (fields.next(), fields.next()) {
                    let mut chars = key.chars();
                    if let Some(key) = chars.next()
                        && chars.next().is_none()
                    {
                        kana.insert(key, value.to_owned());
                    }
                }
            }
        }
        let words = WordReadings::load(&directory.join("edict.txt"))?;
        Ok(Self {
            phones,
            kana,
            words,
        })
    }

    pub(super) fn resolve(&self, lyrics: &str) -> Result<Vec<Phones>> {
        let mut result = Vec::new();
        for token in lyrics
            .split(|ch: char| ch.is_whitespace() || ch == '\'' || ch == '’')
            .filter(|s| !s.is_empty())
        {
            if token.is_ascii() {
                result.extend(self.phones.segment(&token.to_lowercase())?);
            } else if token
                .chars()
                .all(|ch| self.kana.contains_key(&ch) || matches!(ch, 'ー' | 'っ' | 'ッ'))
            {
                result.extend(self.resolve_kana(token)?);
            } else {
                for reading in self.words.resolve(token)? {
                    result.extend(self.resolve_kana(&reading)?);
                }
            }
        }
        if result.is_empty() {
            return Err(unknown(lyrics));
        }
        Ok(result)
    }

    fn resolve_kana(&self, text: &str) -> Result<Vec<Phones>> {
        let mut result: Vec<Phones> = Vec::new();
        let mut pending = Vec::new();
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if matches!(ch, 'っ' | 'ッ') {
                pending.push("cl".to_owned());
                continue;
            }
            if ch == 'ー' {
                let vowel = result
                    .last()
                    .and_then(|phones| phones.last())
                    .filter(|phone| matches!(phone.as_str(), "a" | "i" | "u" | "e" | "o"))
                    .ok_or_else(|| VoiceError::new("日语长音前缺少元音，请填写罗马音或显式音素"))?;
                result.push(vec![vowel.clone()]);
                continue;
            }
            let mut romanized = self
                .kana
                .get(&ch)
                .cloned()
                .ok_or_else(|| unknown(&ch.to_string()))?;
            if let Some(&next) = chars.peek()
                && let Some(small) = small_kana(next)
            {
                let base = romanized.trim_end_matches(['a', 'i', 'u', 'e', 'o']);
                romanized = if let Some(vowel) = small.strip_prefix('y') {
                    if matches!(base, "sh" | "ch" | "j") {
                        format!("{base}{vowel}")
                    } else {
                        format!("{base}{small}")
                    }
                } else if romanized == "u" {
                    format!("w{small}")
                } else if matches!(romanized.as_str(), "ku" | "gu") {
                    format!("{base}w{small}")
                } else {
                    format!("{base}{small}")
                };
                chars.next();
            }
            pending.extend(self.phones.get(&romanized)?);
            result.push(std::mem::take(&mut pending));
        }
        if !pending.is_empty() {
            if let Some(last) = result.last_mut() {
                last.extend(pending);
            } else {
                result.push(pending);
            }
        }
        Ok(result)
    }
}

fn romaji(directory: &Path) -> Result<PhoneTable> {
    let mut table = PhoneTable::load(&directory.join("japanese-romaji-dict.txt"), false)?;
    let catalog = crate::model::PhonemeCatalog::load(directory).map_err(VoiceError::new)?;
    for entry in catalog.for_language(crate::model::Language::Japanese) {
        if matches!(
                entry.category.as_str(),
                "stop" | "fricative" | "affricate" | "aspirate" | "liquid" | "nasal" | "semivowel"
            )
        {

            table.include_literal(&entry.symbol);
        }
    }
    Ok(table)
}

fn small_kana(ch: char) -> Option<&'static str> {
    match ch {
        'ゃ' | 'ャ' => Some("ya"),
        'ゅ' | 'ュ' => Some("yu"),
        'ょ' | 'ョ' => Some("yo"),
        'ぁ' | 'ァ' => Some("a"),
        'ぃ' | 'ィ' => Some("i"),
        'ぅ' | 'ゥ' => Some("u"),
        'ぇ' | 'ェ' => Some("e"),
        'ぉ' | 'ォ' => Some("o"),
        _ => None,
    }
}
