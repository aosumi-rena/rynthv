use super::{model::Model,equalizer::{Equalizer,Sos}};

impl Model {
    fn surface_position(&self,position:[f64;2])->Result<[f64;2],String> {
        let shape=self.p.dimensions(0x108)?;
        if shape.len()!=2 || shape[1]!=2 || !(3..=1024).contains(&shape[0]) {
            return Err("Invalid Hall playable polygon".into());
        }
        let vertices=self.p.values(0x108,shape)?;
        let polygon:Vec<[f64;2]>=vertices.chunks_exact(2).map(|v|v.try_into().unwrap()).collect();
        let mut low=[f64::INFINITY;2];let mut high=[f64::NEG_INFINITY;2];
        let mut inside=true;let mut best=f64::INFINITY;let mut nearest=position;
        for i in 0..polygon.len() {
            let a=polygon[i];let b=polygon[(i+1)%polygon.len()];
            let edge=[b[0]-a[0],b[1]-a[1]];let delta=[position[0]-a[0],position[1]-a[1]];
            inside &= edge[0]*delta[1]-edge[1]*delta[0]>=0.;
            let t=((delta[0]*edge[0]+delta[1]*edge[1])/(edge[0]*edge[0]+edge[1]*edge[1]).max(1e-12)).clamp(0.,1.);
            let p=[a[0]+edge[0]*t,a[1]+edge[1]*t];
            let distance=(p[0]-position[0]).powi(2)+(p[1]-position[1]).powi(2);
            if distance<best { best=distance;nearest=p; }
            for j in 0..2 { low[j]=low[j].min(a[j]);high[j]=high[j].max(a[j]); }
        }
        let p=if inside {position}else{nearest};
        Ok(std::array::from_fn(|i|(p[i]-(low[i]+high[i])*0.5)/((high[i]-low[i])*0.5).max(1e-12)))
    }
    pub fn coloration(&self,position:[f64;2],direction:usize)->Result<[Vec<Vec<Sos>>;2],String> {
        let selected=self.p.integers(0x701,&[])?[0];
        if !(0..=1).contains(&selected) { return Err("Invalid Hall coloration type".into()); }
        let width=if selected==1 {11}else{27};
        let equalizer=Equalizer::read(&self.p,if selected==1 {0x610}else{0x600},width,self.rate)?;
        let values=self.p.values(0x702,&[2,4,7,width-2,6])?;
        let [x,y]=self.surface_position(position)?;
        let mut output=[Vec::with_capacity(7),Vec::with_capacity(7)];
        for (bank,destination) in output.iter_mut().enumerate() {
            for mic in 0..7 {
                let at=((bank*4+direction)*7+mic)*(width-2)*6;
                let mut controls=Vec::with_capacity(width);
                for row in values[at..at+(width-2)*6].chunks_exact(6) {
                    let sum=row[0]+0.;let sum=sum+row[1]*x;let sum=sum+row[2]*y;
                    let sum=sum+row[3]*(x*x);let sum=sum+row[4]*(y*y);let sum=sum+row[5]*(x*y);
                    controls.push(sum);
                }
                controls.insert(0,controls[0]);controls.push(controls[controls.len()-1]);
                destination.push(equalizer.design(&controls)?);
            }
        }
        Ok(output)
    }
}
