// SPDX-License-Identifier: MIT
use super::{Error, Phrase, Result, UnitCostModel};
use std::{collections::HashMap, fs, path::Path};

type Table = HashMap<String, Vec<String>>;
pub(super) struct Phonetics {
    japanese: bool,
    direct: Table,
    kana: Table,
    small: Table,
    words: Table,
    known_japanese: Option<super::japanese_dictionary::KnownWords>,
}
impl Phonetics {
    pub fn open(directory: &Path, japanese: bool) -> Result<Self> {
        if japanese {
            let direct = table(&read(directory, "japanese-romaji-dict.txt")?, false)?;
            let mut kana = table(&read(directory, "japanese-hira2romaji-dict.txt")?, false)?;
            for (key, value) in table(&read(directory, "japanese-kata2romaji-dict.txt")?, false)? {
                kana.entry(key).or_insert(value);
            }
            Ok(Self {
                japanese,
                direct,
                kana,
                small: table(&read(directory, "japanese-sute2romaji-dict.txt")?, false)?,
                words: HashMap::new(),
                known_japanese: Some(super::japanese_dictionary::KnownWords::parse(&read(
                    directory,
                    "edict.txt",
                )?)?),
            })
        } else {
            Ok(Self {
                japanese,
                direct: table(&read(directory, "mandarin-xsampa-dict.txt")?, false)?,
                kana: HashMap::new(),
                small: HashMap::new(),
                words: table(&read(directory, "cedict.txt")?, true)?,
                known_japanese: None,
            })
        }
    }
    pub fn prepare(&self, phrase: &Phrase, model: &UnitCostModel)
        -> Result<super::lyric_assignment::Assignment> {
        use super::lyric_assignment::{Decoded, assign};
        let mut decoded = Vec::with_capacity(phrase.notes.len());
        for note in &phrase.notes {
            let value = if let Some(explicit) = note.pronunciation.as_deref().filter(|s| !s.is_empty()) {
                if hold(&note.lyric) {
                    Err(Error::Text("Held notes cannot specify phonemes".into()))
                } else { self.explicit_units(explicit, model).map(Decoded::Word) }
            } else if hold(&note.lyric) { Ok(Decoded::Hold) }
            else if note.lyric == "+" { Ok(Decoded::Plus) }
            else if note.lyric == "!sil" { Ok(Decoded::Rest) }
            else if let Some(explicit) = note.lyric.trim().strip_prefix('.') {
                self.explicit_units(explicit, model).map(Decoded::Word)
            } else {
                self.word_units(&note.lyric).map(Decoded::Word)
            };
            let value = match value {
                Ok(Decoded::Word(units)) if units.is_empty() || units.iter().any(|u| u.is_empty() ||
                    u.iter().any(|p| p.is_empty() || model.category(p) == 10)) =>
                    Decoded::Failure(format!("Unknown phoneme: {}", units.iter().flatten().find(|p| p.is_empty() || model.category(p) == 10).map(String::as_str).unwrap_or(""))),
                Ok(value) => value,
                Err(Error::Text(message)) => Decoded::Failure(message),
                Err(Error::Unsupported(message)) => Decoded::Failure(message.to_owned()),
                Err(error) => return Err(error),
            };
            decoded.push(value);
        }
        if !self.japanese {
            self.contextual_mandarin(phrase, &mut decoded)?;
        }
        assign(phrase, decoded)
    }
    fn word_units(&self, value: &str) -> Result<Vec<Vec<String>>> {
        let mut output = Vec::new();
        for word in value.split_whitespace() {
            if self.japanese { output.extend(self.japanese_word(word)?); }
            else if hanzi(word) {
                for reading in self.read_word(word)? { output.push(self.pinyin(&reading)?); }
            } else { output.push(self.pinyin(word)?); }
        }
        if output.is_empty() { return Err(Error::Text(format!("No pronunciation for: {value}"))); }
        Ok(output)
    }
    fn explicit_units(&self, text: &str, model: &UnitCostModel) -> Result<Vec<Vec<String>>> {
        let phones = text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        if let Some(phone) = phones.iter().find(|p| p.is_empty() || model.category(p) == 10) {
            return Err(Error::Text(if phone.is_empty() { "Phoneme list contains an empty entry".into() }
                else { format!("Unknown phoneme: {phone}") }));
        }
        let mut units = vec![Vec::new()];
        let mut nucleus = false;
        for phone in phones {
            if model.category(&phone) <= 1 {
                if nucleus { units.push(Vec::new()); }
                nucleus = true;
            }
            units.last_mut().unwrap().push(phone);
        }
        Ok(units)
    }
    fn contextual_mandarin(&self, phrase: &Phrase, decoded: &mut [super::lyric_assignment::Decoded])
        -> Result<()> {
        use super::lyric_assignment::Decoded;
        let mut begin = 0;
        while begin < phrase.notes.len() {
            let eligible = |i: usize| phrase.notes[i].pronunciation.as_deref().is_none_or(str::is_empty) &&
                (hanzi(&phrase.notes[i].lyric) && matches!(decoded[i], Decoded::Word(_)) ||
                 hold(&phrase.notes[i].lyric) && matches!(decoded[i], Decoded::Hold));
            if !hanzi(&phrase.notes[begin].lyric) || !eligible(begin) { begin += 1; continue; }
            let mut end = begin + 1;
            while end < phrase.notes.len() && eligible(end) && phrase.notes[end].local_tick ==
                phrase.notes[end-1].local_tick + phrase.notes[end-1].duration_ticks { end += 1; }
            let joined = phrase.notes[begin..end].iter().filter(|n| !hold(&n.lyric))
                .map(|n| n.lyric.as_str()).collect::<String>();
            let demands = (begin..end).map(|i| match &decoded[i] {
                Decoded::Word(units) => units.len(), Decoded::Hold => 0, _ => unreachable!(),
            }).collect::<Vec<_>>();
            let context = (|| -> Result<Vec<Vec<String>>> {
                let mut units = Vec::new();
                for word in self.segment(&joined) {
                    for reading in self.read_word(&word)? { units.push(self.pinyin(&reading)?); }
                }
                if units.len() != demands.iter().sum::<usize>() {
                    return Err(Error::Text("Contextual Mandarin pronunciation does not match the note syllables".into()));
                }
                Ok(units)
            })();
            match context {
                Ok(units) => {
                    let mut at = 0;
                    for (i, count) in demands.into_iter().enumerate() {
                        if count != 0 { decoded[begin+i] = Decoded::Word(units[at..at+count].to_vec()); }
                        at += count;
                    }
                }
                Err(Error::Text(_)) => for (offset, row) in decoded[begin..end].iter_mut().enumerate() {
                    *row = Decoded::Failure(format!("No pronunciation for: {}", phrase.notes[begin+offset].lyric));
                },
                Err(error) => return Err(error),
            }
            begin = end;
        }
        Ok(())
    }
    fn pinyin(&self, value: &str) -> Result<Vec<String>> {
        let token = value
            .strip_suffix(|c: char| c.is_ascii_digit())
            .unwrap_or(value)
            .to_ascii_lowercase();
        self.direct.get(&token).cloned().ok_or_else(|| {
            Error::Text(format!(
                "No pronunciation for: {value}"
            ))
        })
    }
    fn japanese_word(&self, value: &str) -> Result<Vec<Vec<String>>> {
        if let Some(phones) = self.direct.get(value) {
            return Ok(vec![phones.clone()]);
        }
        if let Some(reading) = self.kana.get(value) {
            if reading.len() == 1 {
                if let Some(phones) = self.direct.get(&reading[0]) {
                    return Ok(vec![phones.clone()]);
                }
            }
        }
        if value.bytes().all(|b| b.is_ascii_lowercase()) {
            return self.japanese_romaji(value);
        }
        self.known_japanese
            .as_ref()
            .ok_or(Error::Format("Japanese dictionary route"))?
            .resolve(value, &self.kana, &self.direct, &self.small)
    }
    fn japanese_romaji(&self, word: &str) -> Result<Vec<Vec<String>>> {
        let missing = || Error::Text(format!("No pronunciation for: {word}"));
        if word.is_empty() || word.len() > 256 { return Err(missing()); }
        let maximum = self.direct.keys().map(String::len).max().unwrap_or(0);
        let mut next = vec![None; word.len()+1];
        next[word.len()] = Some(word.len());
        for start in (0..word.len()).rev() {
            if word[start..].starts_with("nn") && next[start+1].is_some()
                && self.direct.get("n").is_some_and(|row| !row.is_empty()) {
                next[start] = Some(start+1); continue;
            }
            next[start] = (1..=maximum.min(word.len()-start)).rev().find_map(|length| {
                let end = start+length;
                (next[end].is_some() && self.direct.get(&word[start..end])
                    .is_some_and(|row| !row.is_empty())).then_some(end)
            });
        }
        if next[0].is_none() { return Err(missing()); }
        let mut output = Vec::new();
        let mut at = 0;
        let mut count = 0;
        while at < word.len() {
            let end = next[at].ok_or_else(missing)?;
            let phones = &self.direct[&word[at..end]];
            count += phones.len();
            if count > 4096 { return Err(missing()); }
            output.push(phones.clone()); at = end;
        }
        Ok(output)
    }
    fn read_word(&self, word: &str) -> Result<Vec<String>> {
        if let Some(reading) = self.words.get(word) {
            return Ok(reading.clone());
        }
        let mut result = Vec::new();
        for glyph in word.chars() {
            result.extend(
                self.words
                    .get(&glyph.to_string())
                    .ok_or_else(|| {
                        Error::Text(format!("No pronunciation for: {word}"))
                    })?
                    .iter()
                    .cloned(),
            );
        }
        Ok(result)
    }
    fn segment(&self, text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let mut forward = Vec::new();
        let mut at = 0;
        while at < chars.len() {
            let mut n = 6.min(chars.len() - at);
            while n > 1
                && !self
                    .words
                    .contains_key(&chars[at..at + n].iter().collect::<String>())
            {
                n -= 1;
            }
            forward.push(chars[at..at + n].iter().collect::<String>());
            at += n;
        }
        let mut backward = Vec::new();
        at = chars.len();
        while at > 0 {
            let mut n = 6.min(at);
            while n > 1
                && !self
                    .words
                    .contains_key(&chars[at - n..at].iter().collect::<String>())
            {
                n -= 1;
            }
            backward.push(chars[at - n..at].iter().collect::<String>());
            at -= n;
        }
        backward.reverse();
        if forward.len() < backward.len() {
            forward
        } else if forward.len() > backward.len() {
            backward
        } else {
            let fs = forward.iter().filter(|s| s.chars().count() == 1).count();
            let bs = backward.iter().filter(|s| s.chars().count() == 1).count();
            if fs < bs { forward } else { backward }
        }
    }
}
pub(super) fn hold(s: &str) -> bool {
    matches!(s, "-" | "ー")
}
fn hanzi(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
}
fn read(directory: &Path, name: &str) -> Result<String> {
    let bytes = fs::read(directory.join(name))?;
    if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
        return Err(Error::Unsupported("dictionary extent"));
    }
    String::from_utf8(bytes).map_err(|_| Error::Format("dictionary UTF-8"))
}
fn table(text: &str, cedict: bool) -> Result<Table> {
    if text.contains('\0') || text.starts_with('\u{feff}') {
        return Err(Error::Format("Flat dictionary encoding"));
    }
    let mut table = HashMap::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let tokens: Vec<_> = line.split(' ').collect();
        if cedict {
            if tokens.len() < 3 || tokens[0].starts_with('#') {
                continue;
            }
            let mut reading: Vec<String> = tokens[2..].iter().map(|s| s.to_string()).collect();
            reading[0] = reading[0]
                .strip_prefix('[')
                .unwrap_or(&reading[0])
                .to_owned();
            let last = reading.len() - 1;
            reading[last] = reading[last]
                .strip_suffix(']')
                .unwrap_or(&reading[last])
                .to_owned();
            table
                .entry(tokens[0].to_owned())
                .or_insert_with(|| reading.clone());
            table.entry(tokens[1].to_owned()).or_insert(reading);
        } else {
            table
                .entry(tokens[0].to_ascii_lowercase())
                .or_insert_with(|| tokens[1..].iter().map(|s| s.to_string()).collect());
        }
    }
    if table.is_empty() {
        return Err(Error::Format("empty dictionary"));
    }
    Ok(table)
}
