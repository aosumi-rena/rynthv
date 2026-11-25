mod geometry;
mod model;
mod parameters;
mod late;
mod equalizer;
mod coloration;
mod interpolation;
mod filter;
mod delay;
mod source;
mod split;
mod network;
mod matrix;
mod seating;
mod numeric;

use model::Model;
use source::Direction;
use super::flxr::Program;
use crate::synthesis::instruments::microphones::Mix;

pub struct Hall {
    sources:Vec<Vec<Direction>>, network:network::Network, selected:Vec<(usize,[f32;2])>,
    elapsed:usize, silent:usize, maximum_delay:usize,
}
impl Hall {
    pub fn positions(center:[f64;2],count:usize)->Result<Vec<[f64;2]>,String> {seating::positions(center,count)}
    pub fn new(program:Program,selected:&[Mix],positions:&[[f64;2]])->Result<Self,String> {
        let model=Model::new(program)?;
        let mut sources=Vec::with_capacity(positions.len());let mut maximum_delay=0;
        for &position in positions {
            let hits=model.hits(position);
            let mut directions=Vec::with_capacity(4);
            for d in 0..4 {
                let direction=Direction::new(&model,position,d,&hits)?;
                maximum_delay=maximum_delay.max(direction.maximum_delay);directions.push(direction);
            }
            sources.push(directions);
        }
        let mut selected=selected.to_vec();selected.sort_by(|a,b|a.name.cmp(&b.name));
        let selected=selected.iter().filter(|v|!v.name.starts_with("Close")).map(|v| {
            let index=["Decca-C","Decca-L","Decca-R","Wide-L","Wide-R","Ambi-L","Ambi-R"].iter().position(|n|*n==v.name)
                .ok_or_else(||format!("Hall microphone {} absent",v.name))?;
            Ok((index,[(1.-v.pan.max(0.))*v.gain,(1.+v.pan.min(0.))*v.gain]))
        }).collect::<Result<Vec<_>,String>>()?;
        let network=network::Network::new(&model)?;
        Ok(Self{sources,network,selected,elapsed:0,silent:0,maximum_delay})
    }
    pub fn process(&mut self,input:&[Vec<[f32;4]>],frames:usize)->Result<Vec<f32>,String> {
        let microphones=self.process_microphones(input,frames)?;
        let mut output=Vec::with_capacity(frames*2);
        for microphones in microphones {
            let mut sides=[0f32;2];
            for &(i,gains) in &self.selected {for side in 0..2 {sides[side]+=microphones[i]*gains[side];}}
            output.extend(sides);
        }
        Ok(output)
    }
    pub fn process_microphones(&mut self,input:&[Vec<[f32;4]>],frames:usize)->Result<Vec<[f32;7]>,String> {
        if input.len()!=self.sources.len() || input.iter().any(|v|v.len()!=frames) {return Err("Hall member stream dimensions".into());}
        let _numeric=numeric::Scope::enter();
        let mut output=Vec::with_capacity(frames);
        for frame in 0..frames {
            let mut early=[[0f32;7];2];let mut injection=[0f32;64];let mut active=false;
            for (member,source) in self.sources.iter_mut().enumerate() {
                let mut local=[0f32;64];
                for d in 0..4 {active|=input[member][frame][d]!=0.;source[d].sample(input[member][frame][d],&mut early,&mut local);}
                for (a,v) in injection.iter_mut().zip(local) {*a+=v;}
            }
            if active {self.silent=0;}else{self.silent+=1;}
            let microphones=self.network.sample(early,injection);
            if microphones.iter().any(|v|!v.is_finite()) {return Err("Nonfinite Hall PCM".into());}
            output.push(microphones);self.elapsed+=1;
        }
        Ok(output)
    }
    pub fn settled(&self)->bool {self.silent>self.maximum_delay+32 && self.network.settled()}
}
