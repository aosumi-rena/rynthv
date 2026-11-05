mod diagram;
mod slider;

use crate::{app::App,synthesis::voice::VoiceFamily};
use eframe::egui::*;

pub(super) fn show(app:&mut App,ui:&mut Ui) {
    let Some(config)=app.voice_inventory().and_then(|v|v.configuration.as_ref()) else{
        super::weak(ui,crate::i18n::t("加载歌声后显示齐唱设置"));return;
    };
    if config.family!=VoiceFamily::Sv2 {
        super::weak(ui,crate::i18n::t("此歌声不支持齐唱"));return;
    }
    let speakers=config.speakers.clone();
    let Ok((reference,settings))=app.editor.unison_settings() else{return};
    let mut next=settings.clone();
    let id=ui.make_persistent_id(("unison-rails",reference,app.editor.revision));
    let mut visual=settings.clone();
    visual.count=slider::preview(ui,id.with("count"),settings.count as f64) as usize;
    visual.spacing=slider::preview(ui,id.with("spacing"),f64::from(settings.spacing)) as f32;
    let Ok(plan)=visual.seating(&speakers) else{return};
    ui.spacing_mut().item_spacing.y=12.0;
    if !speakers.is_empty() {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y=4.0;
            super::weak(ui,crate::i18n::t("歌声声部"));
            ui.spacing_mut().interact_size.y=28.0;
            ComboBox::from_id_salt(("choir-part",reference)).width(ui.available_width())
                .selected_text(part_label(&plan.part)).show_ui(ui,|ui| {
                    let mut seen=std::collections::BTreeSet::new();
                    for speaker in &speakers {
                        if seen.insert(&speaker.part) {
                            ui.selectable_value(&mut next.part,speaker.part.clone(),part_label(&speaker.part));
                        }
                    }
                });
        });
    }
    let accent=crate::ui::track_color::TrackColors::new(app.editor.current_track()).accent;
    if diagram::show(ui,&visual,&plan,&speakers,accent) {
        loop {let seed=uuid::Uuid::new_v4().as_u128() as u32;if seed!=settings.seed {next.seed=seed;break}}
    }
    if let Some(count)=slider::show(ui,id.with("count"),crate::i18n::t("成员数量"),settings.count as f64,1.0,16.0,1.0) {
        next.count=count as usize;
    }
    ui.add_space(-1.0);
    if let Some(spacing)=slider::show(ui,id.with("spacing"),crate::i18n::t("间距"),f64::from(settings.spacing),0.5,2.0,0.05) {
        next.spacing=spacing as f32;
    }
    if next!=settings && let Err(error)=app.editor.set_unison(reference,next) {app.error=Some(error)}
}

fn part_label(part:&str)->&str {
    match part {"S"=>crate::i18n::t("女高音声部"),"A"=>crate::i18n::t("女低音声部"),"T"=>crate::i18n::t("男高音声部"),"B"=>crate::i18n::t("男低音声部"),_=>part}
}
