use crate::{app::App, model::Parameter, ui::{state::Lane, icons::Icon}};
use eframe::egui::*;
use serde::{Serialize,Deserialize};

#[derive(Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
pub enum Kind { Dynamics, Vibrato, Gain, Noise }
impl Kind {
    const ALL:[Self;4]=[Self::Dynamics,Self::Vibrato,Self::Gain,Self::Noise];
    pub fn label(self)->&'static str {crate::i18n::t(match self {
        Self::Dynamics=>"Dynamics",Self::Vibrato=>"Vibrato",Self::Gain=>"Gain",Self::Noise=>"Noise",
    })}
    fn parameter(self)->Option<Parameter> {match self {
        Self::Vibrato=>Some(Parameter::Vibrato),Self::Gain=>Some(Parameter::Loudness),
        Self::Noise=>Some(Parameter::Breathiness),Self::Dynamics=>None,
    }}
}
#[derive(Clone,Serialize,Deserialize)]
pub struct InstrumentLanes {
    pub open:Vec<Kind>,
    pub active:Option<Kind>,
    pub curve:Lane,
}
impl Default for InstrumentLanes {
    fn default()->Self {Self {open:vec![Kind::Dynamics],active:Some(Kind::Dynamics),
        curve:Lane {foreground:Parameter::Loudness,background:Parameter::Loudness,visible:false,..Lane::new()}}}
}
impl InstrumentLanes {
    pub fn valid(&self)->bool {
        self.open.len()<=4 && self.open.iter().enumerate().all(|(i,k)|!self.open[..i].contains(k))
            && self.active.is_none_or(|k|self.open.contains(&k))
            && self.curve.height.is_finite()&&(64.0..=600.0).contains(&self.curve.height)
            && self.curve.zoom.is_finite()&&(50.0..=200.0).contains(&self.curve.zoom)
    }
    fn select(&mut self,kind:Option<Kind>) {
        self.active=kind;
        self.curve.visible=kind.and_then(Kind::parameter).is_some();
        if let Some(p)=kind.and_then(Kind::parameter) {self.curve.foreground=p.clone();self.curve.background=p;}
    }
}
pub fn tabs(app:&mut App,ui:&mut Ui) {
    let mut choice=None;
    let mut close=None;
    ScrollArea::horizontal().id_salt("instrument-lane-tabs").show(ui,|ui|ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x=0.0;
        for &kind in &app.view.instrument_lanes.open {
            let active=app.view.instrument_lanes.active==Some(kind);
            let (response,closed)=crate::ui::lane_tabs::tab(ui,
                if kind==Kind::Dynamics {Icon::Dynamics}else{Icon::InstrumentAutomation},kind.label(),active,Some(kind as u64));
            if response.clicked(){choice=Some(if active {None}else{Some(kind)});}
            if closed{close=Some(kind);}
        }
        if app.view.instrument_lanes.open.len()<Kind::ALL.len() {ui.menu_button("+",|ui| {
            for kind in Kind::ALL {
                if !app.view.instrument_lanes.open.contains(&kind) && ui.button(kind.label()).clicked() {
                    app.view.instrument_lanes.open.push(kind);
                    choice=Some(Some(kind));ui.close();
                }
            }
        });}
    }));
    if let Some(kind)=close {
        app.view.instrument_lanes.open.retain(|k|*k!=kind);
        if app.view.instrument_lanes.active==Some(kind) {choice=Some(None);}
    }
    if let Some(choice)=choice {
        crate::ui::lanes::parameter_deactivate(app);
        app.view.instrument_lanes.select(choice);
    }
}
pub fn show(app:&mut App,ui:&mut Ui) {
    app.view.instrument_lanes.select(app.view.instrument_lanes.active);
    match app.view.instrument_lanes.active {
        Some(Kind::Dynamics)=>super::dynamics::show(app,ui),
        Some(Kind::Vibrato|Kind::Gain|Kind::Noise)=>{
            let id=app.view.instrument_lanes.curve.id;
            crate::ui::lanes::show(app,ui,id);
        },
        None=>{},
    }
}
