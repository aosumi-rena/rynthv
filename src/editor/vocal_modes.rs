use super::Editor;
use crate::{
    model::{EntityId, Extra, PublicModeComponent, PublicModeMap},
    synthesis::voice::{VoiceInventory, VoiceFamily},
};
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VocalModeEditCapability {
    catalogue: Vec<String>,
    independent_lanes: bool,
    minimum:i32,
    maximum:i32,
}

impl VocalModeEditCapability {
    pub fn from_inventory(inventory: &VoiceInventory) -> Result<Self, String> {
        let config = inventory.configuration.as_ref().ok_or("歌声配置尚未加载。")?;
        if !config.speakers.is_empty() {
            let prepared=inventory.preparation().ok_or("等待合唱成员及唱法目录加载。")?;
            if prepared.member_vocal_modes.len()!=config.speakers.len()
                || prepared.summary().stage_preparation_error.is_some()
            {return Err("合唱成员的唱法目录或模型尚未准备好。".into());}
        }
        let mut capability=Self::from_parts(config.family, Some(config.vocal_mode_names()))?;
        let range=crate::synthesis::experiments::ExperimentalSettings::ui().style_range();
        capability.minimum=range.0 as i32;capability.maximum=range.1 as i32;
        let settings=crate::synthesis::experiments::ExperimentalSettings::ui();
        capability.independent_lanes |= config.family==VoiceFamily::Sv2 && settings.active(settings.component_curves);
        Ok(capability)
    }

    fn from_parts(family: VoiceFamily, resolved: Option<&[String]>) -> Result<Self, String> {
        if family==VoiceFamily::Sv1 && !crate::synthesis::experiments::ExperimentalSettings::ui().enabled {
            return Err("此引擎不支持 R3 唱法。".into());
        }
        let catalogue = resolved.ok_or("唱法目录尚未加载。")?;
        if catalogue.is_empty() { return Err("此歌声没有公开可用的唱法。".into()); }
        Ok(Self { catalogue: catalogue.to_vec(), independent_lanes: family==VoiceFamily::Sv2,minimum:0,maximum:150 })
    }

    pub fn independent_lanes(&self)->bool {self.independent_lanes}
}

pub struct VocalModeSelection {
    pub reference_id: EntityId,
    pub controls: PublicModeMap,
}

fn controls(extra: &Extra) -> Result<PublicModeMap, String> {
    let Some(voice) = extra.get("voice") else { return Ok(PublicModeMap::new()); };
    let voice = voice.as_object().ok_or("歌声 voice 字段不是对象；保留原始值，不能编辑唱法。")?;
    let Some(value) = voice.get("vocalModeParams") else { return Ok(PublicModeMap::new()); };
    let parsed: PublicModeMap = serde_json::from_value(value.clone())
        .map_err(|error| format!("保存的唱法格式不能编辑（原值保留）：{error}"))?;

    Ok(parsed)
}

impl Editor {
    pub fn vocal_mode_selection(&self) -> Result<VocalModeSelection, String> {
        let track = self.current_track();
        if track.is_instrumental() { return Err("伴奏轨不能设置歌声唱法。".into()); }
        let reference = self.active_ref().unwrap_or(&track.main_ref);
        let local = controls(&reference.extra)?;

        Ok(VocalModeSelection {reference_id:reference.id, controls:local})
    }

    pub fn set_vocal_mode_component(
        &mut self, reference_id: EntityId, capability: &VocalModeEditCapability,
        name: &str, component: PublicModeComponent, value: f64,
    ) -> Result<bool, String> {
        self.set_vocal_mode_components(reference_id, capability, name, &[(component, value)])
    }

    pub fn set_vocal_mode_components(
        &mut self, reference_id: EntityId, capability: &VocalModeEditCapability,
        name: &str, changes: &[(PublicModeComponent, f64)],
    ) -> Result<bool, String> {
        if self.is_editing() { return Err("请先完成当前编辑，再调整唱法。".into()); }
        if changes.is_empty() { return Ok(false); }
        if !capability.independent_lanes && ![
            PublicModeComponent::Pitch,PublicModeComponent::Timbre,PublicModeComponent::Pronunciation,
        ].iter().all(|component|changes.iter().any(|(changed,_)|changed==component)) {
            return Err("请将旧版兼容歌声升级版本 2 以单独控制音高、音色和发音。".into());
        }
        if changes.iter().any(|(_, value)| !value.is_finite() || !(f64::from(capability.minimum)..=f64::from(capability.maximum)).contains(value)) {
            return Err(format!("唱法值必须是 {}..{} 范围的有限数值。",capability.minimum,capability.maximum));
        }
        let selected = self.vocal_mode_selection()?;
        if selected.reference_id != reference_id {
            return Err("选中的音符组已经改变；已取消旧的唱法编辑。".into());
        }
        if !capability.catalogue.iter().any(|entry| entry == name) {
            return Err("保存或选择的唱法不属于当前歌声的公开目录；未修改项目。".into());
        }
        if changes.iter().all(|(_, value)| *value == 0.0) && !selected.controls.contains_key(name) {
            return Ok(false);
        }
        let reference = self.active_ref().unwrap_or(&self.current_track().main_ref);
        let mut next = reference.extra.clone();
        let voice = next.entry("voice".into()).or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut().ok_or("voice 字段必须是对象")?;
        let params = voice.entry("vocalModeParams").or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut().ok_or("vocalModeParams 字段必须是对象")?;
        let initial = selected.controls.get(name).cloned().unwrap_or_default();
        let stored = params.entry(name).or_insert_with(|| Value::Object(Map::new()));
        if stored.is_number() {
            *stored = serde_json::to_value(initial).map_err(|error| error.to_string())?;
        }
        let mode = stored
            .as_object_mut().ok_or("唱法必须是独立音高、音色与发音字段的对象")?;
        for &(component, value) in changes {
            let field = match component {
                PublicModeComponent::Timbre => "timbre",
                PublicModeComponent::Pronunciation => "pronunciation",
                PublicModeComponent::Pitch => "pitch",
            };
            mode.insert(field.into(), Value::from(value));
        }

        if ["pitch","timbre","pronunciation"].iter().all(|key|
            mode.get(*key).is_none_or(|value| value.as_f64() == Some(0.0)))
        {
            params.remove(name);
        }
        if next == reference.extra { return Ok(false); }
        self.begin("调整歌声唱法");
        let track = self.project.tracks.iter_mut().find(|track| track.id == self.track)
            .expect("selected track exists");
        let target = std::iter::once(&mut track.main_ref).chain(&mut track.groups)
            .find(|reference| reference.id == reference_id).expect("validated selected reference");
        target.extra = next;
        self.finish();
        Ok(true)
    }
}

#[cfg(test)]
mod vocal_mode_editor_tests {
    use super::*;
    use crate::model::{Group, GroupRef, Note, Project, QUARTER};
    use serde_json::json;

    fn editor() -> Editor {
        let mut project = Project::default();
        project.tracks[0].main_group.notes.push(Note::new(0,QUARTER,60));
        project.tracks[0].main_ref.extra.insert("scriptData".into(),json!({"keep":true}));
        let voice=project.tracks[0].main_ref.extra.get_mut("voice").unwrap().as_object_mut().unwrap();
        voice.insert("paramGender".into(),json!(0));
        Editor::new(project)
    }
    fn capability() -> VocalModeEditCapability {
        VocalModeEditCapability::from_parts(VoiceFamily::Sv2,Some(&["Breathy".into()])).unwrap()
    }

    #[test]
    fn main_self_layer_one_atomic_edit_undo_redo_and_unrelated_fields() {
        let mut e=editor(); let before=e.project.clone(); let id=e.vocal_mode_selection().unwrap().reference_id;
        assert!(e.set_vocal_mode_component(id,&capability(),"Breathy",PublicModeComponent::Timbre,25.0).unwrap());
        assert_eq!(e.revision,1); assert!(e.can_undo());
        let after=e.project.clone();
        assert_eq!(after.tracks[0].main_group,before.tracks[0].main_group);
        assert_eq!(after.tracks[0].main_ref.extra["scriptData"],before.tracks[0].main_ref.extra["scriptData"]);
        assert_eq!(after.tracks[0].main_ref.extra["voice"]["paramGender"],json!(0));
        assert_eq!(after.tracks[0].main_ref.extra["voice"]["vocalModeInherited"],json!(true));
        assert_eq!(e.vocal_mode_selection().unwrap().controls["Breathy"].pitch,None);
        e.undo(); assert_eq!(e.project,before); assert_eq!(e.revision,2);
        e.redo(); assert_eq!(e.project,after); assert_eq!(e.revision,3);
        assert!(!e.set_vocal_mode_component(id,&capability(),"Breathy",PublicModeComponent::Timbre,25.0).unwrap());
        assert_eq!(e.revision,3);
    }

    #[test]
    fn pitch_edits_preserve_other_components_and_stale_references_reject() {
        let mut e=editor(); let id=e.vocal_mode_selection().unwrap().reference_id;
        e.project.tracks[0].main_ref.extra.get_mut("voice").unwrap()["vocalModeParams"]=
            json!({"Breathy":{"pitch":25,"timbre":7,"pronunciation":9}});
        let before=e.project.clone();
        assert!(e.set_vocal_mode_component(id,&capability(),"Breathy",PublicModeComponent::Pitch,100.0).unwrap());
        let control=&e.vocal_mode_selection().unwrap().controls["Breathy"];
        assert_eq!(control.pitch,Some(100.0));
        assert_eq!(control.timbre,Some(7.0));
        assert_eq!(control.pronunciation,Some(9.0));
        e.undo();assert_eq!(e.project,before);
        let mut e=editor(); let before=e.project.clone();
        assert!(e.set_vocal_mode_component(id,&capability(),"Breathy",PublicModeComponent::Timbre,10.0).is_err());
        assert_eq!(e.project,before);
    }

    #[test]
    fn selected_group_edits_only_that_reference() {
        let mut e=editor(); let mut group=Group::new("child");
        group.notes.push(Note::new(0,QUARTER,60));
        let reference=GroupRef::new(&group,-1); let id=reference.id;
        e.project.library.push(group); e.project.tracks[0].groups.push(reference); e.reference=Some(id);
        let main=e.project.tracks[0].main_ref.clone();
        e.set_vocal_mode_component(id,&capability(),"Breathy",PublicModeComponent::Pronunciation,12.34567).unwrap();
        assert_eq!(e.project.tracks[0].main_ref,main);
        assert_eq!(e.vocal_mode_selection().unwrap().controls["Breathy"].pronunciation,Some(12.34567));
    }

    #[test]
    fn capability_range_unknown_and_malformed_controls_fail_closed() {
        assert!(VocalModeEditCapability::from_parts(VoiceFamily::Sv1,Some(&["Breathy".into()])).is_err());
        assert!(VocalModeEditCapability::from_parts(VoiceFamily::Sv2Compatible,Some(&["Breathy".into()])).is_ok());
        assert!(VocalModeEditCapability::from_parts(VoiceFamily::Sv2,None).is_err());
        assert!(VocalModeEditCapability::from_parts(VoiceFamily::Sv2,Some(&[])).is_err());
        let mut e=editor();let id=e.vocal_mode_selection().unwrap().reference_id;let before=e.project.clone();
        for value in [-1.0,150.1,f64::NAN,f64::INFINITY] {
            assert!(e.set_vocal_mode_component(id,&capability(),"Breathy",PublicModeComponent::Timbre,value).is_err());
        }
        assert!(e.set_vocal_mode_component(id,&capability(),"Hidden",PublicModeComponent::Timbre,25.0).is_err());
        assert_eq!(e.project,before);
        e.project.tracks[0].main_ref.extra.get_mut("voice").unwrap()["vocalModeParams"]=json!({"Breathy":{"pitch":null}});
        let malformed=e.project.clone();
        assert!(e.vocal_mode_selection().is_err());
        assert!(e.set_vocal_mode_component(id,&capability(),"Breathy",PublicModeComponent::Timbre,25.0).is_err());
        assert_eq!(e.project,malformed);
    }

    #[test]
    fn explicit_reset_removes_only_zero_mode_and_is_undoable() {
        let mut e=editor();
        e.project.tracks[0].main_ref.extra.get_mut("voice").unwrap()["vocalModeParams"]=
            json!({"Other":{"timbre":8.0,"pronunciation":3.0}});
        let baseline=e.project.clone(); let mut cap=capability(); cap.catalogue.push("Other".into());
        let id=e.vocal_mode_selection().unwrap().reference_id;
        e.set_vocal_mode_component(id,&cap,"Breathy",PublicModeComponent::Timbre,25.0).unwrap();
        e.set_vocal_mode_component(id,&cap,"Breathy",PublicModeComponent::Pronunciation,12.34567).unwrap();
        e.set_vocal_mode_component(id,&cap,"Breathy",PublicModeComponent::Timbre,0.0).unwrap();
        assert_eq!(e.vocal_mode_selection().unwrap().controls["Breathy"].pronunciation,Some(12.34567));
        let before_reset=e.project.clone();
        e.set_vocal_mode_component(id,&cap,"Breathy",PublicModeComponent::Pronunciation,0.0).unwrap();
        let reset=e.vocal_mode_selection().unwrap().controls;
        assert!(!reset.contains_key("Breathy"));
        assert_eq!(reset["Other"].timbre,Some(8.0));
        assert_eq!(e.project,baseline);
        assert_eq!(e.revision,4);
        e.undo(); assert_eq!(e.project,before_reset);
        e.redo(); assert_eq!(e.project,baseline);
    }

    #[test]
    fn zero_on_absent_mode_does_not_create_objects_or_undo() {
        let mut e=editor();e.project.tracks[0].main_ref.extra.remove("voice");
        let before=e.project.clone();let id=e.vocal_mode_selection().unwrap().reference_id;
        assert!(!e.set_vocal_mode_component(id,&capability(),"Breathy",PublicModeComponent::Timbre,0.0).unwrap());
        assert_eq!(e.project,before);assert_eq!(e.revision,0);assert!(!e.can_undo());
    }
}
