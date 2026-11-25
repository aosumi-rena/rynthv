use std::ffi::{CString,c_char,c_int};
unsafe extern "C"{fn rynthv_script_format_number(format:*const c_char,value:f64,integer:c_int,output:*mut c_char,size:usize)->c_int;}
fn conversion(format:&str)->Option<Option<bool>>{
    let bytes=format.as_bytes();let mut i=0;let mut result=None;
    while i<bytes.len(){
        if bytes[i]!=b'%'{i+=1;continue;}i+=1;
        if bytes.get(i)==Some(&b'%'){i+=1;continue;}
        if result.is_some(){return None;}
        while bytes.get(i).is_some_and(|b|b"-+ #0".contains(b)){i+=1;}
        let start=i;while bytes.get(i).is_some_and(u8::is_ascii_digit){i+=1;}
        if i>start&&format[start..i].parse::<u32>().ok().filter(|n|*n<=256).is_none(){return None;}
        if bytes.get(i)==Some(&b'.'){
            i+=1;let start=i;while bytes.get(i).is_some_and(u8::is_ascii_digit){i+=1;}
            if i>start&&format[start..i].parse::<u32>().ok().filter(|n|*n<=32).is_none(){return None;}
        }
        let long=bytes.get(i)==Some(&b'l');if long{i+=1;}
        let kind=*bytes.get(i)?;i+=1;
        if b"fFeEgGaA".contains(&kind){result=Some(false);}
        else if !long&&b"di".contains(&kind){result=Some(true);}
        else{return None;}
    }Some(result)
}
pub(super) fn number(format:&str,value:f64)->String{
    let integer=match conversion(format){Some(Some(v))=>v,Some(None)=>return format.replace("%%","%"),None=>return value.to_string()};
    let Ok(format)=CString::new(format)else{return value.to_string()};
    let mut output=vec![0u8;512];
    let size=unsafe{rynthv_script_format_number(format.as_ptr(),value,i32::from(integer),output.as_mut_ptr().cast(),output.len())};
    if size<0||size>65535{return value.to_string();}
    if size as usize>=output.len(){
        output.resize(size as usize+1,0);
        unsafe{rynthv_script_format_number(format.as_ptr(),value,i32::from(integer),output.as_mut_ptr().cast(),output.len());}
    }
    String::from_utf8_lossy(&output[..size as usize]).into_owned()
}
