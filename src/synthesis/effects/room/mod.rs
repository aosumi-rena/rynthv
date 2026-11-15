mod crossover;
mod path;

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Parameters {
    pub position_x:f32,pub position_y:f32,pub size:f32,pub reflection_gain:f32,
}

pub struct Room { crossover:crossover::Crossover,paths:Vec<path::Path> }
impl Room {
    pub fn new(p:Parameters,reflections:bool)->Self {
        let position=[p.position_y,p.position_x,1.5_f32];
        let y=(p.position_y as f64)-(-1.0);
        let x2=(p.position_x*p.position_x)as f64;
        let near=(y*y+x2+0.25).sqrt()as f32;
        let y=((p.position_y+3.0)as f64)-(-1.0);
        let far=(y*y+x2+0.25).sqrt()as f32;
        let reference=16.25_f64.sqrt()as f32;
        let factor=(reference.max(0.001)/far.max(0.001))*near.max(0.001);
        let level=(factor.log10()*20.0).max(-100.0);
        let mut paths=vec![path::Path::new(position,[level;4],false)];
        if reflections {
            let walls=[[-2.0,-3.0,-4.0,-10.0],[-2.0,-3.0,-4.0,-10.0],
                [-2.0,-3.0,-4.0,-10.0],[-2.0,-3.0,-4.0,-10.0],
                [-1.0,-2.0,-4.0,-8.0],[-3.0,-8.0,-10.0,-13.0]];
            let [x,y,z]=position;let s=p.size;let height=s/3.0;
            for (position,wall) in [[s-x,y,z],[-(s+x),y,z],[x,s-y,z],
                [x,-(s+y),z],[x,y,height-z],[x,y,-(height+z)]].into_iter().zip(walls) {
                let db=wall.map(|v|(p.reflection_gain+v)+level);
                paths.push(path::Path::new(position,db,true));
            }
        }
        Self {crossover:Default::default(),paths}
    }
    pub fn process(&mut self,samples:&mut [[f32;4]]) {
        let _environment=super::floating_point::Guard::enter();
        for sample in samples {
            let bands=self.crossover.process(sample[0]);let mut result=[0.0;4];
            for path in &mut self.paths {path.add(bands,&mut result);}
            *sample=result;
        }
    }
}
