mod actions;
mod dialog;
use crate::app::App;
use eframe::egui::*;
use rynthv::scripting::worker::{self,Entry,Event,Job};
use serde_json::{Value,json};
use std::{collections::VecDeque,sync::mpsc};

pub(super) struct Running {
    job:Job,name:String,project:String,revision:u64,baseline:Value,
}
#[derive(Default)]
pub struct State {
    entries:Vec<Entry>,scan:Option<mpsc::Receiver<Result<Vec<Entry>,String>>>,
    language:Option<crate::i18n::Language>,running:Option<Running>,
    dialog:Option<dialog::Pending>,log:VecDeque<String>,
}
impl State {
    pub fn has_dialog(&self)->bool {self.dialog.is_some()}
    pub fn stop(&mut self){self.dialog=None;self.running=None;}
    fn scan(&mut self,ctx:&Context) {
        self.language=Some(crate::i18n::language());
        let ctx=ctx.clone();self.scan=Some(worker::catalogue(crate::i18n::language().key().into(),move||ctx.request_repaint()));
    }
}
pub fn poll(app:&mut App,ctx:&Context) {
    let mut state=std::mem::take(&mut app.scripts);
    if let Some(result)=state.scan.as_ref().and_then(|rx|rx.try_recv().ok()) {
        state.scan=None;match result {Ok(entries)=>state.entries=entries,Err(error)=>app.error=Some(error)}
    }
    if state.running.as_ref().is_some_and(|r|r.project!=app.editor.project.uuid) {state.stop();}
    for _ in 0..64 {
        let event=state.running.as_ref().and_then(|r|r.job.events.try_recv().ok());
        match event {
            Some(Event::Request(request))=>{
                if request.method=="dialog" {
                    let reply=request.reply.clone();
                    match dialog::Pending::new(request) {Ok(pending)=>state.dialog=Some(pending),Err(error)=>{let _=reply.send(Err(error));}}
                }else{
                    let result=actions::request(app,&mut state,&request.method,&request.args);
                    let _=request.reply.send(result);
                }
            },
            Some(Event::Finished(result))=>{
                let name=state.running.take().map_or(String::new(),|r|r.name);state.dialog=None;
                if let Err(error)=result {app.error=Some(format!("{name}\n{error}"));}
            },None=>break,
        }
    }
    app.scripts=state;
    if app.scripts.running.is_some(){ctx.request_repaint_after(std::time::Duration::from_millis(50));}
}
fn start(app:&mut App,entry:&Entry,ctx:&Context) {
    app.commit_lyric();if app.editor.is_editing(){return;}
    if let Some(error)=&entry.error {app.error=Some(error.clone());return;}
    let result=(||->Result<Running,String>{
        let snapshot=rynthv::scripting::snapshot::encode(&app.editor.project)?;
        let context=json!({"project":snapshot,"language":crate::i18n::language().key(),
            "os":if cfg!(windows){"Windows"}else if cfg!(target_os="macos"){"macOS"}else{"Linux"},
            "fileName":app.path.as_ref().map(|p|p.to_string_lossy()),"track":app.editor.track,
            "reference":app.editor.reference,"selected":app.editor.selected});
        let ctx=ctx.clone();let job=worker::start(entry.path.clone(),context,move||ctx.request_repaint())?;
        Ok(Running{job,name:entry.name.clone(),project:app.editor.project.uuid.clone(),revision:app.editor.revision,baseline:snapshot})
    })();
    match result{Ok(running)=>{app.scripts.stop();app.scripts.running=Some(running);},Err(error)=>app.error=Some(error)}
}
pub fn menu(app:&mut App,ui:&mut Ui) {
    if app.scripts.language!=Some(crate::i18n::language()) {app.scripts.scan(ui.ctx());}
    if ui.button(crate::i18n::t("Rescan")).clicked(){app.scripts.scan(ui.ctx());ui.close();}
    if ui.button(crate::i18n::t("Open Scripts Folder")).clicked(){if let Err(e)=actions::open_folder(){app.error=Some(e);}ui.close();}
    if ui.add_enabled(app.scripts.running.is_some(),Button::new(crate::i18n::t("Abort All Running Scripts"))).clicked(){app.scripts.stop();ui.close();}
    ui.separator();ui.weak(crate::i18n::t("Launch Scripts"));
    let mut selected=None;
    for entry in &app.scripts.entries {
        if ui.add_enabled(app.scripts.running.is_none(),Button::new(&entry.name)).on_hover_text(entry.error.as_deref().unwrap_or_else(||entry.path.to_str().unwrap_or(""))).clicked(){selected=Some(entry.clone());}
    }
    if let Some(entry)=selected {start(app,&entry,ui.ctx());ui.close();}
}
pub fn panel(app:&mut App,ui:&mut Ui) {
    super::panels::controls::padded(ui,|ui|{
        ui.label(RichText::new(crate::i18n::t("User Scripts")).font(super::theme::section_title_font()));
        if app.scripts.language!=Some(crate::i18n::language()) {app.scripts.scan(ui.ctx());}
        if ui.button(crate::i18n::t("Rescan Scripts")).clicked(){app.scripts.scan(ui.ctx());}
        if ui.button(crate::i18n::t("Open Scripts Folder")).clicked(){if let Err(error)=actions::open_folder(){app.error=Some(error);}}
        let mut selected=None;
        for entry in &app.scripts.entries {
            if ui.add_enabled(app.scripts.running.is_none(),Button::new(&entry.name)).on_hover_text(entry.error.as_deref().unwrap_or_else(||entry.path.to_str().unwrap_or(""))).clicked(){selected=Some(entry.clone());}
        }
        if let Some(entry)=selected{start(app,&entry,ui.ctx());}
        if app.scripts.running.is_some()&&ui.button(crate::i18n::t("Abort All Running Scripts")).clicked(){app.scripts.stop();}
        if !app.scripts.log.is_empty(){ui.separator();for line in &app.scripts.log {ui.label(line);}}
    });
}
pub fn dialog(app:&mut App,ctx:&Context) {
    if let Some(pending)=app.scripts.dialog.as_mut()&&pending.show(ctx){app.scripts.dialog=None;}
}
