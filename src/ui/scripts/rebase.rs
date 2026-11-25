use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn project(base:&Value,next:&Value,current:&Value)->Result<Value,String>{
    if base["uuid"]!=current["uuid"]||next["uuid"]!=current["uuid"]{
        return Err("The script's project is no longer open".into());
    }
    merge(base,next,current,"")
}
fn merge(base:&Value,next:&Value,current:&Value,field:&str)->Result<Value,String>{
    if next==base{return Ok(current.clone());}
    if current==base{return Ok(next.clone());}
    if let (Some(b),Some(n),Some(c))=(base.as_object(),next.as_object(),current.as_object()){
        let mut out=c.clone();
        let atomic_values=field=="scriptData"||field=="attributes";
        for key in b.keys().chain(n.keys()).collect::<std::collections::BTreeSet<_>>(){
            match (b.get(key),n.get(key),c.get(key)){
                (Some(old),Some(new),_)if old==new=>(),
                (Some(_),None,_)=>{out.remove(key);},
                (Some(old),Some(new),Some(live))=>{
                    let value=if atomic_values{new.clone()}
                        else if key=="points"&&b.contains_key("mode")&&!b.contains_key("type"){points(old,new,live)?}
                        else{merge(old,new,live,key)?};
                    out.insert(key.clone(),value);
                },
                (_,Some(new),_)=>{out.insert(key.clone(),new.clone());},
                _=>(),
            }
        }
        return Ok(Value::Object(out));
    }
    if let (Some(b),Some(n),Some(c))=(base.as_array(),next.as_array(),current.as_array()){
        if ["tracks","library","notes","groups","pitchControls","tempo","meter","takes"].contains(&field){
            return entities(b,n,c,field);
        }
    }
    Ok(next.clone())
}
fn identity(value:&Value,field:&str)->Result<String,String>{
    let key=match field{"tempo"=>"position","meter"=>"index","takes"=>"id",_=>{
        if value.get("__rynthvId").is_some(){"__rynthvId"}
        else if value.get("uuid").is_some(){"uuid"}else{"id"}
    }};
    value.get(key).filter(|v|v.is_string()||v.is_number()).map(Value::to_string)
        .ok_or_else(||format!("A concurrent {field} edit has no stable object identity"))
}
fn indexed<'a>(values:&'a[Value],field:&str)->Result<BTreeMap<String,&'a Value>,String>{
    let mut out=BTreeMap::new();
    for value in values{if out.insert(identity(value,field)?,value).is_some(){return Err("Duplicate script object identity".into());}}
    Ok(out)
}
fn entities(base:&[Value],next:&[Value],current:&[Value],field:&str)->Result<Value,String>{
    let before=indexed(base,field)?;let requested=indexed(next,field)?;
    let mut out=Vec::new();
    for live in current{
        let key=identity(live,field)?;
        if let Some(old)=before.get(&key){
            if let Some(new)=requested.get(&key){out.push(merge(old,new,live,field)?);}
        }else{out.push(live.clone());}
    }
    for (index,new) in next.iter().enumerate(){
        let key=identity(new,field)?;
        if out.iter().any(|v|identity(v,field).as_ref()==Ok(&key)){continue;}
        if let Some(old)=before.get(&key){
            if **old==*new{continue;}
            if !["tempo","meter","takes"].contains(&field){return Err("A script tried to edit an object that was deleted".into());}
        }
        let following=next[index+1..].iter().map(|v|identity(v,field)).collect::<Result<Vec<_>,_>>()?;
        let at=following.iter().find_map(|id|out.iter().position(|v|identity(v,field).as_ref()==Ok(id))).unwrap_or(out.len());
        out.insert(at,new.clone());
    }
    let clock=match field{"notes"=>Some("onset"),"pitchControls"=>Some("pos"),"tempo"=>Some("position"),"meter"=>Some("index"),_=>None};
    if let Some(clock)=clock{out.sort_by_key(|v|v[clock].as_i64().unwrap_or(0));}
    Ok(Value::Array(out))
}
fn point_map(value:&Value)->Result<BTreeMap<i64,Value>,String>{
    let array=value.as_array().ok_or("Invalid script automation points")?;
    if array.len()%2!=0{return Err("Invalid script automation points".into());}
    let mut map=BTreeMap::new();
    for pair in array.chunks_exact(2){
        let clock=pair[0].as_i64().ok_or("Invalid script automation clock")?;
        if map.insert(clock,pair[1].clone()).is_some(){return Err("Duplicate script automation clock".into());}
    }
    Ok(map)
}
fn points(base:&Value,next:&Value,current:&Value)->Result<Value,String>{
    let before=point_map(base)?;let requested=point_map(next)?;let mut out=point_map(current)?;
    for clock in before.keys(){if !requested.contains_key(clock){out.remove(clock);}}
    for (clock,value) in requested{if before.get(&clock)!=Some(&value){out.insert(clock,value);}}
    Ok(Value::Array(out.into_iter().flat_map(|(clock,value)|[Value::from(clock),value]).collect()))
}
