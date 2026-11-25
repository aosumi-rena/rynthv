use std::{ffi::{CStr,CString,c_char,c_int,c_void},path::Path,ptr::NonNull,sync::{Arc,atomic::{AtomicBool,Ordering}}};
type Request=Box<dyn FnMut(&str)->Result<String,String>>;
struct Callbacks {request:Request,cancel:Arc<AtomicBool>,result:CString}
unsafe extern "C" {
    fn rynthv_script_create(request:extern "C" fn(*mut c_void,*const c_char)->*const c_char,cancel:extern "C" fn(*mut c_void)->c_int,opaque:*mut c_void)->*mut c_void;
    fn rynthv_script_eval(host:*mut c_void,source:*const c_char,length:usize,name:*const c_char)->c_int;
    fn rynthv_script_load_lua(host:*mut c_void,source:*const c_char,length:usize,name:*const c_char)->c_int;
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
    state.result.as_ptr()
}
extern "C" fn cancelled(opaque:*mut c_void)->c_int {
    let state=unsafe{&*opaque.cast::<Callbacks>()};
    i32::from(state.cancel.load(Ordering::Acquire))
}
pub struct Engine {host:NonNull<c_void>,_callbacks:Box<Callbacks>}
impl Engine {
    pub fn new(cancel:Arc<AtomicBool>,callback:impl FnMut(&str)->Result<String,String>+'static)->Result<Self,String> {
        let mut callbacks=Box::new(Callbacks {request:Box::new(callback),cancel,result:CString::default()});
        let host=NonNull::new(unsafe{rynthv_script_create(request,cancelled,(&mut *callbacks as *mut Callbacks).cast())}).ok_or("Cannot allocate script engine")?;
        Ok(Self {host,_callbacks:callbacks})
    }
    pub fn eval(&mut self,source:&str,name:&str)->Result<String,String> {
        if source.len()>64*1024*1024{return Err("Script input exceeds 64 MiB".into());}
        let source=CString::new(source).map_err(|_|"Script contains a NUL character")?;
        let name=CString::new(name).map_err(|_|"Invalid script name")?;
        let failed=unsafe{rynthv_script_eval(self.host.as_ptr(),source.as_ptr(),source.as_bytes().len(),name.as_ptr())};
        let result=unsafe{CStr::from_ptr(rynthv_script_result(self.host.as_ptr()))}.to_string_lossy().into_owned();
        if failed!=0{Err(result)}else{Ok(result)}
    }
    pub fn load(&mut self,source:&str,path:&Path)->Result<(),String> {
        if !path.extension().is_some_and(|e|e.eq_ignore_ascii_case("lua")){return self.eval(source,&path.to_string_lossy()).map(|_|());}
        let source=CString::new(source).map_err(|_|"Script contains a NUL character")?;
        let name=CString::new(path.to_string_lossy().as_bytes()).map_err(|_|"Invalid script path")?;
        let failed=unsafe{rynthv_script_load_lua(self.host.as_ptr(),source.as_ptr(),source.as_bytes().len(),name.as_ptr())};
        if failed==0{Ok(())}else{Err(unsafe{CStr::from_ptr(rynthv_script_result(self.host.as_ptr()))}.to_string_lossy().into_owned())}
    }
}
impl Drop for Engine {fn drop(&mut self){unsafe{rynthv_script_destroy(self.host.as_ptr())}}}
