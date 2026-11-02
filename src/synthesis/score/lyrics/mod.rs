mod dictionary;
pub(super) mod english;
mod japanese;
mod korean;
mod mandarin;
mod spanish;
mod alphabet;
pub(super) mod european;

use crate::{
    model::Language,
    synthesis::{
        plan::{NoteInput, OwnerInput},
        voice::{Result, VoiceError},
    },
};
use dictionary::Dictionary;
use mandarin::MandarinLexicon;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub struct LyricFrontend {
    catalog: crate::model::PhonemeCatalog,
    directory: PathBuf,
    default_language: Language,
    default_language_key:String,
    dictionaries: HashMap<Language, OnceLock<Result<Lexicon>>>,
}

enum Lexicon {
    Mandarin(MandarinLexicon),
    Japanese(japanese::Japanese),
    Korean(korean::Korean),
    Dictionary(Dictionary),
    Spanish(spanish::Spanish),
    European(european::European),
}

impl LyricFrontend {
    pub fn for_voice(&self,default_language:&str,supported:&[String])->Result<Self> {
        Self::new(&self.directory,default_language,supported)
    }
    pub(super) fn default_language(&self) -> Language {
        self.default_language
    }

    pub(super) fn default_language_key(&self)->&str {
        &self.default_language_key
    }

    pub fn new(
        directory: impl AsRef<Path>,
        default_language: &str,
        supported: &[String],
    ) -> Result<Self> {
        let parse = |id: &str| {
            Language::parse(id).ok_or_else(|| VoiceError::new(format!("无法识别声库语言：{id}")))
        };
        let default_language_key=default_language.to_owned();
        let default_language = parse(default_language)?;
        let dictionaries = supported
            .iter()


            .map(|id| Ok((parse(id)?, OnceLock::new())))
            .collect::<Result<HashMap<_, _>>>()?;
        if !dictionaries.contains_key(&default_language) {
            return Err(VoiceError::new("声库未声明支持其默认语言"));
        }
        Ok(Self {
            catalog: crate::model::PhonemeCatalog::load(directory.as_ref()).map_err(VoiceError::new)?,
            directory: directory.as_ref().to_owned(),
            default_language,
            default_language_key,
            dictionaries,
        })
    }

    pub(super) fn language(&self, input: &OwnerInput) -> Result<Language> {
        let language = super::options::language(input, self.default_language)?;
        if !self.dictionaries.contains_key(&language) {
            return Err(VoiceError::new(format!(
                "当前声库不支持{}",
                language.label()
            )));
        }
        Ok(language)
    }

    pub(super) fn note_language(&self, note: &NoteInput, inherited: Language) -> Result<Language> {
        let language = super::options::note_language(note, inherited)?;
        if !self.dictionaries.contains_key(&language) {
            return Err(VoiceError::new(format!(
                "当前声库不支持{}",
                language.label()
            )));
        }
        Ok(language)
    }

    fn dictionary(&self, language: Language) -> Result<&Lexicon> {
        self
            .dictionaries
            .get(&language)
            .ok_or_else(|| VoiceError::new(format!("当前声库不支持{}", language.label())))?
            .get_or_init(|| match language {
                Language::Mandarin => MandarinLexicon::load(&self.directory).map(Lexicon::Mandarin),
                Language::Japanese => {
                    japanese::Japanese::load(&self.directory).map(Lexicon::Japanese)
                }
                Language::Korean => korean::Korean::load(&self.directory).map(Lexicon::Korean),
                Language::Spanish => spanish::Spanish::load(&self.directory).map(Lexicon::Spanish),
                Language::French | Language::German | Language::Portuguese | Language::Italian =>
                    european::European::load(&self.directory,language).map(Lexicon::European),
                _ => Dictionary::load(&self.directory, language).map(Lexicon::Dictionary),
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(super) fn unmapped_phone_class(&self, language: Language, token: &str) -> Result<Option<&str>> {
        if !matches!(language, Language::French | Language::German | Language::Portuguese | Language::Italian) {
            return Ok(None);
        }
        match self.dictionary(language)? {
            Lexicon::European(frontend) => Ok(frontend.phone_class(token)),
            _ => Ok(None),
        }
    }

    pub(super) fn resolve(&self, language: Language, lyrics: &str, european: bool) -> Result<Vec<Vec<String>>> {
        self.resolve_with_inventory(language,lyrics,european,true)
    }

    pub(super) fn resolve_for_model(&self,language:Language,lyrics:&str,european:bool,
        tree:&crate::synthesis::voice::PreparedTree)->Result<Vec<Vec<String>>> {
        let coda_l=language==Language::English && tree.model_phone_token(language.id(),"ll","5").is_ok();
        self.resolve_with_inventory(language,lyrics,european,coda_l)
    }

    fn resolve_with_inventory(&self,language:Language,lyrics:&str,european:bool,coda_l:bool)->Result<Vec<Vec<String>>> {
        let dictionary = self.dictionary(language)?;
        let result = match dictionary {
            Lexicon::Mandarin(dictionary) => dictionary.resolve(lyrics),
            Lexicon::Japanese(dictionary) => dictionary.resolve(lyrics),
            Lexicon::Korean(dictionary) => dictionary.resolve(lyrics),
            Lexicon::Dictionary(dictionary) => dictionary.resolve_with_coda(lyrics,coda_l),
            Lexicon::Spanish(frontend) => frontend.resolve(lyrics, european),
            Lexicon::European(frontend) => frontend.resolve(lyrics),
        }?;
        if result.is_empty() || result.iter().any(Vec::is_empty) {
            return Err(VoiceError::new(format!(
                "{}歌词没有可合成的音素",
                language.label()
            )));
        }
        Ok(result)
    }

    pub(super) fn link_european_words(&self,language:Language,lyrics:&[&str],words:&mut [Vec<Vec<String>>])->Result<()> {
        if let Some(Ok(Lexicon::European(frontend)))=self.dictionaries.get(&language).and_then(OnceLock::get) {
            frontend.link_words(lyrics,words)?;
        }
        Ok(())
    }

    pub fn lexical_group(&self,language:Language,lyrics:&[&str],european:bool)->Result<Vec<Vec<Vec<String>>>> {
        let mut words=lyrics.iter().map(|text|self.resolve(language,text,european)).collect::<Result<Vec<_>>>()?;
        self.link_european_words(language,lyrics,&mut words)?;
        Ok(words)
    }
}
