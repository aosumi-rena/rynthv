use super::{StdBankPaths,Result,ensure,StdError,reader::Reader,models::{Qsa,Kg},data::DataBank,language::Language};
use std::{collections::HashMap,sync::{Arc,Mutex}};
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub(super) struct Address {pub entry:usize,pub syllable:usize,pub phone:usize}
#[derive(Clone)]
pub(super) struct Syllable {pub phones:Vec<usize>,pub start:Vec<f32>,pub end:Vec<f32>,pub onset_start:Vec<f32>,pub onset_end:Vec<f32>,pub blend:Vec<f32>,pub enabled:Vec<bool>,pub features:Option<Vec<Vec<f32>>>,pub encodings:Option<Vec<Vec<f32>>>}
pub(crate) struct StdBank {pub(super) paths:StdBankPaths,pub(super) entries:Vec<Vec<Syllable>>,pub(super) phones:Vec<(String,String)>,
 pub(super) selectors:HashMap<String,usize>,pub(super) silence:usize,pub(super) qsa:Qsa,pub(super) kg:Option<Kg>,pub(super) rules:Vec<Rule>,pub(super) data:DataBank,pub(super) language:Language,pub(super) encoding_cache:Mutex<HashMap<[usize;3],Arc<super::models::Encoding>>>,pub(super) pools:Vec<Vec<Address>>}
pub(super) struct Rule {pub ops:[u16;6],pub arguments:[u16;6],pub replace:[u16;3],pub values:[f32;3]}
impl StdBank {
 pub fn output_rate(&self)->u32 {(self.data.config.nyquist*2.0) as u32}
 pub fn frame_seconds(&self)->f64{self.data.config.hop as f64}
 pub fn load(paths:StdBankPaths)->Result<Self>{
  let file=if paths.language_dir.join("japanese-romaji.txt").is_file(){"japanese-romaji.txt"}else{"japanese-romaji-phones.txt"};
  let table=std::fs::read_to_string(paths.language_dir.join(file))?;
  let mut phones=Vec::new();let mut selectors=HashMap::new();
  for line in table.lines().filter(|line|!line.trim().is_empty()){let fields=line.trim_start_matches('\u{feff}').split_whitespace().collect::<Vec<_>>();ensure(fields.len()>=2&&!fields[0].is_empty(),"Invalid R1 ordered phone resource")?;
   ensure(selectors.insert(fields[0].to_owned(),phones.len()).is_none(),"Duplicate R1 phone resource identity")?;phones.push((fields[0].to_owned(),fields[1].to_owned()));}
  let silence=*selectors.get("sil").ok_or_else(||StdError::Invalid("R1 phone resource lacks sil".into()))?;
  let bytes=std::fs::read(&paths.metadata)?;ensure(bytes.len()<=128*1024*1024,"R1 metadata exceeds capacity")?;let mut r=Reader::new(&bytes);
  ensure(r.take(4)?==b"s5l "&&r.u32()?==3,"Unsupported R1 metadata format")?;r.take(24)?;
  let count=r.count(1_000_000)?;let contexts=r.count(1_000_000)?;
  ensure(r.name()?=="japanese"&&r.name()?=="romaji","R1 bank requires Japanese romaji resources")?;
  let flags=r.take(4)?.to_vec();ensure(flags.iter().all(|b|*b<=1),"Invalid R1 metadata flags")?;
  if flags[3]!=0||contexts!=0{return Err(StdError::Unsupported("Grouped or linked-classifier R1 bank is not supported by this frontend".into()));}
  r.take(32)?;let mut entries=Vec::with_capacity(count);let mut total=0usize;
  for _ in 0..count {let n=r.count(65536)?;let mut entry=Vec::with_capacity(n);for _ in 0..n {
   let n=r.count(500)?;total+=n;ensure(total<=16_777_216,"R1 metadata phone capacity exceeded")?;
   let ids=r.words(n)?.into_iter().map(usize::from).collect::<Vec<_>>();ensure(ids.iter().all(|i|*i<phones.len()),"R1 stored phone outside dictionary")?;
   let(start,end,onset_start,onset_end,blend)=(r.floats(n)?,r.floats(n)?,r.floats(n)?,r.floats(n)?,r.floats(n)?);
   let enabled=r.take(n)?.iter().map(|b|*b!=0).collect();
   fn vectors(r:&mut Reader,n:usize)->Result<Vec<Vec<f32>>>{(0..n).map(|_|{let k=r.count(4096)?;r.floats(k)}).collect()}
   let encodings=if flags[0]!=0{Some(vectors(&mut r,n)?)}else{None};let features=if flags[1]!=0{Some(vectors(&mut r,n)?)}else{None};
   entry.push(Syllable{phones:ids,start,end,onset_start,onset_end,blend,enabled,features,encodings});}entries.push(entry);}
  let qsa=Qsa::read(&r.blob()?)?;ensure(qsa.selectors==phones.len(),"R1 bank model and phone dictionary orders disagree")?;
  let kg=if flags[1]!=0{Some(Kg::read(&r.blob()?)?)}else{None};let mut rules=Vec::new();
  if flags[2]!=0{let n=r.count(65536)?;let patterns=r.take(n*24)?.to_vec();let mut p=Reader::new(&patterns);
   for _ in 0..n{let mut ops=[0;6];let mut arguments=[0;6];for v in &mut ops{*v=p.u16()?.min(12);}for v in &mut arguments{*v=p.u16()?;}
    let mut replace=[0;3];let mut values=[0.;3];for v in &mut replace{*v=r.u16()?;}for v in &mut values{*v=r.f32()?;}rules.push(Rule{ops,arguments,replace,values});}}
  ensure(r.at==bytes.len(),"Unconsumed R1 metadata bytes")?;
  let data=DataBank::load(&paths.data)?;ensure(data.units.len()==entries.len(),"R1 metadata/data entry mismatch")?;
  let language=Language::load(&paths.language_dir,&selectors)?;
  let mut pools=vec![Vec::new();phones.len()];
  for(e,entry)in entries.iter().enumerate(){for(s,syllable)in entry.iter().enumerate(){for p in 0..syllable.phones.len(){
   let prior=if p>0{syllable.enabled[p-1]}else if s>0{entry[s-1].enabled.last().copied().unwrap_or(false)}else{true};
   let next=if p+1<syllable.phones.len(){syllable.enabled[p+1]}else{entry.get(s+1).and_then(|s|s.enabled.first()).copied().unwrap_or(true)};
   if syllable.enabled[p]&&prior&&next{pools[syllable.phones[p]].push(Address{entry:e,syllable:s,phone:p});}
  }}}
  Ok(Self{paths,entries,phones,selectors,silence,qsa,kg,rules,data,language,pools,encoding_cache:Mutex::new(HashMap::new())})
 }
 pub(super) fn syllable(&self,a:Address)->&Syllable{&self.entries[a.entry][a.syllable]}
 pub(super) fn phone(&self,a:Address)->usize{self.syllable(a).phones[a.phone]}
 pub(super) fn category(&self,p:usize)->&str{&self.phones[p].1}
 pub(super) fn neighbor(&self,a:Address,forward:bool)->Option<Address>{let s=self.syllable(a);
  if forward {if a.phone+1<s.phones.len(){Some(Address{phone:a.phone+1,..a})}else if a.syllable+1<self.entries[a.entry].len()&&!self.entries[a.entry][a.syllable+1].phones.is_empty(){Some(Address{syllable:a.syllable+1,phone:0,..a})}else{None}}
  else if a.phone>0{Some(Address{phone:a.phone-1,..a})}else if a.syllable>0{let n=self.entries[a.entry][a.syllable-1].phones.len();(n>0).then_some(Address{syllable:a.syllable-1,phone:n.saturating_sub(1),..a})}else{None}}
 pub(super) fn context(&self,a:Address)->[usize;3]{[self.neighbor(a,false).map(|p|self.phone(p)).unwrap_or(self.silence),self.phone(a),self.neighbor(a,true).map(|p|self.phone(p)).unwrap_or(self.silence)]}
}
