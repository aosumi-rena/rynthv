use super::{Arc, Graph, PhoneConversion, State, invalid};
use crate::synthesis::voice::Result;
use std::{collections::HashMap, io::Read};

struct Reader<'a> { bytes: &'a [u8], offset: usize }
impl Reader<'_> {
    fn byte(&mut self) -> Result<u8> {
        let value=*self.bytes.get(self.offset).ok_or_else(invalid)?;
        self.offset+=1;
        Ok(value)
    }
    fn integer(&mut self) -> Result<usize> {
        let mut value=0_usize;
        for shift in (0..35).step_by(7) {
            let byte=self.byte()?;
            if shift==28 && byte>7 {return Err(invalid())}
            value|=usize::from(byte&127)<<shift;
            if byte&128==0 {return Ok(value)}
        }
        Err(invalid())
    }
    fn string(&mut self) -> Result<String> {
        let len=self.integer()?;
        if len==0 || len>4096 {return Err(invalid())}
        let end=self.offset.checked_add(len).ok_or_else(invalid)?;
        let text=std::str::from_utf8(self.bytes.get(self.offset..end).ok_or_else(invalid)?).map_err(|_|invalid())?;
        self.offset=end;
        if text.contains('\0') {return Err(invalid())}
        Ok(text.to_owned())
    }
    fn graph(&mut self, symbols: usize) -> Result<Graph> {
        let count=self.integer()?;let start=self.integer()?;
        if count==0 || count>4096 || start>=count {return Err(invalid())}
        let mut states=Vec::with_capacity(count);let mut total=0_usize;
        for _ in 0..count {
            let cost=self.integer()?;let arc_count=self.integer()?;
            total=total.checked_add(arc_count).ok_or_else(invalid)?;
            if total>262144 {return Err(invalid())}
            let mut arcs=Vec::with_capacity(arc_count);
            for _ in 0..arc_count {
                let input=self.integer()?;let output=self.integer()?;let target=self.integer()?;let cost=self.integer()?;
                if input==0 || input>=symbols || output>=symbols || target>=count {return Err(invalid())}
                arcs.push(Arc{input,output,target,cost:cost as i32});
            }
            states.push(State{final_cost:(cost!=0).then_some(cost as i32-1),arcs});
        }
        Ok(Graph{start,states,members:HashMap::new()})
    }
}

impl PhoneConversion {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        use sha2::{Digest,Sha256};
        if bytes.len()!=8143 {return Err(invalid())}
        let identity=Sha256::digest(bytes).iter().map(|byte|format!("{byte:02x}")).collect::<String>();
        if identity!=Self::IDENTITY {return Err(invalid())}
        if bytes.len()<6 || bytes[..5]!=[0xc7,0x51,0x9f,0x3a,1] {return Err(invalid())}
        let mut decoder=flate2::read::ZlibDecoder::new(&bytes[5..]);
        let mut decoded=Vec::new();
        decoder.by_ref().take(29231).read_to_end(&mut decoded).map_err(|_|invalid())?;
        if decoded.len()!=29230 || decoder.total_in() as usize!=bytes.len()-5 {return Err(invalid())}
        let mut reader=Reader{bytes:&decoded,offset:0};
        let count=reader.integer()?;
        if count==0 || count>4096 {return Err(invalid())}
        let mut symbols=vec![String::new()];let mut ids=HashMap::new();
        for id in 1..=count {
            let symbol=reader.string()?;
            if ids.insert(symbol.clone(),id).is_some() {return Err(invalid())}
            symbols.push(symbol);
        }
        if reader.byte()?!=2 {return Err(invalid())}
        let mut graphs=[reader.graph(symbols.len())?,reader.graph(symbols.len())?];
        if reader.offset!=decoded.len() {return Err(invalid())}
        let boundary=*ids.get("#").ok_or_else(invalid)?;
        for graph in &mut graphs {graph.members(boundary,symbols.len());}
        Ok(Self{symbols,ids,graphs,boundary})
    }
}

impl Graph {
    fn members(&mut self, boundary: usize, symbols: usize) {
        for language in &self.states[self.start].arcs {
            let mut members=vec![false;symbols];let mut visited=vec![false;self.states.len()];
            let mut pending=self.states[language.target].arcs.iter().filter(|arc|arc.input==boundary).map(|arc|arc.target).collect::<Vec<_>>();
            while let Some(index)=pending.pop() {
                if std::mem::replace(&mut visited[index],true) {continue}
                for arc in &self.states[index].arcs {
                    if arc.input!=boundary {members[arc.input]=true;pending.push(arc.target);}
                }
            }
            self.members.insert(language.input,members);
        }
    }
}
