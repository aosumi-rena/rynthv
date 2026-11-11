mod dictionary;
pub(super) mod english;
mod japanese;
mod korean;
mod mandarin;
mod spanish;
mod alphabet;
mod cache;
mod conversion;
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
    local_alphabet: bool,
    lexical_cache: std::sync::Mutex<cache::LexicalCache>,
    pub(in crate::synthesis::score) pronunciation_cache: std::sync::Mutex<super::pronunciation::Cache>,
    pub(in crate::synthesis::score) incremental:bool,
    conversion: Option<std::sync::Arc<crate::synthesis::phone_conversion::PhoneConversion>>,
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
    /// Fresh full-context frontend for offline comparisons; never a saved setting.
    pub fn without_incremental_cache(&self)->Result<Self> {
        let format=if self.local_alphabet {crate::model::PhonemeFormat::Original} else {crate::model::PhonemeFormat::System};
        let mut copy=Self::with_format(&self.directory,&self.default_language_key,
            &self.dictionaries.keys().map(|language|language.id().to_owned()).collect::<Vec<_>>(),format)?;
        copy.incremental=false;
        copy.conversion=self.conversion.clone();
        Ok(copy)
    }

    pub fn supports_xsampa(&self)->bool {self.catalog.uses_system_symbols()}

    pub(super) fn xsampa(&self, requested:bool)->bool {requested && self.supports_xsampa()}

    pub fn for_voice(directory:impl AsRef<Path>,config:&crate::synthesis::voice::VoiceConfiguration)->Result<Self> {
        let version=crate::synthesis::settings::PhonemeVersion::configuration(config,false).map_err(VoiceError::new)?;
        Self::with_format(directory,&config.default_language,&config.supported_languages,version.format())
    }
    pub fn for_voice_with_conversion(directory:impl AsRef<Path>,config:&crate::synthesis::voice::VoiceConfiguration,programme:Option<&Path>)->Result<Self> {
        let Some(path)=programme else {return Self::for_voice(directory,config)};
        Self::with_phone_conversion(directory,&config.default_language,&config.supported_languages,path)
    }
    pub fn with_phone_conversion(directory:impl AsRef<Path>,default_language:&str,supported:&[String],path:&Path)->Result<Self> {
        let conversion=crate::synthesis::phone_conversion::PhoneConversion::load(path)?;
        let mut frontend=Self::with_format(directory,default_language,supported,crate::model::PhonemeFormat::System)?;
        frontend.conversion=Some(std::sync::Arc::new(conversion));
        frontend.incremental=false;
        Ok(frontend)
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
        Self::with_format(directory,default_language,supported,crate::model::PhonemeFormat::Auto)
    }

    pub fn with_format(
        directory: impl AsRef<Path>,
        default_language: &str,
        supported: &[String],
        format: crate::model::PhonemeFormat,
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
        let catalog=crate::model::PhonemeCatalog::load_format(directory.as_ref(),format).map_err(VoiceError::new)?;
        let local_alphabet=!catalog.uses_system_symbols();
        Ok(Self {
            catalog,
            directory: directory.as_ref().to_owned(),
            default_language,
            default_language_key,
            dictionaries,
            local_alphabet,
            lexical_cache: Default::default(),
            pronunciation_cache: Default::default(),
            incremental:true,
            conversion:None,
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
            .get_or_init(|| {let legacy=self.local_alphabet; match language {
                Language::Mandarin => MandarinLexicon::load(&self.directory,legacy).map(Lexicon::Mandarin),
                Language::Japanese => {
                    japanese::Japanese::load(&self.directory,&self.catalog).map(Lexicon::Japanese)
                }
                Language::Korean => korean::Korean::load(&self.directory,legacy).map(Lexicon::Korean),
                Language::Spanish => spanish::Spanish::load(&self.catalog).map(Lexicon::Spanish),
                Language::French | Language::German | Language::Portuguese | Language::Italian =>
                    european::European::load(&self.directory,language,&self.catalog,legacy).map(Lexicon::European),
                _ => Dictionary::load(&self.directory, language,&self.catalog,legacy).map(Lexicon::Dictionary),
            }})
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
        self.resolve_with_inventory(language,lyrics,european,!self.local_alphabet)
    }

    pub(super) fn resolve_for_model(&self,language:Language,lyrics:&str,european:bool,
        tree:&crate::synthesis::voice::PreparedTree)->Result<Vec<Vec<String>>> {
        let coda_l=language==Language::English && (self.conversion.is_some() || tree.native_phone_token(language.id(),"ll").is_ok());
        self.resolve_with_inventory(language,lyrics,european,coda_l)
    }

    fn resolve_with_inventory(&self,language:Language,lyrics:&str,european:bool,coda_l:bool)->Result<Vec<Vec<String>>> {
        self.cached_lexical(language,lyrics,european,coda_l,||self.resolve_uncached(language,lyrics,european,coda_l))
    }

    fn resolve_uncached(&self,language:Language,lyrics:&str,european:bool,coda_l:bool)->Result<Vec<Vec<String>>> {
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

    pub fn lexical_model_group(&self,tree:&crate::synthesis::voice::PreparedTree,language:Language,
        lyrics:&[&str],european:bool)->Result<Vec<Vec<Vec<String>>>> {
        let mut words=lyrics.iter().map(|text|self.resolve_for_model(language,text,european,tree)).collect::<Result<Vec<_>>>()?;
        self.link_european_words(language,lyrics,&mut words)?;
        let mut words=words.into_iter().map(|word|word.into_iter().map(|syllable|syllable.iter()
            .map(|token|self.model_token(language,token,tree)).collect()).collect()).collect::<Result<Vec<Vec<Vec<String>>>>>()?;
        if self.conversion.is_some() {
            let tokens=words.iter().flatten().flatten().cloned().collect::<Vec<_>>();
            let converted=self.convert_tokens(language,&tokens)?;
            for (token,replacement) in words.iter_mut().flatten().flatten().zip(converted) {*token=replacement;}
        }
        Ok(words)
    }
}
