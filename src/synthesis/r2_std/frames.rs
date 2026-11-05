// SPDX-License-Identifier: MIT
use super::{
    Error, Result, StdBank,
    archive::{Archive, Reader},
    codec::{predictor8, quantized},
};

pub(crate) struct AcousticFrame(pub [f32; 72]);
pub(crate) struct StoredFrames {
    pub hop_seconds: f32,
    pub frames: Vec<StoredFrame>,
}
pub(crate) struct StoredFrame {
    pub compact_step: Option<f32>,
    pub compact_spectrum: Option<Vec<f32>>,
    pub gain: Option<f32>,
    pub f0_hz: f32,
    pub rd: f32,
    pub phase: Option<Vec<f32>>,
    pub noise_psd: [f32; 256],
    pub bands: [f32; 4],
    pub envelopes: [Envelope; 4],
    pub noise_wave: Option<NoiseWave>,
}
#[derive(Clone)]
pub(crate) struct Envelope {
    pub amplitude: Vec<f32>,
    pub phase: Vec<f32>,
}
#[derive(Clone)]
pub(crate) struct NoiseWave {
    pub rate: f32,
    pub offset: f32,
    pub gain: f32,
    pub samples: std::sync::Arc<[f32]>,
}
impl NoiseWave {
    pub fn scale(&mut self, gain: f64) -> Result<()> {
        if !gain.is_finite() { return Err(Error::Format("embedded noise-wave gain")); }
        for value in std::sync::Arc::make_mut(&mut self.samples) {
            *value = finite((f64::from(*value)*gain) as f32)?;
        }
        Ok(())
    }
    pub fn adjust_psd(&mut self, old: &[f32], new: &[f32]) -> Result<()> {
        let alpha=f32::from_bits(0x3e6bd3c3);
        let peak=|values:&[f32]|values.iter().fold(-100.0f32,|p,&x|(x*alpha).max(p));
        let gain=((peak(new)-peak(old))*0.5).exp();
        self.scale(f64::from(gain))
    }
}

#[derive(Clone, Copy)]
struct Field {
    a: u8,
    b: u8,
    count: usize,
    low: f32,
    high: f32,
}
struct Descriptor {
    fields: Vec<Field>,
    offsets: Vec<usize>,
    blob: Vec<u8>,
}
impl Descriptor {
    fn read(archive: &Archive, name: &[u8], flags: &[(u8, u8, usize)]) -> Result<Self> {
        let mut reader = Reader::new(archive.descriptor(name)?);
        if reader.string_bytes()? != name {
            return Err(Error::Format("source frame descriptor name"));
        }
        reader.i32()?;
        let link = reader.u64()?;
        if reader.u16()? as usize != flags.len() {
            return Err(Error::Unsupported("source frame field count"));
        }
        let mut fields = Vec::with_capacity(flags.len());
        for &(want_a, want_b, want_count) in flags {
            let a = reader.u8()?;
            let b = reader.u8()?;
            let count = if a & 1 != 0 {
                length(reader.i32()?, 4096)?
            } else {
                0
            };
            let (low, high) = if b & 0x10 != 0 {
                (finite(reader.f32()?)?, finite(reader.f32()?)?)
            } else {
                (-1.0, 1.0)
            };
            if (a, b, count) != (want_a, want_b, want_count) {
                return Err(Error::Unsupported("source frame field schema"));
            }
            fields.push(Field {
                a,
                b,
                count,
                low,
                high,
            });
        }
        let count = length(reader.i32()?, 65536)?;
        let blob = reader.take(count)?.to_vec();
        reader.end()?;
        Ok(Self {
            fields,
            offsets: archive.frame_offsets(link)?,
            blob,
        })
    }
}

impl StdBank {
    pub fn read_acoustic_frames(&self, unit: usize) -> Result<Vec<AcousticFrame>> {
        let unit = self.units.get(unit).ok_or(Error::Format("unit ordinal"))?;
        let descriptor = Descriptor::read(
            &self.archive,
            &unit.acoustic_name,
            &[
                (0x22, 0x12, 0),
                (0x22, 0, 0),
                (0x22, 0x11, 0),
                (0x33, 0x22, 64),
                (0x33, 2, 5),
            ],
        )?;
        if descriptor.offsets.len() > 131072 {
            return Err(Error::Unsupported("acoustic source frame extent"));
        }
        let mut frames = Vec::with_capacity(descriptor.offsets.len());
        for offset in descriptor.offsets {
            let mut reader = Reader::new(self.archive.frame(offset)?);
            let mut row = [0.0; 72];
            row[0] = quantized(
                u32::from(reader.u8()?),
                descriptor.fields[0].low,
                descriptor.fields[0].high,
                false,
            )?;
            row[1] = finite(reader.f32()?)?;
            row[2] = quantized(
                u32::from(reader.u16()?),
                descriptor.fields[2].low,
                descriptor.fields[2].high,
                true,
            )?;
            let low = finite(reader.f32()?)?;
            let high = finite(reader.f32()?)?;
            row[3..67].copy_from_slice(&predictor8(&mut reader, low, high, 64)?);
            let low = finite(reader.f32()?)?;
            let high = finite(reader.f32()?)?;
            for value in &mut row[67..] {
                *value = quantized(u32::from(reader.u8()?), low, high, false)?;
            }
            reader.end()?;
            frames.push(AcousticFrame(row));
        }
        Ok(frames)
    }

    pub fn read_stored_frames(&self, unit: usize) -> Result<StoredFrames> {
        if self.profile.encoder_only {
            return Err(Error::Unsupported("encoder-only bank has no stored LLSM"));
        }
        for (key, value) in [
            ("voc_fnyq", 24000.0),
            ("voc_nspec", 1025.0),
            ("voc_npsd", 256.0),
            ("voc_nchannel", 4.0),
            ("voc_maxnhar_e", 5.0),
        ] {
            if self.configuration.numbers(key)? != [value] {
                return Err(Error::Unsupported("stored LLSM geometry"));
            }
        }
        let unit = self.units.get(unit).ok_or(Error::Format("unit ordinal"))?;
        let descriptor = Descriptor::read(
            &self.archive,
            &unit.stored_name,
            &[
                (0x20, 0x11, 0),
                (0x30, 0x22, 0),
                (0x20, 0x11, 0),
                (0x22, 0x11, 0),
                (0x22, 0x11, 0),
                (0x30, 0x12, 0),
                (0x33, 0x22, 256),
                (0x33, 2, 4),
                (0, 0, 0),
                (0, 0, 0),
            ],
        )?;
        let hop_seconds = finite(Reader::new(&descriptor.blob).f32()?)?;
        if descriptor.blob.len() != 4 || hop_seconds <= 0.0 {
            return Err(Error::Format("stored LLSM hop"));
        }
        let mut frames = Vec::with_capacity(descriptor.offsets.len());
        for offset in descriptor.offsets {
            let mut reader = Reader::new(self.archive.frame(offset)?);
            let mut values = Vec::with_capacity(10);
            for field in &descriptor.fields {
                values.push(read_value(&mut reader, *field)?);
            }
            reader.end()?;
            let mut iter = values.into_iter();
            let compact_step = optional_scalar(iter.next().unwrap())?;
            let compact_spectrum = optional_vector(iter.next().unwrap())?;
            let gain = optional_scalar(iter.next().unwrap())?;
            let f0_hz =
                optional_scalar(iter.next().unwrap())?.ok_or(Error::Format("LLSM F0 missing"))?;
            let rd =
                optional_scalar(iter.next().unwrap())?.ok_or(Error::Format("LLSM Rd missing"))?;
            let phase = optional_vector(iter.next().unwrap())?;
            let noise_psd = optional_vector(iter.next().unwrap())?
                .ok_or(Error::Format("LLSM PSD missing"))?
                .try_into()
                .map_err(|_| Error::Format("LLSM PSD length"))?;
            let bands = optional_vector(iter.next().unwrap())?
                .ok_or(Error::Format("LLSM bands missing"))?
                .try_into()
                .map_err(|_| Error::Format("LLSM band length"))?;
            let envelopes = match iter.next().unwrap() {
                Value::Blob(bytes) => read_envelopes(&bytes)?,
                Value::Missing => std::array::from_fn(|_| Envelope {
                    amplitude: Vec::new(),
                    phase: Vec::new(),
                }),
                _ => return Err(Error::Format("LLSM envelope field type")),
            };
            let noise_wave = match iter.next().unwrap() {
                Value::Blob(bytes) => Some(read_noise_wave(&bytes)?),
                Value::Missing => None,
                _ => return Err(Error::Format("LLSM waveform field type")),
            };
            frames.push(StoredFrame {
                compact_step,
                compact_spectrum,
                gain,
                f0_hz,
                rd,
                phase,
                noise_psd,
                bands,
                envelopes,
                noise_wave,
            });
        }
        Ok(StoredFrames {
            hop_seconds,
            frames,
        })
    }
}

enum Value {
    Missing,
    Vector(Vec<f32>),
    Blob(Vec<u8>),
}
fn read_value(reader: &mut Reader<'_>, field: Field) -> Result<Value> {
    if field.a & 2 == 0 && reader.u8()? == 0 {
        return Ok(Value::Missing);
    }
    let kind = field.a & 0xf0;
    if kind == 0x20 {
        let code = if field.b & 1 != 0 {
            u32::from(reader.u16()?)
        } else {
            u32::from(reader.u8()?)
        };
        return Ok(Value::Vector(vec![quantized(
            code,
            field.low,
            field.high,
            field.b & 1 != 0,
        )?]));
    }
    let count = if field.a & 1 != 0 {
        field.count
    } else {
        length(reader.i32()?, if kind == 0 { 65536 } else { 4096 })?
    };
    if kind == 0 {
        return Ok(Value::Blob(reader.take(count)?.to_vec()));
    }
    let (low, high) = if field.b & 7 != 0 && field.b & 0x10 == 0 {
        (finite(reader.f32()?)?, finite(reader.f32()?)?)
    } else {
        (field.low, field.high)
    };
    let values = if field.b & 0x20 != 0 {
        predictor8(reader, low, high, count)?
    } else {
        (0..count)
            .map(|_| quantized(u32::from(reader.u8()?), low, high, false))
            .collect::<Result<_>>()?
    };
    Ok(Value::Vector(values))
}
fn optional_vector(value: Value) -> Result<Option<Vec<f32>>> {
    match value {
        Value::Missing => Ok(None),
        Value::Vector(v) => Ok(Some(v)),
        _ => Err(Error::Format("LLSM expected numeric field")),
    }
}
fn optional_scalar(value: Value) -> Result<Option<f32>> {
    match optional_vector(value)? {
        None => Ok(None),
        Some(v) if v.len() == 1 => Ok(Some(v[0])),
        _ => Err(Error::Format("LLSM scalar length")),
    }
}
fn read_envelopes(bytes: &[u8]) -> Result<[Envelope; 4]> {
    let mut reader = Reader::new(bytes);
    let mut result = Vec::with_capacity(4);
    for _ in 0..4 {
        let n = usize::from(reader.u8()?);
        let gain = finite(reader.f32()?)?;
        if n > 5 {
            return Err(Error::Unsupported("LLSM envelope harmonics"));
        }
        let amplitude = (0..n)
            .map(|_| finite((f32::from(reader.u8()?) * f32::from_bits(0x3b808081)) * gain))
            .collect::<Result<_>>()?;
        let phase = (0..n)
            .map(|_| {
                let fraction = f32::from(reader.u8()?) * f32::from_bits(0x3b800000);
                let t = f64::from(fraction) * std::f64::consts::PI;
                finite(((t + t) - std::f64::consts::PI) as f32)
            })
            .collect::<Result<_>>()?;
        result.push(Envelope { amplitude, phase });
    }
    reader.end()?;
    Ok(result.try_into().ok().unwrap())
}
fn read_noise_wave(bytes: &[u8]) -> Result<NoiseWave> {
    let mut reader = Reader::new(bytes);
    let count = length(reader.i32()?, 8192)?;
    let rate = reader.f32()?;
    let offset = finite(reader.f32()?)?;
    let gain = finite(reader.f32()?)?;
    if rate.to_bits() != 0x473b8000 || reader.remaining() != count * 2 {
        return Err(Error::Unsupported("LLSM embedded noise wave rate/extent"));
    }
    let mut samples = Vec::with_capacity(count);
    let bulk = count / 16 * 16;
    let k = f32::from_bits(0x38000100);
    let factored = gain * k;
    for i in 0..count {
        let code = f32::from(reader.u16()? as i16);
        samples.push(finite(if i >= bulk {
            (code * k) * gain
        } else if i % 16 < 12 {
            (code * gain) * k
        } else {
            code * factored
        })?);
    }
    reader.end()?;
    Ok(NoiseWave {
        rate,
        offset,
        gain: 1.0,
        samples: samples.into(),
    })
}
fn finite(value: f32) -> Result<f32> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::Format("nonfinite source frame"))
    }
}
fn length(value: i32, maximum: usize) -> Result<usize> {
    if value < 0 || value as usize > maximum {
        Err(Error::Unsupported("source array extent"))
    } else {
        Ok(value as usize)
    }
}
