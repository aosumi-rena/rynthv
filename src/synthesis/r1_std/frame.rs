// SPDX-License-Identifier: MIT
use super::{word32::{StdNumericError,StdWord32},word64::{StdWideError,StdWord64},mix::mix,phase::phase,log2::log2,noise::{noise_power,noise_log}};

pub(crate) const MAX_WORDS: usize=504;
pub(crate) const MAX_NODES: usize=23;
pub(crate) const MAX_SOURCE_FRAMES: usize=100_000;
const MAX_DESTINATION_FRAMES: usize=8192;
const COPY_CAP: u64=16_777_216;
const LOW64:u64=0x3f84_7ae1_47ae_147b;
const HIGH64:u64=0x3fef_ae14_7ae1_47ae;

#[derive(Clone,Debug)]
pub(crate) enum FrameError {
    Shape {field:&'static str,actual:i64,limit:i64},
    Numeric(StdWideError),
    Capacity {actual:u64,limit:u64},
}
impl From<StdNumericError> for FrameError {fn from(e:StdNumericError)->Self{Self::Numeric(e.into())}}
impl From<StdWideError> for FrameError {fn from(e:StdWideError)->Self{Self::Numeric(e)}}
fn shape(ok:bool,field:&'static str,actual:i64,limit:i64)->Result<(),FrameError>{if ok{Ok(())}else{Err(FrameError::Shape{field,actual,limit})}}

#[derive(Clone,Debug)]
pub(crate) struct RawFrame {pub captured:bool,pub words:Vec<u32>,pub nodes:Vec<[i64;9]>}

#[derive(Clone,Copy,Debug)]
pub(crate) struct Origin {pub role:u8,pub a_frame:i32,pub a_offset:i32,pub b_frame:i32,pub b_offset:i32}
#[derive(Clone,Copy,Debug)]
pub(crate) struct Value {pub word:StdWord32,pub origin:Origin}
#[derive(Clone,Debug)]
pub(crate) struct Band {pub dc:Value,pub amplitudes:Vec<Value>,pub phases:Vec<Value>}
#[derive(Clone,Debug)]
pub(crate) struct Noise {pub scale:Value,pub psd:Vec<Value>,pub bands:Vec<Band>}
#[derive(Clone,Debug)]
pub(crate) struct Transition {pub rate:Value,pub kind:i32,pub scale:Value,pub values:Vec<Value>}
#[derive(Clone,Debug)]
pub(crate) struct SourceFrame {pub f0:Value,pub rd:Value,pub vt:Vec<Value>,pub phases:Vec<Value>,pub noise:Noise,pub transition:Option<Transition>,pub extra:Option<Value>}
#[derive(Clone,Debug)]
pub(crate) struct FrameOutput {pub raw:RawFrame,pub links:Vec<Origin>}
#[derive(Clone,Copy,Debug)]
pub(crate) struct FrameRequest {pub original:u32,pub effective:u32,pub ratio:u32}

#[derive(Clone,Debug)]
pub(crate) struct Probe {pub index:i32,pub bounded:bool,pub is_null:Option<bool>,pub advanced_to:Option<i32>}
#[derive(Clone,Debug,Default)]
pub(crate) struct GainState {
    pub owner:Option<i32>,pub ratio_wide:Option<u64>,pub one_minus:Option<u64>,pub contribution:Option<u32>,pub clamped:Option<u32>,pub log2_word:Option<u32>,pub ln_word:Option<u32>,pub widened:Option<u64>,pub scaled:Option<u64>,pub gain:Option<u32>,pub vt_count:usize,pub vt_completed:usize,
}
#[derive(Clone,Debug,Default)]
pub(crate) struct ExtraState {pub started:bool,pub completed:bool,pub a_null:bool,pub b_null:bool,pub default_a:bool,pub default_b:bool,pub selected_frame:Option<i32>,pub result:Option<u32>}
#[derive(Clone,Debug)]
pub(crate) struct CompletedValue {pub field:u8,pub band:u8,pub index:usize,pub value:Value}
#[derive(Clone,Debug,Default)]
pub(crate) struct RowWork {
    pub cursor:usize,pub request:Option<FrameRequest>,pub previous_word:Option<u32>,pub previous_floor:Option<i32>,pub current_floor:Option<i32>,pub begin:Option<i32>,pub probes:Vec<Probe>,pub terminal_index:Option<i32>,pub selected_frame:Option<i32>,pub copied_before:u64,pub copied_after:Option<u64>,pub transition_type:Option<i32>,pub type_source_node:Option<usize>,pub reset_completed:bool,pub branch:Option<u8>,pub gain:GainState,pub extra:ExtraState,pub completed_values:Vec<CompletedValue>,pub mix_calls:usize,pub phase_calls:usize,pub power_calls:usize,pub log_calls:usize,
}
impl RowWork {
    fn record(&mut self,field:u8,band:u8,index:usize,value:Value)->Result<Value,FrameError>{
        shape(self.completed_values.len()<MAX_WORDS,"partial.words",(self.completed_values.len()+1)as i64,MAX_WORDS as i64)?;
        self.completed_values.push(CompletedValue{field,band,index,value});Ok(value)
    }
}

fn literal(bits:u32)->Result<Value,FrameError>{Ok(Value{word:StdWord32::from_bits(bits)?,origin:Origin{role:1,a_frame:-1,a_offset:-1,b_frame:-1,b_offset:-1}})}
fn derived(word:StdWord32,a:Value,b:Value,role:u8)->Value{Value{word,origin:Origin{role,a_frame:a.origin.a_frame,a_offset:a.origin.a_offset,b_frame:b.origin.a_frame,b_offset:b.origin.a_offset}}}
fn mixed(a:Value,b:Value,ratio:StdWord32,w:&mut RowWork)->Result<Value,FrameError>{let v=mix(a.word,b.word,ratio)?;w.mix_calls+=1;Ok(derived(v,a,b,2))}
fn phased(a:Value,b:Value,ratio:StdWord32,w:&mut RowWork)->Result<Value,FrameError>{let v=phase(a.word,b.word,ratio)?;w.phase_calls+=1;Ok(derived(v,a,b,3))}

struct Assembler {words:Vec<u32>,nodes:Vec<[i64;9]>,links:Vec<Origin>}
impl Assembler {
    fn node(&mut self,n:[i64;9])->Result<usize,FrameError>{shape(self.nodes.len()<MAX_NODES,"output.nodes",(self.nodes.len()+1)as i64,MAX_NODES as i64)?;let at=self.nodes.len();self.nodes.push(n);Ok(at)}
    fn container(&mut self,kind:i64,parent:i64,ordinal:i64,nil:bool,length:usize)->Result<usize,FrameError>{self.node([kind,parent,ordinal,0,i64::from(nil),length as i64,0,0,0])}
    fn vector(&mut self,kind:i64,parent:i64,values:&[Value])->Result<(),FrameError>{
        let offset=self.words.len();let end=offset.checked_add(values.len()).ok_or(FrameError::Capacity{actual:u64::MAX,limit:MAX_WORDS as u64})?;
        shape(end<=MAX_WORDS,"output.words",end as i64,MAX_WORDS as i64)?;
        for v in values {self.words.push(v.word.bits());self.links.push(v.origin);}
        self.node([kind,parent,0,0,0,values.len()as i64,offset as i64,values.len()as i64,0])?;Ok(())
    }
}

pub(crate) fn assemble(s:&SourceFrame)->Result<FrameOutput,FrameError>{
    let mut a=Assembler{words:Vec::new(),nodes:Vec::new(),links:Vec::new()};
    let root=a.container(1,-1,0,false,0)? as i64;
    a.vector(2,root,&[s.f0])?;a.vector(3,root,&[s.rd])?;a.vector(4,root,&s.vt)?;a.vector(5,root,&s.phases)?;
    let noise=a.container(6,root,0,false,0)? as i64;a.vector(7,noise,&[s.noise.scale])?;a.vector(8,noise,&s.noise.psd)?;
    let bands=a.container(9,noise,0,false,s.noise.bands.len())? as i64;
    for(i,b)in s.noise.bands.iter().enumerate(){let parent=a.container(10,bands,i as i64,false,0)? as i64;a.vector(11,parent,&[b.dc])?;a.vector(12,parent,&b.amplitudes)?;a.vector(13,parent,&b.phases)?;}
    let t=a.container(14,root,0,s.transition.is_none(),0)? as i64;
    if let Some(v)=&s.transition {a.vector(15,t,&[v.rate])?;a.node([16,t,0,0,0,1,0,0,i64::from(v.kind)])?;a.vector(17,t,&[v.scale])?;a.vector(18,t,&v.values)?;}
    if let Some(v)=s.extra {a.vector(19,root,&[v])?;}else{a.container(19,root,0,true,0)?;}
    Ok(FrameOutput{raw:RawFrame{captured:true,words:a.words,nodes:a.nodes},links:a.links})
}

fn values(raw:&RawFrame,node:usize,frame:usize)->Result<Vec<Value>,FrameError>{
    let n=raw.nodes.get(node).ok_or(FrameError::Shape{field:"source.node.index",actual:node as i64,limit:raw.nodes.len()as i64})?;
    shape(n[6]>=0&&n[7]>=0,"source.offset.count",n[6].min(n[7]),0)?;
    let offset=n[6]as usize;let count=n[7]as usize;let end=offset.checked_add(count).ok_or(FrameError::Capacity{actual:u64::MAX,limit:MAX_WORDS as u64})?;
    shape(end<=raw.words.len(),"source.vector.extent",end as i64,raw.words.len()as i64)?;
    raw.words[offset..end].iter().enumerate().map(|(i,&bits)|Ok(Value{word:StdWord32::from_bits(bits)?,origin:Origin{role:0,a_frame:frame as i32,a_offset:(offset+i)as i32,b_frame:-1,b_offset:-1}})).collect()
}
fn scalar(raw:&RawFrame,node:usize,frame:usize)->Result<Value,FrameError>{let v=values(raw,node,frame)?;shape(v.len()==1,"source.scalar",v.len()as i64,1)?;Ok(v[0])}
fn bounded(v:&[Value],limit:u32)->Result<(),FrameError>{for x in v {shape(x.word.bits()&0x7fff_ffff<=limit,"source.magnitude",i64::from(x.word.bits()&0x7fff_ffff),i64::from(limit))?;}Ok(())}

pub(crate) fn normalize(raw:&RawFrame,frame:usize)->Result<SourceFrame,FrameError>{
    shape(raw.captured,"source.captured",i64::from(raw.captured),1)?;
    shape(frame<MAX_SOURCE_FRAMES,"source.frame.index",frame as i64,(MAX_SOURCE_FRAMES-1)as i64)?;
    shape(matches!(raw.nodes.len(),19|23),"source.node.count",raw.nodes.len()as i64,23)?;
    shape((29..=MAX_WORDS).contains(&raw.words.len()),"source.word.count",raw.words.len()as i64,MAX_WORDS as i64)?;
    for n in &raw.nodes {shape((0..=16384).contains(&n[3]),"source.alias",n[3],16384)?;}
    for &bits in &raw.words {StdWord32::from_bits(bits)?;}
    let h=raw.nodes[4][5];shape(matches!(h,0|100|101),"source.harmonics",h,101)?;
    shape(raw.nodes[3][5]==3*h,"source.vt.length",raw.nodes[3][5],3*h)?;
    let f0=scalar(raw,1,frame)?;let rd=scalar(raw,2,frame)?;let vt=values(raw,3,frame)?;let phases=values(raw,4,frame)?;
    let bits=f0.word.bits();shape(if h==0{bits&0x7fff_ffff==0}else{bits>0&&bits<0x7f80_0000},"source.f0.harmonics",i64::from(bits),h)?;
    let scale=scalar(raw,6,frame)?;let psd=values(raw,7,frame)?;shape(psd.len()==16,"source.psd.length",psd.len()as i64,16)?;
    let mut bands=Vec::new();
    for j in 0..2 {let at=10+4*j;let dc=scalar(raw,at,frame)?;let amplitudes=values(raw,at+1,frame)?;let phases=values(raw,at+2,frame)?;shape(matches!(amplitudes.len(),2|3)&&phases.len()==amplitudes.len(),"source.band.geometry",amplitudes.len()as i64,3)?;bounded(&[dc],0x3f80_0000)?;bounded(&amplitudes,0x3f80_0000)?;bounded(&phases,0x4080_0000)?;bands.push(Band{dc,amplitudes,phases});}
    let nil=raw.nodes[17][4];shape(matches!(nil,0|1),"source.transition.null",nil,1)?;
    shape(raw.nodes.len()==if nil==1{19}else{23},"source.transition.nodes",raw.nodes.len()as i64,if nil==1{19}else{23})?;
    let transition=if nil==1{None}else{
        let rate=scalar(raw,18,frame)?;let kind=i32::try_from(raw.nodes[19][8]).map_err(|_|FrameError::Shape{field:"source.transition.type",actual:raw.nodes[19][8],limit:i64::from(i32::MAX)})?;let scale=scalar(raw,20,frame)?;let wave=values(raw,21,frame)?;
        shape(wave.len()==64,"source.transition.values",wave.len()as i64,64)?;shape(rate.word.bits()>0&&rate.word.bits()<=0x47bb_8000,"source.transition.rate",i64::from(rate.word.bits()),0x47bb_8000)?;bounded(&[scale],0x3f80_0000)?;bounded(&wave,0x3f80_0000)?;Some(Transition{rate,kind,scale,values:wave})
    };
    let ei=if nil==1{18}else{22};let en=raw.nodes[ei][4];shape(matches!(en,0|1),"source.extra.null",en,1)?;let extra=if en==1{None}else{Some(scalar(raw,ei,frame)?)};
    bounded(&[f0],0x4480_0000)?;bounded(&[rd],0x4300_0000)?;bounded(&vt,0x4300_0000)?;bounded(&phases,0x4080_0000)?;bounded(&[scale],0x3f80_0000)?;bounded(&psd,0x4188_0000)?;if let Some(v)=extra{bounded(&[v],0x3f80_0000)?;}
    let semantic=SourceFrame{f0,rd,vt,phases,noise:Noise{scale,psd,bands},transition,extra};let canonical=assemble(&semantic)?;
    shape(canonical.raw.words==raw.words,"source.word.coverage",raw.words.len()as i64,canonical.raw.words.len()as i64)?;
    shape(canonical.raw.nodes.len()==raw.nodes.len(),"source.node.coverage",raw.nodes.len()as i64,canonical.raw.nodes.len()as i64)?;
    for i in 0..raw.nodes.len(){for j in 0..9{if j!=3{shape(canonical.raw.nodes[i][j]==raw.nodes[i][j],"source.canonical.cell",raw.nodes[i][j],canonical.raw.nodes[i][j])?;}}}
    Ok(semantic)
}

fn floor(bits:u32)->Result<i32,FrameError>{
    let word=StdWord32::from_bits(bits)?;
    shape(word.numeric_cmp(StdWord32::from_bits(0)?)!=std::cmp::Ordering::Less,
        "position.nonnegative.finite",i64::from(bits),0x7f7f_ffff)?;
    Ok(word.trunc_i32()?)
}
fn gain(owner:i32,ratio:StdWord32,w:&mut RowWork)->Result<StdWord32,FrameError>{
    w.gain.owner=Some(owner);
    let contribution=if owner==0 {let wide=StdWord64::widen(ratio)?;w.gain.ratio_wide=Some(wide.bits());let difference=StdWord64::from_bits(0x3ff0_0000_0000_0000)?.sub(wide)?;w.gain.one_minus=Some(difference.bits());difference.narrow()?}else{ratio};
    w.gain.contribution=Some(contribution.bits());let clamped=StdWord32::from_bits(0x2edb_e6ff)?.maximum(contribution);w.gain.clamped=Some(clamped.bits());let q=log2(clamped)?;w.gain.log2_word=Some(q.bits());let ln=q.mul(StdWord32::from_bits(0x3f31_7218)?)?;w.gain.ln_word=Some(ln.bits());let wide=StdWord64::widen(ln)?;w.gain.widened=Some(wide.bits());let product=wide.mul(StdWord64::from_bits(0x4021_5f2c_ec55_4b28)?)?;w.gain.scaled=Some(product.bits());let result=product.narrow()?;w.gain.gain=Some(result.bits());Ok(result)
}

pub(crate) struct FrameWindow {frames:Vec<SourceFrame>,next:usize,previous:Option<i32>,previous_word:Option<u32>,copied:u64}
impl FrameWindow {
    pub(crate) fn new(raw:&[RawFrame])->Result<Self,FrameError>{shape((2..=MAX_SOURCE_FRAMES).contains(&raw.len()),"context.frames",raw.len()as i64,MAX_SOURCE_FRAMES as i64)?;let mut frames=Vec::new();for(i,f)in raw.iter().enumerate(){frames.push(normalize(f,i)?);}Ok(Self{frames,next:0,previous:None,previous_word:None,copied:0})}
    pub(crate) fn next(&mut self,request:FrameRequest,w:&mut RowWork)->Result<FrameOutput,FrameError>{
        shape(self.next<MAX_DESTINATION_FRAMES,"window.rows",self.next as i64,MAX_DESTINATION_FRAMES as i64)?;*w=RowWork::default();w.cursor=self.next;w.request=Some(request);w.copied_before=self.copied;
        let current=floor(request.effective)?;shape(current>=0&&(current as usize)+1<self.frames.len(),"window.neighbor",i64::from(current),(self.frames.len()-2)as i64)?;
        let ratio=StdWord32::from_bits(request.ratio)?;
        shape(ratio.numeric_cmp(StdWord32::from_bits(0)?)!=std::cmp::Ordering::Less
            &&ratio.numeric_cmp(StdWord32::from_bits(0x3f80_0000)?)!=std::cmp::Ordering::Greater,
            "ratio.nonnegative.unit",i64::from(request.ratio),0x3f80_0000)?;
        let prior_word=self.previous_word.unwrap_or(request.original);let previous=if let Some(p)=self.previous{p}else{floor(prior_word)?};w.previous_word=Some(prior_word);w.previous_floor=Some(previous);w.current_floor=Some(current);
        let mut j=previous.checked_add(1).ok_or(FrameError::Capacity{actual:u64::MAX,limit:i32::MAX as u64})?;w.begin=Some(j);let mut transition=None;
        while j<=current {
            shape(w.probes.len()<self.frames.len(),"scan.probes",(w.probes.len()+1)as i64,self.frames.len()as i64)?;let bounded=j>=0&&(j as usize)<self.frames.len();w.probes.push(Probe{index:j,bounded,is_null:None,advanced_to:None});shape(bounded,"scan.index",i64::from(j),(self.frames.len()-1)as i64)?;
            let at=w.probes.len()-1;let found=&self.frames[j as usize].transition;w.probes[at].is_null=Some(found.is_none());
            if let Some(t)=found {
                w.selected_frame=Some(j);let length=t.values.len()as u64;let total=self.copied.checked_add(length).ok_or(FrameError::Capacity{actual:u64::MAX,limit:COPY_CAP})?;
                if total>COPY_CAP{return Err(FrameError::Capacity{actual:total,limit:COPY_CAP});}self.copied=total;w.transition_type=Some(t.kind);w.type_source_node=Some(19);
                w.record(10,0,0,t.rate)?;w.record(11,0,0,t.scale)?;for(i,&v)in t.values.iter().enumerate(){w.record(12,0,i,v)?;}transition=Some(t.clone());break;
            }
            let next=j.checked_add(1).ok_or(FrameError::Capacity{actual:u64::MAX,limit:i32::MAX as u64})?;w.probes[at].advanced_to=Some(next);j=next;
        }
        if transition.is_none(){w.terminal_index=Some(j);}self.previous=Some(current);self.previous_word=Some(request.effective);self.next+=1;w.reset_completed=true;w.copied_after=Some(self.copied);
        let a=&self.frames[current as usize];let b=&self.frames[current as usize+1];let wide=StdWord64::widen(ratio)?.bits();let az=a.f0.word.bits()&0x7fff_ffff==0;let bz=b.f0.word.bits()&0x7fff_ffff==0;
        let branch=if wide<LOW64{1}else if wide>HIGH64{2}else if az&&bz{3}else if az||bz{4}else{5};w.branch=Some(branch);
        w.extra.a_null=a.extra.is_none();w.extra.b_null=b.extra.is_none();
        if branch==1||branch==2 {
            let mut selected=if branch==1{a.clone()}else{b.clone()};selected.transition=transition;record_nontransition(&selected,w)?;w.extra.started=true;w.extra.completed=true;w.extra.selected_frame=Some(if branch==1{current}else{current+1});w.extra.result=selected.extra.map(|v|v.word.bits());return assemble(&selected);
        }
        let mut result=SourceFrame{f0:literal(0)?,rd:literal(0)?,vt:Vec::new(),phases:Vec::new(),noise:Noise{scale:literal(0)?,psd:Vec::new(),bands:Vec::new()},transition,extra:None};
        if branch==5 {
            let value=mixed(a.f0,b.f0,ratio,w)?;result.f0=w.record(1,0,0,value)?;
            let value=mixed(a.rd,b.rd,ratio,w)?;result.rd=w.record(2,0,0,value)?;
            result.vt=harmonics(&a.vt,&b.vt,ratio,false,3,w)?;result.phases=harmonics(&a.phases,&b.phases,ratio,true,4,w)?;
        }else{
            let owner=if branch==3||!az{a}else{b};let owner_side=if branch==3||!az{0}else{1};
            if branch==3{result.f0=w.record(1,0,0,literal(0)?)?;result.rd=w.record(2,0,0,literal(0x3f80_0000)?)?;}else{let g=gain(owner_side,ratio,w)?;w.gain.vt_count=owner.vt.len();result.f0=w.record(1,0,0,owner.f0)?;result.rd=w.record(2,0,0,owner.rd)?;for(i,&v)in owner.vt.iter().enumerate(){let word=v.word.add(g)?;let value=derived(word,v,v,4);result.vt.push(w.record(3,0,i,value)?);w.gain.vt_completed+=1;}}
            if branch==3{for(i,&v)in owner.vt.iter().enumerate(){result.vt.push(w.record(3,0,i,v)?);}}
            for(i,&v)in owner.phases.iter().enumerate(){result.phases.push(w.record(4,0,i,v)?);}
        }
        result.noise.scale=w.record(5,0,0,a.noise.scale)?;
        for i in 0..16 {let x=a.noise.psd[i];let y=b.noise.psd[i];let p=noise_power(x.word)?;w.power_calls+=1;let q=noise_power(y.word)?;w.power_calls+=1;let power=mix(StdWord32::from_bits(p)?,StdWord32::from_bits(q)?,ratio)?;w.mix_calls+=1;let word=noise_log(power)?;w.log_calls+=1;result.noise.psd.push(w.record(6,0,i,derived(word,x,y,5))?);}
        for j in 0..2 {
            let x=&a.noise.bands[j];let y=&b.noise.bands[j];let dc=mixed(x.dc,y.dc,ratio,w)?;let mut z=Band{dc:w.record(7,j as u8,0,dc)?,amplitudes:Vec::new(),phases:Vec::new()};let shared=x.amplitudes.len().min(y.amplitudes.len());
            for k in 0..shared {let amp=mixed(x.amplitudes[k],y.amplitudes[k],ratio,w)?;z.amplitudes.push(w.record(8,j as u8,k,amp)?);let ph=phased(x.phases[k],y.phases[k],ratio,w)?;z.phases.push(w.record(9,j as u8,k,ph)?);}
            let longer=if x.amplitudes.len()>y.amplitudes.len(){x}else{y};
            for k in shared..longer.amplitudes.len(){z.amplitudes.push(w.record(8,j as u8,k,longer.amplitudes[k])?);z.phases.push(w.record(9,j as u8,k,longer.phases[k])?);}result.noise.bands.push(z);
        }
        w.extra.started=true;
        if branch==3||branch==4 {let owner=if branch==3||!az{a}else{b};w.extra.selected_frame=Some(if branch==3||!az{current}else{current+1});result.extra=owner.extra;if let Some(v)=result.extra{w.record(13,0,0,v)?;}}
        else if a.extra.is_some()||b.extra.is_some(){w.extra.default_a=a.extra.is_none();w.extra.default_b=b.extra.is_none();let x=if let Some(v)=a.extra{v}else{literal(0)?};let y=if let Some(v)=b.extra{v}else{literal(0)?};let value=mixed(x,y,ratio,w)?;result.extra=Some(w.record(13,0,0,value)?);}
        w.extra.result=result.extra.map(|v|v.word.bits());w.extra.completed=true;assemble(&result)
    }
}

fn harmonics(a:&[Value],b:&[Value],ratio:StdWord32,is_phase:bool,field:u8,w:&mut RowWork)->Result<Vec<Value>,FrameError>{
    let common=a.len().min(b.len());let mut out=Vec::new();
    for i in 0..common {let value=if is_phase{phased(a[i],b[i],ratio,w)?}else{mixed(a[i],b[i],ratio,w)?};out.push(w.record(field,0,i,value)?);}
    let longer=if a.len()>b.len(){a}else{b};for(i,&v)in longer.iter().enumerate().skip(common){out.push(w.record(field,0,i,v)?);}Ok(out)
}
fn record_nontransition(s:&SourceFrame,w:&mut RowWork)->Result<(),FrameError>{
    w.record(1,0,0,s.f0)?;w.record(2,0,0,s.rd)?;for(i,&v)in s.vt.iter().enumerate(){w.record(3,0,i,v)?;}for(i,&v)in s.phases.iter().enumerate(){w.record(4,0,i,v)?;}w.record(5,0,0,s.noise.scale)?;for(i,&v)in s.noise.psd.iter().enumerate(){w.record(6,0,i,v)?;}
    for(j,b)in s.noise.bands.iter().enumerate(){w.record(7,j as u8,0,b.dc)?;for(i,&v)in b.amplitudes.iter().enumerate(){w.record(8,j as u8,i,v)?;}for(i,&v)in b.phases.iter().enumerate(){w.record(9,j as u8,i,v)?;}}
    if let Some(v)=s.extra{w.record(13,0,0,v)?;}Ok(())
}
