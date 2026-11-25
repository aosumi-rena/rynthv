mod actions;
mod computed;
mod context;
mod dialog;
mod format;
mod navigation;
mod panel;
mod rebase;
mod widgets;
pub use navigation::set_viewport;
use crate::app::App;
use eframe::egui::*;
use rynthv::scripting::worker::{self,Entry,Event,Job};
use serde_json::{Value,json};
use std::{collections::VecDeque,path::PathBuf};
pub(super) struct Running{
    job:Job,entry:Entry,project:String,revision:u64,baseline:Value,undo_revision:Option<u64>,
    panel:Option<panel::Panel>,last_context:String,metadata:bool,instance:u64,pending:Option<worker::Request>,
}
#[derive(Default)]
pub struct State{
    entries:Vec<Entry>,scan:Option<VecDeque<PathBuf>>,
    language:Option<crate::i18n::Language>,running:Vec<Running>,
    dialogs:VecDeque<dialog::Pending>,log:VecDeque<String>,project:Option<String>,load_panels:bool,
}
impl State{
    pub fn has_dialog(&self)->bool{!self.dialogs.is_empty()}
    pub fn stop(&mut self){self.abort();self.scan=None;self.language=None;self.project=None;}
    fn abort(&mut self){self.dialogs.clear();self.running.clear();self.load_panels=false;}
    fn scan(&mut self)->Result<(),String>{
        let paths=rynthv::scripting::scan()?;self.abort();self.entries.clear();
        self.language=Some(crate::i18n::language());self.scan=Some(paths.into());Ok(())
    }
}
fn start(app:&mut App,state:&mut State,entry:&Entry,ctx:&Context){start_job(app,state,entry,ctx,false);}
fn start_job(app:&mut App,state:&mut State,entry:&Entry,ctx:&Context,metadata:bool){
    if (entry.panel||metadata)&&state.running.iter().any(|r|r.entry.path==entry.path){return;}
    app.commit_lyric();if app.editor.is_editing(){return;}
    if let Some(error)=&entry.error{app.error=Some(format!("{}\n{}",entry.name,error));return;}
    let result=(||->Result<Running,String>{
        let context=context::snapshot(app)?;let baseline=context["project"].clone();let signature=context::selection(app).to_string();
        let ctx=ctx.clone();let wake=move||ctx.request_repaint();
        let job=if metadata{worker::metadata(entry.path.clone(),context,wake)?}else{worker::start(entry.path.clone(),context,wake)?};
        Ok(Running{job,entry:entry.clone(),project:app.editor.project.uuid.clone(),revision:app.editor.revision,baseline,undo_revision:None,panel:None,last_context:signature,metadata,instance:crate::model::new_id(),pending:None})
    })();
    match result{Ok(r)=>state.running.push(r),Err(e)=>app.error=Some(e)}
}
pub fn poll(app:&mut App,ctx:&Context){
    let mut state=std::mem::take(&mut app.scripts);
    if state.language!=Some(crate::i18n::language())&&state.scan.is_none(){if let Err(e)=state.scan(){app.error=Some(e);}}
    if state.project.as_deref()!=Some(&app.editor.project.uuid){state.abort();state.project=Some(app.editor.project.uuid.clone());state.load_panels=true;}
    if state.load_panels&&state.scan.is_none()&&!app.editor.is_editing(){
        state.load_panels=false;
        for entry in state.entries.clone().into_iter().filter(|e|e.panel&&e.error.is_none()){start(app,&mut state,&entry,ctx);}
    }
    let mut jobs=std::mem::take(&mut state.running);
    for mut running in jobs.drain(..){
        if running.project!=app.editor.project.uuid{continue;}
        let mut finished=false;
        if let Some(request)=running.pending.take(){
            if editing(app){running.pending=Some(request);}
            else{let result=actions::request(app,&mut state,&mut running,ctx,&request.method,&request.args);let _=request.reply.send(result);}
        }
        for _ in 0..64{
            if running.pending.is_some(){break;}
            match running.job.events.try_recv(){
                Ok(Event::Request(request))=>{
                    if request.method=="commit"&&editing(app){running.pending=Some(request);break;}
                    if request.method=="dialog"||request.method=="dialog.async"{
                        let reply=request.reply.clone();
                        match dialog::Pending::new(request,running.instance){Ok(p)=>state.dialogs.push_back(p),Err(e)=>{let _=reply.send(Err(e));}}
                    }else{
                        let result=actions::request(app,&mut state,&mut running,ctx,&request.method,&request.args);let _=request.reply.send(result);
                    }
                },
                Ok(Event::Info(info))=>{
                    running.entry.name=info["name"].as_str().unwrap_or(&running.entry.name).to_owned();
                    running.entry.category=info["category"].as_str().unwrap_or("").to_owned();running.entry.panel=info["type"]=="SidePanelSection";
                },
                Ok(Event::Finished(result))=>{
                    if running.metadata{running.entry.error=result.err();if !running.entry.error.as_deref().is_some_and(|e|e=="Script has no name"||e.contains("Required Lua entry point is missing")||e.contains("getClientInfo is not defined")){state.entries.push(running.entry.clone());}}
                    else if let Err(e)=result{app.error=Some(format!("{}\n{}",running.entry.name,e));}finished=true;break;
                },
                Err(_)=>break,
            }
        }
        if finished{state.dialogs.retain(|d|d.instance!=running.instance);continue;}
        let mut context=context::selection(app);let signature=context.to_string();
        if signature!=running.last_context{
            if running.revision!=app.editor.revision&&!app.editor.is_editing(){
                match rynthv::scripting::snapshot::encode(&app.editor.project){
                    Ok(snapshot)=>{context["project"]=snapshot.clone();running.baseline=snapshot;running.revision=app.editor.revision;},
                    Err(e)=>{app.error=Some(e);continue;},
                }
            }
            let _=running.job.commands.send(json!({"type":"context","context":context}));running.last_context=signature;
        }
        state.running.push(running);
    }
    if state.scan.is_some()&&!state.running.iter().any(|r|r.metadata)&&!app.editor.is_editing(){
        if let Some(path)=state.scan.as_mut().and_then(VecDeque::pop_front){
            let name=path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            start_job(app,&mut state,&Entry{path,name,category:String::new(),panel:false,error:None},ctx,true);
        }else{state.scan=None;state.entries.sort_by_cached_key(|e|(e.category.to_lowercase(),e.name.to_lowercase()));state.load_panels=true;}
    }
    if !state.running.is_empty()||state.scan.is_some(){ctx.request_repaint_after(std::time::Duration::from_millis(50));}
    app.scripts=state;
}
fn editing(app:&App)->bool{app.editor.is_editing()||app.gesture.is_some()||app.group_gesture.is_some()}
fn entry_button(ui:&mut Ui,state:&State,entry:&Entry)->bool{
    let running=entry.panel&&state.running.iter().any(|r|r.entry.path==entry.path);
    ui.add_enabled(!running,Button::new(&entry.name)).on_hover_text(entry.error.as_deref().unwrap_or_else(||entry.path.to_str().unwrap_or(""))).clicked()
}
pub fn menu(app:&mut App,ui:&mut Ui){
    let mut state=std::mem::take(&mut app.scripts);
    if state.language!=Some(crate::i18n::language())&&state.scan.is_none(){if let Err(e)=state.scan(){app.error=Some(e);}}
    if ui.button(crate::i18n::t("Rescan")).clicked(){if let Err(e)=state.scan(){app.error=Some(e);}ui.close();}
    if ui.button(crate::i18n::t("Open Scripts Folder")).clicked(){if let Err(e)=actions::open_folder(){app.error=Some(e);}ui.close();}
    if ui.add_enabled(!state.running.is_empty(),Button::new(crate::i18n::t("Abort All Running Scripts"))).clicked(){state.abort();state.scan=None;ui.close();}
    ui.separator();ui.weak(crate::i18n::t("Launch Scripts"));
    let mut selected=None;let mut categories=std::collections::BTreeSet::new();
    for entry in state.entries.iter().filter(|e|!e.panel){
        if entry.category.is_empty(){if entry_button(ui,&state,entry){selected=Some(entry.clone());}}
        else{categories.insert(entry.category.clone());}
    }
    for category in categories{ui.menu_button(&category,|ui|{
        for entry in state.entries.iter().filter(|e|!e.panel&&e.category==category){if entry_button(ui,&state,entry){selected=Some(entry.clone());}}
    });}
    if let Some(entry)=selected{start(app,&mut state,&entry,ui.ctx());ui.close();}app.scripts=state;
}
pub fn panel(app:&mut App,ui:&mut Ui){
    let mut state=std::mem::take(&mut app.scripts);
    let mut displayed=false;
    for running in &mut state.running{
        if let Some(panel)=&mut running.panel{
            displayed=true;
            ui.push_id(&running.entry.path,|ui|{
                for event in panel.show(ui,&running.entry.name,!app.editor.current_track().is_instrument()){let _=running.job.commands.send(event);}
            });
        }
    }
    if !displayed{
        crate::ui::panels::controls::padded(ui,|ui|{
            ui.add(Label::new(crate::i18n::t("Create scripts, or download community scripts and place them in the scripts folder to extend the editor.")).wrap());
            ui.add_space(8.);
            ui.weak(crate::i18n::t("Custom UI widgets created by scripts are displayed in this panel."));
            ui.add_space(12.);
            if ui.add_sized(vec2(ui.available_width(),32.),Button::new(crate::i18n::t("Open Scripts Folder"))).clicked(){if let Err(e)=actions::open_folder(){app.error=Some(e);}}
            ui.add_space(4.);
            if ui.add_sized(vec2(ui.available_width(),32.),Button::new(crate::i18n::t("Rescan Scripts"))).clicked(){if let Err(e)=state.scan(){app.error=Some(e);}}
        });
    }
    app.scripts=state;
}
pub fn dialog(app:&mut App,ctx:&Context){
    let numeric_slider=!app.editor.current_track().is_instrument();
    let result=app.scripts.dialogs.front_mut().and_then(|d|d.show(ctx,numeric_slider));
    if let Some(result)=result&&let Some(pending)=app.scripts.dialogs.pop_front()&&let Some((instance,id,result))=pending.complete(result)
        &&let Some(running)=app.scripts.running.iter().find(|r|r.instance==instance){
        let _=running.job.commands.send(json!({"type":"dialog","id":id,"result":result}));
    }
}
