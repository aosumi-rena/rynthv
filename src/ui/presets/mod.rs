mod store;
mod target;
mod dialog;
pub use target::Kind;
pub use dialog::show as dialog;
use crate::{app::App,ui::{icons::{Icon,IconButton},theme::*}};
use eframe::egui::*;
use std::path::PathBuf;
use store::{Store,Preset};
use target::Target;

#[derive(Default)]
pub struct State { library:Option<Result<(PathBuf,Store),String>>, naming:Option<Naming>, load_error_shown:bool }
impl State {pub fn is_naming(&self)->bool{self.naming.is_some()}}
struct Naming {target:Target,name:String,focus:bool}
fn library(state:&mut State)->Result<&mut (PathBuf,Store),String> {
    if state.library.is_none(){state.library=Some(rynthv::platform::config_directory().and_then(|dir|{
        let path=dir.join("instrument-effects-presets.json");Store::load(&path).map(|s|(path,s))
    }));}
    state.library.as_mut().unwrap().as_mut().map_err(|e|e.clone())
}
fn save(app:&mut App,state:&mut State,target:&Target,name:&str)->Result<(),String> {
    if name.trim().is_empty()||name.chars().any(char::is_control){return Err(crate::i18n::t("请输入预设名称，不能含控制字符").into());}
    if !target.valid(app){return Err(crate::i18n::t("选中的音符组已经改变").into());}
    let (path,store)=library(state)?;let mut next=store.clone();
    next.scopes.entry(target.scope.clone()).or_default().insert(name.into(),Preset {signature:target.signature.clone(),value:target.value.clone()});
    next.save(path)?;*store=next;target.apply(app,target.value.clone(),name)
}
pub fn show(app:&mut App,ui:&mut Ui,kind:Kind)->bool {
    let target=match Target::read(app,kind){Ok(v)=>v,Err(_)=>return false};
    let mut state=std::mem::take(&mut app.control_presets);let mut applied=false;
    let result=(||->Result<(),String>{
        let (_,store)=library(&mut state)?;let presets=store.scopes.get(&target.scope);
        let selected=presets.and_then(|p|p.get(&target.name)).filter(|p|p.signature==target.signature);
        let name=if selected.is_some(){target.name.as_str()}else{""};
        let modified=&target.value!=selected.map_or(&target.default,|p|&p.value);
        let text=format!("{}{}",if name.is_empty(){crate::i18n::t("Default")}else{name},if modified{"*"}else{""});
        let mut action=None;
        ui.scope(|ui|{
            ui.spacing_mut().interact_size.y=28.;ui.visuals_mut().widgets.inactive.bg_fill=BORDER;
            ui.visuals_mut().widgets.inactive.corner_radius=CornerRadius::same(4);
            ComboBox::from_id_salt(("control-preset",kind)).width(ui.available_width()).selected_text(text)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside).show_ui(ui,|ui|{
                ui.set_min_width(ui.available_width().max(180.));
                for (id,label) in [(0,"保存"),(1,"保存为新的预设")] {
                    if ui.add_enabled(!app.editor.is_editing()&&(id!=0||!name.is_empty()),Button::new(crate::i18n::t(label)).frame(false).min_size(vec2(ui.available_width(),28.))).clicked(){action=Some((id,name.to_owned()));ui.close();}
                }
                ui.separator();
                if ui.add_enabled(!app.editor.is_editing(),Button::selectable(name.is_empty(),crate::i18n::t("Default")).min_size(vec2(ui.available_width(),28.))).clicked(){action=Some((2,String::new()));ui.close();}
                for (label,preset) in presets.into_iter().flatten(){
                    ui.horizontal(|ui|{
                        let width=(ui.available_width()-24.).max(20.);
                        if ui.add_enabled(!app.editor.is_editing()&&preset.signature==target.signature,Button::selectable(name==label,label).min_size(vec2(width,28.))).clicked(){action=Some((2,label.clone()));ui.close();}
                        if ui.add(IconButton::new(Icon::Close,crate::i18n::t("删除")).size(vec2(20.,24.)).icon_size(12.)).clicked(){action=Some((3,label.clone()));}
                    });
                }
            });
        });
        match action {
            Some((0,label))=>save(app,&mut state,&target,&label)?,
            Some((1,label))=>state.naming=Some(Naming {target:target.clone(),name:label,focus:true}),
            Some((2,label))=>{
                let value=library(&mut state)?.1.scopes.get(&target.scope).and_then(|p|p.get(&label)).map_or_else(||target.default.clone(),|p|p.value.clone());
                target.apply(app,value,&label)?;applied=true;
            }
            Some((3,label))=>{let(path,store)=library(&mut state)?;let mut next=store.clone();if let Some(p)=next.scopes.get_mut(&target.scope){p.remove(&label);}next.save(path)?;*store=next;}
            _=>(),
        }
        Ok(())
    })();
    if let Err(e)=result{if !state.load_error_shown||!state.library.as_ref().is_some_and(Result::is_err){app.error=Some(e);}state.load_error_shown=true;}
    app.control_presets=state;applied
}
