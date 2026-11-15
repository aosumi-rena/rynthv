use super::State;
use crate::app::App;
use serde_json::{Value,json};
pub(super) fn request(app:&mut App,state:&mut State,method:&str,args:&[Value])->Result<Value,String> {
    let arg=|i:usize|args.get(i).ok_or_else(||"Missing script argument".to_owned());
    match method {
        "commit"=>{
            let running=state.running.as_mut().ok_or("Script has stopped")?;
            let value=arg(0)?;
            if *value!=running.baseline {
                if running.project!=app.editor.project.uuid||running.revision!=app.editor.revision||app.editor.is_editing(){return Err("The project changed while the script was running".into());}
                let project=rynthv::scripting::snapshot::decode(value.clone(),&app.editor.project)?;
                if project!=app.editor.project {
                    app.editor.begin(&running.name);app.editor.project=project;app.editor.finish();
                }
                running.revision=app.editor.revision;running.baseline=value.clone();
            }
            if let Some(selected)=args.get(1).and_then(Value::as_array) {
                let ids=app.editor.notes().iter().map(|n|n.id).collect::<std::collections::BTreeSet<_>>();
                app.editor.selected=selected.iter().filter_map(Value::as_u64).filter(|id|ids.contains(id)).collect();
            }
            Ok(Value::Null)
        },
        "playback.status"=>Ok(json!({"seconds":app.editor.project.time.seconds_at(app.editor.transport.position),"status":if app.editor.transport.playing{"playing"}else{"stopped"}})),
        "playback.play"=>{if !app.editor.transport.playing{app.editor.transport.toggle();}Ok(Value::Null)},
        "playback.pause"=>{app.editor.transport.playing=false;Ok(Value::Null)},
        "playback.stop"=>{app.editor.transport.stop();Ok(Value::Null)},
        "playback.seek"=>{
            let value=arg(0)?.as_f64().filter(|v|v.is_finite()&&*v>=0.).ok_or("Invalid playback position")?;
            app.editor.transport.position=app.editor.project.time.position_at(value);Ok(Value::Null)
        },
        "clipboard.get"=>arboard::Clipboard::new().and_then(|mut clipboard|clipboard.get_text()).map(|s|json!(s)).map_err(|e|e.to_string()),
        "clipboard.set"=>{let text=arg(0)?.as_str().ok_or("Invalid clipboard text")?;arboard::Clipboard::new().and_then(|mut clipboard|clipboard.set_text(text)).map_err(|e|e.to_string())?;Ok(Value::Null)},
        "print"=>{let text=arg(0)?.as_str().ok_or("Invalid script output")?;state.log.push_back(text.chars().take(4096).collect());while state.log.len()>128{state.log.pop_front();}Ok(Value::Null)},
        _=>Err(format!("Unsupported script request: {method}")),
    }
}
pub(super) fn open_folder()->Result<(),String> {
    let path=rynthv::scripting::directory()?;std::fs::create_dir_all(&path).map_err(|e|e.to_string())?;
    let command=if cfg!(windows){"explorer.exe"}else if cfg!(target_os="macos"){"open"}else{"xdg-open"};
    std::process::Command::new(command).arg(path).spawn().map_err(|e|e.to_string())?;Ok(())
}
