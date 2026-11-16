use crate::{
    app::App,
    ui::{
        icons::{Icon, IconButton},
        theme::*,
    },
};
use eframe::egui::*;
pub fn show(app: &mut App, ui: &mut Ui) {
    if ui.button(crate::i18n::t("Mic Mixer")).clicked() {
        app.instrument_mixer = true;
    }
}
pub fn dialog(app: &mut App, ctx: &Context) {
    if !app.instrument_mixer {
        return;
    }
    let Some(entry) = super::current(app).cloned() else {
        app.instrument_mixer = false;
        return;
    };
    let mut config = match crate::synthesis::instruments::microphones::Configuration::read(
        app.editor.current_track().mixer.get("micParams"),&entry.microphones,entry.room.as_deref()) {
        Ok(v)=>v,Err(e)=>{app.error=Some(e);app.instrument_mixer=false;return;}
    };
    let before = config.clone();
    let mut close = false;
    let mut preset_applied=false;
    let modal = Modal::new(Id::new("instrument-mic-mixer"))
        .frame(Frame::new().fill(PANEL).corner_radius(6).inner_margin(16))
        .show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 80.0).clamp(300.0, 1050.0));
            ui.horizontal(|ui| {
                ui.label(&app.editor.current_track().name);
                ui.add_space(20.0);
                ui.allocate_ui_with_layout(vec2(220.,28.),Layout::top_down(Align::Min),|ui|{
                    preset_applied=crate::ui::presets::show(app,ui,crate::ui::presets::Kind::Microphones);
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close = ui
                        .add(IconButton::new(Icon::Close, crate::i18n::t("Close")))
                        .clicked();
                });
            });
            if preset_applied{return;}
            ui.separator();
            ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let (plan, _) = ui.allocate_exact_size(vec2(270.0, 400.0), Sense::hover());
                    let p = ui.painter();
                    p.text(
                        plan.min + vec2(8.0, 14.0),
                        Align2::LEFT_TOP,
                        super::native_label(&format!(
                            "Mic_{}",
                            entry.room.as_deref().unwrap_or("Studio A")
                        )),
                        FontId::proportional(13.0),
                        TEXT,
                    );
                    let player = plan.min + vec2(135.0, 85.0);
                    p.circle_stroke(player, 26.0, Stroke::new(2.0, crate::ui::theme::accent(ui)));
                    if entry.name == "Orchestral Flute 1" {
                        p.text(
                            player,
                            Align2::CENTER_CENTER,
                            "Fl",
                            FontId::proportional(10.0),
                            crate::ui::theme::accent(ui),
                        );
                    } else {
                        Icon::Instrument.paint(
                            ui,
                            Rect::from_center_size(player, vec2(24.0, 24.0)),
                            crate::ui::theme::accent(ui),
                        );
                    }
                    let active=config.mix(&entry.microphones);
                    for (x, y, name) in [
                        (135.0, 145.0, "Close"),
                        (135.0, 270.0, "Decca-C"),
                        (85.0, 300.0, "Decca-L"),
                        (185.0, 300.0, "Decca-R"),
                        (40.0, 345.0, "Wide-L"),
                        (230.0, 345.0, "Wide-R"),
                        (120.0, 355.0, "Ambi-L"),
                        (150.0, 355.0, "Ambi-R"),
                    ] {
                        Icon::Microphone.paint(
                            ui,
                            Rect::from_center_size(plan.min + vec2(x, y), vec2(16.0, 26.0)),
                            if active.iter().any(|v|v.name==name) { crate::ui::theme::accent(ui) } else { MUTED.gamma_multiply(0.4) },
                        );
                    }
                    for group in ["Close", "Decca", "Wide", "Ambi"] {
                        let mut channels: Vec<_> = entry
                            .microphones
                            .iter()
                            .filter(|n| n.starts_with(group))
                            .collect();
                        channels.sort_by_key(|name|match crate::synthesis::instruments::microphones::split(name).1 {"L"=>0,"C"=>1,"R"=>2,_=>0});
                        if channels.is_empty() {
                            continue;
                        }
                        let settings=config.group_mut(group).expect("catalogued microphone group");
                        Frame::new()
                            .fill(BORDER)
                            .inner_margin(10)
                            .corner_radius(3)
                            .show(ui, |ui| {
                                let width = channels.len() as f32 * 68.0;
                                ui.set_width(width);
                                ui.horizontal(|ui| {
                                    ui.label(super::native_label(&format!("Mic_{group}")));
                                    if channels.len()>1 && ui.add(IconButton::new(Icon::Link,crate::i18n::t("Link")).selected(settings.link)).clicked(){settings.link=!settings.link;}
                                    if ui.add(Button::selectable(settings.mute,"M")).clicked(){settings.mute=!settings.mute;}
                                    if ui.add(Button::selectable(settings.solo,"S")).clicked(){settings.solo=!settings.solo;}
                                });
                                ui.add_space(14.0);
                                ui.horizontal(|ui| {
                                    for channel in channels {
                                        ui.vertical(|ui| {
                                            ui.set_width(58.0);
                                            let side = channel
                                                .split_once('-')
                                                .map(|(_, v)| match v {
                                                    "L" => "Mic_Left",
                                                    "R" => "Mic_Right",
                                                    _ => "Mic_Center",
                                                })
                                                .unwrap_or("Mic_Front");
                                            ui.label(super::native_label(side));
                                            let key=crate::synthesis::instruments::microphones::split(channel).1;
                                            let mic=settings.mics.iter().find(|v|v.key==key).expect("catalogued microphone");
                                            let previous=mic.value.gain;
                                            let mut gain=if mic.value.gain>0.{20.*mic.value.gain.log10()}else{-60.};
                                            let slider=ui.add_sized(vec2(40.,255.),Slider::new(&mut gain,-60.0..=6.0).vertical().show_value(false));
                                            let edit=ui.add(DragValue::new(&mut gain).range(-60.0..=6.0).speed(0.1).fixed_decimals(1));
                                            if slider.double_clicked(){gain=0.;}
                                            if slider.changed()||slider.double_clicked()||edit.changed(){
                                                let next=10f32.powf(gain/20.);
                                                for other in &mut settings.mics {
                                                    if other.key==key {other.value.gain=next;}
                                                    else if settings.link {other.value.gain=(other.value.gain*next/previous.max(0.001)).clamp(0.001,10f32.powf(0.3));}
                                                }
                                            }
                                        });
                                    }
                                });
                            });
                    }
                });
            });
        });
    if config != before {
        app.editor.begin("Mic Mixer");
        let id = app.editor.track;
        app.editor
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .mixer.insert("micParams".into(),serde_json::to_value(config).expect("finite microphone controls"));
        app.editor.finish();
    }
    if close || (modal.is_top_modal && !modal.any_popup_open && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))) {
        app.instrument_mixer = false;
    }
}
