use super::{model::{Model,affine,point},geometry::{angle,length,spline,sub,unit}};

pub(super) struct Direct {
    pub gains: [f64;7],
    pub delays: [[f64;3];7],
    pub arrival: [[f64;3];7],
}
impl Model {
    pub fn direct(&self, position: [f64;2], direction: usize) -> Result<Direct,String> {
        let normalized=self.normalized(position)?;
        let origin=[position[0],position[1],0.];
        let base=self.p.values(0x301,&[])?[0];
        let gains=self.p.values(0x302,&[7])?;
        let plane=self.p.values(0x30d,&[4,3,7])?;
        let axes=self.p.values(0x208,&[12,3])?;
        let selected=self.p.integers(0x20c,&[4,3])?;
        let displacement=self.p.values(0x209,&[])?[0];
        let offsets=self.p.values(0x207,&[7])?;
        let weights=self.p.values(0x303,&[13])?;
        let mut out=Direct{gains:[0.;7],delays:[[0.;3];7],arrival:[[0.;3];7]};
        for mic in 0..7 {
            let vector=sub(self.microphones[mic],origin);let distance=length(vector);
            let row=[plane[direction*21+mic],plane[direction*21+7+mic],plane[direction*21+14+mic]];
            out.gains[mic]=(base/distance)*10f64.powf(affine(&row,normalized)/20.)*gains[mic].exp();
            for slot in 0..3 {
                let index=selected[direction*3+slot];
                if !(0..12).contains(&index) { return Err("Invalid Hall source axis".into()); }
                let axis=point(&axes,index as usize);
                let shifted=std::array::from_fn(|i| origin[i]+displacement*axis[i]);
                let delta=sub(self.microphones[mic],shifted);
                let d=((delta[1]*delta[1]+delta[0]*delta[0])+delta[2]*delta[2]).sqrt();
                out.delays[mic][slot]=(d/self.speed+offsets[mic])*self.rate;
                out.arrival[mic][slot]=spline(&self.centers,&weights,angle(unit(vector),axis)).exp();
            }
        }
        Ok(out)
    }
}

pub(super) struct Reflection { pub delay: usize, pub gain: f64 }
impl Model {
    pub fn reflections(&self, position:[f64;2],direction:usize) -> Result<Vec<Vec<Reflection>>,String> {
        let bounds=self.p.values(0x306,&[2])?;
        if bounds[0]<=0. || bounds[1]<bounds[0] { return Err("Invalid Hall wall reflectivity".into()); }
        let weights:Vec<_>=self.p.values(0x305,&[6])?.into_iter().map(|v|v.clamp(bounds[0].ln(),bounds[1].ln())).collect();
        let displacement=self.p.values(0x20b,&[])?[0];
        let threshold=self.p.values(0x307,&[])?[0];let taper=self.p.values(0x308,&[])?[0];
        if taper<=0. { return Err("Invalid Hall reflection window".into()); }
        let angular=self.p.values(0x304,&[13])?;
        let axis=self.axes[direction];let origin=[position[0],position[1],0.];
        let shifted=std::array::from_fn(|i| origin[i]+displacement*axis[i]);
        let mut output=Vec::with_capacity(7);
        for &mic in &self.microphones {
            let direct_distance=length(sub(mic,shifted));
            let direct=spline(&self.centers,&angular,angle(unit(sub(mic,origin)),axis));
            let mut records=Vec::new();
            'path: for sequence in &self.paths {
                let mut image=shifted;let mut images=Vec::new();
                for &index in sequence {
                    let wall=&self.walls[index];
                    let distance=super::geometry::dot(image,wall.normal)-wall.offset;
                    if distance<=self.epsilon { continue 'path; }
                    image=std::array::from_fn(|i| image[i]-2.*distance*wall.normal[i]);images.push(image);
                }
                let mut points=vec![mic;sequence.len()+1];let mut end=mic;
                for i in (0..sequence.len()).rev() {
                    let Some(hit)=self.walls[sequence[i]].hit(end,images[i],self.epsilon) else {continue 'path;};
                    points[i]=hit;end=hit;
                }
                let mut all=Vec::with_capacity(points.len()+1);all.push(shifted);all.extend(&points);
                for i in 0..all.len()-1 {
                    for (index,wall) in self.walls.iter().enumerate() {
                        if sequence[i.saturating_sub(1)..(i+1).min(sequence.len())].contains(&index) { continue; }
                        if wall.hit(all[i],all[i+1],self.epsilon).is_some() { continue 'path; }
                    }
                }
                let distance=length(sub(mic,image));
                let delay=((distance-direct_distance)*self.rate/self.speed).round();
                if delay<0. || delay>self.rate*10. { return Err("Invalid Hall reflection delay".into()); }
                let response=spline(&self.centers,&angular,angle(unit(sub(points[0],origin)),axis));
                let wall_gain=sequence.iter().fold(0.,|sum,&i|sum+weights[i]);
                let relative=(wall_gain+(response-direct)).exp()*direct_distance/distance;
                let db=relative.max(1e-30).log10()*20.;
                let window=(((db-threshold).clamp(-taper,taper)*std::f64::consts::PI/2./taper).sin()+1.)*0.5;
                if window>0. { records.push(Reflection{delay:delay as usize,gain:window*(relative*direct.exp())}); }
            }
            output.push(records);
        }
        Ok(output)
    }
}
