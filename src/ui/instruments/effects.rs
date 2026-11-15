mod graphs;
pub(crate) mod room;
use crate::{app::App, synthesis::effects::Configuration, ui::{knob::{self,Scale}, theme::*}};
use eframe::egui::*;
use serde_json::{Value,json};

fn number(v:&Value,key:&str,default:f64)->f64 {v.get(key).and_then(Value::as_f64).unwrap_or(default)}
fn editable(source:&Value,defaults:&Value)->Result<Value,String> {
    let object=source.as_object().ok_or("Effect settings must be an object")?;
    let mut result=source.clone();
    for (key,default) in defaults.as_object().unwrap() {
        let Some(value)=object.get(key)else{result[key]=default.clone();continue};
        let fields=value.as_object().ok_or_else(||format!("Invalid effect settings: {key}"))?;
        for (name,default) in default.as_object().unwrap() {
            if !fields.contains_key(name){result[key][name]=default.clone();}
        }
    }
    let eq=&result["postRoomEq"];
    if !eq["lowShelf"].is_object() || !eq["filters"].as_array().is_some_and(|v|v.len()==3&&v.iter().all(Value::is_object)) {
        return Err("Equalizer requires three filters and a low shelf".into());
    }
    Ok(result)
}
fn scale(min:f64,max:f64,default:f64,step:f64,logarithmic:bool,suffix:&'static str)->Scale {
    Scale {min,max,default,step,logarithmic,suffix}
}
fn knobs(ui:&mut Ui,value:&mut Value,items:&[(&str,&str,Scale)],disabled:Option<&str>) {
    let width=(ui.available_width()-ui.spacing().item_spacing.x*items.len().saturating_sub(1) as f32)/items.len().max(1) as f32;
    ui.horizontal(|ui|{for (key,label,scale) in items {
        let source=number(value,key,scale.default);let id=ui.make_persistent_id(*key);
        ui.add_enabled_ui(disabled!=Some(*key),|ui|{
            if let Some(v)=knob::show(ui,id,width,super::native_label(label),source,*scale){value[*key]=json!(v as f32);}
        });
    }});
}
fn section(ui:&mut Ui,key:&str,label:&str,value:&mut Value,content:impl FnOnce(&mut Ui,&mut Value)) {
    ui.painter().hline(ui.available_rect_before_wrap().x_range(),ui.next_widget_position().y,Stroke::new(1.0,EDGE));
    Frame::new().inner_margin(16).show(ui,|ui| {
        ui.set_width(ui.available_width());ui.spacing_mut().item_spacing.y=10.0;
        let mut enabled=value.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        let previous=enabled;
        eframe::egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(),ui.make_persistent_id(key),true)
            .show_header(ui,|ui|{
                ui.label(RichText::new(crate::i18n::t(label)).font(section_title_font()));
                ui.with_layout(Layout::right_to_left(Align::Center),|ui|{
                    let (rect,response)=ui.allocate_exact_size(vec2(28.0,20.0),Sense::click());
                    if response.clicked(){enabled=!enabled;}
                    let rail=Rect::from_center_size(rect.center(),vec2(24.0,8.0));
                    ui.painter().rect_filled(rail,4,if enabled{MUTED}else{EDGE});
                    let center=pos2(if enabled{rail.right()-3.0}else{rail.left()+3.0},rail.center().y);
                    ui.painter().circle(center,8.0,BUTTON,Stroke::new(1.0,BORDER));
                    ui.painter().circle_filled(center,3.0,if enabled{TEXT}else{MUTED});
                    response.widget_info(||WidgetInfo::selected(WidgetType::Checkbox,ui.is_enabled(),enabled,crate::i18n::t(label)));
                });
            }).body_unindented(|ui|{ui.add_enabled_ui(enabled,|ui|content(ui,value));});
        if enabled!=previous{value["enabled"]=json!(enabled);}
    });
}

pub fn show(app:&mut App,ui:&mut Ui) {
    let mut defaults=Configuration::defaults();
    if !app.editor.current_track().is_instrument(){
        defaults["room"]=json!({"enabled":false,"positionX":0.0,"positionY":0.0,"size":15.0,"reflectionGain":0.0});
    }
    let source=app.editor.current_track().mixer.get("fxParams").cloned().unwrap_or_else(||defaults.clone());
    let mut value=match editable(&source,&defaults) {Ok(v)=>v,Err(error)=>{ui.colored_label(ui.visuals().error_fg_color,error);return;}};
    let mut initial=value.clone();
    let was_linked=room::linked(&app.editor.project);let mut linked=was_linked;
    ui.push_id(("track-effects",app.editor.track),|ui| {
        Frame::new().inner_margin(16).show(ui,|ui|{
            ui.label(crate::i18n::t("Preset"));
            if crate::ui::presets::show(app,ui,crate::ui::presets::Kind::Effects){
                value=app.editor.current_track().mixer.get("fxParams").cloned().unwrap_or_else(||defaults.clone());initial=value.clone();
            }
        });
        if !app.editor.current_track().is_instrument(){
            let color=room::color(app);
            section(ui,"room","Room Sound",&mut value["room"],|ui,v|room::show(ui,v,&mut linked,color));
        }
        section(ui,"eq","Equalizer",&mut value["postRoomEq"],|ui,v|{
            let selected=graphs::equalizer(ui,v);
            let p=if selected==0 {&mut v["lowShelf"]}else{&mut v["filters"][selected-1]};
            knobs(ui,p,&[("freq","Freq.",scale(24.,19000.,if selected==0{25.}else{[117.7,692.8,4077.7][selected-1]},0.1,true," Hz")),
                ("gain","Effects_Gain",scale(-24.,24.,0.,0.1,false," dB")),
                ("q","Q",scale(0.1,18.,0.71,0.01,true,""))],(selected==0).then_some("q"));
        });
        section(ui,"compressor","Compressor",&mut value["compressor"],|ui,v|{
            graphs::compressor(ui);
            knobs(ui,v,&[("threshold","Thresh.",scale(-70.,12.,-10.,0.1,false," dB")),
                ("attack","Attack",scale(0.,0.5,0.,0.001,false," s")),
                ("ratio","Ratio",scale(1.,40.,4.,0.1,true,":1"))],None);
        });
        section(ui,"reverb","Reverb",&mut value["reverb"],|ui,v|{
            let original=v.get("type").and_then(Value::as_str).unwrap_or("clean").to_owned();let mut selected=original.clone();
            ComboBox::from_id_salt("type").width(ui.available_width()).selected_text(super::native_label(match selected.as_str(){"hall"=>"Reverb_Hall","room"=>"Reverb_Room",_=>"Reverb_Clean"})).show_ui(ui,|ui|{
                for (key,label) in [("clean","Reverb_Clean"),("hall","Reverb_Hall"),("room","Reverb_Room")]{ui.selectable_value(&mut selected,key.to_owned(),super::native_label(label));}
            });
            if selected!=original {
                v["type"]=json!(selected);v["decay"]=json!(if selected=="hall"{2.5}else{1.0});
                v["preDelay"]=json!(if selected=="room"{0.08_f32}else{0.05_f32});
            }
            knobs(ui,v,&[("preDelay","Pre-Delay",scale(0.,0.2,0.05,0.001,false," s")),
                ("decay","Decay",scale(0.1,10.,1.,0.01,true," s")),
                ("dryWetRatio","Dry/Wet",scale(0.,1.,0.5,0.01,false,""))],None);
        });
    });
    if (value!=initial||was_linked!=linked) && !app.editor.is_editing(){
        app.editor.begin(crate::i18n::t("Effects"));
        room::apply_link(&mut app.editor.project,app.editor.track,&initial["room"],&value["room"],was_linked,linked);
        let track=app.editor.project.tracks.iter_mut().find(|t|t.id==app.editor.track).unwrap();
        track.mixer.insert("fxParams".into(),value);
        app.editor.finish();
    }
}
