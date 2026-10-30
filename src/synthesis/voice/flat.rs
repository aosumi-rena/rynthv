mod crypto;
mod materialize;

use super::{Result,VoiceError,config::VoiceConfiguration,PreparedVoice};
use serde_json::Value;
use std::{fs::File,io::Read,path::{Path,PathBuf}};

pub(super) fn read(path:&Path,limit:usize)->Result<Vec<u8>> {
    let file=File::open(path)?;let length=file.metadata()?.len();
    if length>limit as u64 {return Err(VoiceError::new("Flat source exceeds its size limit"));}
    let mut bytes=Vec::new();file.take(limit as u64+1).read_to_end(&mut bytes)?;
    if bytes.len() as u64!=length {return Err(VoiceError::new("Flat source changed during reading"));}
    Ok(bytes)
}
pub(super) fn is_profile(path:&Path)->bool {
    if !path.extension().is_some_and(|e|e.eq_ignore_ascii_case("nofs")) {return false;}
    let Ok(file)=File::open(path) else{return false;};let mut bytes=Vec::new();
    if file.take(196).read_to_end(&mut bytes).is_err() {return false;}
    bytes.get(192..196)==Some(b"SVDB") || bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(&bytes).iter().find(|b|!b.is_ascii_whitespace())==Some(&b'{')
}
pub(super) struct Profile {pub json:Value,pub path:PathBuf,pub configuration:VoiceConfiguration}
impl Profile {
    pub fn open(path:&Path)->Result<Self> {
        let bytes=read(path,4*1024*1024+256)?;
        let bytes=if bytes.get(192..196)==Some(b"SVDB") {
            if bytes.len()<232 {return Err(VoiceError::new("Truncated Flat profile"));}
            crypto::decode(&bytes[196..212],&bytes[212..bytes.len()-4])?
        }else{bytes};
        let json:Value=serde_json::from_slice(bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(&bytes)).map_err(|e|VoiceError::new(format!("Invalid Flat profile: {e}")))?;
        if json.get("enable").and_then(Value::as_bool)==Some(false) {return Err(VoiceError::new("Flat profile is disabled"));}
        let string=|key:&str|json.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
        let language=normalize(&string("language"));
        let f0=json.get("f0_enhancement").and_then(Value::as_f64).unwrap_or(1.0) as f32;
        let mut configuration=VoiceConfiguration::from_materialized(string("name"),string("version"),string("vendor"),language,f0)?;
        let languages=json.get("support_languages").and_then(Value::as_array).map(|a|a.iter().filter_map(Value::as_str).map(normalize).collect::<Vec<_>>()).unwrap_or_else(||vec![configuration.default_language.clone()]);
        if !languages.contains(&configuration.default_language) || languages.is_empty() {return Err(VoiceError::new("Invalid Flat profile languages"));}
        configuration.supported_languages=languages;
        if let Some(styles)=json.get("styles").and_then(Value::as_array) {
            let names=styles.iter().filter_map(|v|v.get("name").and_then(Value::as_str)).filter(|n|!matches!(*n,"(base)"|"(default)")).map(str::to_owned).collect::<Vec<_>>();
            configuration.styles=names.clone();configuration.resolve_embedded_styles(&names)?;
        }
        Ok(Self {json,path:path.to_owned(),configuration})
    }
    pub fn model(&self,role:&str)->Result<PathBuf> {
        let (current,legacy,folders)=match role {
            "timbre"=>("acoustic","base_model",&["acoustic","base"][..]),
            "duration"=>("duration","timing_model",&["duration","timing"][..]),
            "f0"=>("f0","f0_model",&["f0"][..]),
            "vocoder"=>("vocoder","pitch_model",&["vocoder","sing"][..]),_=>return Err(VoiceError::new("Unknown Flat model role")),
        };
        let folder=self.path.parent().ok_or_else(||VoiceError::new("Flat profile has no directory"))?;
        let models=folder.ancestors().take(4).map(|p|p.join("models")).find(|p|p.is_dir()).ok_or_else(||VoiceError::new("Flat shared models directory was not found"))?;
        let reference=self.json.get(current).or_else(||self.json.get(legacy)).and_then(Value::as_str).unwrap_or("");
        let mut choices=Vec::new();
        if !reference.is_empty() {
            let reference=if reference.ends_with(".dnni") {reference.to_owned()}else{format!("{reference}.dnni")};
            let path=Path::new(&reference);
            if path.components().any(|c|!matches!(c,std::path::Component::Normal(_))) {return Err(VoiceError::new("Invalid Flat model reference"));}
            choices.push(folder.join(path));for directory in folders {choices.push(models.join(directory).join(path));}
        }else{
            for directory in folders {if let Ok(items)=std::fs::read_dir(models.join(directory)) {
                choices.extend(items.filter_map(std::result::Result::ok).map(|e|e.path()).filter(|p|p.extension().is_some_and(|e|e.eq_ignore_ascii_case("dnni"))));
            }}
        }
        choices.retain(|p|p.is_file());choices.sort();choices.dedup();
        if !reference.is_empty() {
            let first=choices.first().ok_or_else(||VoiceError::new(format!("Selected Flat {role} model is missing")))?;
            if choices.len()>1 {
                let expected=read(first,128*1024*1024)?;
                for candidate in &choices[1..] {
                    if read(candidate,128*1024*1024)?!=expected {return Err(VoiceError::new(format!("Selected Flat {role} model has conflicting source files")));}
                }
            }
            Ok(first.clone())
        }
        else if choices.len()==1 {Ok(choices.remove(0))}else{Err(VoiceError::new(format!("Flat profile must select an unambiguous {role} model")))}
    }
}
fn normalize(text:&str)->String {
    match text.to_ascii_lowercase().as_str() {"ja"|"jp"=>"japanese","zh"|"cn"=>"mandarin","en"=>"english","ko"|"kr"=>"korean","yue"=>"cantonese","es"=>"spanish",_=>text}.into()
}
pub(super) fn load(path:&Path)->Result<(PreparedVoice,VoiceConfiguration)> {
    let profile=Profile::open(path)?;
    let mut voice=materialize::prepare(&profile)?;
    let mut configuration=profile.configuration;
    let tree=voice.model("timbre")?;let node=tree.node(tree.root_index())?.children[1];
    let count=tree.node(tree.node(node)?.children[0])?.children.len().checked_sub(1).ok_or_else(||VoiceError::new("Flat style map is empty"))?;
    let names=crate::synthesis::vocal_modes::ordered_style_names(tree,node,count)?;
    configuration.resolve_embedded_styles(&names)?;
    voice=voice.with_vocal_modes(Some(names))?;
    if let Some(error)=&voice.summary().stage_preparation_error {return Err(VoiceError::new(error.clone()));}
    Ok((voice,configuration))
}
