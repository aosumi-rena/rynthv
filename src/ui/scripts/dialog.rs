use eframe::egui::*;
use rynthv::scripting::worker::Request;
use serde::Deserialize;
use serde_json::{Value,json};
use crate::ui::theme;
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct Widget {
    name:String,#[serde(rename="type")]kind:String,#[serde(default)]label:String,
    #[serde(default)]text:String,#[serde(default)]default:Value,
    #[serde(default)]min_value:f64,#[serde(default)]max_value:f64,
    #[serde(default)]interval:f64,#[serde(default)]choices:Vec<String>,
    #[serde(default)]format:String,
}
#[derive(Deserialize)]
struct Form {title:String,#[serde(default)]message:String,buttons:String,widgets:Vec<Widget>}
pub(super) struct Pending {request:Request,form:Form,answers:serde_json::Map<String,Value>,focus:bool}
impl Pending {
    pub fn new(request:Request)->Result<Self,String> {
        let form:Form=serde_json::from_value(request.args.first().cloned().ok_or("Missing dialog form")?).map_err(|e|e.to_string())?;
        if !["Ok","OkCancel","YesNoCancel"].contains(&form.buttons.as_str())||form.widgets.len()>128{return Err("Invalid script dialog".into());}
        let mut answers=serde_json::Map::new();
        for widget in &form.widgets {
            if widget.name.is_empty()||answers.contains_key(&widget.name){return Err("Duplicate or empty script widget name".into());}
            let value=match widget.kind.as_str() {
                "TextBox"|"TextArea"=>json!(widget.default.as_str().unwrap_or("")),
                "CheckBox"=>json!(widget.default.as_bool().unwrap_or(false)),
                "Slider"=>{
                    if !widget.min_value.is_finite()||!widget.max_value.is_finite()||widget.min_value>=widget.max_value||!widget.interval.is_finite()||widget.interval<0.{return Err("Invalid script slider range".into());}
                    json!(widget.default.as_f64().unwrap_or(widget.min_value).clamp(widget.min_value,widget.max_value))
                },
                "ComboBox"=>{
                    if widget.choices.is_empty(){return Err("Empty script combo box".into());}
                    json!(widget.default.as_u64().unwrap_or(0).min((widget.choices.len()-1)as u64))
                },_=>return Err(format!("Unsupported script widget: {}",widget.kind)),
            };answers.insert(widget.name.clone(),value);
        }
        Ok(Self{request,form,answers,focus:true})
    }
    pub fn show(&mut self,ctx:&Context)->bool {
        let mut choice=None;
        let response=Modal::new(Id::new("script-dialog"))
            .frame(Frame::new().fill(Color32::from_rgb(61,59,64)).corner_radius(4).inner_margin(24))
            .show(ctx,|ui|{
                ui.set_width(380.);ui.spacing_mut().item_spacing.y=12.;
                ui.label(RichText::new(&self.form.title).font(theme::section_title_font()));
                if !self.form.message.is_empty(){ui.add(Label::new(&self.form.message).wrap());}
                ScrollArea::vertical().max_height((ctx.content_rect().height()-200.).max(120.)).show(ui,|ui|{
                    for widget in &self.form.widgets {
                        ui.push_id(&widget.name,|ui|{
                            if widget.kind!="Slider"&&!widget.label.is_empty(){ui.label(&widget.label);}
                            let value=self.answers.get_mut(&widget.name).unwrap();
                            match widget.kind.as_str(){
                                "TextBox"|"TextArea"=>{
                                    let mut text=value.as_str().unwrap_or("").to_owned();
                                    let input=if widget.kind=="TextArea"{TextEdit::multiline(&mut text).desired_rows(4)}else{TextEdit::singleline(&mut text)};
                                    let response=ui.add_sized(vec2(ui.available_width(),if widget.kind=="TextArea"{84.}else{28.}),input);
                                    if self.focus{response.request_focus();self.focus=false;}*value=json!(text);
                                },
                                "CheckBox"=>{let mut checked=value.as_bool().unwrap();ui.checkbox(&mut checked,&widget.text);*value=json!(checked);},
                                "Slider"=>{let number=value.as_f64().unwrap();
                                    if let Some(updated)=crate::ui::voice_library::parameter_slider(ui,ui.make_persistent_id("value"),&widget.label,number,widget.min_value,widget.max_value,widget.interval.max(0.000001)){*value=json!(updated);}
                                    if !widget.format.is_empty(){ui.response().on_hover_text(format_value(&widget.format,value.as_f64().unwrap()));}},
                                "ComboBox"=>{let mut index=value.as_u64().unwrap()as usize;
                                    ComboBox::from_id_salt("choice").width(ui.available_width()).selected_text(&widget.choices[index]).show_ui(ui,|ui|{
                                        for (i,label) in widget.choices.iter().enumerate(){ui.selectable_value(&mut index,i,label);}
                                    });*value=json!(index);},_=>unreachable!(),
                            }
                        });
                    }
                });
                ui.add_space(8.);
                ui.with_layout(Layout::right_to_left(Align::Center),|ui|{
                    let yes_no=self.form.buttons=="YesNoCancel";
                    let label=crate::i18n::t(if yes_no{"是"}else{"确定"});
                    let width=(ui.available_width()-8.)/if yes_no{3.}else if self.form.buttons=="Ok"{1.}else{2.};
                    if ui.add_sized(vec2(width,32.),Button::new(RichText::new(label).color(Color32::from_rgb(30,30,30))).fill(Color32::from_rgb(116,204,20))).clicked(){choice=Some(if yes_no{json!("yes")}else{json!(true)});}
                    if yes_no&&ui.add_sized(vec2(width,32.),Button::new(crate::i18n::t("否"))).clicked(){choice=Some(json!("no"));}
                    if self.form.buttons!="Ok"&&ui.add_sized(vec2(width,32.),Button::new(crate::i18n::t("取消"))).clicked(){choice=Some(if yes_no{json!("cancel")}else{json!(false)});}
                });
            });
        if (response.is_top_modal && !response.any_popup_open && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)))&&choice.is_none(){choice=Some(if self.form.buttons=="YesNoCancel"{json!("cancel")}else{json!(false)});}
        if let Some(status)=choice {let _=self.request.reply.send(Ok(json!({"status":status,"answers":self.answers})));true}else{false}
    }
}
fn format_value(format:&str,value:f64)->String {
    let Some(start)=format.find('%')else{return format.to_owned();};
    let Some(end)=format[start..].find(['f','g','d']).map(|i|start+i)else{return value.to_string();};
    let precision=format[start..end].split_once('.').and_then(|(_,s)|s.parse::<usize>().ok()).unwrap_or(2).min(8);
    format!("{}{value:.precision$}{}",&format[..start],&format[end+1..]).replace("%%","%")
}
