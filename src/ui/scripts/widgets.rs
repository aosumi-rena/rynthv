use eframe::egui::*;
use serde::Deserialize;
use serde_json::{Value,json};
pub(super) fn display_text(text:&str)->std::borrow::Cow<'_,str>{
    if text.contains('\r'){std::borrow::Cow::Owned(text.replace("\r\n","\n").replace('\r',"\n"))}
    else{std::borrow::Cow::Borrowed(text)}
}
fn one()->f64{1.}
fn enabled()->bool{true}
#[derive(Clone,Deserialize)]
pub(super) struct Binding{pub id:u64,#[serde(default)]pub value:Value,#[serde(default="enabled")]pub enabled:bool}
#[derive(Clone,Deserialize)]
#[serde(rename_all="camelCase")]
pub(super) struct Widget{
    #[serde(default)]pub name:String,#[serde(rename="type")]pub kind:String,
    #[serde(default)]pub label:String,#[serde(default)]pub text:String,
    #[serde(default)]pub default:Value,#[serde(default)]pub value:Option<Binding>,
    #[serde(default)]pub min_value:f64,#[serde(default="one")]pub max_value:f64,
    #[serde(default)]pub interval:f64,#[serde(default)]pub choices:Vec<String>,
    #[serde(default)]pub format:String,#[serde(default)]pub height:Option<f32>,
    #[serde(default="one")]pub width:f64,
}
impl Widget{
    pub fn validate(&self)->Result<(),String>{
        if !["TextBox","TextArea","Slider","CheckBox","ComboBox","Button","Label"].contains(&self.kind.as_str()){return Err(format!("Unknown script widget: {}",self.kind));}
        if self.kind=="Slider"&&(!self.min_value.is_finite()||!self.max_value.is_finite()||self.min_value>=self.max_value||!self.interval.is_finite()||self.interval<0.){return Err("Invalid script slider range".into());}
        if self.height.is_some_and(|h|!h.is_finite()||h<0.)||!self.width.is_finite()||self.width<=0.{return Err("Invalid script widget dimensions".into());}Ok(())
    }
    pub fn initial(&self)->Value{
        match self.kind.as_str(){
            "TextBox"|"TextArea"=>json!(self.default.as_str().unwrap_or("")),
            "CheckBox"=>json!(self.default.as_bool().unwrap_or(false)),
            "ComboBox"=>json!(self.default.as_i64().unwrap_or(0).clamp(-1,self.choices.len().saturating_sub(1)as i64)),
            "Slider"=>json!(self.default.as_f64().unwrap_or(self.min_value).clamp(self.min_value,self.max_value)),
            _=>Value::Null,
        }
    }
    pub fn show(&self,ui:&mut Ui,value:&mut Value,focus:&mut bool,numeric_slider:bool)->bool{
        let mut changed=false;
        match self.kind.as_str(){
            "Label"=>{ui.add(Label::new(display_text(&self.text).as_ref()).wrap());},
            "TextBox"|"TextArea"=>{
                let mut text=value.as_str().unwrap_or("").to_owned();
                if !self.label.is_empty(){ui.label(display_text(&self.label).as_ref());}
                let response=if self.kind=="TextArea"{
                    text=display_text(&text).into_owned();
                    let height=self.height.unwrap_or(64.);
                    if height==0.{ui.allocate_exact_size(vec2(ui.available_width(),0.),Sense::hover()).1}
                    else{ui.add_sized(vec2(ui.available_width(),height),TextEdit::multiline(&mut text).desired_rows(3)
                        .margin(vec2(7.,5.)).background_color(crate::ui::theme::BORDER))}
                }else{crate::ui::panels::controls::field(ui,&mut text)};
                if *focus&&response.rect.height()>0.&&ui.is_enabled(){response.request_focus();*focus=false;}
                changed=response.changed();if changed{*value=json!(text);}
            },
            "CheckBox"=>{let mut checked=value.as_bool().unwrap_or(false);changed=ui.checkbox(&mut checked,display_text(&self.text).as_ref()).changed();if changed{*value=json!(checked);}},
            "ComboBox"=>{
                if !self.label.is_empty(){ui.label(display_text(&self.label).as_ref());}
                let mut index=value.as_i64().unwrap_or(0);
                let selected=self.choices.get(usize::try_from(index).unwrap_or(usize::MAX)).map(String::as_str).unwrap_or("");
                ComboBox::from_id_salt("choice").width(ui.available_width()).selected_text(selected).show_ui(ui,|ui|{
                    for(i,label)in self.choices.iter().enumerate(){changed|=ui.selectable_value(&mut index,i as i64,label).changed();}
                });if changed{*value=json!(index);}
            },
            "Slider"=>{
                let mut number=value.as_f64().unwrap_or(self.min_value).clamp(self.min_value,self.max_value);
                let label=if self.text.is_empty(){&self.label}else{&self.text};
                let format=if self.format.is_empty(){"%.2f"}else{&self.format};
                let appearance=if numeric_slider {crate::ui::slider::Appearance::Voice}
                    else {crate::ui::slider::Appearance::Instrument(crate::ui::theme::accent(ui))};
                if let Some(next)=crate::ui::slider::Slider::new(ui.id().with("script-slider"),label,number,
                    self.min_value,self.max_value,self.default.as_f64().unwrap_or(self.min_value))
                    .step(self.interval).format(&|n|format_value(format,n)).numeric(numeric_slider)
                    .appearance(appearance).live().show(ui).change {
                    changed=next.value!=number;number=next.value;
                }
                if changed{*value=json!(number);}
            },
            "Button"=>{if ui.add_sized(vec2(ui.available_width(),28.),Button::new(&self.text)).clicked(){*value=json!(true);changed=true;}},
            _=>(),
        }
        changed
    }
}
pub(super) fn format_value(format:&str,value:f64)->String{
    super::format::number(format,value)
}
