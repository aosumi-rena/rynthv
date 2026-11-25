pub(super) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub at: usize,
}
impl<'a> Reader<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).filter(|end| *end <= self.bytes.len())
            .ok_or("Truncated instrument room resource")?;
        let value = &self.bytes[self.at..end];
        self.at = end;
        Ok(value)
    }
    pub fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn count(&mut self, maximum: usize) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n > maximum { return Err("Instrument room resource count exceeds limit".into()); }
        Ok(n)
    }
    pub fn f32(&mut self) -> Result<f32, String> {
        let value = f32::from_bits(self.u32()?);
        if !value.is_finite() { return Err("Nonfinite instrument room metadata".into()); }
        Ok(value)
    }
    pub fn text(&mut self) -> Result<String, String> {
        let n = self.count(1024)?;
        let value = std::str::from_utf8(self.take(n)?)
            .map_err(|_| "Invalid instrument room UTF-8")?;
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err("Invalid instrument room name".into());
        }
        Ok(value.into())
    }
}
