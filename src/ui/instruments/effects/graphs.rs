use crate::{synthesis::effects::equalizer::{Kind,Parameters},ui::theme::*};
use eframe::egui::*;
use rustfft::num_complex::Complex64;
use serde_json::{Value,json};

fn frequency(t:f32)->f64 {(24.0_f64.ln()+f64::from(t.clamp(0.,1.))*(19000.0_f64/24.0).ln()).exp()}
fn position(f:f64)->f32 {((f.max(24.)/24.).ln()/(19000.0_f64/24.0).ln()) as f32}
fn response(parameters:Parameters,f:f64)->f64 {
    let [a,b,c,m0,m1,m2]=parameters.coefficients(48000).map(f64::from);
    let z=Complex64::from_polar(1.,std::f64::consts::TAU*f/48000.);
    let a11=2.*a-1.;let a12=-2.*b;let a21=2.*b;let a22=1.-2.*c;
    let determinant=(z-a11)*(z-a22)-a12*a21;
    let s1=((z-a22)*(2.*b)+a12*(2.*c))/determinant;
    let s2=(a21*(2.*b)+(z-a11)*(2.*c))/determinant;
    let out=m0+m1*b+m2*c+s1*(m1*a+m2*b)+s2*(-m1*b+m2*(1.-c));
    20.*out.norm().max(1e-12).log10()
}
#[derive(Clone)]
struct Drag {index:usize,frequency:f64,gain:f64,frame:u64}
pub(super) fn equalizer(ui:&mut Ui,value:&mut Value)->usize {
    let selected_id=ui.make_persistent_id("eq-band");let drag_id=selected_id.with("drag");
    let mut selected=ui.ctx().data(|d|d.get_temp::<usize>(selected_id)).unwrap_or(0).min(3);
    let (rect,_)=ui.allocate_exact_size(vec2(ui.available_width(),128.0),Sense::hover());
    let area=rect.shrink2(vec2(7.,8.));
    let frame=ui.ctx().cumulative_frame_nr();
    let mut drag=ui.ctx().data(|d|d.get_temp::<Drag>(drag_id)).filter(|d|d.frame+1>=frame);
    let mut parameters:Vec<_>=(0..4).map(|i|{
        let v=if i==0{&value["lowShelf"]}else{&value["filters"][i-1]};
        Parameters {kind:if i==0{Kind::LowShelf}else{Kind::Peak},frequency:super::number(v,"freq",25.) as f32,
            gain:super::number(v,"gain",0.) as f32,q:if i==0{1.}else{super::number(v,"q",0.71) as f32}}
    }).collect();
    if let Some(d)=&drag {parameters[d.index].frequency=d.frequency as f32;parameters[d.index].gain=d.gain as f32;}
    let y=|gain:f32|area.center().y-gain/48.0*area.height();
    for (i,p) in parameters.iter_mut().enumerate(){
        let point=pos2(lerp(area.x_range(),position(f64::from(p.frequency))),y(p.gain));
        let r=ui.interact(Rect::from_center_size(point,Vec2::splat(18.)).intersect(rect),selected_id.with(i),Sense::click_and_drag());
        if r.clicked()||r.drag_started(){selected=i;}
        if r.dragged()&&let Some(pointer)=r.interact_pointer_pos(){
            let f=frequency((pointer.x-area.left())/area.width());let gain=f64::from(((area.center().y-pointer.y)/area.height()*48.).clamp(-24.,24.));
            drag=Some(Drag {index:i,frequency:f,gain,frame});p.frequency=f as f32;p.gain=gain as f32;
        }
        if r.drag_stopped()&&let Some(d)=drag.take(){
            let v=if i==0{&mut value["lowShelf"]}else{&mut value["filters"][i-1]};
            v["freq"]=json!(d.frequency as f32);v["gain"]=json!(d.gain as f32);
        }
        if r.double_clicked(){let v=if i==0{&mut value["lowShelf"]}else{&mut value["filters"][i-1]};v["gain"]=json!(0.0);}
    }
    if ui.input(|i|i.key_pressed(Key::Escape)){drag=None;}
    ui.ctx().data_mut(|d|{d.insert_temp(selected_id,selected);if let Some(v)=drag{d.insert_temp(drag_id,v);}else{d.remove::<Drag>(drag_id);}});
    let painter=ui.painter().with_clip_rect(rect);painter.rect_filled(rect,4,CANVAS);
    for (f,label) in [(100.,"100"),(1000.,"1K"),(10000.,"10K")]{let x=lerp(area.x_range(),position(f));painter.vline(x,rect.y_range(),Stroke::new(0.5,BORDER));painter.text(pos2(x+4.,rect.bottom()-5.),Align2::LEFT_BOTTOM,label,FontId::proportional(9.),MUTED);}
    let color=if ui.is_enabled(){TEXT}else{MUTED};
    let curve=(0..=160).map(|i|{let t=i as f32/160.;let gain=parameters.iter().map(|p|response(*p,frequency(t))).sum::<f64>() as f32;pos2(lerp(area.x_range(),t),y(gain))}).collect();
    painter.add(Shape::line(curve,Stroke::new(1.,color)));
    for (i,p) in parameters.iter().enumerate(){let point=pos2(lerp(area.x_range(),position(f64::from(p.frequency))),y(p.gain));painter.circle_filled(point,7.,if i==selected&&ui.is_enabled(){TEXT}else{MUTED});
        if i==0{painter.line_segment([point-vec2(3.,0.),point+vec2(3.,0.)],Stroke::new(1.,CANVAS));painter.line_segment([point-vec2(1.,3.),point+vec2(2.,0.)],Stroke::new(1.,CANVAS));}
        else{painter.text(point,Align2::CENTER_CENTER,i.to_string(),FontId::proportional(10.),CANVAS);}
    }
    selected
}
pub(super) fn compressor(ui:&mut Ui){
    let (rect,_)=ui.allocate_exact_size(vec2(ui.available_width(),128.),Sense::hover());
    ui.painter().rect_filled(rect,4,CANVAS);
    ui.painter().hline(rect.x_range(),rect.top()+rect.height()*0.267,Stroke::new(1.,MUTED));
}
