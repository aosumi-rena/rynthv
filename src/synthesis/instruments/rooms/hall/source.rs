use super::{model::Model,delay::History,equalizer::Equalizer,filter::Cascade,interpolation};

pub(super) struct Direction {
    kernel:Kernel,
    original:History,
    early:History,
    secondary:Cascade,
    split:[Vec<f32>;2],
    late_eq:[Cascade;2],
    late_history:[History;2],
    late_delays:[usize;64],
    late_gains:[[f32;64];2],
    direct:Vec<(usize,Vec<f32>)>,
    reflected:Vec<Vec<(usize,f32)>>,
    coloration:[Vec<Cascade>;2],
    pub maximum_delay:usize,
}
impl Direction {
    pub fn new(model:&Model,position:[f64;2],direction:usize,hits:&super::late::Hits)->Result<Self,String> {
        let direct=model.direct(position,direction)?;
        let reflected=model.reflections(position,direction)?;
        let late=model.late(position,direction,hits)?;
        let eq=Equalizer::read(&model.p,0x600,27,model.rate)?;
        let secondary=Equalizer::read(&model.p,0x610,11,model.rate)?;
        let late_rows=eq.design(&late.equalizer)?;
        let secondary=Cascade::new(&secondary.design(&late.secondary)?,secondary.source_prefix())?;
        let half=late.delays.iter().copied().min().unwrap_or(0).min(128);
        let split=model.split(direction,half)?;
        let late_delays=late.delays.map(|v|v-half);
        let extent=late_delays.iter().copied().max().unwrap_or(0);
        let arrivals=(0..7).map(|i|interpolation::arrivals(&direct.delays[i],&direct.arrival[i],direct.gains[i])).collect::<Result<Vec<_>,_>>()?;
        let reflected:Vec<Vec<_>>=reflected.into_iter().enumerate().map(|(i,rows)| {
            let mean=(direct.delays[i][0]+direct.delays[i][1]+direct.delays[i][2])/3.;
            rows.into_iter().map(|r|(mean.round() as usize+r.delay,(direct.gains[i]*r.gain) as f32)).collect()
        }).collect();
        let maximum_delay=arrivals.iter().map(|(at,row)|at+row.len()).chain(reflected.iter().flatten().map(|(d,_)|*d+1)).max().unwrap_or(1).max(extent+2*half+1);
        let selected=model.p.integers(0x701,&[])?[0];let coloration=model.coloration(position,direction)?;
        let color=Equalizer::read(&model.p,if selected==1 {0x610}else{0x600},if selected==1 {11}else{27},model.rate)?;
        let [a,b]=coloration;
        Ok(Self{kernel:select(),original:History::new(2*half),early:History::new(maximum_delay),secondary,split,
            late_eq:[Cascade::new(&late_rows,eq.source_prefix())?,Cascade::new(&late_rows,eq.source_prefix())?],
            late_history:[History::new(extent),History::new(extent)],late_delays,
            late_gains:[late.directional.map(|v|v as f32),late.common.map(|v|v as f32)],
            direct:arrivals,reflected,coloration:[a.iter().map(|r|Cascade::new(r,color.source_prefix())).collect::<Result<_,_>>()?,
                b.iter().map(|r|Cascade::new(r,color.source_prefix())).collect::<Result<_,_>>()?],maximum_delay})
    }
    pub fn sample(&mut self,input:f32,early:&mut[[f32;7];2],injection:&mut[f32;64]) {
        unsafe{(self.kernel)(self,input,early,injection)};
    }
    #[inline(always)]
    fn process(&mut self,input:f32,early:&mut[[f32;7];2],injection:&mut[f32;64]) {
        self.original.push(input);
        for c in 0..2 {
            let value=self.original.fir(0,&self.split[c]);
            self.late_history[c].push(self.late_eq[c].sample(value));
        }
        let value=self.secondary.sample(input);self.early.push(value);
        for mic in 0..7 {
            let (delay,taps)=&self.direct[mic];let x=self.early.fir(*delay,taps);
            early[0][mic]+=self.coloration[0][mic].sample(x);
            let x=self.reflected[mic].iter().fold(0f32,|a,(d,g)|self.early.read(*d).mul_add(*g,a));
            early[1][mic]+=self.coloration[1][mic].sample(x);
        }
        for (i,destination) in injection.iter_mut().enumerate() {
            let a=self.late_history[0].read(self.late_delays[i]);let b=self.late_history[1].read(self.late_delays[i]);
            *destination=b.mul_add(self.late_gains[1][i],*destination);
            *destination=a.mul_add(self.late_gains[0][i],*destination);
        }
        self.original.advance();self.early.advance();
        for h in &mut self.late_history {h.advance();}
    }
}
type Kernel=unsafe fn(&mut Direction,f32,&mut[[f32;7];2],&mut[f32;64]);
unsafe fn portable(source:&mut Direction,input:f32,early:&mut[[f32;7];2],injection:&mut[f32;64]) {source.process(input,early,injection);}
#[cfg(target_arch="x86_64")]
#[target_feature(enable="fma")]
unsafe fn hardware(source:&mut Direction,input:f32,early:&mut[[f32;7];2],injection:&mut[f32;64]) {source.process(input,early,injection);}
fn select()->Kernel {
    static KERNEL:std::sync::OnceLock<Kernel>=std::sync::OnceLock::new();
    *KERNEL.get_or_init(|| {
        #[cfg(target_arch="x86_64")]
        if std::arch::is_x86_feature_detected!("fma") {return hardware;}
        portable
    })
}
