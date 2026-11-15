use crate::model::{self,Automation,Group,GroupRef,Note,Parameter,Project,TimeAxis};
use serde_json::{Value,json};
pub fn request(method:&str,args:&[Value])->Option<Result<Value,String>> {
    Some((||->Result<Value,String>{
        let arg=|index:usize|args.get(index).ok_or_else(||"Missing script argument".to_owned());
        match method {
            "uuid"=>Ok(json!(uuid::Uuid::new_v4().to_string())),
            "identity"=>Ok(json!(model::new_id())),
            "create"=>match arg(0)?.as_str().unwrap_or("") {
                "Note"=>{let note=Note::new(0,model::QUARTER,60);let mut data=json!(note);data["__rynthvId"]=json!(note.id);Ok(data)},
                "NoteGroup"=>Ok(json!(Group::new(""))),
                "NoteGroupReference"=>{let mut reference=GroupRef::new(&Group::new(""),-1);reference.group_id.clear();let mut data=json!(reference);data["__rynthvId"]=json!(reference.id);Ok(data)},
                "Track"=>Ok(super::snapshot::encode(&Project::default())?["tracks"][0].clone()),
                _=>Err("Unsupported script object type".into()),
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
                let parameter=Parameter::ALL.into_iter().find(|p|p.key()==key).or_else(||key.strip_prefix("vocalMode_").map(|name|Parameter::VocalMode(name.into()))).ok_or("Unknown automation parameter")?;
                let default=parameter.default_value();let (low,high)=parameter.range();
                if method=="automation.definition" {return Ok(json!({"typeName":key,"range":[low,high],"defaultValue":default}));}
                let mut curve:Automation=serde_json::from_value(arg(0)?.clone()).map_err(|e|e.to_string())?;curve.validate()?;
                if args.get(3).and_then(Value::as_bool)==Some(true){curve.mode="linear".into();}
                let at=arg(2)?.as_f64().filter(|v|v.is_finite()).ok_or("Invalid curve time")?;
                Ok(json!(curve.sample(at.round()as i64,default as f32)))
            },
            _=>Err("Not a local request".into()),
        }
    })()).filter(|_|method=="uuid"||method=="identity"||method=="create"||method.starts_with("time.")||method.starts_with("automation."))
}
