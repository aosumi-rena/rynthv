//! Seek-only NOFS metadata discovery. Acoustic/model members are never loaded here.
use std::{collections::{BTreeMap,HashSet},fs::File,io::{Read,Seek,SeekFrom},path::Path};
use super::{configuration::{Configuration,Reader,require},discovery::Discovered};
use super::super::{Result,VoiceError,VoiceFamily,VoiceRevision};

struct Metadata {file:File,length:u64,members:BTreeMap<u32,u64>,warnings:Vec<String>}
impl Metadata {
    fn bytes(&mut self,offset:u64,count:usize)->Result<Vec<u8>> {
        require(offset<=self.length&&count as u64<=self.length-offset,"NOFS metadata range exceeds bank")?;
        self.file.seek(SeekFrom::Start(offset))?;let mut bytes=vec![0;count];self.file.read_exact(&mut bytes)?;Ok(bytes)
    }
    fn block(&mut self,offset:u64,kind:u16)->Result<Vec<u8>> {
        let head=self.bytes(offset,6)?;
        let size=u32::from_le_bytes(head[..4].try_into().unwrap()) as usize;
        require((10..=1_048_576).contains(&size)&&u16::from_le_bytes(head[4..].try_into().unwrap())==kind,"Invalid NOFS metadata block")?;
        let bytes=self.bytes(offset,size)?;
        require(u32::from_le_bytes(bytes[size-4..].try_into().unwrap()) as usize==size,"NOFS metadata block footer differs")?;
        Ok(bytes)
    }
    fn open(path:&Path)->Result<Self> {
        let file=File::open(path)?;let length=file.metadata()?.len();
        require((16..=512*1024*1024).contains(&length),"Unsupported NOFS file size")?;
        let mut this=Self{file,length,members:BTreeMap::new(),warnings:Vec::new()};
        let bytes=this.bytes(0,16)?;let mut r=Reader::new(&bytes);
        require(r.u32()?==0xf580&&(1..=0x13).contains(&r.u32()?),"Invalid NOFS header")?;
        let extent=u64::from_le_bytes(r.take(8)?.try_into().unwrap());
        require(extent==0||extent==length,"NOFS declared extent differs")?;
        let mut offset=16;let mut visited=HashSet::new();
        for chain in 0..64 {
            require(visited.insert(offset),"Cyclic NOFS metadata index")?;
            let block=this.block(offset,if chain==0{0x1000}else{0x2000})?;
            require(block.len()>=18,"Truncated NOFS metadata index")?;
            let end=block.len()-12;let mut cursor=6;
            while cursor+12<=end {
                let key=u32::from_le_bytes(block[cursor..cursor+4].try_into().unwrap());
                let target=u64::from_le_bytes(block[cursor+4..cursor+12].try_into().unwrap());
                if target!=0 {
                    require((16..length).contains(&target),"Invalid NOFS metadata index target")?;
                    if let Some(old)=this.members.insert(key,target) {require(old==target,"NOFS indexed member collision")?;}
                }
                cursor+=12;
            }
            require(this.members.len()<=65536,"NOFS metadata index member limit")?;
            offset=u64::from_le_bytes(block[end..end+8].try_into().unwrap());
            if offset==0 {return Ok(this);}
        }
        Err(VoiceError::new("NOFS metadata index exceeds 64 blocks"))
    }
    fn member(&mut self,name:&[u8])->Result<Option<Vec<u8>>> {
        let mut key=0x811c9dc5u32;
        for byte in name {key=(key^((*byte as i8 as i32) as u32)).wrapping_mul(0x1000193);}
        let Some(&offset)=self.members.get(&key) else {return Ok(None);};
        let block=self.block(offset,1)?;let mut reader=Reader::new(&block[6..block.len()-4]);
        require(reader.string()?==name,"NOFS display/configuration member collision")?;
        let count=reader.u32()? as usize;let bytes=reader.take(count)?.to_vec();reader.end()?;
        require(self.file.metadata()?.len()==self.length,"NOFS changed during discovery")?;
        Ok(Some(bytes))
    }
    fn text(&mut self,name:&[u8])->String {
        // Display-only fields are optional. A bad footer or invalid UTF-8 must
        // not turn a positively classified STD bank into a neural bank, or
        // change its strict configuration/source admission.
        let value=(||->Result<String> {
            let raw=self.member(name)?.unwrap_or_default();
            require(raw.len()<=65536,"NOFS display metadata extent")?;
            String::from_utf8(raw).map(|s|s.trim_end_matches('\0').to_owned()).map_err(|_|VoiceError::new("Invalid NOFS display metadata UTF-8"))
        })();
        match value {Ok(text)=>text,Err(error)=> {
            self.warnings.push(format!("{}: {error}",String::from_utf8_lossy(name)));String::new()
        }}
    }
}
pub(super) fn inspect(path:&Path)->Result<Option<Discovered>> {
    // JSON/SVDB neural profiles must continue through the existing Flat route.
    if super::super::flat::is_profile(path) {return Ok(None);}
    // Only a positively decoded unit-source declaration claims this file.
    // Other NOFS layouts remain the responsibility of the existing neural
    // reader, including its own malformed/unsupported-file diagnostics.
    let mut metadata=match Metadata::open(path) {Ok(value)=>value,Err(_)=>return Ok(None)};
    let Some(raw)=metadata.member(&[0x91])? else {return Ok(None);};
    let config=match Configuration::parse(&raw) {Ok(value)=>value,Err(_)=>return Ok(None)};
    if !["group_mapdir","ac_ext","voc_ext"].iter().all(|key|config.contains(key)) {return Ok(None);}
    let configured_language=config.text("language")?;
    let configured_phoneset=config.text("phoneset")?;
    let name=metadata.text(b".name");
    let name=if name.is_empty(){path.parent().and_then(Path::file_name).unwrap_or_default().to_string_lossy().into_owned()}else{name};
    let mut version=metadata.text(b".version");
    if version.is_empty() {
        version=path.file_stem().and_then(|s|s.to_str()).and_then(|s|s.strip_prefix("voice.")).unwrap_or("0").to_owned();
    }
    let revision:VoiceRevision=version.parse().map_err(VoiceError::new)?;
    let vendor=metadata.text(b".vendor");
    let language=metadata.text(b".language");
    let phoneset=metadata.text(b".phoneset");
    let language=if language.is_empty(){configured_language.clone()}else{language};
    let phoneset=if phoneset.is_empty(){configured_phoneset.clone()}else{phoneset};
    let validation=(||->Result<()> {
        require(language==configured_language&&phoneset==configured_phoneset,"STD language/phone metadata disagrees with its source configuration")?;
        let encoded=config.flag("use_enc_only")?;
        require((encoded&&language=="japanese"&&phoneset=="romaji")||(!encoded&&language=="mandarin"&&phoneset=="xsampa"),"Only Japanese romaji encoder-only and Mandarin xsampa stored R2 STD banks are supported")?;
        require(config.flag("ac_enc")?&&config.flag("ac_thumb")?&&config.text("ac_ext")?=="lcod"&&config.text("voc_ext")?=="llsm"&&config.numbers("ac_order")?==[72.0],"R2 STD requires the supported LCOD72 source configuration")?;
        let models=config.strings("unitsynth")?;
        require(models.len()==2&&models.iter().all(|n|n.is_empty()==encoded),"Unsupported R2 STD source-model declaration")?;
        let mappings=config.strings("group_mapdir")?;
        require(!mappings.is_empty()&&mappings.len()<=12&&mappings.len()%3==0&&mappings.chunks_exact(3).all(|v|!v[0].is_empty()&&!v[1].is_empty()),"Unsupported R2 STD source-directory geometry")?;
        let hop=config.numbers("ac_thop")?;
        require(hop.len()==2&&hop.iter().all(|v|v.is_finite()&&*v>0.0)&&hop[0]<2147483648.0&&(hop[0] as i32)>0,"Unsupported R2 STD source clock")?;
        let nyquist=if encoded{22050.0}else{24000.0};
        for (key,expected) in [("voc_order_spec",64.0),("voc_order_bap",5.0),("voc_nspec",1025.0),("voc_npsd",256.0),("voc_fnyq",nyquist),("voc_nchannel",4.0),("voc_maxnhar_e",5.0),("voc_lipradius",1.5)] {
            require(config.numbers(key)?==[expected],&format!("Unsupported R2 STD decoder geometry: {key}"))?;
        }
        Ok(())
    })();
    Ok(Some(Discovered{path:path.canonicalize()?,data:None,name,vendor,revision,version,language,family:VoiceFamily::R2Std,
        unsupported:validation.err().map(|e|e.to_string()),warnings:metadata.warnings}))
}
