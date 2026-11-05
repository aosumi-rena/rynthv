use super::{Result,StdError,ensure,data::{Config,Frame}};
use std::{ffi::c_void,path::Path,sync::Mutex};
static RENDER_LOCK:Mutex<()>=Mutex::new(());
mod waves;
mod callback;
#[repr(C)]
struct NativeConfig {abi:i32,frames:i32,harmonics:i32,envelope:i32,bins:i32,bands:i32,hop:f32,nyquist:f32,lip:f32}
type Samples=unsafe extern "C" fn(*const NativeConfig)->i32;
type Callback=unsafe extern "C" fn(*mut c_void,i32,i32,*const f32,*const f32)->i32;
type Render=unsafe extern "C" fn(*const NativeConfig,*const f32,i32,*mut f32,i32,Callback,*mut c_void)->i32;
type Axis=unsafe extern "C" fn(i32,f32,f32,*mut f32)->i32;
type Phases=unsafe extern "C" fn(f32,f32,f32,i32,*mut f32)->i32;
pub(super) struct Native {handle:*mut c_void,samples:Samples,render:Render,axis:Axis,phases:Phases}
#[cfg(windows)]
#[link(name="kernel32")]
unsafe extern "system" {
 fn LoadLibraryExW(path:*const u16,file:*mut c_void,flags:u32)->*mut c_void;
 fn GetProcAddress(handle:*mut c_void,name:*const u8)->*mut c_void;
 fn FreeLibrary(handle:*mut c_void)->i32;
}
impl Native {
 #[cfg(windows)]
 pub fn load(path:&Path)->Result<Self>{use std::os::windows::ffi::OsStrExt;
  let full=path.canonicalize()?;let wide=full.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
  let handle=unsafe{LoadLibraryExW(wide.as_ptr(),std::ptr::null_mut(),0x00000100|0x00001000)};
  if handle.is_null(){return Err(StdError::Native(format!("Cannot load independent R1 renderer: {}",std::io::Error::last_os_error())));}
  let symbol=|name:&[u8]|->Result<*mut c_void>{let p=unsafe{GetProcAddress(handle,name.as_ptr())};if p.is_null(){Err(StdError::Native("Independent R1 renderer ABI is incomplete".into()))}else{Ok(p)}};
  let result=(||Ok(Self{handle,samples:unsafe{std::mem::transmute::<*mut c_void,Samples>(symbol(b"r1_llsm2_samples\0")?)},
   render:unsafe{std::mem::transmute::<*mut c_void,Render>(symbol(b"r1_llsm2_render_stream\0")?)},
   axis:unsafe{std::mem::transmute::<*mut c_void,Axis>(symbol(b"r1_llsm2_noise_axis\0")?)},
   phases:unsafe{std::mem::transmute::<*mut c_void,Phases>(symbol(b"r1_llsm2_source_phases\0")?)}}))();
  if result.is_err(){unsafe{FreeLibrary(handle);}}result
 }
 #[cfg(not(windows))]
 pub fn load(_path:&Path)->Result<Self>{Err(StdError::Unsupported("This R1 renderer-library adapter currently requires Windows".into()))}
 pub fn noise_axis(&self,c:&Config)->Result<Vec<f32>>{let mut out=vec![0.;c.bins];let code=unsafe{(self.axis)(out.len()as i32,c.nyquist,c.warp,out.as_mut_ptr())};ensure(code==0,"R1 noise-axis calculation failed")?;Ok(out)}
 pub fn phase_function(&self)->SourcePhases {SourcePhases(self.phases)}
 pub fn render(&self,c:&Config,frames:&[Frame],spectra:&[Option<Vec<f32>>],cancel:&std::sync::atomic::AtomicBool,emit:&mut dyn FnMut(usize,&[f32])->Result<()>)->Result<Vec<f32>>{
  ensure((2..=8192).contains(&frames.len())&&frames.len()==spectra.len(),"R1 render frame extent exceeds capacity")?;
  let native=NativeConfig{abi:1,frames:frames.len()as i32,harmonics:c.harmonics as i32,envelope:c.envelope as i32,bins:c.bins as i32,bands:c.edges.len()as i32+1,hop:c.hop,nyquist:c.nyquist,lip:c.lip};
  let stride=4+1025+c.harmonics+(c.edges.len()+1)+c.bins+(c.edges.len()+1)*(1+2*c.envelope);
  let mut payload=Vec::with_capacity(c.edges.len()+frames.len()*stride);payload.extend_from_slice(&c.edges);
  fn fixed(out:&mut Vec<f32>,input:&[f32],count:usize)->Result<()>{ensure(input.len()<=count,"R1 renderer field exceeds declared capacity")?;out.extend_from_slice(input);out.resize(out.len()+count-input.len(),0.);Ok(())}
  for(f,spectrum)in frames.iter().zip(spectra){
   payload.extend_from_slice(&[f.f0,f.rd,f.phases.len()as f32,1.]);if f.f0>0.{ensure(spectrum.as_ref().is_some_and(|s|s.len()==1025),"R1 voiced frame lacks vocal-tract spectrum")?;}
   fixed(&mut payload,spectrum.as_deref().unwrap_or(&[]),1025)?;fixed(&mut payload,&f.phases,c.harmonics)?;
   ensure(f.noise.bands.len()==c.edges.len()+1&&f.noise.psd.len()==c.bins,"R1 renderer noise geometry mismatch")?;
   for b in &f.noise.bands{payload.push(b.amplitudes.len()as f32);}fixed(&mut payload,&f.noise.psd,c.bins)?;
   for b in &f.noise.bands{payload.push(b.dc);fixed(&mut payload,&b.amplitudes,c.envelope)?;fixed(&mut payload,&b.phases,c.envelope)?;}}
  ensure(payload.iter().all(|v|v.is_finite()),"R1 renderer payload contains nonfinite operands")?;
  let _lock=RENDER_LOCK.lock().map_err(|_|StdError::Native("R1 renderer lock is poisoned".into()))?;
  let samples=unsafe{(self.samples)(&native)};ensure(samples>0&&samples<=4_194_304,"R1 native output extent exceeds capacity")?;
  let plan=waves::Waves::new(c,frames)?;
  let mut context=callback::Context {emit,waves:&plan,cancel,error:None,count:samples as usize};
  let mut output=vec![0.;samples as usize*3];
  let code=unsafe{(self.render)(&native,payload.as_ptr(),payload.len()as i32,output.as_mut_ptr(),output.len()as i32,
      callback::receive,(&mut context as *mut callback::Context<'_>).cast())};
  if let Some(error)=context.error {return Err(error);}
  if code!=0{return Err(StdError::Native(format!("Independent R1 renderer failed ({code})")));}
  let count=samples as usize;
  let output=plan.mix(&output[count..2*count],&output[2*count..],0);
  ensure(output.iter().all(|v|v.is_finite()),"R1 renderer returned nonfinite PCM")?;Ok(output)
 }
}
impl Drop for Native {fn drop(&mut self){#[cfg(windows)]unsafe{FreeLibrary(self.handle);}}}

#[derive(Clone,Copy)]
pub(super) struct SourcePhases(Phases);
impl SourcePhases {
 pub fn evaluate(&self,rd:f32,original:f32,target:f32,count:usize)->Result<Vec<f32>>{ensure(count<=1600,"R1 source harmonic extension exceeds capacity")?;let mut out=vec![0.;count];
  let code=unsafe{(self.0)(rd,original,target,count as i32,out.as_mut_ptr())};ensure(code==0,"R1 LF phase extension failed")?;Ok(out)}
}
