pub(super) struct Guard(u32);
impl Guard {
    pub fn enter() -> Self {
        #[cfg(target_arch="x86_64")]
        {
            let mut previous=0u32;
            unsafe {std::arch::asm!("stmxcsr [{}]",in(reg)&mut previous,options(nostack,preserves_flags));}
            let current=previous | 0x8040;
            unsafe {std::arch::asm!("ldmxcsr [{}]",in(reg)&current,options(nostack,preserves_flags));}
            Self(previous)
        }
        #[cfg(not(target_arch="x86_64"))] {Self(0)}
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        #[cfg(target_arch="x86_64")]
        unsafe {std::arch::asm!("ldmxcsr [{}]",in(reg)&self.0,options(nostack,preserves_flags));}
    }
}
