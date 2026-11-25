use super::{Running,State};
use crate::app::App;
use eframe::egui::Context;
use serde_json::{Value,json};
pub(super) fn request(app:&mut App,state:&mut State,running:&mut Running,ctx:&Context,method:&str,args:&[Value])->Result<Value,String>{
    let arg=|i:usize|args.get(i).ok_or_else(||"Missing script argument".to_owned());
    match method{
        "commit"=>{
            let value=arg(0)?;
            let revision=args.get(2).and_then(Value::as_str).and_then(|v|v.parse::<u64>().ok()).ok_or("Missing script revision")?;
            if running.project!=app.editor.project.uuid{return Err("The script's project is no longer open".into());}
            let rebased=revision!=app.editor.revision;
            let value=if rebased{
                let Some(base)=args.get(3)else{return Ok(json!({"needBase":true}));};
                let current=rynthv::scripting::snapshot::encode(&app.editor.project)?;
                super::rebase::project(base,value,&current)?
            }else{value.clone()};
            if value!=running.baseline{
                let project=rynthv::scripting::snapshot::decode(value.clone(),&app.editor.project)?;
                if project!=app.editor.project{
                    let coalesce=running.undo_revision==Some(app.editor.revision);
                    app.editor.begin(&running.entry.name);app.editor.project=project;app.editor.finish_script(coalesce);
                    running.undo_revision=Some(app.editor.revision);
                }
            }
            running.revision=app.editor.revision;running.baseline=value;
            if let Some(selection)=args.get(1){super::context::apply_selection(app,selection)?;}
            let project=if rebased{Some(rynthv::scripting::snapshot::encode(&app.editor.project)?)}else{None};
            if let Some(project)=&project{running.baseline=project.clone();}
            Ok(json!({"revision":app.editor.revision.to_string(),"project":project}))
        },
        "undo.boundary"=>{running.undo_revision=None;Ok(Value::Null)},
        "panel"=>{running.panel=Some(super::panel::Panel::parse(arg(0)?.clone())?);Ok(Value::Null)},
        "navigation"=>super::navigation::request(app,ctx,args),
        "computed"=>super::computed::request(app,args),
        "playback.status"=>Ok(json!({"seconds":app.editor.project.time.seconds_at(app.editor.transport.position),"status":if app.editor.transport.playing{if app.editor.transport.looping{"looping"}else{"playing"}}else{"stopped"}})),
        "playback.play"=>{if !app.editor.transport.playing{app.editor.transport.toggle();}Ok(Value::Null)},
        "playback.pause"=>{app.editor.transport.playing=false;Ok(Value::Null)},
        "playback.stop"=>{app.editor.transport.script_stop();Ok(Value::Null)},
        "playback.seek"=>{
            let value=arg(0)?.as_f64().filter(|v|v.is_finite()&&*v>=0.).ok_or("Invalid playback position")?;
            app.editor.transport.position=app.editor.project.time.position_at(value);Ok(Value::Null)
        },
        "playback.loop"=>{
            let start=arg(0)?.as_f64().filter(|v|v.is_finite()&&*v>=0.).ok_or("Invalid loop start")?;
            let end=arg(1)?.as_f64().filter(|v|v.is_finite()&&*v>start).ok_or("Invalid loop end")?;
            let start=app.editor.project.time.position_at(start);let end=app.editor.project.time.position_at(end);
            app.editor.transport.loop_region=crate::editor::LoopRegion::from_saved(start,end).ok_or("Loop range is too short")?;
            app.editor.transport.looping=true;if !app.editor.transport.playing{app.editor.transport.toggle();}app.editor.transport.position=start;Ok(Value::Null)
        },
        "clipboard.get"=>arboard::Clipboard::new().and_then(|mut clipboard|clipboard.get_text()).map(|s|json!(s)).map_err(|e|e.to_string()),
        "clipboard.set"=>{let text=arg(0)?.as_str().ok_or("Invalid clipboard text")?;arboard::Clipboard::new().and_then(|mut clipboard|clipboard.set_text(text)).map_err(|e|e.to_string())?;Ok(Value::Null)},
        "print"=>{let text=arg(0)?.as_str().ok_or("Invalid script output")?;println!("{text}");state.log.push_back(text.chars().take(4096).collect());while state.log.len()>128{state.log.pop_front();}Ok(Value::Null)},
        _=>Err(format!("Unknown script request: {method}")),
    }
}
pub(super) fn open_folder()->Result<(),String>{
    let path=rynthv::scripting::directory()?;std::fs::create_dir_all(&path).map_err(|e|e.to_string())?;
    let command=if cfg!(windows){"explorer.exe"}else if cfg!(target_os="macos"){"open"}else{"xdg-open"};
    std::process::Command::new(command).arg(path).spawn().map_err(|e|e.to_string())?;Ok(())
}
