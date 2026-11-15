use super::{Result, fail, tensor::{Tensor, affine}};
use rayon::prelude::*;
use std::borrow::Cow;

pub fn run(x:&Tensor,w:&[f32],bias:Option<&[f32]>,shape:&[usize],stride:usize,
    dilation:usize,groups:usize,transpose:bool,left:usize,right:usize)->Result<Tensor> {
    if shape.len()!=3 || x.shape.len()!=3 || stride==0 || dilation==0 || groups==0 {
        return fail("Instrument convolution shape/clock");
    }
    let (k,o,input)=(shape[0],shape[1],shape[2]);
    if input%groups!=0 || o%groups!=0 || x.shape[2]!=input {
        return fail("Instrument convolution channels");
    }
    let (c,per,batch,t)=(input/groups,o/groups,x.shape[0],x.shape[1]);
    let frames=if transpose {(t-1)*stride+(k-1)*dilation+1}
        else {t.saturating_sub((k-1)*dilation+1)/stride+usize::from(t>=(k-1)*dilation+1)};
    let xd=x.real_slice();
    if transpose && dilation==1 && input==o && groups==input {
        if left+right>=frames {return fail("Transposed convolution crop");}
        let kept=frames-left-right;
        let mut y=vec![0.;batch*kept*o];
        y.par_chunks_mut(64*o).enumerate().for_each(|(block,ys)| {
            for (i,row) in ys.chunks_exact_mut(o).enumerate() {
                let flat=block*64+i;
                let (b,r)=(flat/kept,left+flat%kept);
                let minimum=r.saturating_sub((t-1)*stride);
                let maximum=r.min(k-1);
                let first=minimum+(r-minimum)%stride;
                if first<=maximum {
                    for j in (first..=maximum).step_by(stride) {
                        let at=(b*t+(r-j)/stride)*input;
                        for ((v,a),w) in row.iter_mut().zip(&xd[at..at+input]).zip(&w[j*o..(j+1)*o]) {
                            *v+=a*w;
                        }
                    }
                }
                if let Some(bias)=bias {for (v,b) in row.iter_mut().zip(bias){*v+=b;}}
            }
        });
        return Ok(Tensor::new(vec![batch,kept,o],y));
    }
    let mut y=vec![0.;batch*frames*o];
    for j in 0..k {for g in 0..groups {
        let rows=if transpose {batch*t}else{batch*frames};
        let q=if batch==1 && groups==1 && (transpose || stride==1) {
            let begin=(if transpose{0}else{j*dilation*input}).min(xd.len());
            Cow::Borrowed(&xd[begin..begin+rows*c])
        } else {
            let mut q=Vec::with_capacity(rows*c);
            for b in 0..batch {for ti in 0..if transpose{t}else{frames} {
                let xt=if transpose{ti}else{j*dilation+ti*stride};
                q.extend_from_slice(&xd[(b*t+xt)*input+g*c..(b*t+xt)*input+(g+1)*c]);
            }}
            Cow::Owned(q)
        };
        let mat=&w[(j*o+g*per)*c..(j*o+(g+1)*per)*c];
        let z=affine(&q,rows,c,mat,per,None);
        for b in 0..batch {for ti in 0..if transpose{t}else{frames} {
            let yt=if transpose{j*dilation+ti*stride}else{ti};
            let zr=(b*if transpose{t}else{frames}+ti)*per;
            let yr=(b*frames+yt)*o+g*per;
            for ch in 0..per {y[yr+ch]+=z[zr+ch];}
        }}
    }}
    if let Some(bias)=bias {for row in y.chunks_mut(o){for (v,b) in row.iter_mut().zip(bias){*v+=b;}}}
    if transpose && (left!=0 || right!=0) {
        if left+right>=frames {return fail("Transposed convolution crop");}
        let mut crop=Vec::with_capacity(batch*(frames-left-right)*o);
        for b in 0..batch {crop.extend_from_slice(&y[(b*frames+left)*o..(b*frames+frames-right)*o]);}
        return Ok(Tensor::new(vec![batch,frames-left-right,o],crop));
    }
    Ok(Tensor::new(vec![batch,frames,o],y))
}
