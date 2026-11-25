// Flush subnormals during room processing and restore the caller's mode on drop.
pub(super) struct Scope {
    #[cfg(target_arch="x86_64")]
    previous:u32,
}
impl Scope {
    pub fn enter()->Self {
        #[cfg(target_arch="x86_64")]
        {
            let mut previous=0u32;
            unsafe {std::arch::asm!("stmxcsr [{value}]",value=in(reg)&mut previous,options(nostack,preserves_flags));}
            let current=previous|0x8040;
            unsafe {std::arch::asm!("ldmxcsr [{value}]",value=in(reg)&current,options(nostack,preserves_flags,readonly));}
            Self{previous}
        }
        #[cfg(not(target_arch="x86_64"))]
        Self{}
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        #[cfg(target_arch="x86_64")]
        unsafe {std::arch::asm!("ldmxcsr [{value}]",value=in(reg)&self.previous,options(nostack,preserves_flags,readonly));}
    }
}
