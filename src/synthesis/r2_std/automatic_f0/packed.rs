pub(super) fn dot(input: &[i16], weights: &[i16], output: &mut [i16]) {
    #[cfg(target_arch="x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {
        unsafe { avx2(input, weights, output); }
        return;
    }
    let width=output.len();
    for (&x,row) in input.iter().zip(weights.chunks_exact(width)) {
        for (out,&w) in output.iter_mut().zip(row) {
            *out=out.saturating_add(((i32::from(x)*i32::from(w)+16384)>>15) as i16);
        }
    }
}

#[cfg(target_arch="x86_64")]
#[target_feature(enable="avx2")]
unsafe fn avx2(input: &[i16], weights: &[i16], output: &mut [i16]) {
    use std::arch::x86_64::*;
    let width=output.len();let stop=width/16*16;
    for base in (0..stop).step_by(16) {
        let mut sum=_mm256_setzero_si256();
        for (i,&x) in input.iter().enumerate() {
            let w=unsafe {_mm256_loadu_si256(weights.as_ptr().add(i*width+base).cast())};
            sum=_mm256_adds_epi16(sum,_mm256_mulhrs_epi16(_mm256_set1_epi16(x),w));
        }
        unsafe {_mm256_storeu_si256(output.as_mut_ptr().add(base).cast(),sum);}
    }
    for lane in stop..width {
        let mut sum=0_i16;
        for (i,&x) in input.iter().enumerate() {
            sum=sum.saturating_add(((i32::from(x)*i32::from(weights[i*width+lane])+16384)>>15) as i16);
        }
        output[lane]=sum;
    }
}
