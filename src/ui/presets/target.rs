use crate::{app::App,model::EntityId};
use serde_json::{Value,json};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum Kind { Instrument, Effects, Microphones }
#[derive(Clone)]
pub(super) struct Target {
    pub kind:Kind,pub track:EntityId,pub reference:EntityId,
    pub scope:String,pub signature:String,pub name:String,pub value:Value,pub default:Value,
}
impl Target {
    pub fn read(app:&App,kind:Kind)->Result<Self,String> {
        let track=app.editor.current_track();let reference=app.editor.active_ref().unwrap_or(&track.main_ref);
        let mut result=Self {kind,track:track.id,reference:reference.id,scope:String::new(),signature:String::new(),name:String::new(),value:Value::Null,default:Value::Null};
        match kind {
            Kind::Instrument=>{
                let entry=app.synthesis.instruments.for_track(track).ok_or("Instrument is not available")?;
                result.scope=format!("instrument:{}",entry.selection.database_id);
                result.signature=entry.selection.revision.clone();
                result.default=json!({});
                result.value=reference.extra.get("voice").cloned().unwrap_or_else(||json!({}));
                let values=result.value.as_object_mut().ok_or("Instrument parameters must be an object")?;
                values.retain(|key,value|!(matches!(key.as_str(),"performanceDirection")&&value.as_str()==Some(""))
                    && !(matches!(key.as_str(),"transposeCents"|"transposeSemitones"|"paramLoudness"|"paramBreathiness")&&value.as_f64()==Some(0.)));
                result.name=reference.extra.get("voicePresetName").and_then(Value::as_str).unwrap_or("").into();
            }
            Kind::Effects=>{
                result.scope=if track.is_instrument(){"instrument-effects"}else{"voice-effects"}.into();
                result.signature=result.scope.clone();result.default=crate::synthesis::effects::Configuration::defaults();
                if !track.is_instrument(){result.default["room"]=json!({"enabled":false,"positionX":0.,"positionY":0.,"size":15.,"reflectionGain":0.});}
                result.value=track.mixer.get("fxParams").cloned().unwrap_or_else(||result.default.clone());
                result.name=track.mixer.get("fxPresetName").and_then(Value::as_str).unwrap_or("").into();
            }
            Kind::Microphones=>{
                use crate::synthesis::instruments::microphones::Configuration;
                let entry=app.synthesis.instruments.for_track(track).ok_or("Instrument is not available")?;
                result.scope=format!("microphones:{}",entry.selection.database_id);
                result.signature=serde_json::to_string(&(&entry.room,&entry.microphones)).map_err(|e|e.to_string())?;
                result.default=serde_json::to_value(Configuration::read(None,&entry.microphones,entry.room.as_deref())?).map_err(|e|e.to_string())?;
                result.value=serde_json::to_value(Configuration::read(track.mixer.get("micParams"),&entry.microphones,entry.room.as_deref())?).map_err(|e|e.to_string())?;
                result.name=track.mixer.get("micPresetName").and_then(Value::as_str).unwrap_or("").into();
            }
        }
        Ok(result)
    }
    pub fn valid(&self,app:&App)->bool {
        Self::read(app,self.kind).is_ok_and(|v|v.track==self.track&&v.reference==self.reference&&v.scope==self.scope&&v.signature==self.signature)
    }
    pub fn apply(&self,app:&mut App,value:Value,name:&str)->Result<(),String> {
        if app.editor.is_editing()||!self.valid(app){return Err(crate::i18n::t("选中的音符组已经改变").into());}
        if !value.is_object(){return Err("Preset parameters must be an object".into());}
        if self.kind==Kind::Effects{crate::synthesis::effects::Configuration::read(&value)?;}
        if self.kind==Kind::Microphones{crate::synthesis::instruments::microphones::Configuration::parse(&value)?;}
        app.commit_lyric();app.editor.begin(crate::i18n::t("Preset"));
        if self.kind==Kind::Effects && self.scope=="voice-effects" {
            let linked=crate::ui::instruments::room_settings_linked(&app.editor.project);
            crate::ui::instruments::link_room_settings(&mut app.editor.project,self.track,
                &self.value["room"],&value["room"],linked,linked);
        }
        let track=app.editor.project.tracks.iter_mut().find(|t|t.id==self.track).expect("qualified preset track");
        match self.kind {
            Kind::Instrument=>{
                let reference=std::iter::once(&mut track.main_ref).chain(&mut track.groups).find(|r|r.id==self.reference).expect("qualified reference");
                reference.extra.insert("voice".into(),value);reference.extra.insert("voicePresetName".into(),json!(name));
            }
            Kind::Effects=>{track.mixer.insert("fxParams".into(),value);track.mixer.insert("fxPresetName".into(),json!(name));}
            Kind::Microphones=>{track.mixer.insert("micParams".into(),value);track.mixer.insert("micPresetName".into(),json!(name));}
        }
        app.editor.finish();Ok(())
    }
}
