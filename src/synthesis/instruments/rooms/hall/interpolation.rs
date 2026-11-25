use std::f64::consts::PI;
pub(super) fn window()->[f64;32] {
    fn bessel(value:f64)->f64 {
        let mut term=1.;let mut total=1.;
        for n in (2..1000).step_by(2) {
            term=(value/n as f64).powi(2)*term;total+=term;
            if total*1e-18>term { break; }
        }
        total
    }
    let divisor=bessel(8.);
    std::array::from_fn(|i|bessel(8.*(1.-((i as f64-15.5)/15.5).powi(2)).max(0.).sqrt())/divisor)
}
pub(super) fn arrivals(delays:&[f64;3],weights:&[f64;3],gain:f64)->Result<(usize,Vec<f32>),String> {
    if delays.iter().any(|v|!v.is_finite() || !(15. ..=480000.).contains(v)) {return Err("Invalid Hall arrival delay".into());}
    let first=delays.iter().map(|v|v.floor() as usize-15).min().unwrap();
    let last=delays.iter().map(|v|v.floor() as usize-15).max().unwrap();
    let mut values=vec![0f64;last-first+32];let window=window();
    for (delay,weight) in delays.iter().zip(weights) {
        let integer=delay.floor();let start=integer as usize-15-first;
        for i in 0..32 {
            let phase=i as f64-15.+(integer-delay);
            let angle=phase*PI;let sinc=if phase==0. {1.}else{angle.sin()/angle};
            values[start+i]+=((weight*gain)*sinc)*window[i];
        }
    }
    Ok((first,values.into_iter().map(|v|v as f32).collect()))
}
