use super::widgets::Widget;
use eframe::egui::*;
use serde::Deserialize;
use serde_json::{Value,json};
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct Row{#[serde(rename="type")]kind:String,#[serde(default)]text:String,#[serde(default)]columns:Vec<Widget>}
#[derive(Deserialize)]
pub(super) struct Panel{#[serde(default)]title:String,#[serde(default)]rows:Vec<Row>}
impl Panel{
    pub fn parse(value:Value)->Result<Self,String>{
        let panel:Self=serde_json::from_value(value).map_err(|e|e.to_string())?;
        for row in &panel.rows{
            if !["Label","Container"].contains(&row.kind.as_str()){return Err(format!("Unknown script panel row: {}",row.kind));}
            for widget in &row.columns{widget.validate()?;}
        }Ok(panel)
    }
    pub fn show(&mut self,ui:&mut Ui,name:&str,numeric_slider:bool)->Vec<Value>{
        let mut changes=Vec::new();let title=if self.title.is_empty(){name}else{&self.title};
        crate::ui::panels::controls::section(ui,title,|ui|{
            ui.add_space(8.);
            for(index,row)in self.rows.iter_mut().enumerate(){
                ui.push_id(index,|ui|{
                    if row.kind=="Label"{ui.add(Label::new(super::widgets::display_text(&row.text).as_ref()).wrap());return;}
                    let sum=row.columns.iter().map(|w|w.width).sum::<f64>();let gaps=8.*row.columns.len().saturating_sub(1)as f32;
                    let available=(ui.available_width()-gaps).max(1.);
                    ui.horizontal_top(|ui|{
                        ui.spacing_mut().item_spacing.x=8.;
                        for(column,widget)in row.columns.iter_mut().enumerate(){
                            let width=(f64::from(available)*widget.width/sum)as f32;
                            let id=widget.value.as_ref().map_or(column as u64,|v|v.id);
                            let mut value=widget.value.as_ref().map_or_else(||widget.initial(),|v|v.value.clone());
                            let enabled=widget.value.as_ref().is_none_or(|v|v.enabled);let mut focus=false;
                            ui.push_id(id,|ui|ui.allocate_ui_with_layout(vec2(width,0.),Layout::top_down(Align::Min),|ui|{
                                ui.set_width(width);ui.add_enabled_ui(enabled,|ui|{
                                    if widget.show(ui,&mut value,&mut focus,numeric_slider)&&let Some(binding)=&mut widget.value{
                                        binding.value=value.clone();changes.push(json!({"type":"widget","id":id,"value":value}));
                                    }
                                });
                            }));
                        }
                    });
                });
                ui.add_space(8.);
            }
        });changes
    }
}
