// SPDX-License-Identifier: MIT
use super::{Error, Result};
use std::collections::HashMap;

type Table = HashMap<String, Vec<String>>;
pub(super) struct KnownWords {
    readings: Table,
}
impl KnownWords {
    pub fn parse(text: &str) -> Result<Self> {
        if text.contains('\0') || text.starts_with('\u{feff}') {
            return Err(Error::Format("Japanese EDICT encoding"));
        }
        let mut readings: Table = HashMap::new();
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<_> = line.split_ascii_whitespace().collect();
            if fields.len() != 2 || fields[0].is_empty() {
                return Err(Error::Format("Japanese EDICT word/reading row"));
            }
            let reading = fields[1]
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .filter(|s| !s.is_empty())
                .ok_or(Error::Format("Japanese EDICT bracketed reading"))?;
            let entries = readings.entry(fields[0].to_owned()).or_default();
            if !entries.iter().any(|s| s == reading) {
                entries.push(reading.to_owned());
            }
        }
        if readings.is_empty() {
            return Err(Error::Format("empty Japanese EDICT"));
        }
        Ok(Self { readings })
    }

    pub fn resolve(&self, word: &str, kana: &Table, romaji: &Table, small: &Table) -> Result<Vec<Vec<String>>> {
        let reading = if !word.is_empty() && word.chars().all(kana_scalar) {
            word
        } else {
            let entries = self.readings.get(word).ok_or_else(|| {
                Error::Text(format!(
                    "No pronunciation for: {word}"
                ))
            })?;
            if entries.len() != 1 {
                return Err(Error::Text(format!(
                    "Ambiguous pronunciation: {word}"
                )));
            }
            &entries[0]
        };
        let mut syllables: Vec<String> = Vec::new();
        for glyph in reading.chars() {
            let key = glyph.to_string();
            if let Some(row) = small.get(&key) {
                let value = first(row, &key)?;
                if value == "tsu" {
                    syllables.push("cl".into());
                } else if let Some(previous) = syllables.last_mut() {
                    if previous.len() > 1 {
                        *previous = combine(previous, value);
                    } else if previous == "u" {
                        *previous = format!("w{value}");
                    } else if previous == "i" {
                        *previous = format!("y{value}");
                    } else {
                        syllables.push(value.to_owned());
                    }
                } else {
                    syllables.push(value.to_owned());
                }
                continue;
            }
            let roman = if let Some(row) = kana.get(&key) {
                first(row, &key)?.to_owned()
            } else if glyph == 'ー' {
                syllables.last().and_then(|s| s.chars().last()).map(|c| c.to_string())
                    .ok_or_else(|| Error::Text(format!("No pronunciation for: {word}")))?
            } else {
                return Err(Error::Text(format!("No pronunciation for: {word}")));
            };
            if syllables.last().is_some_and(|s| s == "cl") {
                *syllables.last_mut().unwrap() = format!("{}{roman}", &roman[..1]);
            } else {
                syllables.push(roman);
            }
        }
        let mut output = Vec::new();
        for syllable in syllables {
            let phones = romaji
                .get(&syllable)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    Error::Text(format!("No pronunciation for: {word}"))
                })?;
            output.push(phones.clone());
        }
        if output.is_empty() {
            return Err(Error::Text(
                format!("No pronunciation for: {word}"),
            ));
        }
        Ok(output)
    }
}
fn kana_scalar(c: char) -> bool {
    ('\u{3041}'..='\u{3096}').contains(&c) || ('\u{30a1}'..='\u{30fa}').contains(&c) || c == 'ー'
}
fn first<'a>(row: &'a [String], _key: &str) -> Result<&'a str> {
    let value = row.first().filter(|_| row.len() == 1)
        .filter(|s| !s.is_empty() && s.len() <= 256 && s.bytes().all(|b| (33..=126).contains(&b)))
        .ok_or_else(|| Error::Format("Japanese kana dictionary unit"))?;
    Ok(value)
}
fn combine(previous: &str, vowel: &str) -> String {
    if previous.len() == 2 {
        if previous == "ku" { return format!("kw{vowel}"); }
        if previous == "gu" { return format!("gw{vowel}"); }
        if ["hi", "ki", "gi", "mi", "ni", "bi", "pi", "ri"].contains(&previous) {
            return format!("{}y{}", &previous[..1], &vowel[vowel.len()-1..]);
        }
        return format!("{}{vowel}", &previous[..1]);
    }
    if previous.len() == 3 && vowel.len() == 1 {
        return format!("{}{vowel}", &previous[..2]);
    }
    format!("{}{}", &previous[..previous.len()-1], &vowel[1..])
}
