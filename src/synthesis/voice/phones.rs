use serde::Serialize;
use std::collections::HashMap;
pub(super) mod resource;
pub use resource::{symbol as resource_phone_symbol, token as resource_phone_token, converted_token as converted_phone_token,is_converted as is_converted_phone};

use super::{Result, VoiceError, array, range, sum, u32_at};

pub const PHONE_SET_TYPE_ID: [u8; 16] = [
    0x4f, 0x0a, 0xa7, 0x7c, 0x74, 0xf4, 0xea, 0x4f, 0x14, 0x5d, 0xf4, 0x36, 0xab, 0xa8, 0xcb, 0x04,
];

#[derive(Clone, Debug, Serialize)]
pub struct PhoneSet {
    pub name: String,
    pub symbols: Vec<String>,
    pub classes_by_symbol: Vec<String>,
    pub mapped_symbols: Vec<String>,
    pub classes: Vec<String>,
    #[serde(skip)]
    indices: HashMap<String, usize>,
    #[serde(skip)]
    class_indices: HashMap<String, usize>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PhoneLookup<'a> {
    pub stored_index: usize,
    pub symbol: &'a str,
    pub class_index: usize,
    pub class: &'a str,
    pub mapped_symbol: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ResolvedPhone<'a> {
    pub language_index: usize,
    pub selected_phone_set: &'a str,
    pub local_stored_index: Option<usize>,

    pub source_class: Option<&'a str>,
    pub unified_phone_index: usize,
    pub unified_symbol: &'a str,
    pub used_silence_fallback: bool,
}

pub fn resolve_explicit<'a>(
    languages: &[&'a PhoneSet],
    unified: &'a PhoneSet,
    language_query: &str,
    token: &str,
) -> Result<ResolvedPhone<'a>> {
    let selected = (!language_query.is_empty())
        .then(|| {
            languages
                .iter()
                .position(|table| table.name.contains(language_query))
        })
        .flatten();
    resolve_selected(selected.map(|index|(index,languages[index])),unified,token)
}

pub(super) fn resolve_selected<'a>(selected:Option<(usize,&'a PhoneSet)>,
    unified:&'a PhoneSet,token:&str)->Result<ResolvedPhone<'a>> {
    let source = selected.map_or(unified, |(_,table)|table);
    let local = source.lookup(token);

    let mapped = local.and_then(|phone| phone.mapped_symbol).unwrap_or(token);
    let found = unified.lookup(mapped);
    let output = found.or_else(|| unified.lookup("sil")).ok_or_else(|| {
        VoiceError::new("Unified phone table has neither the mapped symbol nor sil")
    })?;
    Ok(ResolvedPhone {
        language_index: selected.map_or(0,|(index,_)|index),
        selected_phone_set: &source.name,
        local_stored_index: local.map(|phone| phone.stored_index),
        source_class: local.or(found).map(|phone| phone.class),
        unified_phone_index: output.stored_index,
        unified_symbol: output.symbol,
        used_silence_fallback: found.is_none(),
    })
}

pub fn resolve_alphabet<'a>(languages:&[&'a PhoneSet],unified:&'a PhoneSet,
    language:&str,token:&str,xsampa:bool)->Result<ResolvedPhone<'a>> {
    if resource_phone_symbol(token).is_some() {
        let (index,table)=languages.iter().enumerate().find(|(_,table)|table.name.contains(language))
            .ok_or_else(||VoiceError::new(format!("Model has no {language} phone table")))?;
        return resource::resolve(index,table,unified,token);
    }
    if !xsampa{return resolve_explicit(languages,unified,language,token);}
    let language_index=languages.iter().position(|table|!language.is_empty() && table.name.contains(language))
        .ok_or_else(||VoiceError::new(format!("Model has no language conditioning for {language}")))?;
    resolve_unified(language_index,languages[language_index],unified,token)
}

pub(super) fn resolve_unified<'a>(language_index:usize,_language:&'a PhoneSet,
    unified:&'a PhoneSet,token:&str)->Result<ResolvedPhone<'a>> {
    let phone=unified.lookup(token).ok_or_else(||VoiceError::new(format!("Unsupported X-SAMPA phoneme: {token}")))?;
    Ok(ResolvedPhone{language_index,selected_phone_set:&unified.name,local_stored_index:None,
        source_class:Some(phone.class),unified_phone_index:phone.stored_index,unified_symbol:phone.symbol,
        used_silence_fallback:false})
}

impl PhoneSet {

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        if array::<16>(bytes, 0)? != PHONE_SET_TYPE_ID {
            return Err(VoiceError::new("Resource is not a phone set"));
        }
        let mut reader = Reader { bytes, offset: 16 };
        let name = reader.string()?;
        let symbols = reader.strings()?;
        let classes_by_symbol = reader.strings()?;
        let mapped_symbols = reader.strings()?;
        let classes = reader.strings()?;
        if reader.offset != bytes.len() {
            return Err(VoiceError::new("Phone set has trailing bytes"));
        }
        if name.is_empty() || symbols.is_empty() || classes.is_empty() {
            return Err(VoiceError::new(
                "Phone set name and vocabulary must be nonempty",
            ));
        }
        if symbols.len() != classes_by_symbol.len() {
            return Err(VoiceError::new(
                "Phone set class count differs from symbol count",
            ));
        }
        validate_unique(&symbols, "symbols")?;
        validate_unique(&classes, "classes")?;
        if classes_by_symbol
            .iter()
            .any(|class| !classes.contains(class))
        {
            return Err(VoiceError::new("Phone set references an absent class"));
        }
        if mapped_symbols.len() < symbols.len() {
            return Err(VoiceError::new("Phone set is missing mapped symbols"));
        }

        let indices=symbols.iter().cloned().enumerate().map(|(i,key)|(key,i)).collect();
        let class_indices=classes.iter().cloned().enumerate().map(|(i,key)|(key,i)).collect();
        Ok(Self {
            name,
            symbols,
            classes_by_symbol,
            mapped_symbols,
            classes,
            indices,
            class_indices,
        })
    }

    pub fn lookup(&self, token: &str) -> Option<PhoneLookup<'_>> {
        let index = *self.indices.get(token)?;
        let class = self.classes_by_symbol.get(index)?;
        Some(PhoneLookup {
            stored_index: index,
            symbol: &self.symbols[index],
            class_index: *self.class_indices.get(class)?,
            class,
            mapped_symbol: self.mapped_symbols.get(index).map(String::as_str),
        })
    }
}

fn validate_unique(values: &[String], field: &str) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        if value.is_empty() || values[..index].contains(value) {
            return Err(VoiceError::new(format!(
                "Phone set {field} contain an empty or duplicate entry"
            )));
        }
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn count(&mut self) -> Result<usize> {
        let value = u32_at(self.bytes, self.offset)? as usize;
        self.offset = sum(self.offset, 4)?;
        Ok(value)
    }

    fn string(&mut self) -> Result<String> {
        let length = self.count()?;
        let bounds = range(self.offset, length, self.bytes.len())?;
        let value = std::str::from_utf8(&self.bytes[bounds.clone()])
            .map_err(|_| VoiceError::new("Phone set contains invalid UTF-8"))?;
        self.offset = bounds.end;
        Ok(value.to_owned())
    }

    fn strings(&mut self) -> Result<Vec<String>> {
        let count = self.count()?;
        if count > (self.bytes.len() - self.offset) / 4 {
            return Err(VoiceError::new(
                "Phone set string count exceeds remaining bytes",
            ));
        }

        let mut values = Vec::new();
        for _ in 0..count {
            values.push(self.string()?);
        }
        Ok(values)
    }
}
