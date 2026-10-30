use crate::{app::App,synthesis::{experiments::LocalBlendWeight,plan::OwnerId,score::PhoneSource}};
use eframe::egui::*;

pub(super) fn available(app:&App)->bool {
    let settings=app.synthesis.library.settings.experiments;
    settings.active(settings.voice_blending) && app.editor.experiment_controls().is_ok_and(|(_,c)|!c.acoustic_blend && !c.voices.is_empty())
        && app.voice_inventory().and_then(|v|v.configuration.as_ref()).is_some_and(|c|c.family!=crate::synthesis::voice::VoiceFamily::Sv1)
}
pub(super) fn show(app:&mut App,ui:&mut Ui) {
    let Ok((reference,controls))=app.editor.experiment_controls() else{return;};
    let notes:Vec<_>=app.editor.notes().iter().filter(|n|app.editor.selected.contains(&n.id)).cloned().collect();
    if notes.is_empty() {ui.weak(crate::i18n::t("在钢琴卷帘中选择音符"));return;}
    let selection:Vec<_>=notes.iter().map(|n|n.id).collect();
    for target in controls.voices {
        let key=format!("{}:{}",target.voice.database_id,target.voice.revision);
        let title=app.synthesis.library.entry(&target.voice).map_or_else(||target.voice.revision.to_string(),|e|format!("{} · {}",e.display_name,e.revision));
        ui.push_id((&key,reference,&selection),|ui| {
            ui.label(title);
            let local=match LocalBlendWeight::read(&notes[0].extra,&key) {Ok(v)=>v,Err(e)=>{ui.colored_label(Color32::LIGHT_RED,e.to_string());return;}};
            let mut edits=Vec::new();
            if let Some(value)=weight(ui,"note",crate::i18n::t("覆盖音符混合比例"),local.note,target.weight) {edits.push((None,value));}
            if notes.len()>1 && notes[1..].iter().any(|n|LocalBlendWeight::read(&n.extra,&key).is_ok_and(|v|v.note!=local.note)) {ui.weak(crate::i18n::t("多个值"));}
            if notes.len()==1 {
                let mut phones=std::collections::BTreeMap::new();
                for snapshot in app.synthesis.phonemes(OwnerId::new(reference)) {
                    if !snapshot.input.notes.iter().any(|n|n.id==notes[0].id && n.lyrics==notes[0].lyrics && n.phonemes==notes[0].phonemes) {continue;}
                    for phone in snapshot.phones.0.iter() {if let Some(source)=phone.source.filter(|s|s.note_id==notes[0].id) {
                        phones.insert(source.phone_index,phone.token.clone());
                    }}
                }
                CollapsingHeader::new(crate::i18n::t("逐音素混合")).id_salt("phone-blend").show(ui,|ui| {
                    if phones.is_empty() {ui.weak(crate::i18n::t("等待音素准备完成。"));}
                    for (index,token) in phones {
                        if let Some(value)=weight(ui,index,&format!("{} · {token}",index+1),local.phones.get(&index).copied(),local.note.unwrap_or(target.weight)) {
                            edits.push((Some(PhoneSource{note_id:notes[0].id,phone_index:index}),value));
                        }
                    }
                });
            }
            for (phone,value) in edits {if let Err(error)=app.editor.set_local_blend_weight(&key,phone,value) {app.error=Some(error);}}
        });
    }
}
fn weight(ui:&mut Ui,salt:impl std::hash::Hash + std::fmt::Debug,label:&str,original:Option<f32>,default:f32)->Option<Option<f32>> {
    let id=ui.make_persistent_id(salt);
    let mut enabled=original.is_some();
    let mut result=None;
    ui.push_id(id,|ui| {
        if ui.checkbox(&mut enabled,label).changed() {result=Some(enabled.then_some(original.unwrap_or(default)));}
        if enabled {
            let mut current=ui.ctx().data(|d|d.get_temp::<f32>(id)).unwrap_or(original.unwrap_or(default));
            let response=ui.add(Slider::new(&mut current,0.0..=1.0));
            if response.dragged() {ui.ctx().data_mut(|d|d.insert_temp(id,current));}
            if response.drag_stopped() || response.changed() && !response.dragged() {
                ui.ctx().data_mut(|d|d.remove::<f32>(id));result=Some(Some(current));
            }
        }else{ui.ctx().data_mut(|d|d.remove::<f32>(id));}
    });
    result
}
