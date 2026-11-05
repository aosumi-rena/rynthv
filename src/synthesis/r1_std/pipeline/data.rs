use super::{Result,ensure,StdError,reader::Reader};
use std::{path::Path,sync::atomic::AtomicBool};
#[derive(Clone,Debug)]
pub(super) struct Config {pub hop:f32,pub harmonics:usize,pub envelope:usize,pub bins:usize,pub warp:f32,pub nyquist:f32,pub edges:Vec<f32>,pub lip:f32}
#[derive(Clone,Debug)]
pub(super) struct Band {pub dc:f32,pub amplitudes:Vec<f32>,pub phases:Vec<f32>}
#[derive(Clone,Debug)]
pub(super) struct Noise {pub scale:f32,pub psd:Vec<f32>,pub bands:Vec<Band>}
impl Noise {pub fn empty(c:&Config)->Self{Self{scale:0.,psd:vec![-120.;c.bins],bands:(0..=c.edges.len()).map(|_|Band{dc:1e-5,amplitudes:vec![0.;c.envelope],phases:vec![0.;c.envelope]}).collect()}}}
#[derive(Clone,Debug)]
pub(super) struct Wave {pub rate:f32,pub offset:f32,pub values:Vec<f32>}
#[derive(Clone,Debug)]
pub(super) struct Frame {pub f0:f32,pub rd:f32,pub vt:Vec<f32>,pub phases:Vec<f32>,pub noise:Noise,pub wave:Option<Wave>,pub contour:f32}
impl Frame {pub fn empty(c:&Config)->Self{Self{f0:0.,rd:1.,vt:vec![],phases:vec![],noise:Noise::empty(c),wave:None,contour:0.}}}
#[derive(Clone,Debug)]
pub(super) struct Map {pub x:Vec<f32>,pub y:Vec<f32>}
impl Map {
 fn read(r:&mut Reader,n:usize)->Result<Self>{let mut x=Vec::with_capacity(n);let mut y=Vec::with_capacity(n);for _ in 0..n{x.push(r.f32()?);y.push(r.f32()?);}
  ensure(n>=2&&x.windows(2).all(|w|w[0]<w[1]),"Invalid R1 timbre map knots")?;Ok(Self{x,y})}
 pub fn at(&self,q:f32)->f32{if q<=self.x[0]{return self.y[0];}let i=self.x.partition_point(|x|*x<q).min(self.x.len()-1);if q>=self.x[i]{return self.y[i];}
  self.y[i-1]+(q-self.x[i-1])*(self.y[i]-self.y[i-1])/(self.x[i]-self.x[i-1])}
}
#[derive(Clone,Debug)]
pub(super) struct Transform {pub rd:Map,pub noise:Map,pub spectrum:Map,pub scale:f32}
impl Transform {fn read(r:&mut Reader)->Result<Self>{let a=r.count(4096)?;let b=r.count(4096)?;let c=r.count(4096)?;
 let out=Self{rd:Map::read(r,a)?,noise:Map::read(r,b)?,spectrum:Map::read(r,c)?,scale:r.f32()?};ensure(out.scale>0.,"Nonpositive R1 timbre scale")?;Ok(out)}}
#[derive(Clone,Debug)]
pub(super) struct TimbreClass {pub f0:f32,pub positions:Map,pub targets:Vec<Transform>}
#[derive(Clone,Debug)]
pub(super) struct Timbre {pub base:Transform,pub classes:Vec<TimbreClass>}
pub(super) struct DataBank {bytes:Vec<u8>,pub config:Config,pub units:Vec<Vec<usize>>,ends:Vec<usize>,pub timbre:Option<Timbre>}
impl DataBank {
 pub fn load(path:&Path)->Result<Self>{let length=std::fs::metadata(path)?.len();ensure(length<=1024*1024*1024,"R1 data exceeds capacity")?;let bytes=std::fs::read(path)?;let mut r=Reader::new(&bytes);
  ensure(r.take(4)?==b"s5d ","Invalid R1 data magic")?;let version=r.u32()?;ensure((1..=9).contains(&version),"Unsupported R1 data version")?;r.take(24)?;let count=r.count(1_000_000)?;let flags=r.u32()?;r.take(32)?;
  let hop=r.f32()?;let harmonics=r.count(512)?;let envelope=r.count(32)?;let bins=r.count(512)?;let warp=r.f32()?;let nyquist=r.f32()?;let channels=r.count(8)?;
  ensure((0.001..=0.02).contains(&hop)&&(8000.0..=48000.0).contains(&nyquist)&&channels>=2&&harmonics>0&&bins>=4&&warp>0.,"Unsupported R1 synthesis geometry")?;
  let edges=r.floats(channels-1)?;ensure(edges.first().is_some_and(|x|*x>0.)&&edges.last().is_some_and(|x|*x<nyquist)&&edges.windows(2).all(|w|w[0]<w[1]),"Invalid R1 noise bands")?;
  let config=Config{hop,harmonics,envelope,bins,warp,nyquist,edges,lip:r.f32()?};ensure(config.lip>0.,"Invalid R1 lip radius")?;
  let timbre=if flags&255!=0 {let present=r.byte()?;ensure(present==1,"R1 timbre base transform is missing")?;let base=Transform::read(&mut r)?;
   let n=r.count(128)?;let mut classes=Vec::with_capacity(n);for _ in 0..n{let f0=r.f32()?;let count=r.count(128)?;ensure(f0>=30.&&count>=2,"Unsupported R1 timbre class")?;
    let(mut x,mut y,mut targets)=(Vec::new(),Vec::new(),Vec::new());for i in 0..count{x.push(r.f32()?);y.push(i as f32);targets.push(Transform::read(&mut r)?);}
    ensure(x.windows(2).all(|w|w[0]<w[1]),"Unordered R1 timbre targets")?;classes.push(TimbreClass{f0,positions:Map{x,y},targets});}
   ensure(!classes.is_empty(),"R1 timbre lacks classes")?;Some(Timbre{base,classes})}else{None};
  let base=r.at;let positions=(0..count).map(|_|r.u32().map(|v|base+v as usize)).collect::<Result<Vec<_>>>()?;
  ensure(positions.windows(2).all(|w|w[0]<w[1]),"Unordered R1 unit directory")?;let mut units=Vec::with_capacity(count);let mut ends=Vec::with_capacity(count);
  for (i,start)in positions.iter().copied().enumerate(){let end=positions.get(i+1).copied().unwrap_or(bytes.len());ensure(start>=base+count*4&&start<end&&end<=bytes.len(),"R1 unit outside data")?;
   r.at=start;let frames=r.count(100_000)?;let offsets=(0..frames).map(|_|r.u32().map(|v|start+v as usize)).collect::<Result<Vec<_>>>()?;
   ensure(offsets.first().copied()==Some(r.at)&&offsets.windows(2).all(|w|w[0]<w[1])&&offsets.last().is_some_and(|x|*x<end),"Invalid R1 frame directory")?;units.push(offsets);ends.push(end);}
  Ok(Self{bytes,config,units,ends,timbre})
 }
 pub fn decode(&self,unit:usize,cancel:&AtomicBool)->Result<Vec<Frame>>{
  let offsets=self.units.get(unit).ok_or_else(||StdError::Invalid("Unknown R1 unit".into()))?;let mut out=Vec::with_capacity(offsets.len());let mut budget=0usize;
  for (i,start)in offsets.iter().copied().enumerate(){super::cancel(cancel)?;let end=offsets.get(i+1).copied().unwrap_or(self.ends[unit]);let mut r=Reader::new(&self.bytes[start..end]);
   let transition=r.byte()?;let extra=r.byte()?;ensure(transition<=1&&extra<=1,"Invalid R1 frame flags")?;
   let f0=r.f32()?;let rd=r.f32()?;let n=r.u16()? as usize;ensure(n<=self.config.harmonics,"R1 harmonic count exceeds configuration")?;
   let vt=if n==0{vec![]}else{delta(&mut r,n*3)?};let phases=(0..n).map(|_|r.u16().map(|v|((v as i16 as f64)/32767.*std::f64::consts::PI)as f32)).collect::<Result<Vec<_>>>()?;
   let spectral=r.byte()?;ensure(spectral<=1,"Invalid R1 spectral flag")?;
   let noise=if spectral==1{Noise::empty(&self.config)}else{let channels=self.config.edges.len()+1;let counts=(0..channels).map(|_|r.byte().map(usize::from)).collect::<Result<Vec<_>>>()?;
    ensure(counts.iter().all(|n|*n<=self.config.envelope),"R1 noise harmonic count exceeds configuration")?;let scale=r.f32()?;let psd=delta(&mut r,self.config.bins)?;
    let dc=(0..channels).map(|_|r.u16().map(|v|(v as f64/65535.*scale as f64)as f32)).collect::<Result<Vec<_>>>()?;
    let mut bands=Vec::with_capacity(channels);for j in 0..channels{let amplitudes=(0..counts[j]).map(|_|r.u16().map(|v|(v as f64/65535.*scale as f64)as f32)).collect::<Result<Vec<_>>>()?;
     let phases=(0..counts[j]).map(|_|r.byte().map(|v|((v as i8 as f64)/127.*std::f64::consts::PI)as f32)).collect::<Result<Vec<_>>>()?;bands.push(Band{dc:dc[j],amplitudes,phases});}Noise{scale,psd,bands}};
   let wave=if transition!=0{let n=r.count(16_777_216)?;let rate=r.f32()?;let offset=f32::from_bits(r.u32()?);let scale=r.f32()?;
    ensure(rate>0.&&offset.is_finite(),"Invalid R1 stored wave clock")?;let values=(0..n).map(|_|r.u16().map(|v|(v as i16 as f64/32767.*scale as f64)as f32)).collect::<Result<Vec<_>>>()?;Some(Wave{rate,offset,values})}else{None};
   let contour=if extra!=0{r.f32()?}else{0.};ensure(r.at==end-start,"R1 frame did not consume its exact extent")?;
   budget+=vt.len()+phases.len()+noise.psd.len()+noise.bands.iter().map(|b|1+b.amplitudes.len()+b.phases.len()).sum::<usize>()+wave.as_ref().map_or(0,|w|w.values.len());ensure(budget<=16_777_216,"R1 decoded unit exceeds capacity")?;
   out.push(Frame{f0,rd,vt,phases,noise,wave,contour});}Ok(out)
 }
}
fn delta(r:&mut Reader,n:usize)->Result<Vec<f32>>{ensure(n>0&&n<=12288,"Invalid R1 delta extent")?;let bias=r.f32()?;let scale=r.f32()?;let first=r.f32()?;let step=0.0078125_f32*scale;
 let mut v=Vec::with_capacity(n);v.push(first);for i in 1..n{let code=r.byte()? as i8;let x=(v[i-1]+code as f32*step)+bias;ensure(x.is_finite(),"R1 delta overflow")?;v.push(x);}Ok(v)}
