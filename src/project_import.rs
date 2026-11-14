use std::{path::{Path,PathBuf},process::Command};
use serde::Deserialize;
use crate::model::{Project,Track,InstrumentalTrack,AudioClip};
mod data;

pub const EXTENSIONS:&[&str]=&["rvp","svp","vsqx","vpr","vsq","mid","midi","ust","ustx","ccs","xml","musicxml","s5p","dv","ppsf","tssln","ufdata","zip"];
#[derive(Deserialize)]
pub struct Asset {
    pub track_name:String,
    pub onset_blick:i64,
    pub existing_group:Option<String>,
    pub path:Option<PathBuf>,
    pub region:Option<serde_json::Value>,
    #[serde(default)]pub muted:bool,
}
pub struct Imported {pub project:Project,pub assets:Vec<Asset>,pub source:PathBuf}

fn helper(action:&str,path:&Path,destination:Option<&Path>)->Result<serde_json::Value,String> {
    const BUNDLE:&[u8]=include_bytes!("../assets/project-import.mjs");
    let directory=crate::platform::config_directory()?.join("project-import");
    std::fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
    let script=directory.join("convert.mjs");
    if std::fs::read(&script).ok().as_deref()!=Some(BUNDLE) {crate::storage::atomic_write(&script,BUNDLE)?;}
    let mut command=Command::new("node");
    command.arg(&script).arg(action).arg(path);
    if let Some(destination)=destination {command.arg(destination);}
    #[cfg(windows)]{
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output=command.output().map_err(|e|format!("无法启动本地工程转换器（需要 Node.js 20 或更新版本）：{e}"))?;
    if !output.status.success(){return Err(format!("工程转换失败：{}",String::from_utf8_lossy(&output.stderr)));}
    serde_json::from_slice(&output.stdout).map_err(|e|format!("转换器返回无效结果：{e}"))
}
pub fn convert(path:&Path)->Result<Imported,String> {
    if crate::storage::ProjectFormat::from_path(path).is_some() {
        return Ok(Imported{project:crate::storage::load(path)?,assets:Vec::new(),source:path.into()});
    }
    let result=helper("convert",path,None)?;
    let project=if result["svp"].is_null() {data::project(result["data"].clone())?}else{
        let bytes=serde_json::to_vec(&result["svp"]).map_err(|e|e.to_string())?;
        crate::storage::load_svp_bytes(&bytes,path)?
    };
    let assets=serde_json::from_value(result["assets"].clone()).map_err(|e|format!("归档音频信息无效：{e}"))?;
    Ok(Imported{project,assets,source:path.into()})
}
impl Imported {
    pub fn extract(&mut self,destination:&Path)->Result<PathBuf,String> {
        let result=helper("extract",&self.source,Some(destination))?;
        let files:Vec<Asset>=serde_json::from_value(result["files"].clone()).map_err(|e|e.to_string())?;
        for file in files {
            let path=file.path.ok_or("归档音频缺少解压路径")?;
            if let Some(group)=&file.existing_group {

                let clip=self.project.tracks.iter_mut().find(|t|&t.main_ref.group_id==group)
                    .and_then(|t|t.instrumental.as_mut()).and_then(|t|t.clip.as_mut())
                    .ok_or("归档音轨引用不存在")?;
                clip.path=path;
                continue;
            }
            let audio=crate::synthesis::instrumental::load(&path)?;
            let duration=audio.audio.duration_seconds();
            let onset_seconds=self.project.time.seconds_at(file.onset_blick);
            self.project.tracks.push(Track::new(self.project.tracks.len()));
            let track=self.project.tracks.last_mut().unwrap();
            track.name=file.track_name;
            track.instrumental=Some(InstrumentalTrack{clip:Some(AudioClip{path,onset_seconds,duration_seconds:duration,trim_begin_seconds:None,trim_end_seconds:None})});
            track.mixer.insert("mute".into(),file.muted.into());
            if let Some(region)=file.region {track.extra.insert("vprSourceRegion".into(),region);}
        }
        self.assets.clear();
        result["folder"].as_str().map(PathBuf::from).ok_or("归档音频缺少目标目录".into())
    }
}
