use super::{Result,archive::Archive,auxiliary,metadata::{self,Value},score::{self,Note,Record},tensor::Tensor};
use super::super::{Entry,InstrumentSettings};
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};

#[derive(Clone,Debug,serde::Serialize)]
pub struct Prediction {pub index:usize,pub articulations:Option<Vec<String>>,pub dynamic:Option<f32>}

fn catalogue(root:&Value)->Result<Vec<(usize,u32,f32)>> {
    fn collect(value:&Value,out:&mut Vec<Vec<(usize,u32,f32)>>) {
        match value {
            Value::Map(map)=>for v in map.values(){collect(v,out)},
            Value::Array(array)=>{
                let Some(Value::Map(last))=array.last() else{return};
                if last.len()!=3{return;}
                let Some((id_key,_))=last.iter().find(|(_,v)|matches!(v,Value::Int(4294967295))) else{return};
                let Some((channel_key,_))=last.iter().find(|(_,v)|matches!(v,Value::Int(16))) else{return};
                let Some((value_key,_))=last.iter().find(|(_,v)|matches!(v,Value::Float(v) if *v==666.666)) else{return};
                let mut rows=Vec::new();
                for row in &array[..array.len()-1] {
                    let Value::Map(row)=row else{return};
                    let (Some(Value::Int(id)),Some(Value::Int(channel)),Some(Value::Float(value)))=
                        (row.get(id_key),row.get(channel_key),row.get(value_key)) else{return};
                    if !(0..16).contains(channel)||!(0..u32::MAX as i64).contains(id)||!value.is_finite(){return;}
                    rows.push((*channel as usize,*id as u32,*value as f32));
                }
                rows.sort_by_key(|v|(v.0,v.1));out.push(rows);
            },
            _=>{}
        }
    }
    let mut found=Vec::new();collect(root,&mut found);
    let first=found.first().ok_or("Instrument articulation vocabulary is absent")?;
    if first.is_empty()||found.iter().any(|row|row!=first){return Err("Instrument articulation vocabularies disagree".into());}
    Ok(first.clone())
}
fn name(id:u32)->Result<&'static str> {
    Ok(match id {
        0=>"Slur",2=>"Accent",4=>"Staccato",5=>"Tenuto",6=>"Fall",
        15=>"Trill Major",16=>"Trill Minor",17=>"Tremolo",21=>"Breath",22=>"Harmonics",
        _=>return Err(format!("Unsupported generated instrument articulation {id}")),
    })
}

pub(super) fn articulate(main:&Archive,aux:&Arc<Archive>,notes:&mut [Note],records:&[Record],
    embedded:&Tensor,selected:&[bool],cancel:Arc<AtomicBool>)->Result<()> {
    let seeds=score::record_seeds(notes,records,0);
    let output=auxiliary::execute(aux,"707be5f834616c3a08bedbeaec271d29",
        vec![(600,embedded.clone()),(610,seeds)],620,cancel)?;
    if output.shape!=[1,records.len(),16]{return Err("Instrument articulation output shape".into());}
    let (root,_)=metadata::description(main)?;
    let table=catalogue(&root)?;
    for (row,record) in records.iter().enumerate() {
        let Some(index)=record.note.filter(|i|selected[*i]) else{continue};
        let mut names=Vec::new();
        for &(channel,id,value) in &table {
            let prediction=output.get(row*16+channel);
            if !prediction.is_finite(){return Err("Nonfinite instrument articulation prediction".into());}
            if prediction.round()==value{names.push(name(id)?.to_owned());}
        }
        notes[index].articulations=names;
    }
    Ok(())
}

pub fn predict(settings:&InstrumentSettings,entry:&Entry,mut notes:Vec<Note>,
    selected:&[bool],articulation:bool,dynamics:bool,cancel:Arc<AtomicBool>)->Result<Vec<Prediction>> {
    if selected.len()!=notes.len()||!selected.iter().any(|v|*v)||!(articulation||dynamics){return Err("No instrument notes selected for rewrite".into());}
    let records=score::records(&notes)?;
    let aux=auxiliary::open(entry, settings.accept_any_v4_signature)?;
    let embedded=auxiliary::embedding(settings,entry,&aux,&notes,&records,cancel.clone())?;
    if articulation {
        let main = Archive::open(&entry.main, settings.accept_any_v4_signature)?;
        articulate(&main,&aux,&mut notes,&records,&embedded,selected,cancel.clone())?;
    }
    let timing=if dynamics {
        let input=auxiliary::timing_input(&notes,&records,&embedded)?;
        let seeds=score::record_seeds(&notes,&records,1);
        let output=auxiliary::execute(&aux,"6d673c66a228e5d47c9a714619edef29",
            vec![(800,input),(810,seeds)],820,cancel.clone())?;
        if output.shape!=[1,records.len(),2]{return Err("Instrument dynamics output shape".into());}
        Some(output)
    }else{None};
    if cancel.load(Ordering::Relaxed){return Err("Instrument rewrite cancelled".into());}
    let mut result=Vec::new();
    for (row,record) in records.iter().enumerate() {
        let Some(index)=record.note.filter(|i|selected[*i]) else{continue};
        let dynamic=timing.as_ref().map(|t|t.get(row*2));
        if dynamic.is_some_and(|d|!d.is_finite()||!(0.0..=1.0).contains(&d)){return Err("Invalid instrument dynamics prediction".into());}
        result.push(Prediction {index,articulations:articulation.then(||notes[index].articulations.clone()),dynamic});
    }
    Ok(result)
}
