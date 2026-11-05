use super::Complex;
use std::sync::OnceLock;
pub(super) struct Plan {pub swaps:Vec<(usize,usize)>,pub stages:Vec<Vec<Complex>>}
pub(super) fn get(n:usize,inverse:bool)->&'static Plan {
    static PLANS:[OnceLock<Plan>;34]=[const {OnceLock::new()};34];
    PLANS[n.trailing_zeros() as usize*2+usize::from(inverse)].get_or_init(|| {
        let mut swaps=Vec::new();let mut j=0;
        for i in 1..n {let mut bit=n>>1;while j&bit!=0 {j^=bit;bit>>=1;}j^=bit;if i<j {swaps.push((i,j));}}
        let mut stages=Vec::new();let mut span=2;
        while span<=n {
            let angle=(if inverse {1.0}else{-1.0})*std::f64::consts::TAU/span as f64;
            let root=Complex {re:angle.cos(),im:angle.sin()};
            let mut twiddle=Complex {re:1.0,im:0.0};let mut row=Vec::with_capacity(span/2);
            for _ in 0..span/2 {row.push(twiddle);twiddle=twiddle.mul(root);}
            stages.push(row);span*=2;
        }
        Plan {swaps,stages}
    })
}
pub(super) fn butterfly(chunk:&mut [Complex],twiddle:&[Complex]) {
    #[cfg(target_arch="x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {
        unsafe {avx2(chunk,twiddle);}return;
    }
    scalar(chunk,twiddle,0);
}
fn scalar(chunk:&mut [Complex],twiddle:&[Complex],start:usize) {
    let half=twiddle.len();
    for k in start..half {let a=chunk[k];let b=chunk[k+half].mul(twiddle[k]);
        chunk[k]=Complex {re:a.re+b.re,im:a.im+b.im};chunk[k+half]=Complex {re:a.re-b.re,im:a.im-b.im};}
}
#[cfg(target_arch="x86_64")]
#[target_feature(enable="avx2")]
unsafe fn avx2(chunk:&mut [Complex],twiddle:&[Complex]) {
    use std::arch::x86_64::*;
    let half=twiddle.len();let stop=half/2*2;
    for k in (0..stop).step_by(2) {unsafe {
        let a=_mm256_loadu_pd(chunk.as_ptr().add(k).cast());
        let b=_mm256_loadu_pd(chunk.as_ptr().add(k+half).cast());
        let w=_mm256_loadu_pd(twiddle.as_ptr().add(k).cast());
        let re=_mm256_movedup_pd(w);let im=_mm256_permute_pd(w,15);
        let cross=_mm256_mul_pd(_mm256_permute_pd(b,5),im);
        let product=_mm256_addsub_pd(_mm256_mul_pd(b,re),cross);
        _mm256_storeu_pd(chunk.as_mut_ptr().add(k).cast(),_mm256_add_pd(a,product));
        _mm256_storeu_pd(chunk.as_mut_ptr().add(k+half).cast(),_mm256_sub_pd(a,product));
    }}
    scalar(chunk,twiddle,stop);
}
