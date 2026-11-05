use super::{Result,ensure,reader::Reader};
#[derive(Clone)]
struct Layer {w:Vec<f32>,b:Vec<f32>}
impl Layer {
 fn apply(&self,x:&[f32])->Result<Vec<f32>> {ensure(self.w.len()==self.b.len()*x.len(),"R1 model input dimension mismatch")?;
  let mut out=vec![0.;self.b.len()];for(c,v)in x.iter().enumerate(){for(r,o)in out.iter_mut().enumerate(){*o+=self.w[c*self.b.len()+r] * *v;}}
  for(o,b)in out.iter_mut().zip(&self.b){*o+=b;}ensure(out.iter().all(|x|x.is_finite()),"Nonfinite R1 model output")?;Ok(out)
 }
 fn column(&self,i:usize)->Vec<f32>{self.b.iter().enumerate().map(|(r,b)|self.w[i*self.b.len()+r]+b).collect()}
 fn read(r:&mut Reader,rows:usize,columns:usize)->Result<Self>{Ok(Self{w:r.floats(rows*columns)?,b:r.floats(rows)?})}
}
#[derive(Clone)]
pub(super) struct Encoding {center:Vec<f32>,right:Vec<f32>,left:Vec<f32>}
#[derive(Clone)]
pub(super) struct Qsa {pub selectors:usize,embedding:Layer,center:Layer,left:[Layer;3],right:[Layer;3],scorer:Layer}
impl Qsa {
 pub fn read(data:&[u8])->Result<Self>{let mut r=Reader::new(data);let mut d=[0usize;6];for v in &mut d{let n=r.f32()?;ensure(n>=1.&&n<=4096.&&n==n.trunc(),"Invalid R1 QSA dimension")?;*v=n as usize;}
  ensure(d[5]==3,"Unsupported R1 QSA output geometry")?;let(n,e,c,a,b)=(d[0],d[1],d[2],d[3],d[4]);
  let out=Self{selectors:n,embedding:Layer::read(&mut r,e,n)?,center:Layer::read(&mut r,c,n)?,
   left:[Layer::read(&mut r,a,e*2)?,Layer::read(&mut r,b,a)?,Layer::read(&mut r,c,b)?],
   right:[Layer::read(&mut r,a,e*2)?,Layer::read(&mut r,b,a)?,Layer::read(&mut r,c,b)?],scorer:Layer::read(&mut r,3,c)?};
  ensure(r.at==data.len(),"R1 QSA parameter extent mismatch")?;Ok(out)
 }
 pub fn encode(&self,phones:[usize;3])->Result<Encoding>{ensure(phones.iter().all(|i|*i<self.selectors),"R1 QSA phone outside vocabulary")?;
  let middle=self.embedding.column(phones[1]);
  let side=|phone:usize,l:&[Layer;3]|->Result<Vec<f32>>{let mut input=middle.clone();input.extend(self.embedding.column(phone));
   let h=l[0].apply(&input)?.into_iter().map(f32::tanh).collect::<Vec<_>>();let h=l[1].apply(&h)?.into_iter().map(f32::tanh).collect::<Vec<_>>();l[2].apply(&h)};
  Ok(Encoding{center:self.center.column(phones[1]),right:side(phones[2],&self.right)?,left:side(phones[0],&self.left)?})
 }
 pub fn compare(&self,a:&Encoding,b:&Encoding)->Result<[f32;3]>{let d=(0..a.center.len()).map(|i|{
  let(x,y,z)=(a.center[i]-b.center[i],a.right[i]-b.right[i],a.left[i]-b.left[i]);(x*x+y*y+z*z).min(5.)}).collect::<Vec<_>>();
  let v=self.scorer.apply(&d)?;let out=[v[0].exp(),v[1].exp(),v[2].exp()];ensure(out.iter().all(|x|x.is_finite()),"R1 QSA cost overflow")?;Ok(out)
 }
}
#[derive(Clone)]
pub(super) struct Kg {pub inputs:usize,first:Layer,second:Layer}
impl Kg {
 pub fn read(data:&[u8])->Result<Self>{let mut r=Reader::new(data);let i=r.f32()?;let h=r.f32()?;
  ensure(i>=1.&&i<=4096.&&h>=1.&&h<=4096.&&i==i.trunc()&&h==h.trunc(),"Invalid R1 KG dimensions")?;
  let out=Self{inputs:i as usize,first:Layer::read(&mut r,h as usize,i as usize)?,second:Layer::read(&mut r,3,h as usize)?};
  ensure(r.at==data.len(),"R1 KG parameter extent mismatch")?;Ok(out)
 }
 pub fn compare(&self,a:&[f32],b:&[f32])->Result<[f32;3]>{ensure(a.len()==self.inputs&&b.len()==self.inputs,"R1 feature dimension mismatch")?;
  let x=a.iter().zip(b).map(|(a,b)|((a-b)*(a-b)).min(5.)).collect::<Vec<_>>();let h=self.first.apply(&x)?.into_iter().map(f32::tanh).collect::<Vec<_>>();
  let v=self.second.apply(&h)?;let out=[v[0].exp(),v[1].exp(),v[2].exp()];ensure(out.iter().all(|x|x.is_finite()),"R1 KG cost overflow")?;Ok(out)
 }
}
