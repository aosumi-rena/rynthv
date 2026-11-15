use std::{ffi::{CStr,CString,c_char,c_int,c_void},ptr::NonNull,sync::{Arc,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
type Request=Box<dyn FnMut(&str)->Result<String,String>>;
struct Callbacks {request:Request,cancel:Arc<AtomicBool>,started:Instant,result:CString}
unsafe extern "C" {
    fn rynthv_script_create(request:extern "C" fn(*mut c_void,*const c_char)->*const c_char,cancel:extern "C" fn(*mut c_void)->c_int,opaque:*mut c_void)->*mut c_void;
    fn rynthv_script_eval(host:*mut c_void,source:*const c_char,length:usize,name:*const c_char)->c_int;
    fn rynthv_script_result(host:*mut c_void)->*const c_char;
    fn rynthv_script_destroy(host:*mut c_void);
}
extern "C" fn request(opaque:*mut c_void,input:*const c_char)->*const c_char {
    let state=unsafe{&mut *opaque.cast::<Callbacks>()};
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
        let input=unsafe{CStr::from_ptr(input)}.to_str().map_err(|e|e.to_string())?;
        (state.request)(input)
    })).unwrap_or_else(|_|Err("Script request failed".into()));
    let value=match result {Ok(text)=>text,Err(error)=>serde_json::json!({"error":error}).to_string()};
    state.result=CString::new(value).unwrap_or_default();
    state.started=Instant::now();state.result.as_ptr()
}
extern "C" fn cancelled(opaque:*mut c_void)->c_int {
    let state=unsafe{&*opaque.cast::<Callbacks>()};
    i32::from(state.cancel.load(Ordering::Acquire)||state.started.elapsed()>Duration::from_secs(5))
}
pub struct Engine {host:NonNull<c_void>,callbacks:Box<Callbacks>}
impl Engine {
    pub fn new(cancel:Arc<AtomicBool>,callback:impl FnMut(&str)->Result<String,String>+'static)->Result<Self,String> {
        let mut callbacks=Box::new(Callbacks {request:Box::new(callback),cancel,started:Instant::now(),result:CString::default()});
        let host=NonNull::new(unsafe{rynthv_script_create(request,cancelled,(&mut *callbacks as *mut Callbacks).cast())}).ok_or("Cannot allocate script engine")?;
        Ok(Self {host,callbacks})
    }
    pub fn eval(&mut self,source:&str,name:&str)->Result<String,String> {
        if source.len()>64*1024*1024{return Err("Script input exceeds 64 MiB".into());}
        let source=CString::new(source).map_err(|_|"Script contains a NUL character")?;
        let name=CString::new(name).map_err(|_|"Invalid script name")?;
        self.callbacks.started=Instant::now();
        let failed=unsafe{rynthv_script_eval(self.host.as_ptr(),source.as_ptr(),source.as_bytes().len(),name.as_ptr())};
        let result=unsafe{CStr::from_ptr(rynthv_script_result(self.host.as_ptr()))}.to_string_lossy().into_owned();
        if failed!=0{Err(result)}else{Ok(result)}
    }
}
impl Drop for Engine {fn drop(&mut self){unsafe{rynthv_script_destroy(self.host.as_ptr())}}}
