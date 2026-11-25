use super::geometry::{Plane, Point, dot};
use crate::synthesis::instruments::rooms::flxr::Program;

pub(super) struct Model {
    pub p: Program,
    pub walls: Vec<Plane>,
    pub rays: Vec<Point>,
    pub paths: Vec<Vec<usize>>,
    pub centers: Vec<f64>,
    pub microphones: Vec<Point>,
    pub axes: Vec<Point>,
    pub rate: f64,
    pub speed: f64,
    pub epsilon: f64,
}
impl Model {
    pub fn new(p: Program) -> Result<Self, String> {
        let normals=p.values(0x20d,&[6,3])?;
        let offsets=p.values(0x20e,&[6])?;
        let u=p.values(0x20f,&[6,3])?;
        let v=p.values(0x210,&[6,3])?;
        let counts=p.integers(0x211,&[6])?;
        let count=counts.iter().try_fold(0usize,|sum,&n| {
            if !(3..=1024).contains(&n) { return Err("Invalid Hall wall polygon"); }
            sum.checked_add(n as usize).ok_or("Hall polygon extent overflow")
        })?;
        let vertices=p.values(0x212,&[count,3])?;
        let mut at=0;
        let mut walls=Vec::new();
        for i in 0..6 {
            let u=point(&u,i);let v=point(&v,i);let n=counts[i] as usize;
            let polygon=(at..at+n).map(|j| [dot(point(&vertices,j),u),dot(point(&vertices,j),v)]).collect();
            walls.push(Plane{normal:point(&normals,i),offset:offsets[i],u,v,polygon});at+=n;
        }
        let count=usize::try_from(p.integers(0x314,&[])?[0]).map_err(|_|"Invalid Hall ray count")?;
        if !(1..=65536).contains(&count) { return Err("Invalid Hall ray count".into()); }
        let rays=(0..count).map(|i| {
            let z=1.-2.*(i as f64+0.5)/count as f64;
            let phase=(i as f64+0.5)*10.166407384630519;
            let radial=(1.-z*z).sqrt();
            [radial*phase.cos(),radial*phase.sin(),z]
        }).collect();
        let order=p.integers(0x30a,&[])?[0];
        if !(1..=4).contains(&order) { return Err("Invalid Hall reflection order".into()); }
        let mut paths=Vec::new();let mut previous=vec![Vec::new()];
        for _ in 0..order {
            let mut next=Vec::new();
            for prefix in &previous { for i in 0..6 {
                if prefix.last()==Some(&i) { continue; }
                let mut path=prefix.clone();path.push(i);next.push(path);
            } }
            paths.extend(next.iter().cloned());previous=next;
        }
        let centers=p.values(0x213,&[13])?;
        let microphones=p.values(0x206,&[7,3])?.chunks_exact(3).map(|v|v.try_into().unwrap()).collect();
        let axes=p.values(0x20a,&[4,3])?.chunks_exact(3).map(|v|v.try_into().unwrap()).collect();
        let rate=p.integers(0x201,&[])?[0] as f64;let speed=p.values(0x205,&[])?[0];
        let epsilon=p.values(0x309,&[])?[0];
        if rate!=48000. || speed<=0. || !(0. ..0.01).contains(&epsilon) {
            return Err("Invalid Hall physical constants".into());
        }
        Ok(Self{p,walls,rays,paths,centers,microphones,axes,rate,speed,epsilon})
    }
    pub fn normalized(&self, position: [f64;2]) -> Result<[f64;2],String> {
        let mut output=[0.;2];
        for i in 0..2 {
            let bounds=self.p.values(0x30b+i as u32,&[2])?;
            if bounds[1]<=bounds[0] { return Err("Invalid Hall position bounds".into()); }
            output[i]=(2.*(position[i]-bounds[0])/(bounds[1]-bounds[0])-1.).clamp(-1.,1.);
        }
        Ok(output)
    }
}
pub(super) fn point(values: &[f64], i: usize) -> Point { values[i*3..i*3+3].try_into().unwrap() }
pub(super) fn affine(row: &[f64], normalized: [f64;2]) -> f64 {
    (row[1]*normalized[0]+row[0])+row[2]*normalized[1]
}
