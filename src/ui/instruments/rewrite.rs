use crate::{app::App,model::InstrumentSelection,synthesis::instruments::rewrite::{self,Kind,Edit}};
use eframe::egui::Context;
use std::sync::{Arc,atomic::{AtomicBool,Ordering},mpsc::{self,Receiver}};

pub struct Job {
    track:u64,reference:u64,revision:u64,selection:InstrumentSelection,identity:String,key:u64,
    zero_gru_weights: bool,
    accept_any_v4_signature: bool,
    rx:Receiver<Result<Vec<Edit>,String>>,cancel:Arc<AtomicBool>,
}
impl Drop for Job {fn drop(&mut self){self.cancel.store(true,Ordering::Release);}}

pub fn enabled(app:&App)->bool {
    app.instrument_rewrite.is_none()&&!app.editor.is_editing()&&!app.editor.selected.is_empty()
        &&app.editor.active_ref().is_some()&&super::current(app).is_some_and(|e|e.auxiliary.is_some())
}
pub fn start(app:&mut App,ctx:&Context,kind:Kind) {
    if !enabled(app){return;}
    app.commit_lyric();
    let entry=super::current(app).unwrap().clone();
    let reference=app.editor.active_ref().unwrap();
    let request=match rewrite::prepare(&app.editor.project,reference,entry.clone(),&app.editor.selected,kind) {
        Ok(r)=>r,Err(e)=>{app.error=Some(e);return;}
    };
    let settings=app.synthesis.library.settings.instruments.clone();
    let zero_gru_weights=app.synthesis.library.settings.experiments.zero_gru_weights;
    let (tx,rx)=mpsc::channel();let cancel=Arc::new(AtomicBool::new(false));let stop=cancel.clone();
    app.instrument_rewrite=Some(Job {track:app.editor.track,reference:reference.id,revision:app.editor.revision,
        key:settings.key(&entry.selection),selection:entry.selection,identity:entry.identity,zero_gru_weights,
        accept_any_v4_signature:settings.accept_any_v4_signature,rx,cancel});
    let ctx=ctx.clone();
    std::thread::spawn(move || {
        let result=crate::synthesis::execution::scope_gru_weights(zero_gru_weights, || rewrite::execute(&settings,request,stop));
        let _=tx.send(result);
        ctx.request_repaint();
    });
}
pub fn poll(app:&mut App,ctx:&Context) {
    let Some(job)=app.instrument_rewrite.as_ref()else{return;};
    if app.editor.track!=job.track||app.editor.reference!=Some(job.reference)||app.editor.revision!=job.revision
        ||app.synthesis.library.settings.instruments.key(&job.selection)!=job.key
        ||app.synthesis.library.settings.instruments.accept_any_v4_signature!=job.accept_any_v4_signature
        ||app.synthesis.library.settings.experiments.zero_gru_weights!=job.zero_gru_weights
        ||!super::current(app).is_some_and(|e|e.selection==job.selection&&e.identity==job.identity) {
        app.instrument_rewrite=None;return;
    }
    let result=match job.rx.try_recv() {
        Ok(r)=>r,
        Err(mpsc::TryRecvError::Empty)=>{ctx.request_repaint_after(std::time::Duration::from_millis(100));return;},
        Err(mpsc::TryRecvError::Disconnected)=>Err("Instrument rewrite worker stopped".into()),
    };
    if app.editor.is_editing(){app.instrument_rewrite=None;return;}
    app.instrument_rewrite=None;
    match result {
        Err(e)=>app.error=Some(e),
        Ok(edits)=>{
            app.editor.begin(crate::i18n::t("AI Rewrites"));
            if let Some(group)=app.editor.group_mut() {
                for edit in edits {
                    if let Some(note)=group.notes.iter_mut().find(|n|n.id==edit.id) {
                        if let Some(names)=edit.articulations {
                            note.attributes.insert("articulations".into(),serde_json::json!(names));
                            note.attributes.insert("articulationsFixed".into(),true.into());
                        }
                        if let Some(dynamic)=edit.dynamic {note.attributes.insert("dynamic".into(),serde_json::json!(dynamic));}
                    }
                }
            }
            app.editor.finish();
        }
    }
}
