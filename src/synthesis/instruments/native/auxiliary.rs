use super::{Result,archive::Archive,id,program::Program,score::{self,Note,Record},tensor::Tensor};
use super::super::{Entry,InstrumentSettings};
use std::sync::{Arc,atomic::AtomicBool};

pub fn open(entry:&Entry)->Result<Arc<Archive>> {
    let path=entry.auxiliary.as_ref().ok_or("Instrument auxiliary model is not installed")?;
    let main=entry.main.canonicalize().map_err(|e|e.to_string())?;
    let auxiliary=path.canonicalize().map_err(|e|e.to_string())?;
    if main.parent().is_none() || main.parent()!=auxiliary.parent() {
        return Err("Instrument main and auxiliary models must be in the same version directory".into());
    }
    Archive::open(path)
}

pub fn execute(archive:&Arc<Archive>, program:&str, inputs:Vec<(u32,Tensor)>, output:u32,
    cancel:Arc<AtomicBool>)->Result<Tensor> {
    Program::new(archive.clone(),id(program),cancel)?.run(inputs,&[])?.remove(&output)
        .ok_or_else(||format!("Instrument output {output} absent"))
}

pub fn embedding(settings:&InstrumentSettings,entry:&Entry,aux:&Arc<Archive>,
    notes:&[Note],records:&[Record],cancel:Arc<AtomicBool>)->Result<Tensor> {
    let key=settings.key(&entry.selection);
    let basic=score::basic(records,8,notes);
    let mut input=vec![0.;records.len()*72];
    for row in 0..records.len() {
        for bit in 0..64 {input[row*72+bit]=((key>>(63-bit))&1)as f32;}
        input[row*72+64..(row+1)*72].copy_from_slice(&basic.data[row*8..(row+1)*8]);
    }
    let embedded=execute(aux,"6995abb671c2e5f5b08621023e47c736",
        vec![(700,Tensor::new(vec![1,records.len(),72],input))],710,cancel)?;
    if embedded.shape!=[1,records.len(),512]{return Err("Instrument auxiliary embedding shape".into());}
    let mut data=vec![0.;records.len()*512];
    for (row,record) in records.iter().enumerate() {
        if record.note.is_some() {
            for column in 0..512 {data[row*512+column]=half::f16::from_f32(embedded.get(row*512+column)).to_f32();}
        }
    }
    Ok(Tensor::new(vec![1,records.len(),512],data))
}

pub fn timing_input(notes:&[Note],records:&[Record],embedded:&Tensor)->Result<Tensor> {
    let mut data=vec![0.;records.len()*528];
    for (row,record) in records.iter().enumerate() {
        if let Some(note)=record.note {
            data[row*528..row*528+16].copy_from_slice(&super::super::articulation::features(&notes[note].articulations)?);
            data[row*528+16..(row+1)*528].copy_from_slice(&embedded.data[row*512..(row+1)*512]);
        }
    }
    Ok(Tensor::new(vec![1,records.len(),528],data))
}
