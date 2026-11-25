use super::equalizer::Sos;

pub(super) struct Cascade { sections:Vec<Section>, kernel:unsafe fn(&mut[Section],f32)->f32 }
struct Section { c:[f32;6], svf:bool, state:[f32;2] }
impl Cascade {
    pub fn magnitude(&self)->f32 {
        self.sections.iter().flat_map(|s|s.state).fold(0.,|a,v|a.max(v.abs()))
    }
    pub fn new(sections:&[Sos],svf:usize)->Result<Self,String> {
        let sections=sections.iter().enumerate().map(|(index,row)| {
            let [mut b0,mut b1,mut b2,a0,mut a1,mut a2]=*row;
            if a0==0. { return Err("Invalid Hall filter denominator".to_string()); }
            b0/=a0;b1/=a0;b2/=a0;a1/=a0;a2/=a0;
            let svf=index<svf;
            let c=if svf {
                let top=(a1+1.)+a2;let bottom=(1.-a1)+a2;
                let g=(top/bottom).sqrt();
                let m0=((b0-b1)+b2)/bottom;
                let m2=b1-a1*m0;let m1=(b0-b2)-(1.-a2)*m0;
                let kg=(2.*(1.-a2))/(bottom*g)+g;
                let den=g*kg+1.;let inv=1./den;
                let m2=(m2*den)/(g*(g+g));let m1=(m1*den)/(g+g);
                [(-2.*g*kg)*inv,(g*inv)*2.,((g*inv)*g)*2.,m0,m1*0.5,m2*0.5]
            } else { [b0,b1,b2,a1,a2,0.] }.map(|v|v as f32);
            if c.iter().any(|v|!v.is_finite()) { return Err("Nonfinite Hall filter coefficients".into()); }
            Ok(Section{c,svf,state:[0.;2]})
        }).collect::<Result<_,String>>()?;
        Ok(Self{sections,kernel:select_kernel()})
    }
    pub fn sample(&mut self,mut x:f32)->f32 {
        // SAFETY: select_kernel checks CPU support when the cascade is created.
        x=unsafe{(self.kernel)(&mut self.sections,x)};x
    }
}
#[inline(always)]
fn process(sections:&mut[Section],mut x:f32)->f32 {
        for section in sections {
            let [s1,s2]=section.state;let c=section.c;
            let (y,n1,n2)=if section.svf {
                let delta=x-s2;
                let n1=c[1].mul_add(delta,c[0].mul_add(s1,s1));
                let n2=c[2].mul_add(delta,c[1].mul_add(s1,s2));
                let y=(s2+n2).mul_add(c[5],(s1+n1).mul_add(c[4],x*c[3]));
                (y,n1,n2)
            } else {
                let y=x.mul_add(c[0],s1);
                let n1=(-y).mul_add(c[3],x.mul_add(c[1],s2));
                let n2=(-y).mul_add(c[4],x*c[2]);
                (y,n1,n2)
            };
            section.state=[n1,n2];x=y;
        }
        x
}

unsafe fn portable(sections:&mut[Section],x:f32)->f32 {process(sections,x)}
#[cfg(target_arch="x86_64")]
#[target_feature(enable="fma")]
unsafe fn hardware(sections:&mut[Section],x:f32)->f32 {process(sections,x)}
fn select_kernel()->unsafe fn(&mut[Section],f32)->f32 {
    static KERNEL:std::sync::OnceLock<unsafe fn(&mut[Section],f32)->f32>=std::sync::OnceLock::new();
    *KERNEL.get_or_init(|| {
        #[cfg(target_arch="x86_64")]
        if std::arch::is_x86_feature_detected!("fma") {return hardware;}
        portable
    })
}

pub(super) fn bank(rows:&[Vec<Sos>],rate:f64)->Result<Vec<Cascade>,String> {
    let count=rows.iter().map(Vec::len).max().unwrap_or(0);
    if rows.iter().any(|r|r.len()!=count) {return Err("Hall filter bank dimensions".into());}
    let indices:Vec<_>=(0..count).filter(|&i|rows.iter().any(|r| {
        let [b0,b1,b2,a0,a1,a2]=r[i];b0!=a0 || b1!=0. || b2!=0. || a1!=0. || a2!=0.
    })).collect();
    let mut prefix=0;
    for &i in &indices {
        if rows.iter().any(|r| {
            let a1=r[i][4]/r[i][3];let a2=r[i][5]/r[i][3];let disc=a1*a1-4.*a2;
            let angle=if disc>=0. {
                let root=disc.sqrt();if (-a1-root)*0.5 < -1e-12 || (root-a1)*0.5 < -1e-12 {std::f64::consts::PI}else{0.}
            } else {(-a1/(2.*a2.sqrt())).clamp(-1.,1.).acos()};
            angle>8168.140899333462/rate
        }) {break;}
        prefix+=1;
    }
    rows.iter().map(|r|Cascade::new(&indices.iter().map(|&i|r[i]).collect::<Vec<_>>(),prefix)).collect()
}
