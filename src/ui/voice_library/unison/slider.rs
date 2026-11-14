use crate::ui::theme::*;
use eframe::egui::{text::{CCursor,CCursorRange},*};

#[derive(Clone)]
struct Drag {source:u64,value:f64,frame:u64}
#[derive(Clone)]
struct Entry {text:String,source:u64,focus:bool,frame:u64}

pub(super) fn preview(ui:&Ui,id:Id,source:f64)->f64 {
    let frame=ui.ctx().cumulative_frame_nr();
    ui.ctx().data(|d|d.get_temp::<Drag>(id.with("drag")))
        .filter(|v|v.source==source.to_bits() && v.frame+1>=frame).map_or(source,|v|v.value)
}

pub(super) fn show(ui:&mut Ui,id:Id,label:&str,source:f64,min:f64,max:f64,step:f64)->Option<f64> {
    let (rect,_)=ui.allocate_exact_size(vec2(ui.available_width(),40.0),Sense::hover());
    let rail=Rect::from_min_max(rect.left_top()+vec2(9.5,22.0),rect.right_top()+vec2(-9.5,42.0));
    let response=ui.interact(rail.expand2(vec2(9.5,0.0)),id.with("rail"),Sense::click_and_drag());
    let mut value=preview(ui,id,source);
    let current_frame=ui.ctx().cumulative_frame_nr();
    let quantize=|v:f64| ((v/step).round()*step).clamp(min,max);
    if (response.dragged()||response.clicked()||response.double_clicked()) && let Some(p)=response.interact_pointer_pos() {
        value=quantize(min+f64::from(((p.x-rail.left())/rail.width()).clamp(0.0,1.0))*(max-min));
        ui.ctx().data_mut(|d|d.insert_temp(id.with("drag"),Drag {source:source.to_bits(),value,frame:current_frame}));
    }
    let mut commit=None;
    if response.drag_stopped()||response.clicked()||response.double_clicked() {
        ui.ctx().data_mut(|d|d.remove::<Drag>(id.with("drag")));commit=Some(value);
    }
    ui.painter().text(rect.min+vec2(0.0,1.0),Align2::LEFT_TOP,label,FontId::proportional(12.0),MUTED);
    let y=rail.center().y;
    let center=pos2(lerp(rail.x_range(),((value-min)/(max-min)) as f32),y);
    ui.painter().hline(rail.x_range(),y,Stroke::new(2.0,EDGE));
    ui.painter().line_segment([pos2(rail.left(),y),center],Stroke::new(2.0,MUTED));
    ui.painter().circle(center,9.5,EDGE,Stroke::NONE);
    ui.painter().circle(center,8.5,Color32::from_rgb(65,63,68),Stroke::new(0.7,BUTTON_ACTIVE));
    ui.painter().circle_filled(center,3.0,TEXT);
    let value_rect=Rect::from_min_size(rect.right_top()+vec2(-75.0,1.0),vec2(75.0,16.0));
    let value_response=ui.interact(value_rect,id.with("value"),Sense::click());
    let input_id=id.with("input");let frame=ui.ctx().cumulative_frame_nr();
    let mut entry=ui.ctx().data(|d|d.get_temp::<Entry>(input_id))
        .filter(|e|e.source==source.to_bits()&&e.frame+1>=frame);
    let format=|n:f64| {if step==1.0 {format!("{n:.0}")}
        else{format!("{n:.2}").trim_end_matches('0').trim_end_matches('.').to_owned()}};
    if value_response.double_clicked(){entry=Some(Entry {text:format(value),source:source.to_bits(),focus:true,frame})}
    if let Some(mut entry)=entry {
        let mut child=crate::ui::child(ui,value_rect,id.with("entry-area"));
        let edit=child.add_sized(value_rect.size(),TextEdit::singleline(&mut entry.text).id(input_id)
            .font(FontId::proportional(12.0)).horizontal_align(Align::RIGHT).vertical_align(Align::Center)
            .margin(vec2(2.0,0.0)).char_limit(12));
        if entry.focus {
            edit.request_focus();
            if let Some(mut state)=TextEdit::load_state(ui.ctx(),input_id) {
                state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0),CCursor::new(entry.text.chars().count()))));
                state.store(ui.ctx(),input_id);
            }
            entry.focus=false;ui.ctx().request_repaint();
        }
        let escape=ui.input(|i|i.key_pressed(Key::Escape));
        let enter=ui.input(|i|i.key_pressed(Key::Enter));
        let finished=escape||enter||edit.lost_focus();
        if finished {
            if !escape && let Ok(number)=entry.text.trim().parse::<f64>() && number.is_finite() {commit=Some(quantize(number));}
            edit.surrender_focus();ui.ctx().data_mut(|d|d.remove::<Entry>(input_id));
        } else {
            entry.frame=frame;ui.ctx().data_mut(|d|d.insert_temp(input_id,entry));
        }
    } else {
        ui.ctx().data_mut(|d|d.remove::<Entry>(input_id));
        if response.hovered()||response.dragged()||value_response.hovered() {
            ui.painter().text(value_rect.right_top(),Align2::RIGHT_TOP,format(value),FontId::proportional(12.0),MUTED);
        }
    }
    response.widget_info(||WidgetInfo::labeled(WidgetType::Slider,true,label));
    commit
}
