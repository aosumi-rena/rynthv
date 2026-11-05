use super::{Result,ensure,StdError};
pub(super) struct Reader<'a>{pub data:&'a[u8],pub at:usize}
impl<'a> Reader<'a> {
 pub fn new(data:&'a[u8])->Self{Self{data,at:0}}
 pub fn take(&mut self,n:usize)->Result<&'a[u8]> {
  let end=self.at.checked_add(n).ok_or_else(||StdError::Invalid("R1 offset overflow".into()))?;
  ensure(end<=self.data.len(),"Truncated R1 bank")?;let out=&self.data[self.at..end];self.at=end;Ok(out)
 }
 pub fn byte(&mut self)->Result<u8>{Ok(self.take(1)?[0])}
 pub fn u16(&mut self)->Result<u16>{Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))}
 pub fn u32(&mut self)->Result<u32>{Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))}
 pub fn f32(&mut self)->Result<f32>{let x=f32::from_bits(self.u32()?);ensure(x.is_finite(),"Nonfinite R1 bank parameter")?;Ok(x)}
 pub fn count(&mut self,max:usize)->Result<usize>{let n=self.u32()? as usize;ensure(n<=max,"R1 bank count exceeds capacity")?;Ok(n)}
 pub fn floats(&mut self,n:usize)->Result<Vec<f32>>{ensure(n<=self.data.len().saturating_sub(self.at)/4,"Truncated R1 float vector")?;(0..n).map(|_|self.f32()).collect()}
 pub fn words(&mut self,n:usize)->Result<Vec<u16>>{ensure(n<=self.data.len().saturating_sub(self.at)/2,"Truncated R1 phone vector")?;(0..n).map(|_|self.u16()).collect()}
 pub fn name(&mut self)->Result<String>{let n=self.data[self.at..].iter().position(|b|*b==0).ok_or_else(||StdError::Invalid("Missing R1 string terminator".into()))?;ensure(n<=255,"R1 identity exceeds capacity")?;
  let out=std::str::from_utf8(self.take(n)?).map_err(|_|StdError::Invalid("Invalid R1 UTF-8 identity".into()))?.to_owned();self.byte()?;Ok(out)}
 pub fn blob(&mut self)->Result<Vec<u8>>{let n=self.count(32*1024*1024)?;Ok(self.take(n)?.to_vec())}
}
