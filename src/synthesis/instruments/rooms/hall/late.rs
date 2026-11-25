use super::{geometry::{dot,angle,spline},model::{Model,affine}};

pub(super) struct Hits { planes: Vec<i32>, distances: Vec<f64> }
pub(super) struct Late {
    pub directional: [f64;64],
    pub common: [f64;64],
    pub delays: [usize;64],
    pub equalizer: Vec<f64>,
    pub secondary: Vec<f64>,
}
impl Model {
    pub fn hits(&self, position: [f64;2]) -> Hits {
        let origin=[position[0],position[1],0.];
        let mut planes=Vec::with_capacity(self.rays.len());let mut distances=Vec::with_capacity(self.rays.len());
        for &ray in &self.rays {
            let mut best=f64::INFINITY;let mut selected=-1;
            for (i,wall) in self.walls.iter().enumerate() {
                let distance=(wall.offset-dot(origin,wall.normal))/dot(ray,wall.normal);
                if distance<=self.epsilon || !distance.is_finite() || distance>=best { continue; }
                let p=std::array::from_fn(|j| origin[j]+ray[j]*distance);
                if wall.contains(p) { best=distance;selected=i as i32; }
            }
            planes.push(selected);distances.push(best);
        }
        Hits{planes,distances}
    }
    fn fourier(&self, position:[f64;2],direction:usize,common:bool) -> Result<[f64;64],String> {
        let base=if common {0x410} else {0x400};
        let waves=self.p.values(base+1,&[256,2])?;
        let phase=self.p.values(base+2,&[256])?;
        let weights=self.p.values(base+3,&[if common {1}else{4},64,256])?;
        let scale=self.p.values(base+4,&[])?[0];let normal=self.p.values(base+5,&[])?[0];
        let input:Vec<_>=waves.chunks_exact(2).zip(phase).map(|(v,p)| ((v[1]*position[1]+v[0]*position[0])+p).cos()).collect();
        let offset=if common {0} else {direction*64*256};
        Ok(std::array::from_fn(|i| {
            let row=&weights[offset+i*256..offset+(i+1)*256];
            let sum=row.iter().zip(&input).fold(0.,|sum,(&a,&b)|a*b+sum);
            ((sum*normal)*scale).tanh()
        }))
    }
    pub fn late(&self,position:[f64;2],direction:usize,hits:&Hits) -> Result<Late,String> {
        let normalized=self.normalized(position)?;
        let angular=self.p.values(0x304,&[13])?;
        let mut histogram=vec![[0f64;181];6];let mut minimum=[f64::INFINITY;6];
        let ray_weight=1./self.rays.len() as f64;
        for (i,&plane) in hits.planes.iter().enumerate() {
            if plane<0 { continue; } let plane=plane as usize;
            let angle=angle(self.rays[i],self.axes[direction]).round().clamp(0.,180.) as usize;
            histogram[plane][angle]+=ray_weight;minimum[plane]=minimum[plane].min(hits.distances[i]);
        }
        let response:Vec<_>=(0..=180).map(|a|(2.*(spline(&self.centers,&angular,a as f64)-angular[0])).exp()).collect();
        let levels=self.p.values(0x313,&[6])?;let mut gains=[0.;6];let mut delays=[0usize;6];
        for i in 0..6 {
            let sum=histogram[i].iter().zip(&response).fold(0.,|sum,(&a,&b)|a*b+sum);
            gains[i]=sum.max(0.).sqrt()*levels[i].exp();
            let distance=if minimum[i].is_finite() {minimum[i]} else {1.};
            let delay=(distance*self.rate/self.speed).round();
            if !(0. ..=480000.).contains(&delay) { return Err("Invalid Hall late-field delay".into()); }
            delays[i]=delay as usize;
        }
        let mapping=self.p.integers(0x312,&[64])?;
        if mapping.iter().any(|v|!(0..6).contains(v)) { return Err("Invalid Hall wall channel".into()); }
        let scales=self.p.values(0x30e,&[4,3])?;
        let scale=affine(&scales[direction*3..direction*3+3],normalized).exp();
        let mut channels=[[0.;64];2];
        for (common,destination) in channels.iter_mut().enumerate() {
            let mut values=self.fourier(position,direction,common==1)?;
            let levels=if common==1 {self.p.values(0x420,&[64])?} else {
                self.p.values(0x30f,&[4,64])?[direction*64..direction*64+64].to_vec()
            };
            let mut sums=[0.;6];
            for i in 0..64 { values[i]*=levels[i].exp(); let wall=mapping[i] as usize;sums[wall]+=values[i]*values[i]; }
            for i in 0..64 {
                let wall=mapping[i] as usize;
                if sums[wall]<=0. { return Err("Invalid Hall injection normalization".into()); }
                destination[i]=(values[i]/sums[wall].sqrt())*scale*gains[wall];
            }
        }
        let controls=|tag,width|->Result<Vec<f64>,String> {
            let values=self.p.values(tag,&[4,3,width])?;let begin=direction*3*width;
            Ok((0..width).map(|i|affine(&[values[begin+i],values[begin+width+i],values[begin+2*width+i]],normalized)).collect())
        };
        Ok(Late{directional:channels[0],common:channels[1],delays:std::array::from_fn(|i|delays[mapping[i] as usize]),
            equalizer:controls(0x310,27)?,secondary:controls(0x311,11)?})
    }
}
