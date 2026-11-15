use super::{Engine,initialize,source};
use serde_json::{Value,json};
use std::{path::PathBuf,sync::{Arc,atomic::{AtomicBool,Ordering},mpsc},time::Duration};
pub struct Request {pub method:String,pub args:Vec<Value>,pub reply:mpsc::Sender<Result<Value,String>>}
pub enum Event {Request(Request),Finished(Result<(),String>)}
pub struct Job {pub events:mpsc::Receiver<Event>,pub cancel:Arc<AtomicBool>}
impl Drop for Job {fn drop(&mut self){self.cancel.store(true,Ordering::Release);}}
pub fn start(path:PathBuf,context:Value,wake:impl Fn()+Send+Clone+'static)->Result<Job,String> {
    let script=source(&path)?;let (send,events)=mpsc::channel();let cancel=Arc::new(AtomicBool::new(false));let flag=cancel.clone();
    std::thread::Builder::new().name("editor-script".into()).stack_size(4*1024*1024).spawn(move||{
        let tx=send.clone();let stop=flag.clone();let repaint=wake.clone();
        let result=(||->Result<(),String>{
            let mut engine=Engine::new(flag.clone(),move|text| {
                let value:Value=serde_json::from_str(text).map_err(|e|e.to_string())?;
                let method=value["method"].as_str().ok_or("Invalid script request")?.to_owned();
                let args=value["args"].as_array().ok_or("Invalid script arguments")?.clone();
                let value=if let Some(result)=super::local::request(&method,&args){result?}
                else if method=="sleep" {
                    let ms=args.first().and_then(Value::as_f64).filter(|v|v.is_finite()&&*v>=0.).ok_or("Invalid script delay")?.min(3600000.)as u64;
                    let end=std::time::Instant::now()+Duration::from_millis(ms);
                    while let Some(remaining)=end.checked_duration_since(std::time::Instant::now()) {
                        if stop.load(Ordering::Acquire){return Err("Script stopped".into());}
                        std::thread::sleep(remaining.min(Duration::from_millis(50)));
                    }Value::Null
                }else{
                    let (reply,rx)=mpsc::channel();tx.send(Event::Request(Request {method,args,reply})).map_err(|_|"Editor closed")?;repaint();
                    loop {
                        if stop.load(Ordering::Acquire){return Err("Script stopped".into());}
                        match rx.recv_timeout(Duration::from_millis(50)){Ok(value)=>break value?,Err(mpsc::RecvTimeoutError::Timeout)=>(),Err(_)=>return Err("Editor closed".into())}
                    }
                };
                Ok(json!({"value":value}).to_string())
            })?;
            initialize(&mut engine,&context)?;engine.eval(&script,&path.to_string_lossy())?;
            engine.eval("if(typeof main!=='function')throw new Error('Script has no main function');main();","main")?;
            engine.eval("__commit();","commit")?;
            while !flag.load(Ordering::Acquire)&&engine.eval("String(__drainTimer())","timer")?=="true" {}
            if flag.load(Ordering::Acquire){return Ok(());}
            engine.eval("__commit();","commit")?;Ok(())
        })();
        let _=send.send(Event::Finished(result));wake();
    }).map_err(|e|e.to_string())?;
    Ok(Job {events,cancel})
}
#[derive(Clone)]
pub struct Entry {pub path:PathBuf,pub name:String,pub error:Option<String>}
pub fn catalogue(language:String,wake:impl Fn()+Send+'static)->mpsc::Receiver<Result<Vec<Entry>,String>> {
    let (tx,rx)=mpsc::channel();std::thread::spawn(move||{
        let result=(||->Result<Vec<Entry>,String>{
            let mut entries=Vec::new();
            for path in super::scan()? {
                let name=path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                let metadata=(||->Result<String,String>{
                    let mut engine=Engine::new(Arc::new(AtomicBool::new(false)),|_|Err("Scripts may only access editor data while running".into()))?;
                    let context=json!({"project":null,"language":language});initialize(&mut engine,&context)?;
                    engine.eval(&source(&path)?,&path.to_string_lossy())?;
                    let info:Value=serde_json::from_str(&engine.eval("JSON.stringify(getClientInfo())","getClientInfo")?).map_err(|e|e.to_string())?;
                    if info["minEditorVersion"].as_u64().is_some_and(|v|v>0x020300){return Err("Script requires a newer scripting API".into());}
                    if info["type"].as_str().is_some_and(|v|v=="SidePanelSection"){return Err("Custom side-panel scripts are not supported".into());}
                    info["name"].as_str().filter(|s|!s.trim().is_empty()).map(str::to_owned).ok_or("Script has no name".into())
                })();
                entries.push(match metadata {Ok(name)=>Entry{path,name,error:None},Err(error)=>Entry{path,name,error:Some(error)}});
            }
            entries.sort_by_cached_key(|e|e.name.to_lowercase());Ok(entries)
        })();let _=tx.send(result);wake();
    });rx
}
