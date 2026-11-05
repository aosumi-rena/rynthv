use std::{collections::BTreeMap, sync::{Arc,Mutex,OnceLock}};
pub(super) const RADIUS:i64=32;
pub(super) const PHASES:usize=1024;
type Table=Arc<Vec<Vec<f64>>>;
pub(super) fn get(source:u32,target:u32)->Table {
    static TABLES:OnceLock<Mutex<BTreeMap<(u32,u32),Table>>>=OnceLock::new();
    let mut tables=TABLES.get_or_init(Default::default).lock().unwrap();
    if let Some(table)=tables.get(&(source,target)) {return Arc::clone(table);}
    let cutoff=(f64::from(target)/f64::from(source)).min(1.0)*0.96;
    let table=Arc::new((0..=PHASES).map(|phase| {
        let fraction=phase as f64/PHASES as f64;
        let mut weights:Vec<f64>=(-RADIUS+1..=RADIUS).map(|tap| {
            let x=tap as f64-fraction;let z=std::f64::consts::PI*x*cutoff;
            let sinc=if z.abs()<1e-12 {1.0}else{z.sin()/z};
            let window=0.42+0.5*(std::f64::consts::PI*x/RADIUS as f64).cos()
                +0.08*(2.0*std::f64::consts::PI*x/RADIUS as f64).cos();
            cutoff*sinc*window
        }).collect();
        let sum:f64=weights.iter().sum();for w in &mut weights {*w/=sum;}weights
    }).collect());
    if tables.len()<8 {tables.insert((source,target),Arc::clone(&table));} table
}
