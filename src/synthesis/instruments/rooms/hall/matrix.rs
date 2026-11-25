type Kernel=unsafe fn(&[f32;64],&[f32])->[f32;72];
pub(super) struct Matrix { coefficients:Vec<f32>, kernel:Kernel }
impl Matrix {
    pub fn new(feedback:&[f64],projection:&[f64])->Self {
        assert_eq!(feedback.len(),64*64);assert_eq!(projection.len(),7*64);
        let mut coefficients=vec![0.;64*72];
        for input in 0..64 {for out in 0..71 {
            coefficients[input*72+out]=if out<64 {feedback[out*64+input]}else{projection[(out-64)*64+input]} as f32;
        }}
        let mut kernel:Kernel=portable;
        #[cfg(target_arch="x86_64")]
        if std::arch::is_x86_feature_detected!("avx2")&&std::arch::is_x86_feature_detected!("fma") {kernel=hardware;}
        Self{coefficients,kernel}
    }
    pub fn run(&self,input:&[f32;64])->[f32;72] {unsafe{(self.kernel)(input,&self.coefficients)}}
}
unsafe fn portable(input:&[f32;64],coefficients:&[f32])->[f32;72] {
    std::array::from_fn(|out|(0..64).fold(0f32,|sum,i|input[i].mul_add(coefficients[i*72+out],sum)))
}
#[cfg(target_arch="x86_64")]
#[target_feature(enable="avx2,fma")]
unsafe fn hardware(input:&[f32;64],coefficients:&[f32])->[f32;72] {
    use std::arch::x86_64::*;
    let mut accumulators=[_mm256_setzero_ps();9];
    for i in 0..64 {
        let value=_mm256_set1_ps(input[i]);
        for (block,sum) in accumulators.iter_mut().enumerate() {
            let row=unsafe{_mm256_loadu_ps(coefficients.as_ptr().add(i*72+block*8))};
            *sum=_mm256_fmadd_ps(value,row,*sum);
        }
    }
    let mut output=[0f32;72];
    for (block,value) in accumulators.into_iter().enumerate() {unsafe{_mm256_storeu_ps(output.as_mut_ptr().add(block*8),value)};}
    output
}
