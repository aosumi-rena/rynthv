use crate::{
    model::Language,
    synthesis::voice::{Result, VoiceError},
};
use std::{collections::HashMap, fs, path::Path};

pub(super) type Phones = Vec<String>;

#[derive(Default)]
pub(super) struct PhoneTable {
    entries: HashMap<String, Readings>,
    max_key_bytes: usize,
}

#[derive(Default)]
struct Readings {
    alternatives:Vec<Phones>,
    last:usize,
}

impl PhoneTable {
    pub(super) fn load(path: &Path, english: bool) -> Result<Self> {
        let source = read(path)?;
        let mut table = Self::default();
        for line in source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with(['#', ';']))
        {
            let mut fields = line.trim_start_matches('\u{feff}').split_whitespace();
            let Some(key) = fields.next() else {
                continue;
            };
            let key = if english {
                key.split('(').next().unwrap_or(key)
            } else {
                key
            };
            let phones: Phones = fields
                .map(|phone| {
                    if english {

                        let phone = phone.to_lowercase();
                        if phone == "ah0" {"ax".into()}
                        else {phone.trim_end_matches(['0', '1', '2']).to_owned()}
                    } else {
                        phone.to_owned()
                    }
                })
                .collect();
            let key = key.to_lowercase();
            table.max_key_bytes = table.max_key_bytes.max(key.len());
            let readings = table.entries.entry(key).or_default();

            readings.last=match readings.alternatives.iter().position(|v|*v==phones) {
                Some(index)=>index,
                None=>{let index=readings.alternatives.len();readings.alternatives.push(phones);index},
            };
        }
        if table.entries.is_empty() {
            return Err(VoiceError::new(format!("词典为空：{}", path.display())));
        }
        Ok(table)
    }

    pub(super) fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub(super) fn include_literal(&mut self, phone: &str) {
        let key = phone.to_lowercase();
        self.max_key_bytes = self.max_key_bytes.max(key.len());
        self.entries
            .entry(key)
            .or_insert_with(|| Readings {alternatives:vec![vec![phone.to_owned()]],last:0});
    }

    pub(super) fn get(&self, key: &str) -> Result<Phones> {
        let alternatives = &self.entries.get(key).ok_or_else(|| unknown(key))?.alternatives;
        if alternatives.len() != 1 {
            return Err(VoiceError::new(format!(
                "{key:?} 有多个词典读音，请填写显式音素"
            )));
        }
        Ok(alternatives[0].clone())
    }

    fn first(&self, key: &str) -> Result<Phones> {

        self.entries.get(key).and_then(|readings| readings.alternatives.first()).cloned()
            .ok_or_else(|| unknown(key))
    }

    pub(super) fn last(&self,key:&str)->Result<Phones> {
        self.entries.get(key).and_then(|readings|readings.alternatives.get(readings.last)).cloned()
            .ok_or_else(||unknown(key))
    }

    pub(super) fn segment(&self, token: &str) -> Result<Vec<Phones>> {
        let mut rest = token;
        let mut output = Vec::new();
        while !rest.is_empty() {
            let end = rest
                .char_indices()
                .map(|(index, _)| index)
                .chain(std::iter::once(rest.len()))
                .take_while(|&end| end <= self.max_key_bytes)
                .filter(|&end| end > 0 && self.contains(&rest[..end]))
                .max()
                .ok_or_else(|| unknown(rest))?;
            let phones = self.get(&rest[..end])?;
            if !phones.is_empty() {
                output.push(phones);
            }
            rest = &rest[end..];
        }
        Ok(output)
    }
}

pub(super) struct Dictionary {
    language: Language,
    phones: PhoneTable,
    words: WordReadings,
    classes: HashMap<String,String>,
}

impl Dictionary {
    pub(super) fn load(directory: &Path, language: Language, catalog:&crate::model::PhonemeCatalog, legacy:bool) -> Result<Self> {
        let english = language == Language::English;
        let name = if english {
            if legacy && directory.join("english-arpabet-dict.txt").is_file() {"english-arpabet-dict.txt"}else{"cmudict-07b.txt"}.into()
        } else {
            format!("{}-dict.txt", if legacy {language.phone_set()}else{language.id()})
        };
        let phones = PhoneTable::load(&directory.join(name), english)?;
        let words = if language == Language::Cantonese {
            WordReadings::load(&directory.join("cedict-canto.txt"))?
        } else {
            WordReadings::default()
        };
        let classes = catalog.classes(language);
        Ok(Self {
            language,
            phones,
            words,
            classes,
        })
    }

    pub(super) fn resolve_with_coda(&self, lyrics: &str, coda_l:bool) -> Result<Vec<Phones>> {
        let mut result = Vec::new();
        for token in lyrics.split_whitespace() {
            if self.language == Language::Cantonese && !token.is_ascii() {
                for reading in self.words.resolve(token)? {
                    result.extend(self.romanized(&reading,coda_l)?);
                }
            } else {
                result.extend(self.romanized(token,coda_l)?);
            }
        }
        if result.is_empty() {
            return Err(unknown(lyrics));
        }
        Ok(result)
    }

    fn romanized(&self, token: &str, coda_l:bool) -> Result<Vec<Phones>> {
        let mut token = token.to_lowercase();

        if self.language == Language::English && token.len()>3 && token.ends_with("in'") {
            token.pop(); token.push('g');
        }
        match self.language {
            Language::English => {
                let mut phones=self.phones.first(&token)?;
                if coda_l {
                    // Distinguish clear and dark L within each word.
                    for i in 0..phones.len() {
                        if phones[i]=="l" && !phones.get(i+1).is_some_and(|next|
                            next=="y" || self.classes.get(next).is_some_and(|class|
                                matches!(class.as_str(),"vowel"|"diphthong"))) {
                            phones[i]="ll".into();
                        }
                    }
                }
                let classes:Vec<_>=phones.iter().map(|p|self.classes.get(p).map(String::as_str)).collect();
                Ok(super::english::split(phones,&classes))
            },
            Language::Cantonese => {
                Ok(vec![self.phones.get(
                    token.trim_end_matches(['1', '2', '3', '4', '5', '6']),
                )?])
            }

            _ => unreachable!("dedicated language adapters"),
        }
    }
}

pub(super) fn split_nuclei(phones: Phones, nuclei: &[String]) -> Vec<Phones> {
    let positions: Vec<_> = phones
        .iter()
        .enumerate()
        .filter_map(|(i, phone)| nuclei.contains(phone).then_some(i))
        .collect();
    let mut start = 0;
    let mut result = Vec::new();
    for &index in positions.iter().take(positions.len().saturating_sub(1)) {
        result.push(phones[start..=index].to_vec());
        start = index + 1;
    }
    if start < phones.len() {
        result.push(phones[start..].to_vec());
    }
    result
}

#[derive(Default)]
pub(super) struct WordReadings {
    entries: HashMap<String, Vec<String>>,
    max_key_bytes: usize,
}

impl WordReadings {
    pub(super) fn load(path: &Path) -> Result<Self> {
        let mut result = Self::default();
        for (index, line) in read(path)?.lines().enumerate() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (keys, reading) = line
                .split_once('[')
                .and_then(|(keys, rest)| Some((keys, rest.strip_suffix(']')?)))
                .ok_or_else(|| {
                    VoiceError::new(format!(
                        "{}:{}: 词典读音格式错误",
                        path.display(),
                        index + 1
                    ))
                })?;
            for key in keys.split_whitespace() {
                let values = result.entries.entry(key.to_owned()).or_default();

                let reading = reading
                    .split_whitespace()
                    .map(|s| s.trim_end_matches(['1', '2', '3', '4', '5', '6']))
                    .collect::<Vec<_>>()
                    .join(" ");
                if !values.contains(&reading) {
                    values.push(reading);
                }
                result.max_key_bytes = result.max_key_bytes.max(key.len());
            }
        }
        Ok(result)
    }

    pub(super) fn resolve(&self, text: &str) -> Result<Vec<String>> {
        let mut rest = text;
        let mut result = Vec::new();
        while !rest.is_empty() {
            let end = rest
                .char_indices()
                .map(|(index, _)| index)
                .chain(std::iter::once(rest.len()))
                .take_while(|&end| end <= self.max_key_bytes)
                .filter(|&end| end > 0 && self.entries.contains_key(&rest[..end]))
                .max()
                .ok_or_else(|| unknown(rest))?;
            let readings = &self.entries[&rest[..end]];
            if readings.len() != 1 {
                return Err(VoiceError::new(format!(
                    "{:?} 有多个读音，请使用注音或显式音素",
                    &rest[..end]
                )));
            }
            result.extend(readings[0].split_whitespace().map(str::to_owned));
            rest = &rest[end..];
        }
        Ok(result)
    }
}

pub(super) fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .map_err(|error| VoiceError::new(format!("无法读取发音词典 {}：{error}", path.display())))
}

pub(super) fn unknown(text: &str) -> VoiceError {
    VoiceError::new(format!(
        "词典未覆盖歌词 {text:?}，请使用对应语言的注音或显式音素"
    ))
}
