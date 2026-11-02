use super::Editor;
use crate::model::{Language, PronunciationDictionary};
use std::collections::BTreeMap;

impl Editor {
    pub fn select_dictionary(&mut self, name: &str) {
        if !name.is_empty() && !self.project.dictionaries.contains_key(name) {
            return;
        }
        self.begin("选择发音词典");
        self.assign_dictionary(name);
        self.finish();
    }

    fn assign_dictionary(&mut self, name: &str) {
        let id = self
            .active_ref()
            .unwrap_or(&self.current_track().main_ref)
            .id;
        let track = self
            .project
            .tracks
            .iter_mut()
            .find(|track| track.id == self.track)
            .expect("selected track");
        if let Some(reference) = std::iter::once(&mut track.main_ref)
            .chain(&mut track.groups)
            .find(|r| r.id == id)
        {
            reference.extra.insert("dictionary".into(), name.into());
        }
    }

    pub fn create_dictionary(&mut self, name: &str, language: Language) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("请输入词典名称".into());
        }
        if self.project.dictionaries.contains_key(name) {
            return Err("已有同名词典".into());
        }
        self.begin("新建发音词典");
        self.project.dictionaries.insert(
            name.into(),
            PronunciationDictionary {
                language,
                entries: BTreeMap::new(),
                xsampa_entries: Default::default(),
            },
        );
        self.assign_dictionary(name);
        self.finish();
        Ok(())
    }

    pub fn remove_dictionary(&mut self, name: &str) {
        self.begin("移除发音词典");
        self.project.dictionaries.remove(name);
        for track in &mut self.project.tracks {
            for reference in std::iter::once(&mut track.main_ref).chain(&mut track.groups) {
                if reference
                    .extra
                    .get("dictionary")
                    .and_then(serde_json::Value::as_str)
                    == Some(name)
                {
                    reference.extra.insert("dictionary".into(), "".into());
                }
            }
        }
        self.finish();
    }

    pub fn set_dictionary_entry(
        &mut self,
        name: &str,
        word: &str,
        phones: Option<&str>,
        xsampa: bool,
    ) -> Result<(), String> {
        let word = word.trim();
        if word.is_empty() {
            return Err("请输入字词".into());
        }
        let phones = phones.map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "));
        if phones.as_ref().is_some_and(String::is_empty) {
            return Err("请输入音素，以空格分隔".into());
        }
        if !self.project.dictionaries.contains_key(name) {
            return Err("请先新建或选择词典".into());
        }
        self.begin("编辑发音词典");
        let dictionary = self
            .project
            .dictionaries
            .get_mut(name)
            .expect("checked dictionary");
        if let Some(phones) = phones {
            dictionary.entries.insert(word.into(), phones);
            if xsampa {dictionary.xsampa_entries.insert(word.into());}
            else {dictionary.xsampa_entries.remove(word);}
        } else {
            dictionary.entries.remove(word);
            dictionary.xsampa_entries.remove(word);
        }
        self.finish();
        Ok(())
    }
}
