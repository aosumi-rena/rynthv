use super::LyricFrontend;
use crate::{model::Language,synthesis::{score::ScoreNote,voice::{self,PreparedTree,Result,VoiceError}}};

impl LyricFrontend {
    pub fn conversion_identity(&self) -> Option<&'static str> {self.conversion.as_ref().map(|value|value.identity())}

    pub(in crate::synthesis::score) fn converting(&self) -> bool {self.conversion.is_some()}

    pub(in crate::synthesis::score) fn phone_class<'a>(&'a self,language:Language,token:&str,tree:&'a PreparedTree)->Result<Option<&'a str>> {
        if self.conversion.is_some() && !voice::is_converted_phone(token) {
            let entry=self.catalog.system(language,token).ok_or_else(||VoiceError::new(format!("{}不支持音素：{token}",language.label())))?;
            return Ok(Some(entry.category.as_str()));
        }
        Ok(self.resolve_token(language,token,tree)?.source_class)
    }

    pub(in crate::synthesis::score) fn convert_tokens(&self,language:Language,tokens:&[String])->Result<Vec<String>> {
        let Some(conversion)=&self.conversion else {return Ok(tokens.to_vec())};
        let converted=conversion.convert(language.id(),tokens)?;
        let mut output=tokens.iter().map(|token|voice::converted_phone_token(token,token)).collect::<Vec<_>>();
        for ((slot,original),native) in output.iter_mut().zip(tokens).zip(converted) {
            *slot=voice::converted_phone_token(original,&native);
        }
        Ok(output)
    }

    pub(in crate::synthesis::score) fn convert_score(&self,notes:&mut [ScoreNote],tree:&PreparedTree)->Result<()> {
        if self.conversion.is_none() {return Ok(())}
        let mut pending=Vec::new();let mut language=None;
        for index in 0..notes.len() {
            if notes[index].kind!=0 || notes[index].note_error.is_some() {
                self.convert_score_run(notes,&mut pending,&mut language,tree)?;
                continue;
            }
            for phone in 0..notes[index].phones.len() {
                let current=notes[index].phones[phone].language.clone();
                if language.as_ref().is_some_and(|previous|previous!=&current) {
                    self.convert_score_run(notes,&mut pending,&mut language,tree)?;
                }
                if pending.len()>=65536 {return Err(VoiceError::new(crate::i18n::t("PHONE_CONVERSION_LIMIT")))}
                language=Some(current);pending.push((index,phone));
            }
        }
        self.convert_score_run(notes,&mut pending,&mut language,tree)
    }

    fn convert_score_run(&self,notes:&mut [ScoreNote],pending:&mut Vec<(usize,usize)>,language:&mut Option<String>,tree:&PreparedTree)->Result<()> {
        if pending.is_empty() {*language=None;return Ok(())}
        let label=language.take().ok_or_else(||VoiceError::new("Missing conversion language"))?;
        let language=Language::parse(&label)
            .ok_or_else(||VoiceError::new("Unsupported conversion language"))?;
        let tokens=pending.iter().map(|&(note,phone)|notes[note].phones[phone].token.clone()).collect::<Vec<_>>();
        let converted=self.convert_tokens(language,&tokens)?;
        for (&(note,index),replacement) in pending.iter().zip(converted) {
            tree.resolve_phone(language.id(),&replacement)?;
            let phone=&mut notes[note].phones[index];
            if phone.display_token.is_none() {phone.display_token=Some(phone.token.clone());}
            if phone.vocoder_token.is_none() {phone.vocoder_token=Some(phone.token.clone());}
            phone.token=replacement;
        }
        pending.clear();
        Ok(())
    }
}
