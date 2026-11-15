use serde_json::{Map, Value, json};
type Result<T> = std::result::Result<T, String>;
const LIMIT: usize = 64 * 1024 * 1024;
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    items: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|e| *e <= self.bytes.len())
            .ok_or("Truncated TLPX")?;
        let start = self.at;
        self.at = end;
        Ok(&self.bytes[start..end])
    }
    fn size(&mut self, info: u8) -> Result<u64> {
        Ok(match info {
            0..=23 => info as u64,
            24 => self.take(1)?[0] as u64,
            25 => u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as u64,
            26 => u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as u64,
            27 => u64::from_be_bytes(self.take(8)?.try_into().unwrap()),
            _ => return Err("Invalid TLPX length".into()),
        })
    }
    fn value(&mut self, depth: usize) -> Result<Value> {
        self.items += 1;
        if depth > 96 || self.items > LIMIT / 4 {
            return Err("TLPX nesting/item limit".into());
        }
        let tag = self.take(1)?[0];
        let (major, info) = (tag >> 5, tag & 31);
        if major == 7 {
            return match info {
                20 => Ok(json!(false)),
                21 => Ok(json!(true)),
                22 => Ok(Value::Null),
                25..=27 => {
                    let bits = self.size(info)?;
                    let x = match info {
                        25 => half::f16::from_bits(bits as u16).to_f64(),
                        26 => f32::from_bits(bits as u32) as f64,
                        _ => f64::from_bits(bits),
                    };
                    serde_json::Number::from_f64(x)
                        .map(Value::Number)
                        .ok_or("Nonfinite TLPX number".into())
                }
                _ => Err("Unsupported TLPX scalar".into()),
            };
        }
        let n = if info == 31 {
            None
        } else {
            Some(self.size(info)?)
        };
        match major {
            0 => Ok(json!(n.ok_or("Invalid TLPX integer")?)),
            1 => {
                let n = n
                    .filter(|n| *n <= i64::MAX as u64)
                    .ok_or("TLPX integer overflow")?;
                Ok(json!(-1i64 - n as i64))
            }
            2 | 3 => {
                let mut bytes = Vec::new();
                if let Some(n) = n {
                    let n = usize::try_from(n)
                        .ok()
                        .filter(|n| *n <= LIMIT)
                        .ok_or("TLPX string limit")?;
                    bytes.extend(self.take(n)?);
                } else {
                    while self.bytes.get(self.at) != Some(&255) {
                        let tag = self.take(1)?[0];
                        if tag >> 5 != major || tag & 31 == 31 {
                            return Err("Invalid TLPX string chunk".into());
                        }
                        let count = usize::try_from(self.size(tag & 31)?)
                            .map_err(|_| "TLPX string limit")?;
                        if bytes.len().saturating_add(count) > LIMIT {
                            return Err("TLPX string limit".into());
                        }
                        bytes.extend(self.take(count)?);
                    }
                    self.take(1)?;
                }
                if major == 3 {
                    Ok(Value::String(
                        String::from_utf8(bytes).map_err(|_| "Invalid TLPX UTF-8")?,
                    ))
                } else {
                    Ok(
                        json!({"__rynthvCborBytes": bytes.iter().map(|b|format!("{b:02x}")).collect::<String>()}),
                    )
                }
            }
            4 | 5 => {
                if n.is_some_and(|n| n > (LIMIT / 4) as u64) {
                    return Err("TLPX collection limit".into());
                }
                let mut array = Vec::new();
                let mut map = Map::new();
                let mut count = 0;
                while n.map_or(self.bytes.get(self.at) != Some(&255), |n| count < n) {
                    let v = self.value(depth + 1)?;
                    if major == 4 {
                        array.push(v);
                    } else {
                        let key = v.as_str().ok_or("TLPX map key is not text")?.to_owned();
                        if map.insert(key, self.value(depth + 1)?).is_some() {
                            return Err("Duplicate TLPX field".into());
                        }
                    }
                    count += 1;
                }
                if n.is_none() {
                    self.take(1)?;
                }
                Ok(if major == 4 {
                    Value::Array(array)
                } else {
                    Value::Object(map)
                })
            }
            6 if n == Some(55799) => self.value(depth + 1),
            _ => Err("Unsupported TLPX value".into()),
        }
    }
}
pub fn decode(bytes: &[u8]) -> Result<Value> {
    let mut r = Reader {
        bytes,
        at: 0,
        items: 0,
    };
    let value = r.value(0)?;
    if r.at != bytes.len() {
        return Err("Trailing TLPX data".into());
    }
    Ok(value)
}
fn header(out: &mut Vec<u8>, major: u8, n: u64) {
    let tag = major << 5;
    if n < 24 {
        out.push(tag | n as u8);
    } else if n <= 255 {
        out.extend([tag | 24, n as u8]);
    } else if n <= 65535 {
        out.push(tag | 25);
        out.extend((n as u16).to_be_bytes());
    } else if n <= u32::MAX as u64 {
        out.push(tag | 26);
        out.extend((n as u32).to_be_bytes());
    } else {
        out.push(tag | 27);
        out.extend(n.to_be_bytes());
    }
}
pub fn encode(value: &Value) -> Result<Vec<u8>> {
    fn write(out: &mut Vec<u8>, v: &Value, depth: usize) -> Result<()> {
        if depth > 96 || out.len() > LIMIT {
            return Err("TLPX size/nesting limit".into());
        }
        match v {
            Value::Null => out.push(246),
            Value::Bool(b) => out.push(if *b { 245 } else { 244 }),
            Value::Number(n) => {
                if let Some(i) = n.as_u64() {
                    header(out, 0, i);
                } else if let Some(i) = n.as_i64() {
                    header(out, 1, (-1 - i) as u64);
                } else {
                    out.push(251);
                    out.extend(n.as_f64().ok_or("Invalid TLPX number")?.to_be_bytes());
                }
            }
            Value::String(s) => {
                header(out, 3, s.len() as u64);
                out.extend(s.as_bytes());
            }
            Value::Array(a) => {
                header(out, 4, a.len() as u64);
                for v in a {
                    write(out, v, depth + 1)?;
                }
            }
            Value::Object(m) => {
                if m.len() == 1
                    && let Some(s) = m.get("__rynthvCborBytes").and_then(Value::as_str)
                {
                    if s.len() % 2 != 0 {
                        return Err("Invalid retained TLPX bytes".into());
                    }
                    header(out, 2, (s.len() / 2) as u64);
                    for i in (0..s.len()).step_by(2) {
                        out.push(
                            u8::from_str_radix(&s[i..i + 2], 16)
                                .map_err(|_| "Invalid retained TLPX bytes")?,
                        );
                    }
                } else {
                    header(out, 5, m.len() as u64);
                    for (k, v) in m
                        .iter()
                        .filter(|(k, _)| k.as_str() == "version")
                        .chain(m.iter().filter(|(k, _)| k.as_str() != "version"))
                    {
                        write(out, &Value::String(k.clone()), depth + 1)?;
                        write(out, v, depth + 1)?;
                    }
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    write(&mut out, value, 0)?;
    Ok(out)
}
