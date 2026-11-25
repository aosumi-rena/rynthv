pub(super) type Point = [f64; 3];
pub(super) fn dot(a: Point, b: Point) -> f64 { (a[0]*b[0]+a[1]*b[1])+a[2]*b[2] }
pub(super) fn sub(a: Point, b: Point) -> Point { std::array::from_fn(|i| a[i]-b[i]) }
pub(super) fn length(a: Point) -> f64 { dot(a,a).sqrt() }
pub(super) fn angle(a: Point, b: Point) -> f64 { dot(a,b).clamp(-1.,1.).acos()*57.29577951308232 }
pub(super) fn unit(a: Point) -> Point { let n=length(a); a.map(|v|v/n) }

pub(super) struct Plane {
    pub normal: Point,
    pub offset: f64,
    pub u: Point,
    pub v: Point,
    pub polygon: Vec<[f64;2]>,
}
impl Plane {
    pub fn contains(&self, p: Point) -> bool {
        let [x,y]=[dot(p,self.u),dot(p,self.v)];
        let mut inside=false;
        for i in 0..self.polygon.len() {
            let [px,py]=self.polygon[i];
            let [qx,qy]=self.polygon[(i+1)%self.polygon.len()];
            if (y<py)!=(y<qy) && (qx-px)*(y-py)/(qy-py)+px>x { inside=!inside; }
        }
        inside
    }
    pub fn hit(&self, a: Point, b: Point, epsilon: f64) -> Option<Point> {
        let da=dot(a,self.normal)-self.offset;
        let db=dot(b,self.normal)-self.offset;
        let denominator=da-db;
        if denominator.abs()<1e-12 { return None; }
        let t=da/denominator;
        if t<=epsilon || t>=1.-epsilon || (da>0.)==(db>0.) { return None; }
        let p=std::array::from_fn(|i| a[i]+(b[i]-a[i])*t);
        self.contains(p).then_some(p)
    }
}

pub(super) fn spline(centers: &[f64], weights: &[f64], angle: f64) -> f64 {
    centers.iter().zip(weights).fold(0., |sum,(&c,&w)| sum+(1.-(angle-c).abs()/15.).max(0.)*w)
}
