type Kernel=unsafe fn(&History,usize,&[f32])->f32;
pub(super) struct History { data:Vec<f32>, cursor:usize, kernel:Kernel }
impl History {
    pub fn new(maximum:usize)->Self { Self{data:vec![0.;(maximum+1).next_power_of_two()],cursor:0,kernel:select()} }
    pub fn push(&mut self,value:f32) {self.data[self.cursor]=value;}
    pub fn read(&self,delay:usize)->f32 {self.data[self.cursor.wrapping_sub(delay)&(self.data.len()-1)]}
    pub fn advance(&mut self) {self.cursor=(self.cursor+1)&(self.data.len()-1);}
    pub fn fir(&self,delay:usize,taps:&[f32])->f32 {
        unsafe{(self.kernel)(self,delay,taps)}
    }
}
#[inline(always)]
fn process(history:&History,delay:usize,taps:&[f32])->f32 {
    taps.iter().enumerate().fold(0f32,|sum,(i,&c)|history.read(delay+i).mul_add(c,sum))
}
unsafe fn portable(history:&History,delay:usize,taps:&[f32])->f32 {process(history,delay,taps)}
#[cfg(target_arch="x86_64")]
#[target_feature(enable="fma")]
unsafe fn hardware(history:&History,delay:usize,taps:&[f32])->f32 {process(history,delay,taps)}
fn select()->Kernel {
    static KERNEL:std::sync::OnceLock<Kernel>=std::sync::OnceLock::new();
    *KERNEL.get_or_init(|| {
        #[cfg(target_arch="x86_64")]
        if std::arch::is_x86_feature_detected!("fma") {return hardware;}
        portable
    })
}
