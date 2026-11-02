use super::Editor;
use crate::model::Language;

impl Editor {
    pub fn set_note_phoneme_alphabet(&mut self, xsampa: bool,
        catalog: &crate::model::PhonemeCatalog, inherited: Language) -> Result<(), String> {
        // Validate every conversion before creating the undo transaction.
        let replacements = self.notes().iter().filter(|note|self.selected.contains(&note.id))
            .map(|note| {
                let language=note.attributes.get("languageOverride").and_then(serde_json::Value::as_str)
                    .filter(|value|!value.is_empty()).map_or(Ok(inherited), |value|
                        Language::parse(value).ok_or_else(||format!("不支持语言：{value}")))?;
                let old=note.attributes.get("phoneset").and_then(serde_json::Value::as_str)==Some("xsampa");
                let text=if old==xsampa || note.phonemes.trim().is_empty() {note.phonemes.clone()}
                    else {catalog.convert(language,&note.phonemes,xsampa)?};
                Ok((note.id,text))
            }).collect::<Result<Vec<_>,String>>()?;
        self.edit_selected_notes("切换音素符号", |note| {
            if let Some((_,text))=replacements.iter().find(|(id,_)|*id==note.id) {
                note.phonemes.clone_from(text);
                if xsampa {note.attributes.insert("phoneset".into(),"xsampa".into());} else {note.attributes.remove("phoneset");}
                note.attributes.remove("phonemes");
            }
        });
        Ok(())
    }
    pub fn set_note_phonemes(&mut self, group_id: &str, notes: &[u64], phonemes: &str) {
        let value = phonemes.split_whitespace().collect::<Vec<_>>().join(" ");
        self.begin("编辑音素");
        if let Some(group) = self.project.group_mut(group_id) {
            for note in &mut group.notes {
                if notes.contains(&note.id) && note.phonemes != value {
                    note.phonemes.clone_from(&value);
                    note.attributes.remove("phonemes");
                }
            }
        }
        self.finish();
    }

    pub fn set_note_language(&mut self, language: Option<Language>) {
        self.edit_selected_notes("切换音符语言", |note| {
            let current_language = note.attributes.get("languageOverride").cloned();
            let current_alphabet = note.attributes.get("phoneset").cloned();
            if let Some(language) = language {
                note.attributes
                    .insert("languageOverride".into(), language.id().into());
                note.attributes.remove("phoneset");
            } else {
                note.attributes.remove("languageOverride");
                note.attributes.remove("phoneset");
            }
            if current_language.as_ref() != note.attributes.get("languageOverride")
                || current_alphabet.as_ref() != note.attributes.get("phoneset")
            {
                note.phonemes.clear();
                note.attributes.remove("phonemes");
            }
        });
    }

    pub fn set_even_syllables(&mut self, even: bool) {
        self.edit_selected_notes("调整音节分配", |note| {
            note.attributes
                .insert("evenSyllableDuration".into(), even.into());
        });
    }

    fn edit_selected_notes(&mut self, label: &str, mut edit: impl FnMut(&mut crate::model::Note)) {
        if self.selected.is_empty() {
            return;
        }
        self.begin(label);
        let selected = self.selected.clone();
        if let Some(group) = self.group_mut() {
            for note in &mut group.notes {
                if selected.contains(&note.id) {
                    edit(note);
                }
            }
        }
        self.finish();
    }
}
