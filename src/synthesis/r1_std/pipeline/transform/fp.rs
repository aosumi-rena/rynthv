pub(super) fn get()->u32 {
    #[cfg(target_arch="x86_64")] {
        let mut bits=0u32;
        unsafe {std::arch::asm!("stmxcsr [{}]",in(reg)&mut bits,options(nostack,preserves_flags));}bits
    }
    #[cfg(not(target_arch="x86_64"))] {0}
}
fn set(bits:u32) {
    #[cfg(target_arch="x86_64")]
    unsafe {std::arch::asm!("ldmxcsr [{}]",in(reg)&bits,options(nostack,preserves_flags));}
    #[cfg(not(target_arch="x86_64"))] let _=bits;
}
pub(super) struct Guard(u32);
impl Guard {pub fn enter(bits:u32)->Self {let previous=get();set(bits);Self(previous)}}
impl Drop for Guard {fn drop(&mut self){set(self.0);}}
