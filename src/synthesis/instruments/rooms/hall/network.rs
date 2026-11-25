use super::{model::Model,delay::History,equalizer::{Equalizer,Sos},filter::{self,Cascade}};

pub(super) struct Network {
    delays:Vec<History>, delay_samples:Vec<usize>, feedback:Vec<Cascade>, matrix:super::matrix::Matrix,
    microphones:Vec<History>, mic_delay:Vec<usize>,
    post:Vec<Cascade>, final_early:Vec<Cascade>, gain:f32,
    pub quiet:usize, pub settling:usize, peak:f32,
}
impl Network {
    pub fn new(model:&Model)->Result<Self,String> {
        let delay_samples=model.p.integers(0x501,&[64])?.into_iter().map(|v| {
            usize::try_from(v).ok().filter(|v|(1..=480000).contains(v)).ok_or("Invalid Hall feedback delay")
        }).collect::<Result<Vec<_>,_>>()?;
        let feedback=model.p.values(0x506,&[64,12,6])?.chunks_exact(72)
            .map(|v|v.chunks_exact(6).map(|v|v.try_into().unwrap()).collect()).collect::<Vec<Vec<Sos>>>();
        let feedback=filter::bank(&feedback,model.rate)?;
        let equalizer=Equalizer::read(&model.p,0x600,27,model.rate)?;
        let mut post=model.p.values(0x507,&[7,27,6])?.chunks_exact(27*6)
            .map(|v|v.chunks_exact(6).map(|v|v.try_into().unwrap()).collect()).collect::<Vec<Vec<Sos>>>();
        for row in model.p.values(0x703,&[2,7,25])?.chunks_exact(25) {
            let mut control=vec![row[0]];control.extend(row);control.push(row[row.len()-1]);
            post.push(equalizer.design(&control)?);
        }
        let post=filter::bank(&post,model.rate)?;
        let final_early=model.p.values(0x508,&[7,11,6])?.chunks_exact(66)
            .map(|v|v.chunks_exact(6).map(|v|v.try_into().unwrap()).collect()).collect::<Vec<Vec<Sos>>>();
        let final_early=filter::bank(&final_early,model.rate)?;
        let mic_delay=model.p.values(0x504,&[7])?.into_iter().map(|v| {
            if !(0. ..=480000.).contains(&v) {return Err("Invalid Hall microphone delay");} Ok(v.round() as usize)
        }).collect::<Result<Vec<_>,_>>()?;
        let settling=delay_samples.iter().chain(&mic_delay).copied().max().unwrap()+32+256;
        Ok(Self{delays:delay_samples.iter().map(|&v|History::new(v)).collect(),delay_samples,feedback,
            matrix:super::matrix::Matrix::new(&model.p.values(0x502,&[64,64])?,&model.p.values(0x503,&[7,64])?),
            microphones:mic_delay.iter().map(|&v|History::new(v)).collect(),mic_delay,
            post,final_early,
            gain:model.p.values(0x204,&[])?[0] as f32,quiet:0,settling,peak:0.})
    }
    pub fn sample(&mut self,early:[[f32;7];2],injection:[f32;64])->[f32;7] {
        let values:[f32;64]=std::array::from_fn(|i|self.feedback[i].sample(self.delays[i].read(self.delay_samples[i])));
        let projected=self.matrix.run(&values);
        let mut magnitude=0f32;
        for i in 0..64 {
            let y=projected[i]+injection[i];
            magnitude=magnitude.max(y.abs());self.delays[i].push(y);self.delays[i].advance();
        }
        let mut output=[0.;7];
        for mic in 0..7 {
            let y=projected[64+mic];
            self.microphones[mic].push(y);let y=self.microphones[mic].read(self.mic_delay[mic]);
            self.microphones[mic].advance();
            let reverberant=self.post[mic].sample(y);
            let direct=self.post[7+mic].sample(early[0][mic]);let reflected=self.post[14+mic].sample(early[1][mic]);
            let early=self.final_early[mic].sample(direct+reflected);
            magnitude=magnitude.max(reverberant.abs()).max(early.abs());
            output[mic]=(reverberant+early)*self.gain;
        }
        self.peak=self.peak.max(magnitude);
        if magnitude<=self.peak*1e-7 {self.quiet+=1;}else{self.quiet=0;}
        output
    }
    pub fn settled(&self)->bool {
        self.quiet>self.settling && self.feedback.iter().chain(&self.post).chain(&self.final_early)
            .all(|v|v.magnitude()<=self.peak*1e-7)
    }
}
