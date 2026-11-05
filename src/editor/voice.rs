use super::Editor;
use crate::model::Language;
use crate::synthesis::voice::{VoiceFamily, VoiceRevision};

impl Editor {
    pub fn pronunciation_option(&self, key: &str, default: bool) -> Result<(crate::model::EntityId, bool), String> {
        pronunciation_label(key)?;
        if self.current_track().is_instrumental() {return Err("伴奏轨没有歌声发音设置".into());}
        let reference = self.active_ref().unwrap_or(&self.current_track().main_ref);
        let value = match reference.extra.get("voice").and_then(|v|v.get(key)) {
            None => default,
            Some(value) => match value.as_str() {
                Some("default") => default,
                Some("true") => true,
                Some("false") => false,
                _ => return Err("保存的发音设置无效".into()),
            },
        };
        Ok((reference.id, value))
    }

    pub fn set_pronunciation_option(&mut self, reference_id: crate::model::EntityId, key: &str, enabled: bool)
        -> Result<(), String> {
        let label=pronunciation_label(key)?;
        if self.is_editing() {return Err("请先完成当前编辑".into());}
        if self.current_track().is_instrumental() {return Err("伴奏轨没有歌声发音设置".into());}
        let reference = self.active_ref().unwrap_or(&self.current_track().main_ref);
        if reference.id != reference_id {return Err("选中的音符组已经改变".into());}
        let mut next = reference.extra.clone();
        next.entry("voice".into()).or_insert_with(||serde_json::json!({}))
            .as_object_mut().ok_or("voice 字段必须是对象")?
            .insert(key.into(), if enabled {"true"} else {"false"}.into());
        if next == reference.extra {return Ok(());}
        self.begin(label);
        let track = self.project.tracks.iter_mut().find(|track|track.id==self.track)
            .expect("selected track exists");
        std::iter::once(&mut track.main_ref).chain(&mut track.groups)
            .find(|reference|reference.id==reference_id).expect("validated reference").extra=next;
        self.finish();
        Ok(())
    }

    pub fn set_track_voice(
        &mut self,
        singer: Option<(&str, &str, VoiceRevision, VoiceFamily)>,
    ) -> Result<(), String> {
        if self.current_track().is_instrumental() {
            return Err("伴奏轨不能设置歌手".into());
        }
        if self
            .current_track()
            .references()
            .any(|r| !r.database.is_object())
        {
            return Err("音轨的歌声 database 字段必须是对象".into());
        }
        self.begin("切换音轨歌手");
        let track = self
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == self.track)
            .expect("selected track exists");
        for reference in std::iter::once(&mut track.main_ref).chain(&mut track.groups) {
            let database = reference
                .database
                .as_object_mut()
                .expect("validated database");
            database.remove("id");
            let (name, version, backend) = match singer {
                Some((id, name, version, family)) => {
                    database.insert("id".into(), id.into());
                    (name, version.to_string(), match family {
                        VoiceFamily::Sv2 | VoiceFamily::Sv2Compatible => "SVR3",
                        VoiceFamily::Sv1 | VoiceFamily::R1Std | VoiceFamily::R2Std => "",
                    })
                }
                None => ("", "-2".to_owned(), ""),
            };
            database.insert("name".into(), name.into());
            database.insert("version".into(), version.into());

            for key in ["language", "phoneset"] {
                database.insert(key.into(), "".into());
            }

            database.insert("backendType".into(), backend.into());
        }
        self.finish();
        Ok(())
    }

    pub fn set_voice_language(&mut self, language: Option<Language>) -> Result<(), String> {
        let target = self.active_ref().unwrap_or(&self.current_track().main_ref);
        let target_id = target.id;
        let database = target
            .database
            .as_object()
            .ok_or("歌声的 database 字段必须是对象")?;
        let value = language.map_or("", Language::id);
        let alphabet = language.map_or("", Language::phoneme_alphabet);
        if database
            .get("languageOverride")
            .and_then(serde_json::Value::as_str)
            == Some(value)
            && database
                .get("phonesetOverride")
                .and_then(serde_json::Value::as_str)
                == Some(alphabet)
        {
            return Ok(());
        }
        self.begin("切换歌声语言");
        let track = self
            .project
            .tracks
            .iter_mut()
            .find(|track| track.id == self.track)
            .expect("selected track exists");
        let reference = std::iter::once(&mut track.main_ref)
            .chain(&mut track.groups)
            .find(|reference| reference.id == target_id)
            .expect("selected reference exists");
        let database = reference
            .database
            .as_object_mut()
            .expect("validated database");
        database.insert("languageOverride".into(), value.into());
        database.insert("phonesetOverride".into(), alphabet.into());
        self.finish();
        Ok(())
    }
}

fn pronunciation_label(key: &str) -> Result<&'static str,String> {
    match key {
        "relaxedPronunciation" => Ok("切换松弛辅音"),
        "europeanPronunciation" => Ok("切换欧式发音"),
        _ => Err("未知歌声发音设置".into()),
    }
}
