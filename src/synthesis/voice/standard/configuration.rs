//! Bounded, metadata-only STD configuration decoding.
use std::collections::BTreeMap;
use super::super::{Result,VoiceError};

pub(super) fn require(ok:bool,message:&str)->Result<()> {
    if ok {Ok(())} else {Err(VoiceError::new(message))}
}
pub(super) struct Reader<'a> {bytes:&'a[u8],at:usize}
impl<'a> Reader<'a> {
    pub fn new(bytes:&'a[u8])->Self {Self {bytes,at:0}}
    pub fn take(&mut self,n:usize)->Result<&'a[u8]> {
        let end=self.at.checked_add(n).filter(|n|*n<=self.bytes.len())
            .ok_or_else(||VoiceError::new("Truncated STD metadata"))?;
        let result=&self.bytes[self.at..end];self.at=end;Ok(result)
    }
    pub fn u16(&mut self)->Result<u16> {Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))}
    pub fn u32(&mut self)->Result<u32> {Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))}
    pub fn string(&mut self)->Result<&'a[u8]> {let n=usize::from(self.u16()?);self.take(n)}
    pub fn end(&self)->Result<()> {require(self.at==self.bytes.len(),"Trailing STD metadata")}
}
enum Value {Strings(Vec<Vec<u8>>),Numbers(Vec<f64>),Flags(Vec<u32>)}
pub(super) struct Configuration(BTreeMap<String,Value>);
impl Configuration {
    pub fn parse(bytes:&[u8])->Result<Self> {
        require(bytes.len()<=1_048_576,"STD configuration exceeds 1 MiB")?;
        let mut reader=Reader::new(bytes);
        require(reader.u32()?==0xfeff,"Invalid STD configuration header")?;
        let count=reader.u32()?;require(count<=4096,"STD configuration row limit")?;
        let mut fields=BTreeMap::new();
        for row in 0..count {
            let kind=reader.u32()?;
            require(kind<=2&&reader.u32()?==row,"Invalid STD configuration row")?;
            let mut state=0xbefcb5e0u32;
            let clear:Vec<u8>=reader.string()?.iter().map(|byte| {
                let value=byte^(state as u8);state=state.wrapping_mul(0x41c64e6d).wrapping_add(0x3039);value
            }).collect();
            let key=String::from_utf8(clear).map_err(|_|VoiceError::new("Invalid STD configuration key"))?;
            let n=reader.u32()?;require(n<=4096,"STD configuration value limit")?;
            let value=match kind {
                0=>Value::Strings((0..n).map(|_|reader.string().map(<[u8]>::to_vec)).collect::<Result<_>>()?),
                1=>Value::Numbers((0..n).map(|_|reader.take(8).map(|b|f64::from_le_bytes(b.try_into().unwrap()))).collect::<Result<_>>()?),
                _=>Value::Flags((0..n).map(|_|reader.u32()).collect::<Result<_>>()?),
            };
            if let Some(old)=fields.get_mut(&key) {
                match (old,value) {
                    (Value::Strings(a),Value::Strings(b))=>a.extend(b),
                    (Value::Numbers(a),Value::Numbers(b))=>a.extend(b),
                    (Value::Flags(a),Value::Flags(b))=>a.extend(b),
                    _=>return Err(VoiceError::new("STD configuration repeated key changes type")),
                }
            } else {fields.insert(key,value);}
        }
        reader.end()?;Ok(Self(fields))
    }
    pub fn contains(&self,key:&str)->bool {self.0.contains_key(key)}
    pub fn strings(&self,key:&str)->Result<&[Vec<u8>]> {match self.0.get(key) {
        Some(Value::Strings(values))=>Ok(values),_=>Err(VoiceError::new(format!("Missing STD string configuration: {key}")))}}
    pub fn text(&self,key:&str)->Result<String> {
        let values=self.strings(key)?;require(values.len()==1,"STD text cardinality")?;
        String::from_utf8(values[0].clone()).map_err(|_|VoiceError::new("Invalid STD configuration text"))
    }
    pub fn numbers(&self,key:&str)->Result<&[f64]> {match self.0.get(key) {
        Some(Value::Numbers(values))=>Ok(values),_=>Err(VoiceError::new(format!("Missing STD numeric configuration: {key}")))}}
    pub fn flag(&self,key:&str)->Result<bool> {match self.0.get(key) {
        Some(Value::Flags(values)) if values.len()==1=>Ok(values[0]&1!=0),_=>Err(VoiceError::new(format!("Missing STD flag: {key}")))}}
}
