use crate::{app::App,model::QUARTER};
use eframe::egui::*;
use serde_json::{Value,json};
pub fn set_viewport(ctx:&Context,view:&str,rect:Rect){ctx.data_mut(|d|d.insert_temp(Id::new(("script-viewport",view)),rect));}
pub(super) fn request(app:&mut App,ctx:&Context,args:&[Value])->Result<Value,String>{
    let view=args.first().and_then(Value::as_str).ok_or("Missing editor view")?;
    if !["piano","arrangement"].contains(&view){return Err("Unknown editor view".into());}
    let method=args.get(1).and_then(Value::as_str).ok_or("Missing navigation method")?;
    let values=args.get(2).and_then(Value::as_array).ok_or("Missing navigation values")?;
    let number=||values.first().and_then(Value::as_f64).filter(|v|v.is_finite()).ok_or_else(||"Invalid navigation value".to_owned());
    let rect=ctx.data(|d|d.get_temp::<Rect>(Id::new(("script-viewport",view)))).unwrap_or(Rect::ZERO);
    let piano=view=="piano";let viewport=if piano{&mut app.view.piano}else{&mut app.view.arrangement};
    let scale=f64::from(viewport.beat_width)/QUARTER as f64;
    let left=viewport.start*QUARTER as f64;let right=left+f64::from(rect.width())/scale;
    let vertical=f64::from(if piano{app.view.row_height}else{64.});
    let top=f64::from(if piano{app.view.top_pitch+0.5}else{0.});
    let height=f64::from(rect.height());
    Ok(match method{
        "timeScale"=>json!(scale),"valueScale"=>json!(vertical),"timeRange"=>json!([left,right]),"valueRange"=>json!([top-height/vertical,top]),
        "timeLeft"=>{viewport.start=(number()?/QUARTER as f64).max(0.);Value::Null},
        "timeRight"=>{viewport.start=((number()?-f64::from(rect.width())/scale)/QUARTER as f64).max(0.);Value::Null},
        "setTimeScale"=>{let v=number()?*QUARTER as f64;if v<=0.{return Err("Scale must be positive".into());}viewport.beat_width=(v as f32).clamp(if piano{10.}else{2.},if piano{500.}else{100.});Value::Null},
        "valueCenter"=>{if piano{app.view.top_pitch=(number()?+height/vertical/2.-0.5)as f32;app.view.top_pitch=app.view.top_pitch.clamp(12.,127.);}Value::Null},
        "snap"=>json!(crate::model::ticks(app.view.snap_beat(number()?/QUARTER as f64))),
        "t2x"=>json!((number()?-left)*scale),"x2t"=>json!(left+number()?/scale),
        "v2y"=>json!((top-number()?)*vertical),"y2v"=>json!(top-number()?/vertical),
        _=>return Err(format!("Unknown navigation method: {method}")),
    })
}
