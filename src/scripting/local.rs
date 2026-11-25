use crate::model::{self,Automation,Group,GroupRef,Note,Parameter,Project,TimeAxis};
use serde_json::{Value,json};
pub fn request(method:&str,args:&[Value])->Option<Result<Value,String>> {
    Some((||->Result<Value,String>{
        let arg=|index:usize|args.get(index).ok_or_else(||"Missing script argument".to_owned());
        match method {
            "uuid"=>Ok(json!(uuid::Uuid::new_v4().to_string())),
            "translation"=>Ok(json!(crate::i18n::t(arg(0)?.as_str().ok_or("Invalid translation text")?))),
            "identity"=>Ok(json!(model::new_id().to_string())),
            "create"=>match arg(0)?.as_str().unwrap_or("") {
                "Note"=>{let mut note=Note::new(0,0,60);note.lyrics.clear();note.attributes.extend([
                    ("languageOverride".into(),json!("")),("phoneset".into(),json!("")),("phonemes".into(),json!([]))]);
                    let mut data=json!(note);data["__rynthvId"]=json!(note.id.to_string());Ok(data)},
                "NoteGroup"=>Ok(json!(Group::new(""))),
                "NoteGroupReference"=>{let mut reference=GroupRef::new(&Group::new(""),-1);reference.group_id.clear();let mut data=json!(reference);data["__rynthvId"]=json!(reference.id.to_string());Ok(data)},
                "Track"=>{let mut value=super::snapshot::encode(&Project::default())?["tracks"][0].clone();value["name"]=json!("");value["mainGroup"]["name"]=json!("");value["dispOrder"]=json!(-1);value["dispColor"]=json!("00000000");Ok(value)},
                "TrackMixer"=>Ok(json!({"gainDecibel":0.,"pan":0.,"mute":false,"solo":false})),
                "TimeAxis"=>Ok(json!(TimeAxis::default())),
                "Project"=>{let mut value=super::snapshot::encode(&Project::default())?;for track in value["tracks"].as_array_mut().unwrap(){track["name"]=json!("");track["dispOrder"]=json!(-1);track["dispColor"]=json!("00000000");}Ok(value)},
                _=>Err("Unsupported script object type".into()),
            },
            "track.color"=>{let project:Project=serde_json::from_value(arg(0)?.clone()).map_err(|e|e.to_string())?;Ok(json!(String::from(project.next_track_color())))},
            "retake.generate"=>{
                let mut set=model::TakeSet::from_value(arg(0)?)?;
                let id=set.takes.iter().map(|t|t.id).max().unwrap_or(0).checked_add(1).filter(|v|*v<=i32::MAX as u32).ok_or("Retake IDs exhausted")?;
                let mut take=set.active().clone();take.id=id;take.liked=false;
                for (index,component) in [(1,model::RetakeComponent::Duration),(2,model::RetakeComponent::Pitch),(3,model::RetakeComponent::Timbre)] {
                    if args.get(index).and_then(Value::as_bool)==Some(true){take=take.rerolled(id,component);}
                }
                set.takes.push(take);set.active_take_id=id;Ok(set.to_value())
            },
            key if key.starts_with("time.")=>{
                let axis:TimeAxis=serde_json::from_value(arg(0)?.clone()).map_err(|e|e.to_string())?;
                if axis.tempo.first().is_none_or(|v|v.position!=0)||axis.meter.first().is_none_or(|v|v.index!=0)
                    ||!axis.start_time_seconds.is_finite()
                    ||axis.tempo.iter().any(|v|v.position<0||v.bpm<=0.||!v.bpm.is_finite())
                    ||axis.tempo.windows(2).any(|v|v[0].position>=v[1].position)
                    ||axis.meter.iter().any(|v|v.index<0||!v.denominator.is_power_of_two()||v.denominator>64||v.numerator==0||v.numerator>64)
                    ||axis.meter.windows(2).any(|v|v[0].index>=v[1].index){return Err("Invalid script time axis".into());}
                let value=arg(1)?.as_f64().filter(|v|v.is_finite()).ok_or("Invalid script time")?;
                match key {"time.seconds"=>Ok(json!(axis.seconds_at(value.round()as i64))),"time.blick"=>Ok(json!(axis.position_at(value))),
                    "time.measure"=>Ok(json!(axis.bar_position(value as i32))),"time.measureAt"=>Ok(json!(axis.bar_at(value.round()as i64).0)),
                    _=>Err("Unknown time-axis method".into())}
            },
            "automation.sample"|"automation.definition"=>{
                let key=arg(if method=="automation.sample"{1}else{0})?.as_str().ok_or("Invalid parameter name")?;
                let parameter=Parameter::ALL.into_iter().find(|p|p.key()==key).or_else(||key.strip_prefix("vocalMode_").map(|name|Parameter::VocalMode(name.into())))
                    .or_else(||(key=="dynamics").then(||Parameter::Experimental {key:key.into(),label:"Dynamics".into()})).ok_or("Unknown automation parameter")?;
                let default=parameter.default_value();let (low,high)=parameter.range();
                if method=="automation.definition" {
                    let instrument=args.get(1).and_then(Value::as_bool)==Some(true);
                    let display=match &parameter {Parameter::Pitch=>"Pitch Deviation",Parameter::Vibrato=>if instrument{"Vibrato"}else{"Vibrato Envelope"},Parameter::Loudness=>if instrument{"Gain"}else{"Loudness"},
                        Parameter::Tension=>"Tension",Parameter::Breathiness=>if instrument{"Roughness"}else{"Breathiness"},Parameter::Voicing=>"Voicing",Parameter::Gender=>"Gender",Parameter::ToneShift=>"Tone Shift",Parameter::Mouth=>"Mouth Opening",Parameter::Rap=>"Rap Intonation",_=>parameter.label()};
                    let (low,high)=if key=="dynamics"{(-1.,1.)}else if key.starts_with("vocalMode_"){(0.,150.)}else{(low,high)};
                    return Ok(json!({"displayName":display,"typeName":key,"range":[low,high],"defaultValue":default}));
                }
                let mut curve:Automation=serde_json::from_value(arg(0)?.clone()).map_err(|e|e.to_string())?;curve.validate()?;
                if args.get(3).and_then(Value::as_bool)==Some(true){curve.mode="linear".into();}
                let at=arg(2)?.as_f64().filter(|v|v.is_finite()).ok_or("Invalid curve time")?;
                Ok(json!(curve.sample(at.round()as i64,default as f32)))
            },
            _=>Err("Not a local request".into()),
        }
    })()).filter(|_|method=="uuid"||method=="identity"||method=="translation"||method=="create"||method=="track.color"||method=="retake.generate"||method.starts_with("time.")||method.starts_with("automation."))
}
