use super::{Engine,initialize,source};
use serde_json::{Value,json};
use std::{path::PathBuf,sync::{Arc,atomic::{AtomicBool,Ordering},mpsc},time::Duration};
pub struct Request{pub method:String,pub args:Vec<Value>,pub reply:mpsc::Sender<Result<Value,String>>}
pub enum Event{Request(Request),Info(Value),Finished(Result<(),String>)}
pub struct Job{pub events:mpsc::Receiver<Event>,pub commands:mpsc::Sender<Value>,pub cancel:Arc<AtomicBool>}
impl Drop for Job{fn drop(&mut self){self.cancel.store(true,Ordering::Release);}}
pub fn start(path:PathBuf,context:Value,wake:impl Fn()+Send+Clone+'static)->Result<Job,String>{
    launch(path,context,wake,false)
}
pub fn metadata(path:PathBuf,context:Value,wake:impl Fn()+Send+Clone+'static)->Result<Job,String>{
    launch(path,context,wake,true)
}
fn launch(path:PathBuf,context:Value,wake:impl Fn()+Send+Clone+'static,metadata:bool)->Result<Job,String>{
    let script=source(&path)?;let(send,events)=mpsc::channel();let(commands,receive)=mpsc::channel::<Value>();
    let cancel=Arc::new(AtomicBool::new(false));let flag=cancel.clone();
    std::thread::Builder::new().name("editor-script".into()).stack_size(4*1024*1024).spawn(move||{
        let tx=send.clone();let stop=flag.clone();let repaint=wake.clone();
        let result=(||->Result<(),String>{
            let mut engine=Engine::new(flag.clone(),move|text|{
                let value:Value=serde_json::from_str(text).map_err(|e|e.to_string())?;
                let method=value["method"].as_str().ok_or("Invalid script request")?.to_owned();
                let args=value["args"].as_array().ok_or("Invalid script arguments")?.clone();
                let value=if let Some(result)=super::local::request(&method,&args){result?}else{
                    let(reply,rx)=mpsc::channel();tx.send(Event::Request(Request{method,args,reply})).map_err(|_|"Editor closed")?;repaint();
                    loop{
                        if stop.load(Ordering::Acquire){return Err("Script stopped".into());}
                        match rx.recv_timeout(Duration::from_millis(50)){Ok(value)=>break value?,Err(mpsc::RecvTimeoutError::Timeout)=>(),Err(_)=>return Err("Editor closed".into())}
                    }
                };Ok(json!({"value":value}).to_string())
            })?;
            initialize(&mut engine,&context)?;engine.load(&script,&path)?;
            let info:Value=serde_json::from_str(&engine.eval("JSON.stringify(getClientInfo())","getClientInfo")?).map_err(|e|e.to_string())?;
            validate_info(&info)?;
            if metadata {engine.eval("__commit();","metadata changes")?;let _=send.send(Event::Info(info));return Ok(());}
            if info["type"]=="SidePanelSection"{
                engine.eval("__panel=true;__panelRefresh=true;__publishPanel();","side panel")?;
            }else{
                engine.eval("if(typeof main!=='function')throw new Error('Script has no main function');main();","main")?;
            }
            loop{
                if flag.load(Ordering::Acquire){return Ok(());}
                engine.eval("__commit();__runTimers();","script event loop")?;
                let wait=engine.eval("String(__nextWait())","script wait")?.parse::<i64>().map_err(|_|"Invalid script wait")?;
                if wait<0{break;}
                match receive.recv_timeout(Duration::from_millis(wait.clamp(1,50)as u64)){
                    Ok(event)=>{engine.eval(&format!("__event({event});"),"editor event")?;},
                    Err(mpsc::RecvTimeoutError::Timeout)=>(),
                    Err(_)=>break,
                }
            }
            engine.eval("__commit();","commit")?;Ok(())
        })();
        let _=send.send(Event::Finished(result));wake();
    }).map_err(|e|e.to_string())?;
    Ok(Job{events,commands,cancel})
}
fn validate_info(info:&Value)->Result<(),String>{
    if info["minEditorVersion"].as_u64().is_some_and(|v|v>0x020300){return Err("Script requires a newer scripting API".into());}
    if info["name"].as_str().is_none_or(|s|s.trim().is_empty()){return Err("Script has no name".into());}
    if info["type"].as_str().is_some_and(|v|v!="SidePanelSection"&&!v.is_empty()){return Err("Unknown script type".into());}Ok(())
}
#[derive(Clone)]
pub struct Entry{pub path:PathBuf,pub name:String,pub category:String,pub panel:bool,pub error:Option<String>}
