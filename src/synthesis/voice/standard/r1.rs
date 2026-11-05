//! Original S5L/S5D identity and fixed-header geometry, without loading wave data.
use std::{collections::BTreeMap,fs::File,io::Read,path::Path};
use super::{configuration::require,discovery::{Discovered,read_bounded}};
use super::super::{Result,VoiceError,VoiceFamily,VoiceRevision};

fn name(file:&mut File)->Result<String> {
    let mut bytes=Vec::new();
    for _ in 0..256 {
        let mut byte=[0];file.read_exact(&mut byte)?;
        if byte[0]==0 {return String::from_utf8(bytes).map_err(|_|VoiceError::new("Invalid R1 language identity"));}
        require((32..=126).contains(&byte[0]),"Invalid R1 language identity")?;bytes.push(byte[0]);
    }
    Err(VoiceError::new("R1 language identity exceeds 255 bytes"))
}
fn xml_text(raw:&str)->Result<String> {
    let mut output=String::new();let mut rest=raw;
    while let Some(at)=rest.find('&') {
        output.push_str(&rest[..at]);rest=&rest[at+1..];
        let end=rest.find(';').ok_or_else(||VoiceError::new("Invalid R1 settings entity"))?;
        let token=&rest[..end];
        let value=match token {"amp"=>'&',"lt"=>'<',"gt"=>'>',"quot"=>'"',"apos"=>'\'',_=> {
            let number=if let Some(s)=token.strip_prefix("#x"){u32::from_str_radix(s,16).ok()}
                else{token.strip_prefix('#').and_then(|s|s.parse().ok())};
            number.and_then(char::from_u32).ok_or_else(||VoiceError::new("Unsupported R1 settings entity"))?
        }};
        output.push(value);rest=&rest[end+1..];
    }
    output.push_str(rest);require(output.len()<=1024&&!output.chars().any(char::is_control),"Invalid R1 display attribute")?;Ok(output)
}
// Only the bounded Database start tag is needed. External entities and DTDs
// are not processed, and this reader never follows a file named by XML.
fn settings(path:&Path)->Result<BTreeMap<String,String>> {
    if !path.is_file(){return Ok(BTreeMap::new());}
    let bytes=read_bounded(path,1_048_576)?;
    let mut text=std::str::from_utf8(bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(&bytes))
        .map_err(|_|VoiceError::new("R1 settings must use UTF-8"))?.trim_start();
    require(!text.contains("<!DOCTYPE")&&!text.contains("<!ENTITY"),"R1 settings DTD/entities are unsupported")?;
    if text.starts_with("<?xml") {
        let end=text.find("?>").ok_or_else(||VoiceError::new("Unclosed R1 XML declaration"))?;text=text[end+2..].trim_start();
    }
    let mut rest=text.strip_prefix("<Database").ok_or_else(||VoiceError::new("R1 settings root must be Database"))?;
    require(rest.starts_with(|c:char|c.is_ascii_whitespace()||c=='/'||c=='>'),"Invalid R1 settings root")?;
    let mut result=BTreeMap::new();
    for _ in 0..64 {
        rest=rest.trim_start();if rest.starts_with('>')||rest.starts_with("/>"){return Ok(result);}
        let end=rest.find(|c:char|c.is_ascii_whitespace()||c=='=').ok_or_else(||VoiceError::new("Invalid R1 XML attribute"))?;
        let key=&rest[..end];require(!key.is_empty(),"Empty R1 XML attribute")?;
        rest=rest[end..].trim_start().strip_prefix('=').ok_or_else(||VoiceError::new("Missing R1 XML attribute equals"))?.trim_start();
        let quote=rest.chars().next().filter(|c|*c=='"'||*c=='\'').ok_or_else(||VoiceError::new("Unquoted R1 XML attribute"))?;
        rest=&rest[1..];let end=rest.find(quote).ok_or_else(||VoiceError::new("Unclosed R1 XML attribute"))?;
        require(result.insert(key.to_owned(),xml_text(&rest[..end])?).is_none(),"Duplicate R1 XML attribute")?;
        rest=&rest[end+1..];
    }
    Err(VoiceError::new("Too many R1 XML attributes"))
}
pub(super) fn inspect(path:&Path)->Result<Option<Discovered>> {
    let folder=if path.is_dir(){path}else{path.parent().ok_or_else(||VoiceError::new("R1 bank has no directory"))?};
    let metadata=folder.join("master.s5l");
    if !metadata.is_file(){return Ok(None);}
    let mut file=File::open(&metadata)?;
    require(file.metadata()?.len()<=128*1024*1024,"R1 metadata exceeds 128 MiB")?;
    let mut header=[0;40];file.read_exact(&mut header)?;
    require(&header[..4]==b"s5l "&&u32::from_le_bytes(header[4..8].try_into().unwrap())==3,"Unsupported R1 metadata format")?;
    let language=name(&mut file)?;let phoneset=name(&mut file)?;
    let mut flags=[0;4];file.read_exact(&mut flags)?;
    let fields=settings(&folder.join("settings.xml"))?;
    let display=fields.get("name").filter(|s|!s.trim().is_empty()).cloned()
        .unwrap_or_else(||folder.file_name().unwrap_or_default().to_string_lossy().into_owned());
    // R1 settings use zero-padded decimal revisions, e.g. 002. Preserve the
    // display text and use its numeric value for the existing revision selector.
    let version=fields.get("version").filter(|s|!s.is_empty()).cloned().unwrap_or_else(||"0".into());
    let revision=version.parse::<u32>().map(VoiceRevision::release).or_else(|_|version.parse::<VoiceRevision>())
        .map_err(|_|VoiceError::new("Unsupported R1 bank revision in settings.xml"))?;
    let data=folder.join("master.s5d");
    let validation=(||->Result<()> {
        require(language=="japanese"&&phoneset=="romaji","Only Japanese romaji R1 banks are supported")?;
        for (key,expected) in [("language",language.as_str()),("phoneset",phoneset.as_str())] {
            require(fields.get(key).is_none_or(|s|s==expected),"R1 settings disagree with bank language/phone set")?;
        }
        require(flags.iter().all(|v|*v<=1)&&flags[3]==0&&u32::from_le_bytes(header[36..40].try_into().unwrap())==0,"Grouped or linked-classifier R1 banks are not supported")?;
        let mut data=File::open(&data).map_err(|_|VoiceError::new("R1 bank needs master.s5d beside master.s5l"))?;
        require(data.metadata()?.len()<=1024*1024*1024,"R1 data exceeds 1 GiB")?;
        let mut bytes=[0;100];data.read_exact(&mut bytes)?;
        let u=|at|u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
        let f=|at|f32::from_bits(u(at));
        require(&bytes[..4]==b"s5d "&&(1..=9).contains(&u(4)),"Unsupported R1 data format")?;
        require(u(32)==u32::from_le_bytes(header[32..36].try_into().unwrap()),"R1 metadata/data unit counts disagree")?;
        require((0.001..=0.02).contains(&f(72))&&(1..=512).contains(&u(76))&&u(80)<=32&&(4..=512).contains(&u(84))&&
            f(88).is_finite()&&f(88)>0.0&&(8000.0..=48000.0).contains(&f(92))&&(2..=8).contains(&u(96)),"Unsupported R1 synthesis geometry")?;
        Ok(())
    })();
    Ok(Some(Discovered {path:metadata.canonicalize()?,data:Some(data),name:display,vendor:String::new(),revision,version,
        language,family:VoiceFamily::R1Std,unsupported:validation.err().map(|e|e.to_string()),warnings:Vec::new()}))
}
