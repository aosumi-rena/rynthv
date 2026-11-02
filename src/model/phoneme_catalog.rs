use super::Language;
use std::{collections::HashMap, path::Path};

/// Installed phoneme spellings, descriptions and alphabet conversions.
#[derive(Clone, Debug)]
pub struct PhonemeEntry {
    pub language: Option<Language>,
    pub symbol: String,
    pub xsampa: String,
    pub category: String,
    pub example: String,
}

#[derive(Clone, Debug)]
pub struct PhonemeCatalog { pub entries: Vec<PhonemeEntry> }

impl PhonemeCatalog {
    pub fn cached(directory: &Path) -> Result<std::sync::Arc<Self>, String> {
        use std::sync::{Arc, Mutex, OnceLock};
        type Cache = HashMap<std::path::PathBuf, (Option<std::time::SystemTime>, Result<Arc<PhonemeCatalog>, String>)>;
        static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
        let stamp=std::fs::metadata(directory.join("phoneme-table.tsv")).ok().and_then(|m|m.modified().ok());
        let mut cache=CACHE.get_or_init(Default::default).lock().map_err(|_|"音素表缓存不可用")?;
        if let Some((previous,value))=cache.get(directory) && *previous==stamp { return value.clone(); }
        let value=Self::load(directory).map(Arc::new);
        cache.insert(directory.to_owned(),(stamp,value.clone()));
        value
    }
    pub fn load(directory: &Path) -> Result<Self, String> {
        let source = std::fs::read_to_string(directory.join("phoneme-table.tsv"))
            .map_err(|e| format!("phoneme-table.tsv: {e}"))?;
        let mut lines = source.trim_start_matches('\u{feff}').lines();
        if lines.next() != Some("Language\tPhoneme (System)\tPhoneme (XSAMPA)\tSys. Category\tExample") {
            return Err("无法识别音素表列格式".into());
        }
        let mut entries = Vec::new();
        for (index, line) in lines.enumerate() {
            if line.trim().is_empty() { continue; }
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != 5 || fields[1].is_empty() || fields[3].is_empty() {
                return Err(format!("音素表第 {} 行无效", index + 2));
            }
            let language = if fields[0] == "Common" { None } else {
                Some(Language::parse(&fields[0].to_ascii_lowercase())
                    .ok_or_else(|| format!("音素表语言无效：{}", fields[0]))?)
            };
            if entries.iter().any(|e: &PhonemeEntry| e.language == language && e.symbol == fields[1]) {
                return Err(format!("音素表有重复符号：{}", fields[1]));
            }
            entries.push(PhonemeEntry { language, symbol: fields[1].into(),
                xsampa: if fields[2].is_empty() {fields[1]} else {fields[2]}.into(),
                category: fields[3].into(), example: fields[4].into() });
        }
        if entries.is_empty() { return Err("音素表为空".into()); }
        Ok(Self { entries })
    }

    pub fn for_language(&self, language: Language) -> impl Iterator<Item=&PhonemeEntry> {
        self.entries.iter().filter(move |entry| entry.language.is_none() || entry.language == Some(language))
    }

    pub fn system(&self, language: Language, symbol: &str) -> Option<&PhonemeEntry> {
        self.for_language(language).find(|entry| entry.symbol == symbol)
    }

    pub fn xsampa(&self, language: Language, symbol: &str) -> Option<&PhonemeEntry> {
        self.for_language(language).find(|entry| entry.xsampa == symbol)
    }

    pub fn unified(&self) -> Vec<&PhonemeEntry> {
        let mut seen=std::collections::HashSet::new();
        self.entries.iter().filter(|entry|seen.insert(entry.xsampa.as_str())).collect()
    }

    pub fn convert(&self, language: Language, text: &str, to_xsampa: bool) -> Result<String, String> {
        text.split_whitespace().map(|symbol| {
            let entry = self.for_language(language).find(|entry|
                if to_xsampa {entry.symbol == symbol} else {entry.xsampa == symbol})
                .ok_or_else(|| format!("{}不支持音素：{symbol}", language.label()))?;
            Ok(if to_xsampa {entry.xsampa.clone()} else {entry.symbol.clone()})
        }).collect::<Result<Vec<_>, String>>().map(|phones| phones.join(" "))
    }

    pub fn classes(&self, language: Language) -> HashMap<String, String> {
        self.for_language(language).map(|entry| (entry.symbol.clone(), entry.category.clone())).collect()
    }
}
