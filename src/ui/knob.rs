use super::theme::*;
use eframe::egui::{text::{CCursor, CCursorRange}, *};

pub fn arc(p: &Painter, center: Pos2, radius: f32, fraction: f32, color: Color32) {
    let sweep=fraction.clamp(0.0,1.0)*270.0;
    if sweep==0.0 {return;}
    let steps=(sweep/3.0).ceil() as usize;
    p.add(Shape::line((0..=steps).map(|i| {
        let a=(135.0+sweep*i as f32/steps as f32).to_radians();
        center+vec2(a.cos(),a.sin())*radius
    }).collect(),Stroke::new(2.0,color)));
}

#[derive(Clone, Copy)]
pub struct Scale {pub min:f64,pub max:f64,pub default:f64,pub step:f64,pub logarithmic:bool,pub suffix:&'static str}
impl Scale {
    fn position(self,v:f64)->f64 {
        if self.logarithmic {(v.max(self.min).ln()-self.min.ln())/(self.max.ln()-self.min.ln())}
        else {(v-self.min)/(self.max-self.min)}.clamp(0.0,1.0)
    }
    fn value(self,t:f64)->f64 {
        let t=t.clamp(0.0,1.0);
        let v=if self.logarithmic {(self.min.ln()+t*(self.max.ln()-self.min.ln())).exp()}
            else {self.min+t*(self.max-self.min)};
        ((v/self.step).round()*self.step).clamp(self.min,self.max)
    }
    fn text(self,v:f64)->String {
        let digits=if self.step>=1.0 {0}else if self.step>=0.1 {1}else if self.step>=0.01 {2}else{3};
        format!("{v:.digits$}")
    }
}
#[derive(Clone)]
struct Draft {source:u64,position:f64,frame:u64}
#[derive(Clone)]
struct Entry {source:u64,text:String,focus:bool,frame:u64}

pub fn show(ui:&mut Ui,id:Id,width:f32,label:&str,source:f64,scale:Scale)->Option<f64> {
    let (rect,_)=ui.allocate_exact_size(vec2(width,74.0),Sense::hover());
    let center=rect.center_top()+vec2(0.0,26.0);
    let response=ui.interact(Rect::from_center_size(center,Vec2::splat(42.0)),id,Sense::click_and_drag());
    let frame=ui.ctx().cumulative_frame_nr();
    let mut draft=ui.ctx().data(|d|d.get_temp::<Draft>(id)).filter(|v|v.source==source.to_bits() && v.frame+1>=frame);
    let mut commit=None;
    if response.dragged() {
        let d=draft.get_or_insert(Draft {source:source.to_bits(),position:scale.position(source),frame});
        let movement=response.drag_delta();d.position=(d.position+f64::from(movement.x-movement.y)/200.0).clamp(0.0,1.0);d.frame=frame;
    }
    let mut value=draft.as_ref().map_or(source,|d|scale.value(d.position));
    if response.double_clicked(){value=scale.default;commit=Some(value);draft=None;}
    if ui.input(|i|i.key_pressed(Key::Escape)){draft=None;value=source;}
    if response.drag_stopped(){if draft.is_some(){commit=Some(value);}draft=None;}
    ui.ctx().data_mut(|d|{if let Some(v)=draft {d.insert_temp(id,v);}else{d.remove::<Draft>(id);}});
    let caption=Rect::from_center_size(rect.center_bottom()-vec2(0.0,12.0),vec2(width,22.0));
    let input_id=id.with("precise");
    let caption_response=ui.interact(caption,id.with("caption"),Sense::click());
    let mut entry=ui.ctx().data(|d|d.get_temp::<Entry>(input_id)).filter(|v|v.source==source.to_bits() && v.frame+1>=frame && ui.is_enabled());
    if caption_response.double_clicked(){entry=Some(Entry {source:source.to_bits(),text:scale.text(value),focus:true,frame});}
    let editing=entry.is_some();
    if let Some(mut entry)=entry {
        let mut child=super::child(ui,caption,input_id.with("area"));
        let edit=child.add_sized(caption.size(),TextEdit::singleline(&mut entry.text).id(input_id).font(FontId::proportional(12.0)).margin(vec2(2.0,0.0)).horizontal_align(Align::Center).vertical_align(Align::Center).char_limit(16));
        if entry.focus {
            edit.request_focus();
            if let Some(mut state)=TextEdit::load_state(ui.ctx(),input_id){state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0),CCursor::new(entry.text.chars().count()))));state.store(ui.ctx(),input_id);}
            entry.focus=false;
        }
        let escape=ui.input(|i|i.key_pressed(Key::Escape));
        if escape||ui.input(|i|i.key_pressed(Key::Enter))||edit.lost_focus(){
            if !escape && let Ok(v)=entry.text.trim().parse::<f64>() && v.is_finite(){value=v.clamp(scale.min,scale.max);commit=Some(value);}
            edit.surrender_focus();ui.ctx().data_mut(|d|d.remove::<Entry>(input_id));
        }else{entry.frame=frame;ui.ctx().data_mut(|d|d.insert_temp(input_id,entry));}
    }
    let p=ui.painter();let active=if ui.is_enabled(){TEXT}else{MUTED};
    arc(p,center,18.0,1.0,BUTTON);arc(p,center,18.0,scale.position(value) as f32,active);
    p.circle(center,13.0,BORDER,Stroke::new(0.8,BUTTON));
    let angle=(135.0+270.0*scale.position(value) as f32).to_radians();
    p.line_segment([center,center+vec2(angle.cos(),angle.sin())*11.0],Stroke::new(2.0,active));
    if !editing {let caption=if response.hovered()||response.dragged()||caption_response.hovered(){format!("{}{}",scale.text(value),scale.suffix)}else{label.to_owned()};p.text(rect.center_bottom()-vec2(0.0,12.0),Align2::CENTER_CENTER,caption,FontId::proportional(12.0),MUTED);}
    response.widget_info(||WidgetInfo::labeled(WidgetType::Slider,ui.is_enabled(),label));
    commit.filter(|v|*v!=source)
}
