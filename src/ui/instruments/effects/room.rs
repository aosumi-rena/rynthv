use crate::{app::App,model::Project,ui::{icons::Icon,theme::*}};
use eframe::egui::*;
use serde_json::{Value,json};

pub(crate) fn linked(project:&Project)->bool {
    project.extra.get("projectMixer").and_then(|v|v.get("linkRoomSettings"))
        .and_then(Value::as_bool).unwrap_or(true)
}
pub(crate) fn apply_link(project:&mut Project,source:u64,before:&Value,after:&Value,was:bool,now:bool) {
    if was!=now {
        let mixer=project.extra.entry("projectMixer".into()).or_insert_with(||json!({}));
        mixer["linkRoomSettings"]=json!(now);
    }
    if !now{return;}
    for key in ["size","reflectionGain"] {
        if was && before.get(key)==after.get(key){continue;}
        let Some(value)=after.get(key)else{continue};
        for track in &mut project.tracks {
            if track.id==source||track.is_instrument()||track.is_instrumental(){continue;}
            let fx=track.mixer.entry("fxParams".into()).or_insert_with(crate::synthesis::effects::Configuration::defaults);
            let Some(fx)=fx.as_object_mut()else{continue};
            let room=fx.entry("room").or_insert_with(||json!({"enabled":false,"positionX":0.,"positionY":0.,"size":15.,"reflectionGain":0.}));
            if room.is_object(){room[key]=value.clone();}
        }
    }
}

#[derive(Clone)]
struct Drag {original:[f32;2],current:[f32;2],origin:Pos2,frame:u64}
pub(super) fn show(ui:&mut Ui,value:&mut Value,linked:&mut bool,color:Color32) {
    ui.checkbox(linked,crate::i18n::t("Link settings across all tracks"));
    let width=ui.available_width();
    let (rect,_)=ui.allocate_exact_size(vec2(width,width*0.68),Sense::hover());
    let area=Rect::from_min_max(rect.min,rect.right_bottom()-vec2(0.,40.));
    let size=super::number(value,"size",15.) as f32;
    let pixels=rect.width()/size.max(0.001);
    let original=[super::number(value,"positionX",0.) as f32,super::number(value,"positionY",0.) as f32];
    let id=ui.make_persistent_id("room-position");let frame=ui.ctx().cumulative_frame_nr();
    let mut drag=ui.ctx().data(|d|d.get_temp::<Drag>(id)).filter(|v|v.frame+1>=frame);
    let position=|v:[f32;2]|pos2(area.center().x+v[0]*pixels,area.bottom()-v[1]*pixels);
    let point=position(drag.as_ref().map_or(original,|d|d.current));
    let response=ui.interact(Rect::from_center_size(point,vec2(22.,22.)).intersect(area),id.with("point"),Sense::drag());
    if response.drag_started()&&let Some(origin)=ui.input(|i|i.pointer.press_origin()) {
        drag=Some(Drag {original,current:original,origin,frame});
    }
    if response.dragged()&&let Some(d)=&mut drag&&let Some(pointer)=response.interact_pointer_pos(){
        let delta=pointer-d.origin;
        d.current=[((d.original[0]+delta.x/pixels)*2.).round()*0.5,
            ((d.original[1]-delta.y/pixels)*2.).round()*0.5];
        d.current[0]=d.current[0].clamp(-size*0.5,size*0.5);
        d.current[1]=d.current[1].clamp(0.,size*0.5);d.frame=frame;
    }
    if ui.input(|i|i.key_pressed(Key::Escape)){drag=None;}
    if response.drag_stopped()&&let Some(d)=drag.take(){value["positionX"]=json!(d.current[0]);value["positionY"]=json!(d.current[1]);}
    let current=drag.as_ref().map_or([super::number(value,"positionX",0.) as f32,super::number(value,"positionY",0.) as f32],|d|d.current);
    ui.ctx().data_mut(|d|{if let Some(v)=drag{d.insert_temp(id,v);}else{d.remove::<Drag>(id);}});
    let p=ui.painter().with_clip_rect(rect);p.rect_filled(rect,4,CANVAS);
    if ui.is_enabled(){
        for i in -(size.ceil() as i32)/2..=(size.ceil() as i32)/2 {
            let x=area.center().x+i as f32*pixels;p.vline(x,area.y_range(),Stroke::new(0.5,BORDER));
            let y=area.bottom()-i as f32*pixels;
            if i>=0&&y>=area.top(){p.hline(area.x_range(),y,Stroke::new(0.5,BORDER));}
        }
        p.hline(area.x_range(),area.bottom(),Stroke::new(1.,MUTED));
        for offset in [-16.,16.] {Icon::Microphone.paint(ui,Rect::from_center_size(rect.center_bottom()+vec2(offset,-20.),vec2(19.,27.)),MUTED);}
        p.circle_filled(position(current),6.,color);
        p.text(rect.right_bottom()-vec2(8.,16.),Align2::RIGHT_CENTER,format!("{size}×{size}m"),FontId::proportional(10.),MUTED);
    }
    super::knobs(ui,value,&[("size","Room Size",super::scale(5.,30.,15.,0.1,false," m")),
        ("reflectionGain","Reflection",super::scale(-15.,15.,0.,0.1,false," dB"))],None);
}

pub(super) fn color(app:&App)->Color32{crate::ui::track_color::TrackColors::new(app.editor.current_track()).accent}
