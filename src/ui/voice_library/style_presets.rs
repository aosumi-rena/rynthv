mod dialog;
pub(super) use dialog::show as dialog;
use crate::{app::App, model::EntityId, ui::{icons::{Icon,IconButton}, theme::*}};
use rynthv::style_presets::{StylePreset, StylePresets};
use eframe::egui::*;
use std::path::PathBuf;

#[derive(Default)]
pub struct PresetLibrary {
    library: Option<Result<(PathBuf,StylePresets),String>>,
    naming: Option<Naming>,
    load_error_shown: bool,
}
impl PresetLibrary {
    pub fn is_naming(&self)->bool {self.naming.is_some()}
}
struct Naming { bank:String, revision:String, reference:EntityId, name:String, focus:bool }
fn library(state:&mut PresetLibrary)->Result<&mut (PathBuf,StylePresets),String> {
    if state.library.is_none() {
        state.library=Some(rynthv::platform::config_directory().and_then(|dir| {
            let path=dir.join("singing-style-presets.json");
            StylePresets::load(&path).map(|store|(path,store))
        }));
    }
    state.library.as_mut().unwrap().as_mut().map_err(|e|e.clone())
}
fn defaults(revision:&str)->StylePreset {
    let voice=serde_json::json!({"vocalModeInherited":true,"vocalModePreset":""}).as_object().unwrap().clone();
    StylePreset {revision:revision.into(),modes:Default::default(),voice:voice.into_iter().collect(),experiments:None}
}
fn equivalent(a:&StylePreset,b:&StylePreset)->bool {
    fn canonical(p:&StylePreset)->serde_json::Value {
        let mut v=serde_json::Map::from_iter(p.voice.clone());

        if v.get("choirSeatingSeparation").and_then(serde_json::Value::as_f64)
            .is_some_and(|value|value as f32==0.7_f32) {v.remove("choirSeatingSeparation");}
        for key in ["relaxedPronunciation","europeanPronunciation"] {
            if v.get(key).and_then(serde_json::Value::as_str)==Some("default") {v.remove(key);}
        }
        for (key,value) in [
            ("vocalModeInherited",serde_json::json!(true)),("vocalModePreset",serde_json::json!("")),
            ("choirNumStems",serde_json::json!(1)),("choirPartName",serde_json::json!("")),
            ("choirSeatingSeed",serde_json::json!(0)),("choirSeatingFullSize",serde_json::json!(0)),
            ("choirSeatingCenterIndex",serde_json::json!(0))] {
            if v.get(key)==Some(&value) {v.remove(key);}
        }
        v.retain(|key,value|!(key.starts_with("param") || key.starts_with("consonant") || key.starts_with("transpose")) || value.as_f64()!=Some(0.0));
        v.insert("vocalModeParams".into(),serde_json::to_value(&p.modes).unwrap());v.into()
    }
    canonical(a)==canonical(b)
}
pub(super) fn show(app:&mut App,ui:&mut Ui) {
    let Some(voice)=app.voice_selection() else {super::widgets::field(ui,crate::i18n::t("默认"),true,crate::i18n::t("先选择歌声"));return;};
    let bank=voice.database_id.to_string();let revision=voice.revision.to_string();
    let Some(inventory)=app.voice_inventory() else {super::widgets::field(ui,crate::i18n::t("默认"),true,crate::i18n::t("歌声正在加载，请稍候"));return;};
    let names=inventory.configuration.as_ref().map(|c|c.vocal_mode_names().to_vec()).unwrap_or_default();
    let Ok((reference,name,current))=app.editor.voice_preset(&revision) else {return;};
    let mut state=std::mem::take(&mut app.preset_library);
    let result=(||->Result<(),String> {
        let (_,store)=library(&mut state)?;
        let selected=if name.is_empty() || store.get(&bank,&name).is_some(){name.as_str()}else{""};
        let program=defaults(&revision);let saved=store.get(&bank,selected).unwrap_or(&program);
        let modified=!equivalent(&current,saved);
        let label=format!("{}{}",if selected.is_empty(){crate::i18n::t("默认")}else{selected},if modified{"*"}else{""});
        let mut action=None;
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y=28.0;
            ui.visuals_mut().widgets.inactive.bg_fill=BORDER;
            ui.visuals_mut().widgets.inactive.corner_radius=CornerRadius::same(4);
            ComboBox::from_id_salt("voice-preset")
                .width(ui.available_width()).selected_text(label)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside).show_ui(ui,|ui| {
                    ui.set_min_width(ui.available_width().max(180.0));
                    for (id,label) in [(0,crate::i18n::t("重置")),(1,crate::i18n::t("保存")),(2,crate::i18n::t("保存为新的预设"))] {
                        if ui.add_enabled(!app.editor.is_editing(),Button::new(label).frame(false).min_size(vec2(ui.available_width(),28.0))).clicked(){action=Some((id,selected.to_owned()));ui.close();}
                        if id==0 {ui.separator();}
                    }
                    ui.separator();
                    let default_label=format!("{}{}",crate::i18n::t("默认"),if selected.is_empty()&&modified{"*"}else{""});
                    if ui.add_enabled(!app.editor.is_editing(),Button::selectable(selected.is_empty(),default_label).min_size(vec2(ui.available_width(),28.0))).clicked(){action=Some((3,String::new()));ui.close();}
                    for (label,preset) in store.list(&bank).filter(|(label,_)|!label.is_empty()) {
                        let compatible=preset.validate_for(&revision,&names);
                        ui.horizontal(|ui| {
                            let width=(ui.available_width()-24.0).max(20.0);
                            let row_label=format!("{}{}",label,if selected==label&&modified{"*"}else{""});
                            let response=ui.add_enabled(compatible.is_ok()&&!app.editor.is_editing(),Button::selectable(selected==label,row_label).min_size(vec2(width,28.0)));
                            if response.clicked(){action=Some((3,label.clone()));ui.close();}
                            if let Err(reason)=compatible {response.on_disabled_hover_text(crate::i18n::message(&reason));}
                            if ui.add(IconButton::new(Icon::Close,crate::i18n::t("删除")).size(vec2(20.0,24.0)).icon_size(12.0)).clicked(){action=Some((4,label.clone()));}
                        });
                    }
                });
        });
        match action {
            Some((0,_))=>{app.commit_lyric();app.editor.apply_voice_preset(reference,&names,&program,"")?;},
            Some((1,label))=>save(app,&mut state,&bank,&revision,reference,&label,&names)?,
            Some((2,label))=>state.naming=Some(Naming {bank,revision,reference,name:label,focus:true}),
            Some((3,label))=>{
                let preset=library(&mut state)?.1.get(&bank,&label).cloned().unwrap_or(program);
                preset.validate_for(&revision,&names)?;app.commit_lyric();app.editor.apply_voice_preset(reference,&names,&preset,&label)?;
            },
            Some((4,label))=>{let (path,store)=library(&mut state)?;let mut next=store.clone();next.remove(&bank,&label)?;next.save(path)?;*store=next;},
            _=>(),
        }
        Ok(())
    })();
    if let Err(error)=result {
        if state.library.as_ref().is_some_and(Result::is_err) {
            super::widgets::field(ui,crate::i18n::t("默认"),true,&error);
            if !state.load_error_shown {app.error=Some(error);state.load_error_shown=true;}
        }else {app.error=Some(error);}
    }
    app.preset_library=state;
}
fn save(app:&mut App,state:&mut PresetLibrary,bank:&str,revision:&str,reference:EntityId,name:&str,catalogue:&[String])->Result<(),String> {
    let (selected,_,preset)=app.editor.voice_preset(revision)?;
    if reference!=selected {return Err(crate::i18n::t("选中的音符组已经改变").into());}
    let (path,store)=library(state)?;let mut next=store.clone();next.store(bank,name,preset.clone(),catalogue)?;next.save(path)?;*store=next;
    app.editor.apply_voice_preset(reference,catalogue,&preset,name)?;Ok(())
}
