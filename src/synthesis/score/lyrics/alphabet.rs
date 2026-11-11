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
        if crate::synthesis::voice::is_converted_phone(token) {
            if self.conversion.is_none() {return Err(VoiceError::new(crate::i18n::t("PHONE_CONVERSION_REQUIRED")))}
            tree.resolve_phone(language.id(),token)?;return Ok(token.into());
        }
        if self.conversion.is_some() {
            self.catalog.system(language,token).ok_or_else(||VoiceError::new(format!("{}不支持音素：{token}",language.label())))?;
            return Ok(token.into());
        }
        if self.local_alphabet {return tree.native_phone_token(language.id(),token);}
        let entry=self.catalog.system(language,token)
            .ok_or_else(||VoiceError::new(format!("{}不支持音素：{token}",language.label())))?;
        if entry.language.is_none() {return tree.native_phone_token(language.id(),token);}
        tree.model_phone_token(language.id(),&entry.symbol,&entry.xsampa)
    }

    pub(in crate::synthesis::score) fn input_token(&self,language:Language,token:&str,xsampa:bool,tree:&PreparedTree)->Result<String> {
        if self.conversion.is_some() {
            return (if xsampa {self.catalog.xsampa(language,token)}else{self.catalog.system(language,token)})
                .map(|entry|entry.symbol.clone()).ok_or_else(||VoiceError::new(format!("{}不支持音素：{token}",language.label())));
        }
        if !self.xsampa(xsampa) {return self.model_token(language,token,tree);}
        if let Some(entry)=self.catalog.xsampa(language,token) {
            return self.model_token(language,&entry.symbol,tree);
        }
        tree.resolve_phone_alphabet(language.id(),token,true)?;
        Ok(format!("xsampa:{token}"))
    }

    pub(in crate::synthesis::score) fn display_token(&self,language:Language,token:&str,xsampa:bool,tree:&PreparedTree)->Result<String> {
        if self.conversion.is_some() && !crate::synthesis::voice::is_converted_phone(token) {
            let entry=self.catalog.system(language,token)
                .ok_or_else(||VoiceError::new(format!("{}不支持音素：{token}",language.label())))?;
            return Ok(if self.xsampa(xsampa) {entry.xsampa.clone()} else {entry.symbol.clone()});
        }
        if self.local_alphabet {return tree.native_phone_token(language.id(),token);}
        let xsampa=self.xsampa(xsampa);
        if let Some(symbol)=crate::synthesis::voice::resource_phone_symbol(token) {
            return if xsampa {self.catalog.convert(language,symbol,true).map_err(VoiceError::new)}else{Ok(symbol.into())};
        }
        if let Some(symbol)=token.strip_prefix("xsampa:") {
            return if xsampa {Ok(symbol.into())}else{self.catalog.convert(language,symbol,false).map_err(VoiceError::new)};
        }
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
