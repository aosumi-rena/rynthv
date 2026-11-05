// SPDX-License-Identifier: MIT
use super::{Error, Result, StoredFrames, frames::NoiseWave};

pub(super) fn place(
    source: &StoredFrames, coordinates: &[Option<f32>], crop: (usize,usize),
    hop: f64, left_count: usize, types: [u8;2],
) -> Result<Vec<Option<NoiseWave>>> {
    let period=f64::from(source.hop_seconds);
    let mut positions=Vec::new();
    let mut prior:Option<usize>=None;
    let mut crossed=0;
    for (assembled,&time) in coordinates.iter().enumerate() {
        let Some(time)=time else { continue; };
        let (base,_)=super::source_frames::position(time,period,source.frames.len())?;
        if let Some(old)=prior { crossed+=base.saturating_sub(old); }
        if crossed>65536 { return Err(Error::Unsupported("embedded wave crossed-index budget")); }
        positions.push((assembled,base,f64::from(time)*(1.0/period)));
        prior=Some(base);
    }
    if crop.0>=crop.1 || crop.1>source.frames.len() || !hop.is_finite() || hop<=0.0 {
        return Err(Error::Format("embedded wave mapping geometry"));
    }
    let mut output=vec![None;coordinates.len()];
    prior=None;
    for (i,&(assembled,base,q)) in positions.iter().enumerate() {
        if let Some(old)=prior {
            for j in old+1..=base {
                let Some(key)=&source.frames[j.clamp(crop.0,crop.1-1)].noise_wave else { continue; };
                let delta=(j as f64-q)*period;
                let back_q=-delta/hop;
                if !back_q.is_finite() || back_q<i32::MIN as f64 || back_q>=2147483648.0 {
                    return Err(Error::Format("embedded wave placement offset"));
                }
                let back=(back_q as i32).min(i as i32);
                let residual=back as f64*hop+delta;
                if !(-0.25..=0.25).contains(&residual) { continue; }
                let mut moved=key.clone();moved.offset=(f64::from(key.offset)+residual) as f32;
                if !moved.offset.is_finite() { return Err(Error::Format("embedded wave finite offset")); }
                let destination=if back>0 { positions[i-back as usize].0 } else { assembled };
                output[destination]=Some(moved);
                break; // First accepted crossing, not every wave at this position.
            }
        }
        prior=Some(base);
    }
    for (i,key) in output.iter_mut().enumerate() {
        let kind=types[usize::from(i>=left_count)];
        if kind<=1 || kind==9 { *key=None; }
    }
    Ok(output)
}
