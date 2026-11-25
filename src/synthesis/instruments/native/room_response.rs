use super::{Result, archive::Archive, metadata::{self, Value}, u32at};
use crate::synthesis::instruments::{microphones::Mix, rooms};
use rustfft::{FftPlanner, num_complex::Complex};
use std::{collections::BTreeMap, path::Path};

pub(super) struct Response {
    pub width: usize,
    pub channels: Vec<Vec<f32>>,
}

fn reorder(raw: &[Vec<f32>], width: usize) -> Result<Vec<Vec<f32>>> {
    let length = raw.first().map_or(0, Vec::len);
    if length == 0 || raw.len() != 4 || raw.iter().any(|c| c.len() != length) {
        return Err("Invalid instrument microphone response dimensions".into());
    }
    let mut output = vec![vec![0.; width]; 4];
    for (source, destination) in raw.iter().zip(&mut output) {
        for (i, value) in destination.iter_mut().take(length).enumerate() {
            *value = source[(i + length / 2 + length - 64 % length) % length];
        }
    }
    Ok(output)
}

fn recording(a: &Archive, id: &[u8;16], width: usize) -> Result<Vec<Vec<f32>>> {
    let raw = a.read(id)?;
    if raw.is_empty() || raw.len() % 16 != 0 {
        return Err("Invalid instrument microphone response".into());
    }
    let length = raw.len() / 16;
    let mut channels = vec![vec![0.; length]; 4];
    for (c, values) in channels.iter_mut().enumerate() {
        for (i, value) in values.iter_mut().enumerate() {
            *value = f32::from_bits(u32at(&raw, (c * length + i) * 4));
            if !value.is_finite() { return Err("Nonfinite instrument microphone response".into()); }
        }
    }
    reorder(&channels, width)
}

fn correct(channels: &mut [Vec<f32>], correction: &[f32]) -> Result<()> {
    if correction.is_empty() { return Ok(()); }
    let width = channels[0].len();
    let size = (width + correction.len() - 1).next_power_of_two();
    let mut planner = FftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(size);
    let inverse = planner.plan_fft_inverse(size);
    let mut filter = vec![Complex::new(0.,0.); size];
    for (v, &sample) in filter.iter_mut().zip(correction) { v.re = f64::from(sample); }
    forward.process(&mut filter);
    for channel in channels {
        let mut spectrum = vec![Complex::new(0.,0.); size];
        for (v, &sample) in spectrum.iter_mut().zip(channel.iter()) { v.re = f64::from(sample); }
        forward.process(&mut spectrum);
        for (v, f) in spectrum.iter_mut().zip(&filter) { *v *= f; }
        inverse.process(&mut spectrum);
        for (v, sample) in channel.iter_mut().zip(spectrum) { *v = (sample.re / size as f64) as f32; }
    }
    Ok(())
}

pub(super) fn prepare(a: &Archive, selected: &[Mix], selected_room: Option<&str>,
    pack_path: Option<&Path>) -> Result<Response> {
    let (root, _) = metadata::description(a)?;
    let catalog = metadata::microphone_order(&root)?;
    let extra = metadata::extra(a)?;
    let recorded_room = match extra.get("room") { Some(Value::String(s)) => Some(s.as_str()), _ => None };
    let room = selected_room.filter(|s| !s.is_empty());
    let cross = room.is_some() && room != recorded_room;
    let pack = if cross {
        Some(rooms::load(pack_path.ok_or("Instrument room pack not selected or installed")?)?)
    } else { None };
    let descriptor = pack.as_ref().map(|p| p.room(room.unwrap())
        .ok_or_else(|| format!("Instrument room {} absent", room.unwrap()))).transpose()?;
    if descriptor.is_some_and(|r| r.microphones.iter().any(|m| m.responses.iter().any(|r| r.channels == 0))) {
        return Err("Spatial instrument room requires the Hall processor".into());
    }
    let width = descriptor.and_then(|r| r.microphones.iter().flat_map(|m| &m.responses)
        .map(|v| v.samples.saturating_sub(1000)).max())
        .filter(|n| *n > 0).map_or(24000, |n| n + 64);
    let mut correction = BTreeMap::new();
    if cross && let Some(Value::Id(id)) = extra.get("mic_correction_filters") {
        let raw = a.read(id)?;
        if catalog.is_empty() || raw.len() % (4 * catalog.len()) != 0 {
            return Err("Instrument microphone correction dimensions".into());
        }
        let length = raw.len() / (4 * catalog.len());
        if length > 65536 { return Err("Instrument microphone correction exceeds limit".into()); }
        for ((name, _), row) in catalog.iter().zip(raw.chunks_exact(length.max(1)*4)) {
            let values: Vec<_> = row.chunks_exact(4).map(|v| f32::from_le_bytes(v.try_into().unwrap())).collect();
            if values.iter().any(|v| !v.is_finite()) { return Err("Nonfinite microphone correction".into()); }
            correction.insert(name.clone(), values);
        }
    }
    let mut selected = selected.to_vec();
    selected.sort_by(|a,b| a.name.cmp(&b.name));
    if selected.windows(2).any(|v| v[0].name == v[1].name) { return Err("Duplicate instrument microphone".into()); }
    let mut channels = vec![vec![0f32; width]; 8];
    for channel in selected {
        let close = crate::synthesis::instruments::microphones::split(&channel.name).0 == "Close";
        let raw = if !cross || close {
            let (_, id) = catalog.iter().find(|(n,_)| n == &channel.name)
                .ok_or_else(|| format!("Instrument microphone {} absent", channel.name))?;
            recording(a, id, width)?
        } else {
            let p = pack.as_ref().unwrap();
            let mic = descriptor.unwrap().microphones.iter().find(|m| m.name == channel.name)
                .ok_or_else(|| format!("Room microphone {} absent", channel.name))?;
            if mic.responses.len() != 1 { return Err("Ambiguous room microphone response".into()); }
            let mut raw = reorder(&p.response(&mic.responses[0])?, width)?;
            if let Some(filter) = correction.get(&channel.name) { correct(&mut raw, filter)?; }
            raw
        };
        let gains = [(1.-channel.pan.max(0.))*channel.gain, (1.+channel.pan.min(0.))*channel.gain];
        for c in 0..4 { for side in 0..2 { for (v, &sample) in channels[c*2+side].iter_mut().zip(&raw[c]) {
            *v += sample * gains[side];
        } } }
    }
    Ok(Response {width, channels})
}
