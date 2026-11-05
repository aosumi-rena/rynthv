// SPDX-License-Identifier: MIT
use super::{
    AcousticFrame, DecodedFrame, Error, Result, StdBank, StoredFrames,
    decoder::{fast_log, interpolate, uniform},
    directional::Directions,
    directional_model::finite,
    selection::{Handle, Target},
    source_frames,
    unit_cost::ZH,
};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct Adapted {
    working_begin: usize,
    working: Vec<AcousticFrame>,
    stored_begin: usize,
    stored_hop: f64,
    stored: Vec<DecodedFrame>,
}
impl Adapted {
    pub fn working(&self, absolute: usize) -> [f32; 72] {
        self.working[absolute
            .saturating_sub(self.working_begin)
            .min(self.working.len() - 1)]
        .0
    }
    pub fn sample(&self, time: f32, hop: f64) -> Result<(DecodedFrame, [f32; 64])> {
        let (i, w) =
            source_frames::cropped_position(time, hop, self.working.len(), self.working_begin)?;
        let mut sp = [0.0; 64];
        for k in 0..64 {
            sp[k] = (f64::from(self.working[i].0[k + 3])
                + f64::from(self.working[i + 1].0[k + 3] - self.working[i].0[k + 3]) * w)
                as f32;
        }
        let (i, w) = source_frames::cropped_position(
            time,
            self.stored_hop,
            self.stored.len(),
            self.stored_begin,
        )?;
        Ok((
            source_frames::dense(&self.stored[i], &self.stored[i + 1], w as f32)?,
            sp,
        ))
    }
}
pub(super) fn adapt(
    bank: &StdBank,
    handle: Handle,
    target: &Target,
    encoded: &[AcousticFrame],
    stored: &StoredFrames,
    directions: &mut Directions,
    rng: &mut u32,
    cancel: &AtomicBool,
) -> Result<Option<Adapted>> {
    let unit = &bank.units[handle.unit];
    let h = handle.phone;
    if h + 1 >= unit.timeline.len() {
        return Err(Error::Format("P1 selected source pair"));
    }
    let backward = target.current != unit.timeline[h].symbol;
    let forward = target.next != unit.timeline[h + 1].symbol;
    if !backward && !forward {
        return Ok(None);
    }
    let left = if backward { 0 } else { 5 };
    let right = if forward { 0 } else { 5 };
    let start = if h == 0 {
        0.0
    } else {
        unit.timeline[h - 1].end_seconds
    };
    let finish = unit.timeline[h + 1].end_seconds;
    let hop = bank.profile.source_period;
    let (begin, end) = crop(start, finish, hop, encoded.len(), left, right)?;
    let stored_hop = f64::from(stored.hop_seconds);
    let (sb, se) = crop(start, finish, stored_hop, stored.frames.len(), left, right)?;
    let mut working: Vec<_> = encoded[begin..end]
        .iter()
        .map(|r| AcousticFrame(r.0))
        .collect();
    let mut output = Vec::with_capacity(se - sb);
    for row in &stored.frames[sb..se] {
        output.push(source_frames::stored(row, row, 0.0)?);
    }
    let raw = conditions(bank, handle, target, encoded, begin, end)?;
    let axis = compact_axis(24000.0);
    for reverse in [true, false] {
        if !(if reverse { backward } else { forward }) {
            continue;
        }
        let net = if reverse {
            &mut directions.backward
        } else {
            &mut directions.forward
        };
        net.select_group(&unit.logical_group);
        let threshold = (unit.timeline[h].end_seconds + if reverse { finish } else { start }) * 0.5;
        for call in 0..working.len() + 5 {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let k = if reverse {
                working.len() as isize - 1 - call as isize
            } else {
                call as isize
            };
            let core = k.clamp(0, working.len() as isize - 1) as usize;
            let time = (begin as isize + k) as f64 * hop;
            let init = if reverse {
                time > threshold
            } else {
                time < threshold
            };
            let publication = begin as isize + k + if reverse { 5 } else { -5 };
            let seed = (publication + if reverse { 1 } else { -1 })
                .clamp(begin as isize, end as isize - 1) as usize
                - begin;
            let wi = publication.clamp(begin as isize, end as isize - 1) as usize - begin;
            let si = publication.clamp(sb as isize, se as isize - 1) as usize - sb;
            let predicted = net.push(
                &raw[core],
                if init { Some(&working[seed].0) } else { None },
                rng,
            )?;
            let mut delta = [0.0; 64];
            for j in 0..64 {
                delta[j] =
                    (f64::from(predicted[j + 3] - working[wi].0[j + 3]) * 4.342162396873643) as f32;
                working[wi].0[j + 3] = predicted[j + 3];
            }
            let row = &mut output[si];
            feedback(&axis, &delta, &mut row.noise_psd_db)?;
            if !row.vocal_tract_db.is_empty() {
                feedback(&axis, &delta, &mut row.vocal_tract_db)?;
            }
        }
    }
    Ok(Some(Adapted {
        working_begin: begin,
        working,
        stored_begin: sb,
        stored_hop,
        stored: output,
    }))
}
pub(super) fn crop(
    start: f64,
    end: f64,
    hop: f64,
    count: usize,
    left: isize,
    right: isize,
) -> Result<(usize, usize)> {
    if !hop.is_finite() || hop <= 0.0 {
        return Err(Error::Format("P1 source period"));
    }
    let reciprocal = 1.0 / hop;
    let begin = ((start * reciprocal) as isize - left - 1).max(0) as usize;
    let end = ((end * reciprocal) as isize + right + 1)
        .min(count as isize)
        .max(0) as usize;
    if end <= begin + 1 {
        return Err(Error::Format("P1 source crop"));
    }
    Ok((begin, end))
}
fn conditions(
    bank: &StdBank,
    handle: Handle,
    target: &Target,
    encoded: &[AcousticFrame],
    begin: usize,
    end: usize,
) -> Result<Vec<[f32; 177]>> {
    let unit = &bank.units[handle.unit];
    for label in &target.context {
        if !ZH.contains(&label.as_str()) {
            return Err(Error::Unsupported("P1 K57 target symbol"));
        }
    }
    let mut current = handle.phone as isize - 1;
    let mut cursor = -2isize;
    let mut rows = Vec::with_capacity(end - begin);
    for index in begin..end {
        let time = index as f64 * bank.profile.source_period;
        let endpoint = |at: isize| {
            unit.timeline
                .get(at as usize)
                .map(|p| p.end_seconds)
                .unwrap_or(0.0)
        };
        if time > endpoint(current) {
            current += 1;
            cursor += 1;
        } // ONE advance, strict >.
        let valid = current >= 0 && current < unit.timeline.len() as isize;
        let start = if valid && current > 0 {
            endpoint(current - 1)
        } else {
            0.0
        };
        let finish = if valid { endpoint(current) } else { 0.0 };
        let duration = finish - start;
        let q = ((time - start) / duration.max(0.005)).clamp(0.0, 1.0);
        let inverse = 1.0 - q;
        let mut raw = [0.0; 177];
        for (group, relative) in [cursor + 1, cursor + 2, cursor].into_iter().enumerate() {
            let label = target
                .context
                .get((2 + relative) as usize)
                .map(String::as_str)
                .unwrap_or("sil");
            raw[group * 57 + ZH.iter().position(|p| *p == label).unwrap()] = 1.0;
        }
        raw[171] = q as f32;
        raw[172] = inverse as f32;
        raw[173] = (inverse * inverse + q * q) as f32;
        raw[174] = (5.0 * duration) as f32;
        raw[175..].copy_from_slice(&encoded[index].0[..2]);
        finite(&raw)?;
        rows.push(raw);
    }
    Ok(rows)
}
fn compact_axis(nyquist: f32) -> Vec<f32> {
    let low = fast_log(50.0 * f32::from_bits(0x3abb3ee7) + 1.0);
    let range = fast_log(nyquist * f32::from_bits(0x3abb3ee7) + 1.0) - low;
    (0..65)
        .map(|i| (((i as f32 * (1.0 / 64.0)) * range + low).exp2() - 1.0) * 700.0)
        .collect()
}
fn feedback(axis: &[f32], delta: &[f32; 64], values: &mut [f32]) -> Result<()> {
    let additions = interpolate(&axis[..64], delta, &uniform(values.len(), 24000.0))?;
    for (v, a) in values.iter_mut().zip(additions) {
        *v += a;
    }
    finite(values)
}
