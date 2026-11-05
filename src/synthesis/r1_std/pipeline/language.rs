use super::{Result, ensure, StdError};
use std::{collections::{HashMap, HashSet}, path::Path};
pub(super) struct Language {
    words: HashMap<String, Vec<usize>>,
    ascii_lengths: Vec<usize>,
    kana: HashMap<char, String>,
    small: HashMap<char, String>,
    readings: HashMap<String, Vec<String>>,
    prefixes: HashSet<String>,
    lengths: Vec<usize>,
}
impl Language {
    pub fn load(dir: &Path, phones: &HashMap<String, usize>) -> Result<Self> {
        let mut words = HashMap::new();
        for line in std::fs::read_to_string(dir.join("japanese-romaji-dict.txt"))?.lines() {
            let mut fields = line.trim_end_matches('\r').split(' ');
            let key = fields.next().unwrap_or("");
            if key.is_empty() { continue; }
            let value = fields.map(|p| phones.get(p).copied().ok_or_else(||
                StdError::Invalid(format!("Unknown R1 resource phone {p}"))))
                .collect::<Result<Vec<_>>>()?;
            words.insert(key.to_owned(), value);
        }
        let mut ascii_lengths = words.iter().filter(|(key, value)|
            key.is_ascii() && !key.is_empty() && key.len() <= 256 && !value.is_empty())
            .map(|(key, _)| key.len()).collect::<Vec<_>>();
        ascii_lengths.sort_unstable_by(|a, b| b.cmp(a)); ascii_lengths.dedup();
        let mut kana = HashMap::new();
        let mut small = HashMap::new();
        for name in ["japanese-hira2romaji-dict.txt", "japanese-kata2romaji-dict.txt",
                     "japanese-sute2romaji-dict.txt"] {
            let target = if name == "japanese-sute2romaji-dict.txt" { &mut small } else { &mut kana };
            for line in std::fs::read_to_string(dir.join(name))?.lines() {
                if line.is_empty() { continue; }
                let fields = line.split(' ').collect::<Vec<_>>();
                ensure(fields.len() == 2 && fields[0].chars().count() == 1 &&
                    !fields[1].is_empty() && fields[1].len() <= 256 &&
                    fields[1].bytes().all(|b| (33..=126).contains(&b)), "Invalid R1 kana table")?;
                target.insert(fields[0].chars().next().unwrap(), fields[1].to_owned());
            }
        }
        let mut readings: HashMap<String, Vec<String>> = HashMap::new();
        for line in std::fs::read_to_string(dir.join("edict.txt"))?.lines() {
            if line.starts_with(['#', '/']) || line.is_empty() { continue; }
            let (key, value) = if let Some((key, value)) = line.split_once(" [") {
                (key, value.strip_suffix(']').ok_or_else(|| StdError::Invalid("Invalid R1 EDICT row".into()))?)
            } else {
                ensure(!line.chars().any(|c| c.is_control() || c == ' ' || c == '[' || c == ']'),
                    "Invalid R1 bare EDICT key")?;
                (line, "")
            };
            ensure(!key.is_empty() && key.len() <= 4096, "R1 EDICT key exceeds capacity")?;
            let row = readings.entry(key.to_owned()).or_default();
            if !row.iter().any(|r| r == value) { row.push(value.to_owned()); }
        }
        let mut prefixes: HashSet<String> = readings.keys().cloned().collect();
        let mut combinations = Vec::new();
        for (&glyph, modifier) in &small {
            if modifier == "tsu" { continue; }
            for base in kana.keys() { combinations.push(format!("{base}{glyph}")); }
        }
        combinations.extend(kana.keys().map(char::to_string));
        for key in combinations {
            prefixes.insert(key.clone());
            prefixes.insert(format!("\u{3063}{key}"));
            prefixes.insert(format!("\u{30c3}{key}"));
        }
        ensure(prefixes.len() <= 262144, "R1 normalizer exceeds prefix capacity")?;
        let mut lengths = prefixes.iter().map(String::len).collect::<Vec<_>>();
        lengths.sort_unstable_by(|a, b| b.cmp(a)); lengths.dedup();
        Ok(Self {words, ascii_lengths, kana, small, readings, prefixes, lengths})
    }
    pub fn resolve(&self, text: &str, phones: &HashMap<String, usize>, categories: &[(String, String)]) -> Result<Vec<Vec<usize>>> {
        ensure(!text.is_empty() && text.len() <= 4096, "R1 lyric exceeds capacity")?;
        if let Some(text) = text.strip_prefix('.') {
            let p = text.split(' ').map(|p| phones.get(p).copied().ok_or_else(||
                StdError::Unsupported(format!("Unknown phoneme: {p}"))))
                .collect::<Result<Vec<_>>>()?;
            let tier = |phone: usize| match categories[phone].1.as_str() {
                "vowel" | "diphthong" | "silence" => 0,
                "coda" => -1,
                _ => 1,
            };
            let mut groups: Vec<Vec<usize>> = Vec::new();
            for (i, &phone) in p.iter().enumerate() {
                if i == 0 || tier(phone) >= tier(p[i - 1]) { groups.push(Vec::new()); }
                if categories[phone].1 == "silence" && !groups.last().unwrap().is_empty() {
                    groups.push(Vec::new());
                }
                groups.last_mut().unwrap().push(phone);
            }
            return Ok(groups);
        }
        let mut out = Vec::new();
        for token in text.split_whitespace() {
            if token.is_ascii() {
                out.extend(self.ascii_syllables(token)?); continue;
            }
            let mut at = 0;
            while at < token.len() {
                let remainder = &token[at..];
                let key = self.lengths.iter().find_map(|&length| remainder.get(..length)
                    .filter(|key| self.prefixes.contains(*key))).ok_or_else(||
                    StdError::Unsupported("R1 Japanese normalizer has no resolved prefix".into()))?;
                let reading = if let Some(entries) = self.readings.get(key) {
                    ensure(entries.len() == 1, "Ambiguous R1 EDICT word; supply kana or explicit phones")?;
                    if entries[0].is_empty() { key } else { &entries[0] }
                } else { key };
                for mora in self.romanize(reading)? { out.push(self.lookup(&mora)?); }
                at += key.len();
            }
        }
        ensure(!out.is_empty(), "R1 lyric produced no phones")?;
        Ok(out)
    }
    fn lookup(&self, mora: &str) -> Result<Vec<usize>> {
        self.words.get(mora).filter(|row| !row.is_empty()).cloned().ok_or_else(||
            StdError::Unsupported(format!("No R1 mora pronunciation for {mora}")))
    }
    fn ascii_syllables(&self, token: &str) -> Result<Vec<Vec<usize>>> {
        if self.words.contains_key(token) { return Ok(vec![self.lookup(token)?]); }
        ensure(!token.is_empty() && token.len() <= 256 && token.is_ascii(),
            "R1 romaji word exceeds its bounded ASCII domain")?;
        let mut next = vec![None; token.len() + 1];
        next[token.len()] = Some(token.len());
        for start in (0..token.len()).rev() {
            if token[start..].starts_with("nn") && next[start + 1].is_some()
                && self.words.get("n").is_some_and(|row| !row.is_empty()) {
                next[start] = Some(start + 1); continue;
            }
            for &length in &self.ascii_lengths {
                let end = start + length;
                if end <= token.len() && next[end].is_some()
                    && self.words.get(&token[start..end]).is_some_and(|row| !row.is_empty()) {
                    next[start] = Some(end); break;
                }
            }
        }
        ensure(next[0].is_some(), "R1 romaji word has no complete dictionary segmentation")?;
        let mut result = Vec::new();
        let mut start = 0;
        let mut count = 0;
        while start < token.len() {
            let end = next[start].ok_or_else(|| StdError::Invalid("Incomplete R1 dictionary segmentation".into()))?;
            let phones = self.lookup(&token[start..end])?;
            count += phones.len();
            ensure(count <= 4096, "R1 segmented word exceeds phone capacity")?;
            result.push(phones); start = end;
        }
        Ok(result)
    }
    fn romanize(&self, reading: &str) -> Result<Vec<String>> {
        let mut result: Vec<String> = Vec::new();
        for ch in reading.chars() {
            if let Some(modifier) = self.small.get(&ch) {
                if modifier == "tsu" { result.push("cl".into()); }
                else if let Some(previous) = result.last_mut() {
                    if previous.len() > 1 { *previous = combine(previous, modifier); }
                    else if previous == "u" { *previous = format!("w{modifier}"); }
                    else if previous == "i" { *previous = format!("y{modifier}"); }
                    else { result.push(modifier.clone()); }
                } else { result.push(modifier.clone()); }
                continue;
            }
            let roman = if let Some(value) = self.kana.get(&ch) { value.clone() }
                else if ch == '\u{30fc}' { result.last().and_then(|p| p.chars().last())
                    .map(|p| p.to_string()).ok_or_else(|| StdError::Unsupported("R1 long vowel lacks prior mora".into()))? }
                else { return Err(StdError::Unsupported(format!("No R1 reading for {ch}"))); };
            if result.last().is_some_and(|p| p == "cl") {
                *result.last_mut().unwrap() = format!("{}{roman}", &roman[..1]);
            } else { result.push(roman); }
        }
        for i in 1..result.len() {
            if result[i] == "i" && result[i-1].ends_with('e') { result[i] = "e".into(); }
            else if result[i] == "u" && result[i-1].ends_with('o') { result[i] = "o".into(); }
        }
        Ok(result)
    }
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
    if previous.len() == 3 && vowel.len() == 1 { return format!("{}{vowel}", &previous[..2]); }
    format!("{}{}", &previous[..previous.len()-1], &vowel[1..])
}
