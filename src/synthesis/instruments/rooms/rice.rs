use super::Response;

struct Bits<'a> { bytes: &'a [u8], at: usize }
impl Bits<'_> {
    fn bit(&mut self) -> Result<u32, String> {
        let byte = self.bytes.get(self.at / 8).ok_or("Truncated room impulse Rice stream")?;
        let value = (byte >> (7 - self.at % 8)) & 1;
        self.at += 1;
        Ok(u32::from(value))
    }
    fn integer(&mut self, rice: u32) -> Result<i64, String> {
        let mut quotient = 0u32;
        while self.bit()? != 0 {
            quotient = quotient.checked_add(1).filter(|q| *q <= (u32::MAX >> rice))
                .ok_or("Room impulse Rice integer overflow")?;
        }
        let mut value = quotient << rice;
        for shift in (0..rice).rev() { value |= self.bit()? << shift; }
        Ok(i64::from(value >> 1) ^ -i64::from(value & 1))
    }
}

pub(super) fn decode(bytes: &[u8], response: &Response) -> Result<Vec<Vec<f32>>, String> {
    if response.channels != 4 || response.rate != 48_000 || response.samples > response.rate || response.rice > 31 {
        return Err("Unsupported instrument room impulse dimensions".into());
    }
    let mut bits = Bits { bytes, at: 0 };
    let mut out = vec![vec![0.; response.rate]; response.channels];
    for channel in &mut out {
        let mut value = 0i64;
        for i in 0..response.samples {
            value = value.checked_add(bits.integer(response.rice)?)
                .ok_or("Room impulse delta overflow")?;
            let sample = ((value as f64 / 8_388_607.) * f64::from(response.scale)) as f32;
            if !sample.is_finite() { return Err("Nonfinite instrument room impulse".into()); }
            channel[(i + response.rate / 2 + response.rate - 1000) % response.rate] = sample;
        }
    }
    Ok(out)
}
