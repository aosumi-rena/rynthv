mod onset;
mod french;
mod portuguese;
mod german;
pub(in crate::synthesis::score) use onset::split;

use super::dictionary::{PhoneTable};
use crate::{model::Language,synthesis::voice::{Result,VoiceError}};
use std::path::Path;

pub(super) struct European {
    language:Language,
    words:PhoneTable,
    classes:std::collections::HashMap<String,String>,
    legacy:bool,
}

impl European {
    pub(super) fn phone_class(&self, token: &str) -> Option<&str> {
        self.classes.get(token).map(String::as_str)
    }

    pub fn load(directory:&Path,language:Language,catalog:&crate::model::PhonemeCatalog,legacy:bool)->Result<Self> {
        let name=match language {
            Language::French=>"french-dict.txt",
            Language::German=>"german-dict.txt",
            Language::Portuguese=>"portuguese-brazil-dict.txt",
            Language::Italian=>"italian-dict.txt",
            _=>return Err(VoiceError::new("无效的欧洲语言发音前端")),
        };
        let name=if legacy {match language {
            Language::French=>"french-xsampa-dict.txt",Language::German=>"german-xsampa-dict.txt",
            Language::Portuguese=>"portuguese-brazil-xsampa-dict.txt",_=>name,
        }}else{name};
        let classes=catalog.classes(language);
        Ok(Self {language,words:PhoneTable::load(&directory.join(name),false)?,classes,legacy})
    }

    pub fn resolve(&self,lyrics:&str)->Result<Vec<Vec<String>>> {
        lyrics.split_whitespace().map(|word|self.word(word)).collect::<Result<Vec<_>>>()
            .map(|words|words.into_iter().flatten().collect())
    }

    fn word(&self,word:&str)->Result<Vec<Vec<String>>> {
        let phones=self.words.last(&word.to_lowercase())?;
        let nuclei=phones.iter().enumerate().filter_map(|(i,p)|
            self.classes.get(p).filter(|c|matches!(c.as_str(),"vowel"|"diphthong")).map(|_|i)).collect::<Vec<_>>();
        let mut syllables=split(phones,&nuclei,self.language);
        if self.language==Language::German {german::prepare(&mut syllables,&self.classes);}
        Ok(syllables)
    }

    pub(super) fn link_words(&self,lyrics:&[&str],notes:&mut [Vec<Vec<String>>])->Result<()> {
        if self.legacy {return Ok(())}
        if !matches!(self.language,Language::French|Language::Portuguese) {return Ok(())}
        let mut spellings=Vec::new();let mut words=Vec::new();let mut owners=Vec::new();
        for (index,(&lyrics,parts)) in lyrics.iter().zip(notes.iter()).enumerate() {
            let mut first:usize=0;
            for word in lyrics.split_whitespace() {
                let count=self.word(word)?.len();
                let end=first.checked_add(count).filter(|&end|end<=parts.len())
                    .ok_or_else(||VoiceError::new("自动发音的词语与音节数量不一致"))?;
                spellings.push(word);words.push(parts[first..end].to_vec());owners.push(index);first=end;
            }
            if first!=parts.len(){return Err(VoiceError::new("自动发音的词语与音节数量不一致"))}
        }
        match self.language {
            Language::French=>french::link(&spellings,&mut words,&self.classes),
            Language::Portuguese=>portuguese::link(&spellings,&mut words,&self.classes),
            _=>unreachable!(),
        }
        for parts in notes.iter_mut(){parts.clear()}
        for (owner,parts) in owners.into_iter().zip(words) {notes[owner].extend(parts)}
        Ok(())
    }
}
