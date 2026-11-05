// SPDX-License-Identifier: MIT

#[derive(Clone,Copy,Debug)]
pub(crate) enum SeedError {UnsupportedArchitecture,UnsupportedFeature,InputDomain{bits:u32},NonfiniteSeed{value:u32,seed:u32}}
#[derive(Clone,Copy,Debug)]
pub(crate) struct SeedWord {pub value:u32,pub seed:u32,pub x86_64:bool,pub sse_checked:bool,pub intrinsic_invocations:u8}

pub(crate) fn seed_word(value:u32)->Result<SeedWord,SeedError>{
    if !(0x3c23_d70a..=0x3f80_0000).contains(&value){return Err(SeedError::InputDomain{bits:value});}
    #[cfg(target_arch="x86_64")]
    {
        if !std::is_x86_feature_detected!("sse"){return Err(SeedError::UnsupportedFeature);}
        use std::arch::x86_64::{_mm_set_ss,_mm_rcp_ss,_mm_cvtss_f32};
        let seed=unsafe{_mm_cvtss_f32(_mm_rcp_ss(_mm_set_ss(f32::from_bits(value))))}.to_bits();
        if seed&0x7f80_0000==0x7f80_0000{return Err(SeedError::NonfiniteSeed{value,seed});}
        Ok(SeedWord{value,seed,x86_64:true,sse_checked:true,intrinsic_invocations:1})
    }
    #[cfg(not(target_arch="x86_64"))]
    {Err(SeedError::UnsupportedArchitecture)}
}
