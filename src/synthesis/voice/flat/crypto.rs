use super::super::{Result,VoiceError};

#[cfg(windows)]
pub(super) fn decode(iv:&[u8],input:&[u8])->Result<Vec<u8>> {
    use std::{ffi::c_void,ptr::{null,null_mut}};
    type Handle=*mut c_void;
    #[link(name="bcrypt")]
    unsafe extern "system" {
        fn BCryptOpenAlgorithmProvider(out:*mut Handle,id:*const u16,provider:*const u16,flags:u32)->i32;
        fn BCryptSetProperty(handle:Handle,name:*const u16,value:*const u8,size:u32,flags:u32)->i32;
        fn BCryptGenerateSymmetricKey(handle:Handle,key:*mut Handle,object:*mut u8,object_size:u32,secret:*const u8,secret_size:u32,flags:u32)->i32;
        fn BCryptDecrypt(handle:Handle,input:*const u8,size:u32,padding:*const c_void,iv:*mut u8,iv_size:u32,output:*mut u8,capacity:u32,written:*mut u32,flags:u32)->i32;
        fn BCryptDestroyKey(key:Handle)->i32;
        fn BCryptCloseAlgorithmProvider(handle:Handle,flags:u32)->i32;
    }
    struct Key(Handle);impl Drop for Key {fn drop(&mut self) {unsafe {BCryptDestroyKey(self.0);}}}
    struct Provider(Handle);impl Drop for Provider {fn drop(&mut self) {unsafe {BCryptCloseAlgorithmProvider(self.0,0);}}}
    fn check(status:i32)->Result<()> {if status<0 {Err(VoiceError::new(format!("Flat archive decryption failed ({status:#x})")))}else{Ok(())}}
    let wide=|text:&str|text.encode_utf16().chain([0]).collect::<Vec<_>>();
    if iv.len()!=16 || input.is_empty() || input.len()%16!=0 || input.len()>512*1024*1024 {return Err(VoiceError::new("Invalid encrypted Flat archive"));}
    let mut handle=null_mut();
    unsafe {check(BCryptOpenAlgorithmProvider(&mut handle,wide("AES").as_ptr(),null(),0))?;}
    let provider=Provider(handle);
    let mode=wide("ChainingModeCBC");
    unsafe {check(BCryptSetProperty(provider.0,wide("ChainingMode").as_ptr(),mode.as_ptr().cast(),(mode.len()*2) as u32,0))?;}
    let secret:[u8;32]=[0x9a,0xce,0x85,0xbf,0x9f,0x6e,0x4c,0xde,0x2f,0x5a,0xd2,0xff,0xf3,0x8b,0xeb,0xcf,
        0x8e,0x85,0x36,0x87,0x49,0xf2,0x94,0xe9,0xef,0x99,0xc1,0xd7,0x54,0x28,0x12,0xfe];
    let mut key=null_mut();
    unsafe {check(BCryptGenerateSymmetricKey(provider.0,&mut key,null_mut(),0,secret.as_ptr(),32,0))?;}
    let key=Key(key);let mut output=vec![0;input.len()];let mut iv=iv.to_vec();let mut written=0;
    unsafe {check(BCryptDecrypt(key.0,input.as_ptr(),input.len() as u32,null(),iv.as_mut_ptr(),16,output.as_mut_ptr(),output.len() as u32,&mut written,1))?;}
    output.truncate(written as usize);Ok(output)
}

#[cfg(not(windows))]
pub(super) fn decode(_iv:&[u8],_input:&[u8])->Result<Vec<u8>> {
    Err(VoiceError::new("Convert encrypted Flat archives to the plaintext format before loading on this platform"))
}
