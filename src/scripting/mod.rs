mod engine;
mod local;
pub mod worker;
pub mod snapshot;
pub use engine::Engine;
use serde_json::Value;
use std::path::{Path,PathBuf};

pub fn directory()->Result<PathBuf,String>{Ok(crate::platform::config_directory()?.join("scripts"))}
pub fn scan()->Result<Vec<PathBuf>,String> {
    let root=directory()?;std::fs::create_dir_all(&root).map_err(|e|e.to_string())?;
    let mut result=Vec::new();let mut pending=vec![(root,0)];
    while let Some((dir,depth))=pending.pop() {
        for entry in std::fs::read_dir(dir).map_err(|e|e.to_string())? {
            let entry=entry.map_err(|e|e.to_string())?;let kind=entry.file_type().map_err(|e|e.to_string())?;
            if kind.is_symlink(){continue;}
            if kind.is_dir()&&depth<8{pending.push((entry.path(),depth+1));}
            else if kind.is_file()&&entry.path().extension().is_some_and(|v|v.eq_ignore_ascii_case("js")||v.eq_ignore_ascii_case("lua")){result.push(entry.path());}
            if result.len()>1024{return Err("Too many scripts".into());}
        }
    }
    result.sort_by_cached_key(|p|p.to_string_lossy().to_lowercase());Ok(result)
}
pub fn source(path:&Path)->Result<String,String> {
    if std::fs::metadata(path).map_err(|e|e.to_string())?.len()>2*1024*1024{return Err("Script exceeds 2 MiB".into());}
    std::fs::read_to_string(path).map(|s|s.trim_start_matches('\u{feff}').to_owned()).map_err(|e|e.to_string())
}
pub fn initialize(engine:&mut Engine,context:&Value)->Result<(),String> {
    engine.eval(&format!("var __context={};",context),"editor context")?;
    for (name,source) in [("public methods",include_str!("methods.js")),("objects",include_str!("objects.js")),("references",include_str!("references.js")),
        ("pitch controls",include_str!("pitch_controls.js")),("automation",include_str!("automation.js")),
        ("project",include_str!("project.js")),("selection",include_str!("selection.js")),
        ("navigation",include_str!("navigation.js")),("widgets",include_str!("widgets.js")),
        ("host",include_str!("host.js")),("instrument controls",include_str!("instrument.js")),("Lua adapter",include_str!("lua.js")),
        ("API argument contracts",include_str!("arity.js"))] {engine.eval(source,name)?;}
    Ok(())
}
