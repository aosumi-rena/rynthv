use super::{PreparedModule, PreparedTree};
use crate::synthesis::voice::{PhoneSet, ResolvedPhone, Result, VoiceError};

pub(super) struct PhoneTables {
    languages: Vec<usize>,
    unified: usize,
}

impl PreparedTree {
    fn phone_tables(&self) -> Result<&PhoneTables> {
        self.phone_tables.get_or_init(|| {
            let mut languages = Vec::new();
            let mut unified = None;
            for (index,node) in self.nodes.iter().enumerate() {
                if let PreparedModule::PhoneSet(table)=&node.module {
                    if table.name=="multiphone" {
                        if unified.replace(index).is_some() {
                            return Err(VoiceError::new("Model has multiple unified phone tables; routing is unresolved"));
                        }
                    } else {languages.push(index);}
                }
            }
            Ok(PhoneTables {languages,unified:unified.ok_or_else(||VoiceError::new("Model has no unified phone table"))?})
        }).as_ref().map_err(Clone::clone)
    }

    fn phone_set(&self,index:usize)->&PhoneSet {
        let PreparedModule::PhoneSet(table)=&self.nodes[index].module else {unreachable!("checked phoneset index")};
        table
    }

    fn language_phone_set(&self,language:&str)->Result<Option<(usize,&PhoneSet)>> {
        Ok(self.phone_tables()?.languages.iter().enumerate().find_map(|(index,&node)| {
            let table=self.phone_set(node);
            (!language.is_empty() && table.name.contains(language)).then_some((index,table))
        }))
    }

    pub fn resolve_phone_alphabet(&self, language: &str, token: &str, xsampa: bool) -> Result<ResolvedPhone<'_>> {
        if !xsampa {return self.resolve_phone(language,token);}
        let (language_index,table)=self.language_phone_set(language)?
            .ok_or_else(||VoiceError::new(format!("Model has no language conditioning for {language}")))?;
        super::super::phones::resolve_unified(language_index,table,self.phone_set(self.phone_tables()?.unified),token)
    }

    pub fn model_phone_token(&self, language: &str, system: &str, xsampa: &str) -> Result<String> {
        let (_,table)=self.language_phone_set(language)?
            .ok_or_else(|| VoiceError::new(format!("Model has no {language} phone table")))?;
        // Resolve Korean spellings using each stage's own inventory.
        if table.name=="korean-xsampa-phones" {
            self.resolve_phone_alphabet(language,xsampa,true)?;
            return Ok(format!("xsampa:{xsampa}"));
        }
        // Prefer language-local identities across model generations.
        if let Some(phone)=table.lookup(system) {
            return Ok(phone.symbol.to_owned());
        }
        table.mapped_symbols.iter().take(table.symbols.len()).position(|mapped| mapped == xsampa)
            .map(|index| table.symbols[index].clone())
            .ok_or_else(|| VoiceError::new(format!("Model has no {language} mapping for {xsampa}")))
    }

    pub fn resolve_phone(&self, language: &str, token: &str) -> Result<ResolvedPhone<'_>> {
        super::super::phones::resolve_selected(self.language_phone_set(language)?,
            self.phone_set(self.phone_tables()?.unified),token)
    }
}
