// SPDX-License-Identifier: MIT
mod oscillator;
mod chirp;
mod resample;
mod key4;
mod ring;
use super::{DecodedFrame, Error, Result, noise_tables::NoiseTables};
use ring::Ring;
const RATE: f32 = 44100.0;
const RING: usize = 8820;
const FFT: usize = 1024;

pub(super) struct Renderer {
    power: [Ring;4],
    mixed: Ring,
    noise: Ring,
    classic: Ring,
    hop: f32,
    carry: f32,
    current_hop: usize,
    lookahead: usize,
    window: Vec<f32>,
    cursor: usize,
    delay: isize,
    debt: usize,
    adapter_debt: usize,
    phase: f64,
    previous: Option<DecodedFrame>,
    key4: key4::State,
}
impl Renderer {
    pub fn new(hop: f64, tables: &NoiseTables) -> Result<Self> {
        if !hop.is_finite() || hop<=0.0 || tables.bands.iter().any(|b|b.len()!=44100) {
            return Err(Error::Format("P3 constructor geometry"));
        }
        if (hop as f32).to_bits()!=0.005f32.to_bits() {
            return Err(Error::Unsupported("P3 1024-FFT output-hop profile"));
        }
        let mut renderer = Self { power: std::array::from_fn(|_| Ring::new(RING)),
            mixed: Ring::new(RING),noise: Ring::new(RING),classic: Ring::new(RING),
            hop: hop as f32,carry: 0.0,current_hop: 1,lookahead: 0,window: Vec::new(),
            cursor: 0,delay: 0,debt: 0,adapter_debt: 0,phase: 0.0,previous: None,key4: key4::State::new() };
        renderer.advance()?;
        renderer.delay=-(FFT as isize/2)-(2*renderer.current_hop) as isize;
        renderer.carry=0.0;
        for _ in 0..RING-1 { for ring in &mut renderer.power {
            ring.write(0,&[f32::from_bits(0x3727c5ac)],false)?;ring.advance(1,false)?;
        }}
        for _ in 0..5 { renderer.warmup(RING/5,tables)?; }
        let base=renderer.delay+renderer.current_hop as isize;
        let total=base+2048;
        renderer.debt=total.max(0) as usize;
        renderer.adapter_debt=if total>0 { 2048 } else { (-base) as usize };
        Ok(renderer)
    }
    fn advance(&mut self) -> Result<()> {
        let sum=self.carry+self.hop;
        let h=(RATE*sum).floor();
        if !h.is_finite() || !(1.0..=512.0).contains(&h) { return Err(Error::Unsupported("P3 frame sample span")); }
        let carry=sum-self.current_hop as f32/RATE;
        let lookahead=((carry+self.hop)*RATE).floor();
        if !lookahead.is_finite() || !(0.0..=512.0).contains(&lookahead) { return Err(Error::Format("P3 frame lookahead")); }
        let h=h as usize;
        for ring in &mut self.power { ring.advance(h,true)?; }
        self.noise.advance(h,true)?;self.classic.advance(h,true)?;
        self.carry=carry;self.current_hop=h;self.lookahead=lookahead as usize;
        self.window=oscillator::window(2*h);
        Ok(())
    }
    fn warmup(&mut self, count: usize, tables: &NoiseTables) -> Result<()> {
        if self.current_hop+count>RING { return Err(Error::Format("P3 warmup span")); }
        let lag=-((self.current_hop+count) as isize);
        let mut mixed=vec![0.0f32;count];
        for (c,ring) in self.power.iter().enumerate() {
            let power=ring.read(lag,count)?;
            for i in 0..count {
                if power[i]<0.0 || !power[i].is_finite() { return Err(Error::Format("P3 negative noise power")); }
                mixed[i]=(f64::from(power[i]).sqrt()*f64::from(tables.bands[c][(self.cursor+i)%44100])+f64::from(mixed[i])) as f32;
            }
        }
        self.mixed.advance(count,false)?;self.mixed.write(-(count as isize),&mixed,false)?;
        self.cursor=(self.cursor+count)%44100;
        Ok(())
    }
    pub fn feed(&mut self, mut frame: DecodedFrame, tables: &NoiseTables, nyquist: f32, output_period: f64) -> Result<Vec<f32>> {
        oscillator::rotate(&mut frame,self.phase);
        self.phase += f64::from(frame.f0_hz)*output_period*std::f64::consts::TAU;
        if self.phase>std::f64::consts::PI { self.phase-=std::f64::consts::TAU; }
        self.advance()?;
        let n=self.window.len();
        let powers=oscillator::power(&frame,&self.window)?;
        for (ring,power) in self.power.iter_mut().zip(powers) { ring.write(-(n as isize),&power,true)?; }
        let classic=oscillator::classic(&frame,&self.window,self.carry,nyquist)?;
        self.classic.write(-(n as isize),&classic,true)?;
        self.warmup(self.current_hop,tables)?;
        if let Some(previous)=&self.previous {
            let wave=self.mixed.read(-(n as isize),n)?;
            let noise=super::noise_synthesis::shape(previous,&wave,&self.window,nyquist)?;
            self.noise.write(-(FFT as isize),&noise,true)?;
        }
        self.key4.apply(self.previous.as_ref().and_then(|row|row.noise_wave.as_ref()),
            &mut self.noise,self.lookahead)?;
        let noise=self.noise.read(-((FFT+self.debt) as isize),self.lookahead)?;
        let classic=self.classic.read(self.delay-self.debt as isize,self.lookahead)?;
        self.previous=Some(frame);
        let skip=self.adapter_debt.min(self.lookahead);self.adapter_debt-=skip;
        let values: Vec<_> = (skip..self.lookahead).map(|i|classic[i]+noise[i]).collect();
        if values.iter().any(|v|!v.is_finite()) { return Err(Error::Format("P3 nonfinite output")); }
        Ok(values)
    }
}
