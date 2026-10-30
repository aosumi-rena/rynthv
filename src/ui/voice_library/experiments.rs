use crate::{app::App,synthesis::{experiments::{BlendTarget,ExperimentControls},settings::VoiceSelection}};
use eframe::egui::*;
mod dds;

#[derive(Clone)]
struct Draft { original:ExperimentControls, value:ExperimentControls }

pub(super) fn available(app:&App)->bool {
    let s=app.synthesis.library.settings.experiments;
    s.enabled && app.voice_inventory().and_then(|v|v.configuration.as_ref()).is_some_and(|c|
        s.growl || s.dds_controls || s.voice_blending || s.component_effects || (s.random_styles && !c.vocal_mode_names().is_empty()))
}
pub(super) fn show(app:&mut App,ui:&mut Ui) {
    ui.spacing_mut().slider_width=(ui.available_width()-65.0).max(45.0);
    let settings=app.synthesis.library.settings.experiments;
    let (reference,original)=match app.editor.experiment_controls() {Ok(v)=>v,Err(e)=>{super::error_label(ui,e);return;}};
    let id=ui.make_persistent_id(("experimental-draft",reference));
    let mut controls=ui.ctx().data(|d|d.get_temp::<Draft>(id)).filter(|d|d.original==original).map_or_else(||original.clone(),|d|d.value);
    let has_styles=app.voice_inventory().and_then(|v|v.configuration.as_ref()).is_some_and(|c|!c.vocal_mode_names().is_empty());
    let modern=app.voice_inventory().and_then(|v|v.configuration.as_ref()).is_some_and(|c|c.family==crate::synthesis::voice::VoiceFamily::Sv2);
    if settings.dds_controls {dds::show(app,ui,&mut controls.dds);ui.separator();}
    if settings.growl {
        let track=app.editor.current_track();
        let global=ExperimentControls::from_extra(&track.main_ref.extra).unwrap_or_default().growl.unwrap_or_default();
        let is_track=reference==track.main_ref.id;
        if !is_track {
            let mut inherit=controls.growl.is_none();
            if ui.checkbox(&mut inherit,crate::i18n::t("继承轨道嘶吼设置")).changed() {
                controls.growl=if inherit {None}else{Some(global)};
            }
        }
        let mut value=controls.growl.unwrap_or(global);
        ui.add_enabled_ui(is_track || controls.growl.is_some(),|ui| {
            ui.label(crate::i18n::t("嘶吼幅度"));
            let amplitude=ui.add(Slider::new(&mut value.amplitude,0.0..=1.0));
            ui.label(crate::i18n::t("嘶吼频率"));
            let frequency=ui.add(Slider::new(&mut value.frequency,30.0..=90.0).suffix(" Hz"));
            if amplitude.changed() || frequency.changed() {controls.growl=Some(value);}
        });
        ui.separator();
    }
    if settings.random_styles && has_styles {
        ui.checkbox(&mut controls.random_enabled,crate::i18n::t("随机唱法混合"));
        if controls.random_enabled {
            ui.label(crate::i18n::t("随机种子"));
            ui.horizontal_wrapped(|ui| {
                ui.add(DragValue::new(&mut controls.random_seed).range(0..=i32::MAX as u32));
                if ui.button(crate::i18n::t("重新随机")).clicked() {controls.random_seed=(uuid::Uuid::new_v4().as_u128() as u32)&0x7fff_ffff;}
            });
            ui.label(crate::i18n::t("混合量"));
            ui.add(Slider::new(&mut controls.random_amount,0.0..=300.0).suffix("%"));
            if settings.negative_styles {ui.checkbox(&mut controls.random_signed,crate::i18n::t("随机混合允许负唱法"));}
            ui.horizontal_wrapped(|ui| {
                for (i,label) in ["音高","音色","发音"].into_iter().enumerate() {if modern || i==1 {ui.checkbox(&mut controls.random_lanes[i],crate::i18n::t(label));}}
            });
        }
    }
    if settings.voice_blending {
        if settings.random_styles {ui.separator();}
        ui.label(crate::i18n::t("多歌声混合"));
        if settings.random_voices {
            ui.checkbox(&mut controls.random_voice_enabled,crate::i18n::t("Random voice blend"));
            if controls.random_voice_enabled {
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut controls.random_voice_seed).range(0..=i32::MAX as u32));
                    if ui.button(crate::i18n::t("重新随机")).clicked() {controls.random_voice_seed=(uuid::Uuid::new_v4().as_u128() as u32)&0x7fff_ffff;}
                });
                ui.add(Slider::new(&mut controls.random_voice_amount,0.0..=1.0));
            }
        }
        let was_acoustic=controls.acoustic_blend;
        let has_vectors=app.voice_inventory().and_then(|v|v.configuration.as_ref()).is_some_and(|c|c.family!=crate::synthesis::voice::VoiceFamily::Sv2);
        let mut domain=if has_vectors && settings.dds_controls && controls.dds.speaker_vectors {2}else{controls.acoustic_blend as u8};
        let old_domain=domain;
        ComboBox::from_id_salt(("blend-domain",reference)).selected_text(crate::i18n::t(match domain {2=>"Speaker vectors",1=>"声学特征",_=>"渲染后的歌声"})).show_ui(ui,|ui| {
            ui.selectable_value(&mut domain,0,crate::i18n::t("渲染后的歌声"));
            ui.selectable_value(&mut domain,1,crate::i18n::t("声学特征"));
            if has_vectors && settings.dds_controls {ui.selectable_value(&mut domain,2,crate::i18n::t("Speaker vectors"));}
        });
        if domain!=old_domain {controls.dds.speaker_vectors=domain==2;controls.acoustic_blend=domain!=0;}
        if controls.acoustic_blend && !was_acoustic {for target in &mut controls.voices {target.transpose=0;}}
        if controls.acoustic_blend {ui.weak(crate::i18n::t("沿用主歌声的时长和音高。"));}
        let family=app.voice_inventory().and_then(|v|v.configuration.as_ref()).map(|c|c.family);
        let catalog:Vec<_>=app.synthesis.library.catalog().into_iter().flat_map(|c|c.entries.iter())
            .filter(|e|e.family==family && e.availability.is_loadable())
            .map(|e|(VoiceSelection{database_id:e.database_id.clone(),revision:e.revision},format!("{} · {}",e.display_name,super::selector::source_hint(e)))).collect();
        let mut remove=None;
        for (index,target) in controls.voices.iter_mut().enumerate() {
            ui.push_id(index,|ui| {
                ui.horizontal(|ui| {
                    ui.label(catalog.iter().find(|(key,_)|key==&target.voice).map_or_else(||target.voice.revision.to_string(),|(_,label)|label.clone()));
                    if ui.small_button("×").clicked() {remove=Some(index);}
                });
                ui.label(crate::i18n::t("混合比例"));
                ui.add_enabled(!(settings.random_voices && controls.random_voice_enabled),Slider::new(&mut target.weight,0.0..=1.0));
                if !controls.acoustic_blend {
                    ui.label(crate::i18n::t("和声移调"));
                    ui.add(DragValue::new(&mut target.transpose).range(-24..=24).suffix(crate::i18n::t("半音")));
                }
                ui.checkbox(&mut target.inherit_modes,crate::i18n::t("应用同名唱法"));
                if let Some(config)=app.synthesis.inventory(&target.voice).and_then(|v|v.configuration.as_ref()) {
                    if !config.speakers.is_empty() {
                        ComboBox::from_id_salt("blend-member").selected_text(target.speaker_id.and_then(|id|config.speakers.iter().find(|m|m.id==id as usize)).map_or(crate::i18n::t("默认合唱成员"),|m|m.name.as_str())).show_ui(ui,|ui| {
                            ui.selectable_value(&mut target.speaker_id,None,crate::i18n::t("默认合唱成员"));
                            for member in &config.speakers {ui.selectable_value(&mut target.speaker_id,Some(member.id as i32),format!("{} · {}",member.name,member.part));}
                        });
                    }
                }
            });
        }
        if let Some(index)=remove {controls.voices.remove(index);}
        if controls.voices.len()<3 {
            ComboBox::from_id_salt(("add-blend",reference)).selected_text(crate::i18n::t("添加歌声…")).show_ui(ui,|ui| {
                for (voice,label) in &catalog {
                    if !controls.voices.iter().any(|t|&t.voice==voice) && ui.selectable_label(false,label).clicked() {
                        controls.voices.push(BlendTarget{voice:voice.clone(),weight:0.0,transpose:0,inherit_modes:false,speaker_id:None});ui.close();
                    }
                }
            });
        }
    }
    if settings.component_effects {
        ui.separator();
        ui.label(crate::i18n::t("谐波增益"));
        ui.add(Slider::new(&mut controls.harmonic_gain,-24.0..=24.0).suffix(" dB"));
        ui.label(crate::i18n::t("气声增益"));
        ui.add(Slider::new(&mut controls.aspiration_gain,-24.0..=24.0).suffix(" dB"));
        ui.label(crate::i18n::t("环形调制深度"));
        ui.add(Slider::new(&mut controls.ring_depth,0.0..=1.0));
        ui.label(crate::i18n::t("环形调制频率"));
        ui.add(Slider::new(&mut controls.ring_frequency,20.0..=150.0).suffix(" Hz"));
    }
    if controls!=original {
        if ui.input(|i|i.pointer.any_down()) || app.editor.is_editing() {
            ui.ctx().data_mut(|d|d.insert_temp(id,Draft{original,value:controls}));
        }else{
            ui.ctx().data_mut(|d|d.remove::<Draft>(id));
            if let Err(error)=app.editor.set_experiment_controls(reference,controls) {app.error=Some(error);}
        }
    }else{ui.ctx().data_mut(|d|d.remove::<Draft>(id));}
}
