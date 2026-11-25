use super::widgets::Widget;
use eframe::egui::*;
use rynthv::scripting::worker::Request;
use serde::Deserialize;
use serde_json::{Value,json};
use std::sync::mpsc::Sender;
use crate::ui::theme;
#[derive(Deserialize)]
struct Form{title:String,#[serde(default)]message:String,buttons:String,#[serde(default)]widgets:Vec<Widget>}
pub(super) struct Pending{
    reply:Option<Sender<Result<Value,String>>>,pub instance:u64,pub callback:Option<u64>,
    form:Form,answers:serde_json::Map<String,Value>,focus:bool,
}
impl Pending{
    pub fn new(request:Request,instance:u64)->Result<Self,String>{
        let form:Form=serde_json::from_value(request.args.first().cloned().ok_or("Missing dialog form")?).map_err(|e|e.to_string())?;
        if !["Ok","OkCancel","YesNoCancel"].contains(&form.buttons.as_str()){return Err("Invalid script dialog buttons".into());}
        let mut answers=serde_json::Map::new();
        for widget in &form.widgets{
            widget.validate()?;
            if widget.kind!="Label"&&(widget.name.is_empty()||answers.contains_key(&widget.name)){return Err("Duplicate or empty script widget name".into());}
            if widget.kind!="Label"{answers.insert(widget.name.clone(),widget.initial());}
        }
        let callback=if request.method=="dialog.async"{Some(request.args.get(1).and_then(Value::as_u64).ok_or("Invalid dialog callback")?)}else{None};
        let reply=if callback.is_some(){let _=request.reply.send(Ok(Value::Null));None}else{Some(request.reply)};
        Ok(Self{reply,instance,callback,form,answers,focus:true})
    }
    pub fn complete(mut self,result:Value)->Option<(u64,u64,Value)>{
        if let Some(reply)=self.reply.take(){let _=reply.send(Ok(result));None}else{self.callback.map(|id|(self.instance,id,result))}
    }
    pub fn show(&mut self,ctx:&Context,numeric_slider:bool)->Option<Value>{
        let mut choice=None;
        let response=Modal::new(Id::new("script-dialog"))
            .frame(Frame::new().fill(Color32::from_rgb(61,59,64)).corner_radius(4).stroke(Stroke::new(0.5,theme::MUTED)).inner_margin(24))
            .show(ctx,|ui|{
                ui.set_width(312.);ui.spacing_mut().item_spacing.y=12.;
                ui.label(RichText::new(&self.form.title).font(FontId::new(14.,FontFamily::Name("SectionTitle".into()))));
                if !self.form.message.is_empty(){ui.add(Label::new(super::widgets::display_text(&self.form.message).as_ref()).wrap());}
                ScrollArea::vertical().max_height((ctx.content_rect().height()-200.).max(80.)).show(ui,|ui|{
                    for widget in &self.form.widgets{
                        ui.push_id(&widget.name,|ui|{
                            if widget.kind=="Label"{ui.add(Label::new(super::widgets::display_text(&widget.text).as_ref()).wrap());}
                            else{widget.show(ui,self.answers.get_mut(&widget.name).unwrap(),&mut self.focus,numeric_slider);}
                        });
                    }
                });
                ui.add_space(8.);
                ui.with_layout(Layout::right_to_left(Align::Center),|ui|{
                    let yes_no=self.form.buttons=="YesNoCancel";
                    let count=if yes_no{3.}else if self.form.buttons=="Ok"{1.}else{2.};
                    let width=(ui.available_width()-8.*(count-1.))/count;
                    ui.spacing_mut().item_spacing.x=8.;
                    let label=crate::i18n::t(if yes_no{"Yes"}else{"确定"});
                    let confirm=Button::new(RichText::new(label).color(theme::CANVAS));
                    if crate::ui::prompt::action_button(ui,confirm,vec2(width,32.),true,theme::accent(ui)).clicked(){choice=Some(if yes_no{"Yes"}else{"Ok"});}
                    if yes_no&&ui.add_sized(vec2(width,32.),Button::new(crate::i18n::t("No"))).clicked(){choice=Some("No");}
                    if self.form.buttons!="Ok"&&ui.add_sized(vec2(width,32.),Button::new(crate::i18n::t("取消"))).clicked(){choice=Some("Cancel");}
                });
            });
        if response.is_top_modal&&!response.any_popup_open&&choice.is_none(){
            if ctx.input_mut(|i|i.consume_key(Modifiers::NONE,Key::Escape)){choice=Some("Cancel");}
            else if ctx.input(|i|i.key_pressed(Key::Enter))&&!ctx.egui_wants_keyboard_input(){choice=Some(if self.form.buttons=="YesNoCancel"{"Yes"}else{"Ok"});}
        }
        choice.map(|status|json!({"status":status,"answers":self.answers}))
    }
}
impl Drop for Pending{
    fn drop(&mut self){if let Some(reply)=self.reply.take(){let _=reply.send(Err("Script dialog closed".into()));}}
}
