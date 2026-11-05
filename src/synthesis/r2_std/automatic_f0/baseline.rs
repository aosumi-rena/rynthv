use super::{Record,super::{Error,Result}};
pub(super) fn build(records:&[Record],hop:f64,count:usize)->Result<Vec<f32>>{
    let mut result=vec![0.0;count];let mut origin=0.0;let inverse=1.0/hop;
    for(index,current)in records.iter().enumerate(){
        let last=index+1==records.len();let next=&records[(index+1).min(records.len()-1)];
        let duration=current.duration;let next_duration=if last{count as f64*hop-origin-duration}else{next.duration};
        let end=origin+duration;
        let a=(inverse*origin)as usize;let raw_b=(end*inverse)as usize;
        let c=(((next_duration+duration)+origin)*inverse)as usize;let c=c.min(count);let b=raw_b.min(c);
        if a>b||b>c{return Err(Error::Format("automatic F0 baseline clock"));}
        let left=b-a;let right=c-b;let mut p=f64::from(current.pitch);let mut q=f64::from(next.pitch);
        let mut pair=vec![0.0;c-a];
        if current.boundary{
            p=f64::from(records[index.saturating_sub(1)].pitch);
            for k in 0..left{pair[k]=(p+(q-p)*k as f64/left as f64)as f32;}
            pair[left..].fill(q as f32);
        }else if next.boundary{
            q=f64::from(records[(index+2).min(records.len()-1)].pitch);
            pair[..left].fill(p as f32);
            for k in 0..right{pair[left+k]=(p+(q-p)*k as f64/right as f64)as f32;}
        }else{
            let left_width=current.right*0.3;let right_width=if last{0.025}else{next.left*0.3};
            let slope=(3.0/(right_width+left_width))as f32;
            for(k,v)in pair.iter_mut().enumerate(){let time=(k as f64*hop)as f32;
                let argument=((f64::from(time)-duration)*f64::from(slope))as f32;
                let fraction=if argument< -10.0{0.0}else if argument>10.0{1.0}else{1.0/(1.0+(-argument).exp())};
                *v=(f64::from(fraction)*(q-p)+p)as f32;
            }
        }
        let pi=f64::from_bits(0x400921f9f01b866e);
        for k in 0..left{let frame=a+k;let mut value=pair[k];
            if index!=0{let t=(frame as f64*hop)as f32;let fraction=((f64::from(t)-origin)/duration)as f32;
                value*=(((f64::from(fraction)+1.0)*pi).cos()*0.5+0.5)as f32;}
            result[frame]+=value;
        }
        for k in 0..right{let frame=b+k;let mut value=pair[left+k];
            if !last{let t=(frame as f64*hop)as f32;let fraction=((f64::from(t)-origin-duration)/next_duration)as f32;
                value*=((f64::from(fraction)*pi).cos()*0.5+0.5)as f32;}
            result[frame]+=value;
        }origin=end;
    }Ok(result)
}
