use super::{super::{Result,StdError},waves::Waves};
use std::{ffi::c_void,sync::atomic::{AtomicBool,Ordering}};
pub(super) struct Context<'a> {
    pub emit:&'a mut dyn FnMut(usize,&[f32])->Result<()>,pub waves:&'a Waves,
    pub cancel:&'a AtomicBool,pub error:Option<StdError>,pub count:usize,
}
pub(super) unsafe extern "C" fn receive(user:*mut c_void,start:i32,length:i32,periodic:*const f32,noise:*const f32)->i32 {
    let context=unsafe {&mut *user.cast::<Context<'_>>()};
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if context.cancel.load(Ordering::Acquire) {return Err(StdError::Cancelled);}
        if start<0 || length<0 || start as usize+length as usize>context.count {
            return Err(StdError::Native("Invalid R1 stream callback clock".into()));
        }
        if length==0 {return Ok(());}
        let first=start as usize;let count=length as usize;
        let periodic=unsafe {std::slice::from_raw_parts(periodic.add(first),count)};
        let noise=unsafe {std::slice::from_raw_parts(noise.add(first),count)};
        let samples=context.waves.mix(periodic,noise,first);
        (context.emit)(first,&samples)
    })).unwrap_or_else(|_|Err(StdError::Native("R1 streaming callback panicked".into())));
    if let Err(error)=result {context.error=Some(error);1}else{0}
}
