use super::{Result, fail, program::Node, tensor::Tensor};
use std::sync::Arc;

pub fn apply(n:&Node,x:&Tensor,reference:Option<&Tensor>)->Result<Tensor> {
    if x.shape.len()!=3 || n.p(0)>1 || n.p(3)>1 || n.p(7)==0 {
        return fail("Instrument padding mode");
    }
    let (batch,frames,channels)=(x.shape[0],x.shape[1],x.shape[2]);
    let target=reference.map_or(frames+n.p(2)+n.p(5),|x|x.shape[1]);
    let target=target.div_ceil(n.p(7))*n.p(7);
    let left=n.p(2).min(target);
    let covered=frames.min(target-left);
    let data=x.real_slice();
    let imaginary=x.imag_slice();
    let mut real=vec![0.;batch*target*channels];
    let mut imag=imaginary.as_ref().map(|_|vec![0.;real.len()]);
    for b in 0..batch {for t in 0..target {
        let (source,fill)=if t<left {
            ((n.p(0)==1 && frames>0).then_some(0),n.f(1))
        } else if t<left+covered {(Some(t-left),0.)}
        else {((n.p(3)==1 && frames>0).then_some(frames.saturating_sub(1)),n.f(4))};
        let destination=(b*target+t)*channels..(b*target+t+1)*channels;
        if let Some(t)=source {
            let source=(b*frames+t)*channels..(b*frames+t+1)*channels;
            real[destination.clone()].copy_from_slice(&data[source.clone()]);
            if let(Some(out),Some(input))=(&mut imag,&imaginary) {
                out[destination].copy_from_slice(&input[source]);
            }
        } else {
            real[destination].fill(if n.dtype==242{f32::from_bits(fill as u32)}else{fill});
        }
    }}
    let mut result=Tensor::new(vec![batch,target,channels],real);
    result.imag=imag.map(Arc::new);
    Ok(result)
}
