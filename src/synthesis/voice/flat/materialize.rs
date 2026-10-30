use super::Profile;
use super::super::{Result,VoiceError,PreparedTree,PreparedVoice,VoiceFamily,embedded::{self,Record,types},preparation::{PreparedFile,PreparedModel}};
use std::{collections::BTreeMap,sync::Arc};

pub(super) fn prepare(profile:&Profile)->Result<PreparedVoice> {
    let mut files=Vec::new();
    let dictionaries=crate::synthesis::settings::SynthesisSettings::load().map_err(VoiceError::new)?.dictionaries;
    for (role,expected) in [("duration",types::DURATION_TYPE_ID),("f0",types::F0_WRAPPER_TYPE_ID),("timbre",types::TIMBRE_TYPE_ID),("vocoder",types::VOCODER_TYPE_ID)] {
        let path=profile.model(role)?;
        let bytes=super::read(&path,128*1024*1024)?;
        let bytes=if bytes.get(..8)==Some(&[0xff,0,0xca,0x7f,3,0,0,0]) {
            if bytes.len()<40 {return Err(VoiceError::new("Truncated Flat model"));}
            super::crypto::decode(&bytes[8..24],&bytes[24..])?
        }else{bytes};
        if embedded::root_type(&bytes)?!=expected {return Err(VoiceError::new(format!("Flat {role} model has a different architecture")));}
        let mut records=embedded::parse(&bytes)?;
        if role!="vocoder" {phone_tables(&mut records,dictionaries.as_deref())?;}
        let root=records.len()-1;
        match role {
            "duration"=>if let Some(text)=field(profile,"duration_style","timing") {
                let node=*records[root].children.get(5).ok_or_else(||VoiceError::new("Flat duration condition is missing"))?;
                records[node].bytes=vector(&records[node].bytes,&hex(text)?)?;
            },
            "f0"=>if let Some(text)=field(profile,"f0_style","pitch") {
                let network=*records[root].children.first().ok_or_else(||VoiceError::new("Flat pitch model is empty"))?;
                let node=*records[network].children.get(1).ok_or_else(||VoiceError::new("Flat pitch condition is missing"))?;
                records[node].bytes=vector(&records[node].bytes,&hex(text)?)?;
            },
            "timbre"=>styles(profile,&mut records,root)?,
            _=>{},
        }
        let ordered=order(&records,root)?;
        let tree=PreparedTree::prepare_records(ordered,matches!(role,"f0"|"timbre"|"vocoder"))?;
        files.push(PreparedFile {name:path.file_name().unwrap_or_default().to_string_lossy().into_owned(),trees:vec![PreparedModel{role,tree:Arc::new(tree)}]});
    }
    Ok(PreparedVoice::new(files,VoiceFamily::Sv1,profile.configuration.f0_enhancement,Vec::new()))
}

fn field<'a>(profile:&'a Profile,current:&str,legacy:&str)->Option<&'a str> {
    profile.json.get(current).or_else(||profile.json.get(legacy)).and_then(serde_json::Value::as_str)
}
fn hex(text:&str)->Result<Vec<u8>> {
    if text.len()%2!=0 || text.len()>128*1024 || !text.bytes().all(|b|b.is_ascii_hexdigit()) {return Err(VoiceError::new("Invalid Flat condition vector"));}
    (0..text.len()).step_by(2).map(|i|u8::from_str_radix(&text[i..i+2],16).map_err(|e|VoiceError::new(e.to_string()))).collect()
}
fn vector(template:&[u8],data:&[u8])->Result<Vec<u8>> {
    if template.len()<20 || data.len()%4!=0 || !data.chunks_exact(4).all(|b|f32::from_le_bytes(b.try_into().unwrap()).is_finite()) {
        return Err(VoiceError::new("Invalid Flat vector data"));
    }
    let mut bytes=template[..16].to_vec();bytes.extend_from_slice(&(data.len() as u32/4).to_le_bytes());bytes.extend_from_slice(data);Ok(bytes)
}
fn styles(profile:&Profile,records:&mut Vec<Record>,root:usize)->Result<()> {
    let Some(styles)=profile.json.get("styles").and_then(serde_json::Value::as_array) else{return Ok(());};
    let network=*records[root].children.first().ok_or_else(||VoiceError::new("Flat acoustic model is empty"))?;
    let bank=*records[network].children.get(1).ok_or_else(||VoiceError::new("Flat speaker vector is missing"))?;
    let book=*records[root].children.get(1).ok_or_else(||VoiceError::new("Flat style map is missing"))?;
    let list=*records[book].children.first().ok_or_else(||VoiceError::new("Flat style vectors are missing"))?;
    let neutral=*records[list].children.first().ok_or_else(||VoiceError::new("Flat default style is missing"))?;
    let extras=*records[book].children.get(1).ok_or_else(||VoiceError::new("Flat style coefficients are missing"))?;
    let template=records[neutral].bytes.clone();let mut rows=Vec::new();let mut names=Vec::new();let mut coefficients=Vec::new();
    let mut default=None;let mut seen=std::collections::BTreeSet::new();
    for style in styles {
        let name=style.get("name").and_then(serde_json::Value::as_str).ok_or_else(||VoiceError::new("Flat style has no name"))?;
        if name.is_empty() || name.len()>1024 || !seen.insert(name.to_owned()) {return Err(VoiceError::new("Invalid or duplicate Flat style name"));}
        let data=hex(style.get("data").and_then(serde_json::Value::as_str).ok_or_else(||VoiceError::new("Flat style has no vector"))?)?;
        if data.len()!=template.len()-20 {return Err(VoiceError::new("Flat style vector has a different width"));}
        if name=="(base)" {records[bank].bytes=vector(&records[bank].bytes,&data)?;continue;}
        if name=="(default)" {default=Some(data);continue;}
        names.push(name.to_owned());rows.push(data);
        let extra=style.get("extra").and_then(serde_json::Value::as_str).map(hex).transpose()?.unwrap_or_else(||0.0f32.to_le_bytes().to_vec());
        if extra.len()!=4 {return Err(VoiceError::new("Invalid Flat style coefficient"));}coefficients.extend_from_slice(&extra);
    }
    if !seen.contains("(base)") {return Err(VoiceError::new("Flat profile has no base speaker vector"));}
    let default=default.or_else(||(styles.len()==1).then(||vec![0;template.len()-20])).ok_or_else(||VoiceError::new("Flat profile has no default style"))?;
    let mut children=Vec::new();
    for data in std::iter::once(&default).chain(rows.iter()) {
        children.push(records.len());records.push(Record{bytes:vector(&template,data)?,children:Vec::new()});
    }
    records[list].children=children;
    let mut payload=records[book].bytes[..16].to_vec();payload.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for name in names {string(&mut payload,&name);}
    records[book].bytes=payload;
    records[extras].bytes=vector(&records[extras].bytes,&coefficients)?;
    Ok(())
}
fn phone_tables(records:&mut [Record],directory:Option<&std::path::Path>)->Result<()> {
    for record in records {
        if record.bytes.get(..16)!=Some(super::super::PHONE_SET_TYPE_ID.as_slice()) {continue;}
        if super::super::PhoneSet::parse(&record.bytes).is_ok() {continue;}
        let bytes=&record.bytes[16..];
        if bytes.len()<4 {return Err(VoiceError::new("Flat phone table reference is invalid"));}
        let length=u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let token=std::str::from_utf8(bytes.get(4..4+length).ok_or_else(||VoiceError::new("Flat phone table reference is truncated"))?).map_err(|e|VoiceError::new(e.to_string()))?;
        if !token.ends_with("phones") || !token.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_') {return Err(VoiceError::new("Invalid Flat phone table name"));}
        let directory=directory.ok_or_else(||VoiceError::new("Set the clf-data directory to load shared Flat models"))?;
        let mut data=record.bytes[..16].to_vec();data.extend(super::read(&directory.join(format!("{token}.data")),4*1024*1024)?);
        let classes=["vowel","diphthong","coda","stop","affricate","aspirate","fricative","nasal","liquid","semivowel","silence","breath","trill"];
        data.extend_from_slice(&(classes.len() as u32).to_le_bytes());for name in classes {string(&mut data,name);}
        super::super::PhoneSet::parse(&data)?;record.bytes=data;
    }
    Ok(())
}
fn string(bytes:&mut Vec<u8>,text:&str) {bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());bytes.extend_from_slice(text.as_bytes());}
fn order(records:&[Record],root:usize)->Result<Vec<Record>> {
    fn visit(index:usize,records:&[Record],state:&mut BTreeMap<usize,Option<usize>>,out:&mut Vec<Record>)->Result<usize> {
        if let Some(value)=state.get(&index) {return value.ok_or_else(||VoiceError::new("Flat model has a cycle"));}
        let record=records.get(index).ok_or_else(||VoiceError::new("Flat model child is absent"))?;
        state.insert(index,None);let children=record.children.iter().map(|&child|visit(child,records,state,out)).collect::<Result<Vec<_>>>()?;
        let target=out.len();out.push(Record{bytes:record.bytes.clone(),children});state.insert(index,Some(target));Ok(target)
    }
    let mut out=Vec::new();visit(root,records,&mut BTreeMap::new(),&mut out)?;Ok(out)
}
