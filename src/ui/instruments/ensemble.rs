use crate::{app::App,ui::{theme::*,track_color::TrackColors}};
use eframe::egui::*;
use serde_json::json;

#[derive(Clone)]
struct Drag { value:usize, origin:Pos2, initial:usize, frame:u64 }

pub(super) fn players(app:&mut App,ui:&mut Ui) {
    let reference=app.editor.active_ref().unwrap_or(&app.editor.current_track().main_ref);
    let reference_id=reference.id;
    let original=crate::synthesis::instruments::players(reference).unwrap_or(1);
    let color=TrackColors::new(app.editor.current_track()).accent;
    let frame=ui.ctx().cumulative_frame_nr();
    let id=ui.make_persistent_id(("instrument-players",reference_id));
    let mut drag=ui.ctx().data(|d|d.get_temp::<Drag>(id)).filter(|d|d.frame+1>=frame);
    let (rect,response)=ui.allocate_exact_size(vec2(ui.available_width(),80.),Sense::drag());
    if response.drag_started()&&!app.editor.is_editing()
        &&let Some(origin)=ui.input(|i|i.pointer.press_origin()) {
        drag=Some(Drag {value:original,origin,initial:original,frame});
    }
    if let Some(d)=&mut drag {
        d.frame=frame;
        if response.dragged()&&let Some(position)=response.interact_pointer_pos() {
            d.value=(d.initial as i32+((d.origin.y-position.y)/10.).round() as i32).clamp(1,16) as usize;
        }
    }
    let value=drag.as_ref().map_or(original,|d|d.value);
    ui.painter().rect_filled(rect,4,if response.hovered(){BUTTON_HOVER}else{BORDER});
    ui.painter().text(rect.left_center()+vec2(24.,-10.),Align2::LEFT_CENTER,value.to_string(),FontId::proportional(28.),color);
    ui.painter().text(rect.left_center()+vec2(24.,18.),Align2::LEFT_CENTER,
        crate::i18n::t(if value==1{"Player"}else{"Players"}),FontId::proportional(12.),TEXT);
    let area=Rect::from_min_max(rect.min+vec2(90.,12.),rect.max-vec2(12.,12.));
    let seats=if value<=8{8}else{18};let columns=if seats==8{8}else{6};
    let rows=seats/columns;let spacing=(area.width()/columns as f32).min(18.);
    let first=(seats-value.min(seats))/2;
    for seat in 0..seats {
        let point=area.center()+vec2((seat%columns) as f32-(columns-1) as f32/2.,
            (seat/columns) as f32-(rows-1) as f32/2.)*vec2(spacing,16.);
        ui.painter().circle_filled(point,4.5,if (first..first+value).contains(&seat){color}else{BUTTON});
    }
    response.clone().on_hover_text(crate::i18n::t("Drag up or down to set the number of players"));
    response.widget_info(||WidgetInfo::labeled(WidgetType::DragValue,ui.is_enabled(),crate::i18n::t("Ensemble")));
    if response.drag_stopped() {
        if value!=original&&!app.editor.is_editing() {
            app.editor.begin(crate::i18n::t("Ensemble"));
            if let Some(track)=app.editor.project.tracks.iter_mut().find(|t|t.id==app.editor.track)
                &&let Some(reference)=std::iter::once(&mut track.main_ref).chain(&mut track.groups).find(|r|r.id==reference_id) {
                let voice=reference.extra.entry("voice".into()).or_insert_with(||json!({}));
                voice["choirNumStems"]=json!(value);
            }
            app.editor.finish();
        }
        drag=None;
    }
    ui.ctx().data_mut(|d|{if let Some(v)=drag{d.insert_temp(id,v);}else{d.remove::<Drag>(id);}});
}

pub(super) fn spread(app:&mut App,ui:&mut Ui) {
    let selected=app.editor.notes().iter().find(|n|app.editor.selected.contains(&n.id));
    let enabled=selected.is_some()&&!app.editor.is_editing();
    let values:[f64;2]=std::array::from_fn(|i|selected.and_then(|n|n.attributes
        .get(["cTimeDispersion","cPitchDispersion"][i])).and_then(serde_json::Value::as_f64).unwrap_or([1.,0.][i]));
    ui.add_enabled_ui(enabled,|ui| {
        for (i,label,key,max) in [(0,"Timing","cTimeDispersion",2.),(1,"Pitch","cPitchDispersion",1.)] {
            let id=ui.make_persistent_id(("instrument-spread",app.editor.reference,app.editor.revision,key));
            if let Some(value)=crate::ui::voice_library::parameter_slider(ui,id,crate::i18n::t(label),values[i],0.,max,0.05) {
                super::controls::edit_notes(app,crate::i18n::t("Ensemble Spread"),|n|{n.attributes.insert(key.into(),json!(value));});
            }
        }
    });
}
