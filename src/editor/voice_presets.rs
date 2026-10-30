use super::Editor;
use crate::model::{EntityId, PublicModeMap};
use rynthv::style_presets::StylePreset;

impl Editor {
    pub fn voice_preset(&self, revision: &str) -> Result<(EntityId, String, StylePreset), String> {
        let track = self.current_track();
        if track.is_instrumental() { return Err("伴奏轨不能设置歌声预设".into()); }
        let reference = self.active_ref().unwrap_or(&track.main_ref);
        let mut voice = reference.extra.get("voice").cloned().unwrap_or_else(||serde_json::json!({}))
            .as_object().cloned().ok_or("voice 字段必须为对象")?;
        let modes: PublicModeMap = serde_json::from_value(voice.remove("vocalModeParams")
            .unwrap_or_else(||serde_json::json!({}))).map_err(|e|e.to_string())?;
        let name = reference.extra.get("voicePresetName").and_then(|v|v.as_str()).unwrap_or("").to_owned();
        let experiments=reference.extra.get("rynthvExperiments").map(|_|crate::synthesis::experiments::ExperimentControls::from_extra(&reference.extra)).transpose().map_err(|e|e.to_string())?;
        Ok((reference.id, name, StylePreset { revision:revision.into(), modes, voice:voice.into_iter().collect(),experiments }))
    }

    pub fn apply_voice_preset(&mut self, reference: EntityId, catalogue: &[String], preset: &StylePreset, name: &str) -> Result<bool,String> {
        if self.is_editing() { return Err("请先完成当前编辑".into()); }
        let (selected,_,_) = self.voice_preset(&preset.revision)?;
        if selected != reference { return Err("选中的音符组已经改变".into()); }
        preset.validate_for(&preset.revision,catalogue)?;
        let target = self.active_ref().unwrap_or(&self.current_track().main_ref);
        let mut extra = target.extra.clone();
        let mut voice = serde_json::Map::from_iter(preset.voice.clone());
        voice.insert("vocalModeParams".into(),serde_json::to_value(&preset.modes).map_err(|e|e.to_string())?);
        extra.insert("voice".into(),serde_json::Value::Object(voice));
        extra.insert("voicePresetName".into(),name.into());
        if let Some(controls)=&preset.experiments {extra.insert("rynthvExperiments".into(),serde_json::to_value(controls).map_err(|e|e.to_string())?);}
        else {extra.remove("rynthvExperiments");}
        if crate::synthesis::parameters::VOICE_CONTROLS.iter().any(|spec|
            preset.voice.get(spec.key).and_then(|v|v.as_f64()).is_some_and(|v|v.abs()>f64::from(spec.limit))) {
            let mut controls=crate::synthesis::experiments::ExperimentControls::from_extra(&extra).map_err(|e|e.to_string())?;
            controls.extended_expression=true;
            extra.insert("rynthvExperiments".into(),serde_json::to_value(controls).map_err(|e|e.to_string())?);
        }
        if extra == target.extra { return Ok(false); }
        self.begin("应用歌声预设");
        let track = self.project.tracks.iter_mut().find(|t|t.id==self.track).expect("validated track");
        std::iter::once(&mut track.main_ref).chain(&mut track.groups).find(|r|r.id==reference).expect("validated reference").extra=extra;
        self.finish();
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PublicModeMap;
    use crate::{model::{Note, Project, QUARTER}, synthesis::plan::Planner};
    #[test]
    fn preset_apply_is_single_undoable_voice_edit_and_invalidates_owner() {
        let mut project=Project::default();
        project.tracks[0].main_group.notes.push(Note::new(QUARTER,QUARTER,60));
        project.tracks[0].main_ref.extra.get_mut("voice").unwrap()["paramGender"]=serde_json::json!(0.4);
        let mut editor=Editor::new(project);
        let before=editor.project.clone();
        let reference=editor.vocal_mode_selection().unwrap().reference_id;
        let catalogue=vec!["Breathy".to_owned()];
        let modes:PublicModeMap=serde_json::from_value(serde_json::json!({"Breathy":{"pitch":0,"timbre":37.5,"pronunciation":91}})).unwrap();
        let mut planner=Planner::default();
        let before_owner=planner.update(&editor.project,editor.revision,|_|Some("fixture-content".into())).requests.values().next().unwrap().clone();
        let mut preset=editor.voice_preset("202").unwrap().2;
        preset.modes=modes.clone();
        assert!(editor.apply_voice_preset(reference,&catalogue,&preset,"Mixed").unwrap());
        assert_eq!(editor.revision,1);assert!(editor.dirty());
        let after=editor.project.clone();
        assert_eq!(after.tracks[0].main_group,before.tracks[0].main_group);
        let mut unrelated=after.clone();
        unrelated.tracks[0].main_ref.extra.get_mut("voice").unwrap()["vocalModeParams"]=before.tracks[0].main_ref.extra["voice"]["vocalModeParams"].clone();
        unrelated.tracks[0].main_ref.extra.insert("voicePresetName".into(),serde_json::json!(""));
        assert_eq!(unrelated,before);
        let after_owner=planner.update(&editor.project,editor.revision,|_|Some("fixture-content".into())).requests.values().next().unwrap().clone();
        assert_ne!(before_owner.input,after_owner.input);
        assert_ne!(before_owner.epoch,after_owner.epoch);
        assert!(!editor.apply_voice_preset(reference,&catalogue,&preset,"Mixed").unwrap());
        assert_eq!(editor.revision,1);
        editor.undo();assert_eq!(editor.project,before);assert!(!editor.dirty());assert!(!editor.can_undo());
        editor.redo();assert_eq!(editor.project,after);
        assert!(editor.apply_voice_preset(reference.wrapping_add(1),&catalogue,&preset,"Mixed").is_err());
        assert_eq!(editor.project,after);
        assert!(editor.apply_voice_preset(reference,&catalogue,&preset,"Same Voice, New Name").unwrap());
        let renamed=planner.update(&editor.project,editor.revision,|_|Some("fixture-content".into())).requests.values().next().unwrap().clone();
        assert_eq!(renamed.input,after_owner.input);
        assert_eq!(renamed.epoch,after_owner.epoch);
    }
}
