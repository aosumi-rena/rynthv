//! Convert editor phonemes for the selected model.
use super::LyricFrontend;
use crate::{model::Language,synthesis::voice::{PreparedTree,Result,VoiceError}};

impl LyricFrontend {
    pub(in crate::synthesis::score) fn resolve_token<'a>(&self,language:Language,token:&str,tree:&'a PreparedTree)
        ->Result<crate::synthesis::voice::ResolvedPhone<'a>> {
        match token.strip_prefix("xsampa:") {
            Some(symbol)=>tree.resolve_phone_alphabet(language.id(),symbol,true),
            None=>tree.resolve_phone(language.id(),token),
        }
    }
    pub(in crate::synthesis::score) fn model_token(&self,language:Language,token:&str,tree:&PreparedTree)->Result<String> {
        let entry=self.catalog.system(language,token)
            .ok_or_else(||VoiceError::new(format!("{}不支持音素：{token}",language.label())))?;
        tree.model_phone_token(language.id(),&entry.symbol,&entry.xsampa)
    }

    pub(in crate::synthesis::score) fn input_token(&self,language:Language,token:&str,xsampa:bool,tree:&PreparedTree)->Result<String> {
        if !xsampa {return self.model_token(language,token,tree);}
        if let Some(entry)=self.catalog.xsampa(language,token) {
            return tree.model_phone_token(language.id(),&entry.symbol,&entry.xsampa);
        }
        // Accept foreign phonemes only when the model supports them.
        tree.resolve_phone_alphabet(language.id(),token,true)?;
        // Keep foreign phonemes distinct from local spellings.
        Ok(format!("xsampa:{token}"))
    }

    pub(in crate::synthesis::score) fn display_token(&self,language:Language,token:&str,xsampa:bool,tree:&PreparedTree)->Result<String> {
        if let Some(symbol)=token.strip_prefix("xsampa:") {
            return if xsampa {Ok(symbol.into())}else{self.catalog.convert(language,symbol,false).map_err(VoiceError::new)};
        }
        // Prefer the local spelling of the same sound.
        let entry=self.catalog.system(language,token).filter(|entry|
            tree.model_phone_token(language.id(),&entry.symbol,&entry.xsampa).is_ok_and(|t|t==token))
            .or_else(||self.catalog.for_language(language).find(|entry|
                tree.model_phone_token(language.id(),&entry.symbol,&entry.xsampa).is_ok_and(|t|t==token)));
        if let Some(entry)=entry {return Ok(if xsampa {&entry.xsampa}else{&entry.symbol}.clone());}
        let resolved=tree.resolve_phone(language.id(),token)?;
        if xsampa {return Ok(resolved.unified_symbol.to_owned());}
        self.catalog.convert(language,resolved.unified_symbol,false).map_err(VoiceError::new)
    }
}
