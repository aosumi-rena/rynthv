use super::super::{Error, Result};

pub(super) struct Node { pub op:String, pub body:Vec<u8>, pub children:Vec<Node> }
impl Node {
    pub fn parse(bytes:&[u8])->Result<Self> {
        if bytes.len()<8 || word(bytes,0)?!=0x7fca00ff || word(bytes,4)?!=1 {return Err(Error::Format("loose F0 v1 header"));}
        fn read(bytes:&[u8],at:&mut usize,depth:usize)->Result<Node> {
            if depth>64 || *at+20>bytes.len(){return Err(Error::Format("loose F0 tree extent"));}
            let marker=word(bytes,*at)?;
            if marker!=0x7fca40ff && marker!=0x7fca41ff{return Err(Error::Format("loose F0 node marker"));}
            let op=std::str::from_utf8(&bytes[*at+4..*at+12]).map_err(|_|Error::Format("F0 node tag"))?.trim_end_matches('\0').to_owned();
            let count=word(bytes,*at+12)? as usize;let len=word(bytes,*at+16)? as usize;*at+=20;
            let end=at.checked_add(len).filter(|v|*v<=bytes.len()).ok_or(Error::Format("F0 payload extent"))?;
            let body=bytes[*at..end].to_vec();*at=end;
            if count>4096{return Err(Error::Format("F0 child count"));}
            let children=(0..count).map(|_|read(bytes,at,depth+1)).collect::<Result<_>>()?;
            Ok(Node{op,body,children})
        }
        let mut at=8;let root=read(bytes,&mut at,0)?;
        if at!=bytes.len(){return Err(Error::Format("F0 trailing bytes"));}Ok(root)
    }
    pub fn require(&self,op:&str,count:usize)->Result<()> {
        if self.op!=op || self.children.len()!=count{return Err(Error::Format("F0 graph geometry"));}Ok(())
    }
    pub fn vector(&self)->Result<Vec<f32>> {
        self.require("prim1",0)?;let n=word(&self.body,0)? as usize;
        if n>65536 || self.body.len()!=4+4*n{return Err(Error::Format("F0 vector extent"));}
        (0..n).map(|i|float(&self.body,4+4*i)).collect()
    }
}
pub(super) fn word(bytes:&[u8],at:usize)->Result<u32>{Ok(u32::from_le_bytes(bytes.get(at..at+4).ok_or(Error::Format("F0 scalar extent"))?.try_into().unwrap()))}
pub(super) fn float(bytes:&[u8],at:usize)->Result<f32>{let x=f32::from_bits(word(bytes,at)?);if !x.is_finite(){return Err(Error::Format("F0 nonfinite model"));}Ok(x)}
pub(super) fn text(bytes:&[u8],at:&mut usize)->Result<String>{
    let n=word(bytes,*at)? as usize;*at+=4;let end=at.checked_add(n).filter(|n|*n<=bytes.len()).ok_or(Error::Format("F0 string extent"))?;
    let value=std::str::from_utf8(&bytes[*at..end]).map_err(|_|Error::Format("F0 string encoding"))?.to_owned();*at=end;Ok(value)
}
pub(super) fn strings(bytes:&[u8],at:&mut usize)->Result<Vec<String>>{
    let n=word(bytes,*at)? as usize;*at+=4;if n>4096{return Err(Error::Format("F0 string count"));}(0..n).map(|_|text(bytes,at)).collect()
}

pub(super) struct Matrix {pub input:usize,pub output:usize,exponent:i32,scales:Vec<f32>,weights:Vec<i16>}
impl Matrix {
    pub fn read(node:&Node)->Result<Self>{
        node.require("prim4",0)?;let b=&node.body;let bits=word(b,0)?;
        let exponent=word(b,4)? as i32;let output=word(b,12)? as usize;
        if !matches!(bits,8|16)||!(-16..=24).contains(&exponent)||output==0||output>512||word(b,8)?!=0{return Err(Error::Format("F0 packed matrix header"));}
        let dims=16+4*output;
        if word(b,dims)? as usize!=output{return Err(Error::Format("F0 packed output width"));}
        let input=word(b,dims+4)? as usize;
        if input==0||input>512||b.len()!=dims+8+input*output*(bits as usize/8){return Err(Error::Format("F0 packed matrix extent"));}
        let scales=(0..output).map(|o|Ok(float(b,16+o*4)?*2.0f32.powi(-exponent))).collect::<Result<Vec<_>>>()?;
        let weights=(0..input*output).map(|i|if bits==8 {(b[dims+8+i] as i8 as i16).wrapping_shl(8)}else{i16::from_le_bytes(b[dims+8+2*i..dims+10+2*i].try_into().unwrap())}).collect();
        Ok(Self{input,output,exponent,scales,weights})
    }
    pub fn apply(&self,x:&[f32])->Result<Vec<f32>>{
        let mut result=vec![0.0;self.output];self.accumulate(x,&mut result,false)?;Ok(result)
    }
    pub fn accumulate(&self,x:&[f32],out:&mut[f32],add:bool)->Result<()> {
        if x.len()!=self.input||out.len()!=self.output{return Err(Error::Format("F0 projection shape"));}
        let mut raw=[0i16;512];let mut quantized=[0i16;512];let scale=2.0f32.powi(self.exponent);
        for (i,&x) in x.iter().enumerate(){
            let x=(x*scale).round_ties_even();if !x.is_finite(){return Err(Error::Format("F0 projection input"));}
            quantized[i]=x.clamp(-32768.0,32767.0) as i16;
        }
        super::packed::dot(&quantized[..self.input],&self.weights,&mut raw[..self.output]);
        for o in 0..self.output {out[o]=if add {(raw[o] as f32).mul_add(self.scales[o],out[o])}else{raw[o] as f32*self.scales[o]};}
        Ok(())
    }
}
pub(super) struct Linear {pub matrix:Matrix,pub bias:Vec<f32>}
impl Linear {
    pub fn read(node:&Node)->Result<Self>{
        if node.op!="modl0"||!(1..=2).contains(&node.children.len()){return Err(Error::Format("F0 linear geometry"));}
        let matrix=Matrix::read(&node.children[0])?;
        let bias=if node.children.len()==2{node.children[1].vector()?}else{vec![0.0;matrix.output]};
        if bias.len()!=matrix.output{return Err(Error::Format("F0 linear bias"));}Ok(Self{matrix,bias})
    }
    pub fn apply(&self,x:&[f32])->Result<Vec<f32>>{let mut y=self.matrix.apply(x)?;for(y,b)in y.iter_mut().zip(&self.bias){*y+=b;}Ok(y)}
}
pub(super) struct Conv {pub taps:Vec<Matrix>,pub bias:Vec<f32>,pub dilation:usize}
impl Conv {
    pub fn read(node:&Node)->Result<Self>{
        if node.op!="modl1"||node.body.len()!=20{return Err(Error::Format("F0 convolution"));}
        let k=word(&node.body,0)? as usize;let dilation=(word(&node.body,8)? as usize).max(1);
        if !matches!(k,1|3)||node.children.len()!=k+1||dilation>81{return Err(Error::Format("F0 convolution geometry"));}
        let taps=node.children[..k].iter().map(Matrix::read).collect::<Result<Vec<_>>>()?;let bias=node.children[k].vector()?;
        if taps.iter().any(|m|m.output!=bias.len()||m.input!=taps[0].input){return Err(Error::Format("F0 convolution widths"));}
        Ok(Self{taps,bias,dilation})
    }
    pub fn sequence(&self,x:&[Vec<f32>],cancel:&std::sync::atomic::AtomicBool)->Result<Vec<Vec<f32>>>{
        let zero=vec![0.0;self.taps[0].input];let center=(self.taps.len()/2)*self.dilation;
        (0..x.len()).map(|i|{
            if i%64==0 && cancel.load(std::sync::atomic::Ordering::Relaxed){return Err(Error::Cancelled);}
            let mut y=vec![0.0;self.bias.len()];
            for(t,m)in self.taps.iter().enumerate(){let index=(i+t*self.dilation)as isize-center as isize;
                m.accumulate(if index<0 {&zero}else{x.get(index as usize).unwrap_or(&zero)},&mut y,t!=0)?;}
            for(y,b)in y.iter_mut().zip(&self.bias){*y+=b;}Ok(y)
        }).collect()
    }
    pub fn point(&self,x:&[f32])->Result<Vec<f32>>{let mut y=self.taps[0].apply(x)?;for(y,b)in y.iter_mut().zip(&self.bias){*y+=b;}Ok(y)}
}
