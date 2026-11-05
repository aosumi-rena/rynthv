// SPDX-License-Identifier: MIT
use super::{
    AcousticFrame, DecodedFrame, Error, Result, SourceDecoder, StdBank, StoredFrames,
    frames::StoredFrame, stored_vt,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct SourceFrames {
    sample_cache: HashMap<(usize, usize, u32), (DecodedFrame, [f32; 64])>,
    stored: HashMap<usize, StoredFrames>,
    encoded: HashMap<usize, Vec<AcousticFrame>>,
    adapted: HashMap<usize, super::source_adaptation::Adapted>,
    coordinates: Vec<Vec<Option<f32>>>,
    counts: Vec<usize>,
    waves: Vec<Vec<Option<super::frames::NoiseWave>>>,
    stored_crops: Vec<Option<(usize,usize)>>,
}
impl SourceFrames {
    pub fn new() -> Self {
        Self {
            sample_cache: HashMap::new(),
            stored: HashMap::new(),
            encoded: HashMap::new(),
            adapted: HashMap::new(),
            coordinates: Vec::new(),
            counts: Vec::new(),
            waves: Vec::new(),
            stored_crops: Vec::new(),
        }
    }
    pub fn prepare(
        &mut self,
        bank: &StdBank,
        handles: &[super::selection::Handle],
        targets: &[super::selection::Target],
        timings: &[super::timing::PhoneTiming],
        model: &super::UnitCostModel,
        output_hop: f64,
        directions: &mut Option<super::directional::Directions>,
        rng: &mut u32,
        cancel: &AtomicBool,
    ) -> Result<()> {
        if handles.len() != timings.len() || handles.len() != targets.len() {
            return Err(Error::Format("source pair cardinality"));
        }
        self.counts = timings.iter().map(|timing| {
            let count = (timing.duration / output_hop).round();
            if !count.is_finite() || count < 0.0 || count > super::retaining::MAX_ROWS as f64 {
                return Err(Error::Unsupported("source pair row extent"));
            }
            Ok(count as usize)
        }).collect::<Result<_>>()?;
        for job in 0..self.counts.len() {
            if self.counts[job] + self.counts.get(job + 1).copied().unwrap_or(0)
                > super::retaining::MAX_ROWS {
                return Err(Error::Unsupported("source pair row extent"));
            }
        }
        for (job, (&handle, target)) in handles.iter().zip(targets).enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            if !self.encoded.contains_key(&handle.unit) {
                self.encoded
                    .insert(handle.unit, bank.read_acoustic_frames(handle.unit)?);
            }
            if !bank.profile.encoder_only && !self.stored.contains_key(&handle.unit) {
                self.stored
                    .insert(handle.unit, bank.read_stored_frames(handle.unit)?);
            }
            if let Some(directions) = directions.as_mut() {
                if let Some(value) = super::source_adaptation::adapt(
                    bank,
                    handle,
                    target,
                    &self.encoded[&handle.unit],
                    &self.stored[&handle.unit],
                    directions,
                    rng,
                    cancel,
                )? {
                    self.adapted.insert(job, value);
                }
            }
            let n = self.counts[job];
            self.coordinates.push(super::source_coordinates::side(
                bank,
                model,
                handle,
                target,
                n,
                output_hop,
                true,
                |index| {
                    if let Some(adapted) = self.adapted.get(&job) {
                        adapted.working(index)
                    } else {
                        let rows = &self.encoded[&handle.unit];
                        rows[index.min(rows.len() - 1)].0
                    }
                },
            )?);
            let next = self.counts.get(job + 1).copied().unwrap_or(0);
            let right = super::source_coordinates::side(
                bank,
                model,
                handle,
                target,
                next,
                output_hop,
                false,
                |index| {
                    if let Some(adapted) = self.adapted.get(&job) {
                        adapted.working(index)
                    } else {
                        let rows = &self.encoded[&handle.unit];
                        rows[index.min(rows.len() - 1)].0
                    }
                },
            )?;
            let coordinates: Vec<_> = self.coordinates[job].iter().copied().chain(right).collect();
            self.coordinates[job] = coordinates;
            let waves=if bank.profile.encoder_only {
                self.stored_crops.push(None);
                vec![None;self.coordinates[job].len()]
            } else {
                let unit=&bank.units[handle.unit];
                let labels=&unit.timeline;let h=handle.phone;
                let begin=if h==0 {0.0} else {labels[h-1].end_seconds};
                let end=labels[(h+1).min(labels.len()-1)].end_seconds;
                let data=&self.stored[&handle.unit];
                let crop=super::source_adaptation::crop(begin,end,f64::from(data.hop_seconds),data.frames.len(),
                    if target.current!=labels[h].symbol {0} else {5},
                    if target.next!=labels[(h+1).min(labels.len()-1)].symbol {0} else {5})?;
                self.stored_crops.push(Some(crop));
                super::wave_placement::place(data,&self.coordinates[job],crop,output_hop,n,
                    [model.category(&target.current),model.category(&target.next)])?
            };
            self.waves.push(waves);
        }
        Ok(())
    }
    pub fn current_count(&self, job: usize) -> usize { self.counts[job] }
    pub fn pair_count(&self, job: usize) -> usize { self.coordinates[job].len() }
    pub fn pair_row(
        &mut self, bank: &StdBank, decoder: &SourceDecoder,
        job: usize, unit: usize, index: usize,
    ) -> Result<super::overlap::Row> {
        let coordinate = *self.coordinates[job].get(index)
            .ok_or(Error::Format("mapped pair row index"))?;
        if let Some(time) = coordinate {
            let mut row=self.sample_job(bank, decoder, job, unit, time)?;
            row.0.noise_wave=self.waves[job][index].clone();
            if let Some((begin,end))=self.stored_crops[job] {
                let data=&self.stored[&unit];
                let (base,weight)=position(time,f64::from(data.hop_seconds),data.frames.len())?;
                let left=f64::from(data.frames[base.clamp(begin,end-1)].gain.unwrap_or(0.0));
                let right=f64::from(data.frames[(base+1).clamp(begin,end-1)].gain.unwrap_or(0.0));
                row.0.source_pitch=left+(right-left)*weight;
                if !row.0.source_pitch.is_finite() {return Err(Error::Format("stored pitch wrapper interpolation"));}
            }
            Ok(row)
        } else {
            Ok((DecodedFrame {
                source_pitch: 0.0,
                noise_wave: None,
                f0_hz: 0.0, rd: 1.0, vocal_tract_db: Vec::new(),
                harmonic_phase: Vec::new(), noise_psd_db: [-120.0; 256],
                noise_bands: [f32::from_bits(0x3727c5ac); 4],
                noise_envelopes: std::array::from_fn(|_| super::frames::Envelope {
                    amplitude: vec![0.0; 5], phase: vec![0.0; 5],
                }),
            }, [-12.0; 64]))
        }
    }
    pub fn sample_job(
        &mut self,
        bank: &StdBank,
        decoder: &SourceDecoder,
        job: usize,
        unit: usize,
        time: f32,
    ) -> Result<(DecodedFrame, [f32; 64])> {
        let key = (job, unit, time.to_bits());
        if let Some(row) = self.sample_cache.get(&key) {
            return Ok(row.clone());
        }
        let row = if let Some(value) = self.adapted.get(&job) {
            value.sample(time, bank.profile.source_period)?
        } else {
            self.sample(bank, decoder, unit, time)?
        };
        if self.sample_cache.len() < 8192 {
            self.sample_cache.insert(key, row.clone());
        }
        Ok(row)
    }
    pub fn sample(
        &mut self,
        bank: &StdBank,
        decoder: &SourceDecoder,
        unit: usize,
        time: f32,
    ) -> Result<(DecodedFrame, [f32; 64])> {
        if !self.encoded.contains_key(&unit) {
            self.encoded.insert(unit, bank.read_acoustic_frames(unit)?);
        }
        let rows = &self.encoded[&unit];
        let (i, w) = position(time, bank.profile.source_period, rows.len())?;
        let mut row = [0.0; 72];
        for k in 0..72 {
            row[k] =
                (f64::from(rows[i].0[k]) + f64::from(rows[i + 1].0[k] - rows[i].0[k]) * w) as f32;
        }
        let sp = row[3..67].try_into().unwrap();
        if bank.profile.encoder_only {
            Ok((decoder.decode(&AcousticFrame(row))?, sp))
        } else {
            if !self.stored.contains_key(&unit) {
                self.stored.insert(unit, bank.read_stored_frames(unit)?);
            }
            let data = &self.stored[&unit];
            let (i, w) = position(time, f64::from(data.hop_seconds), data.frames.len())?;
            Ok((stored(&data.frames[i], &data.frames[i + 1], w as f32)?, sp))
        }
    }
}
pub(super) fn position(time: f32, hop: f64, count: usize) -> Result<(usize, f64)> {
    cropped_position(time, hop, count, 0)
}
pub(super) fn cropped_position(
    time: f32,
    hop: f64,
    count: usize,
    begin: usize,
) -> Result<(usize, f64)> {
    if !time.is_finite() || !hop.is_finite() || hop <= 0.0 || count < 2 {
        return Err(Error::Format("physical source position"));
    }
    let q = f64::from(time) * (1.0 / hop) - begin as f64;
    if !q.is_finite() || q < i32::MIN as f64 || q >= 2147483648.0 {
        return Err(Error::Format("physical source position overflow"));
    }
    let base = (q as i32).clamp(0, (count - 2) as i32) as usize;
    Ok((base, (q - base as f64).clamp(0.0, 1.0)))
}
pub(super) fn mix(
    a: &[f32],
    b: &[f32],
    weight: f32,
    phase: bool,
    zero_left_tail: bool,
) -> Vec<f32> {
    let mut out = vec![0.0; a.len().max(b.len())];
    let common = a.len().min(b.len());
    for i in 0..common {
        let (mut x, mut y) = (a[i], b[i]);
        if phase {
            let adjustment = (((f64::from((x - y).abs() * 0.5) / std::f64::consts::PI).round())
                * 2.0)
                * std::f64::consts::PI;
            if x > y {
                y = (f64::from(y) + adjustment) as f32;
            } else {
                x = (f64::from(x) + adjustment) as f32;
            }
        }
        out[i] = (y - x) * weight + x;
    }
    if b.len() > a.len() {
        out[common..].copy_from_slice(&b[common..]);
    } else if !zero_left_tail {
        out[common..].copy_from_slice(&a[common..]);
    }
    out
}
pub(super) fn stored(left: &StoredFrame, right: &StoredFrame, weight: f32) -> Result<DecodedFrame> {
    let (left, right, weight) = if f64::from(weight) < 0.01 {
        (left, left, 0.0)
    } else if f64::from(weight) > 0.99 {
        (right, right, 0.0)
    } else {
        (left, right, weight)
    };
    let lv = left.f0_hz > 0.0;
    let rv = right.f0_hz > 0.0;
    let (f0_hz, rd, phase, compact, step) = if lv && rv {
        (
            left.f0_hz + (right.f0_hz - left.f0_hz) * weight,
            left.rd + (right.rd - left.rd) * weight,
            mix(
                left.phase.as_deref().unwrap_or_default(),
                right.phase.as_deref().unwrap_or_default(),
                weight,
                true,
                true,
            ),
            mix(
                left.compact_spectrum
                    .as_deref()
                    .ok_or(Error::Format("voiced compact spectrum"))?,
                right
                    .compact_spectrum
                    .as_deref()
                    .ok_or(Error::Format("voiced compact spectrum"))?,
                weight,
                false,
                false,
            ),
            left.compact_step
                .ok_or(Error::Format("voiced compact step"))?
                + (right
                    .compact_step
                    .ok_or(Error::Format("voiced compact step"))?
                    - left.compact_step.unwrap())
                    * weight,
        )
    } else if lv || rv {
        let frame = if rv { right } else { left };
        (
            frame.f0_hz,
            frame.rd,
            frame.phase.clone().ok_or(Error::Format("voiced phase"))?,
            frame
                .compact_spectrum
                .clone()
                .ok_or(Error::Format("voiced compact spectrum"))?,
            frame
                .compact_step
                .ok_or(Error::Format("voiced compact step"))?,
        )
    } else {
        (0.0, 1.0, Vec::new(), Vec::new(), 1.0)
    };
    let mut vocal_tract_db = Vec::new();
    if lv || rv {
        let mut compact = compact;
        if lv != rv {
            let amount = f64::from(if rv { weight } else { 1.0 - weight }).max(1e-8);
            let offset = (amount.ln() * 8.685889611636938) as f32;
            for value in &mut compact {
                *value += offset;
            }
        }
        for value in &mut compact {
            *value = value.max(-80.0);
        }
        vocal_tract_db = stored_vt::expand(step, &compact)?;
    }
    let mut noise_psd_db = [0.0; 256];
    for i in 0..256 {
        let a = (left.noise_psd[i] * f32::from_bits(0x3e13bb63)).exp2();
        let b = (right.noise_psd[i] * f32::from_bits(0x3e13bb63)).exp2();
        noise_psd_db[i] = ((b - a) * weight + a).log2() * f32::from_bits(0x40ddce9e);
    }
    if !f0_hz.is_finite()
        || !rd.is_finite()
        || vocal_tract_db
            .iter()
            .chain(&phase)
            .chain(&noise_psd_db)
            .any(|x| !x.is_finite())
    {
        return Err(Error::Format("stored source interpolation"));
    }
    Ok(DecodedFrame {
        source_pitch: 0.0,
        noise_wave: None,
        f0_hz,
        rd,
        vocal_tract_db,
        harmonic_phase: phase,
        noise_psd_db,
        noise_bands: std::array::from_fn(|i| {
            left.bands[i] + (right.bands[i] - left.bands[i]) * weight
        }),
        noise_envelopes: std::array::from_fn(|i| super::frames::Envelope {
            amplitude: mix(
                &left.envelopes[i].amplitude,
                &right.envelopes[i].amplitude,
                weight,
                false,
                false,
            ),
            phase: mix(
                &left.envelopes[i].phase,
                &right.envelopes[i].phase,
                weight,
                true,
                false,
            ),
        }),
    })
}

pub(super) fn dense(
    left: &DecodedFrame,
    right: &DecodedFrame,
    weight: f32,
) -> Result<DecodedFrame> {
    if f64::from(weight) < 0.01 {
        return Ok(left.clone());
    }
    if f64::from(weight) > 0.99 {
        return Ok(right.clone());
    }
    let (lv, rv) = (left.f0_hz > 0.0, right.f0_hz > 0.0);
    let mut out = if rv && !lv {
        right.clone()
    } else {
        left.clone()
    };
    if lv && rv {
        out.f0_hz = left.f0_hz + (right.f0_hz - left.f0_hz) * weight;
        out.rd = left.rd + (right.rd - left.rd) * weight;
        out.harmonic_phase = mix(
            &left.harmonic_phase,
            &right.harmonic_phase,
            weight,
            true,
            true,
        );
        out.vocal_tract_db = mix(
            &left.vocal_tract_db,
            &right.vocal_tract_db,
            weight,
            false,
            false,
        );
    } else if !lv && !rv {
        out.f0_hz = 0.0;
        out.rd = 1.0;
    }
    if lv != rv {
        let amount = f64::from(if rv { weight } else { 1.0 - weight }).max(1e-8);
        let offset = (amount.ln() * 8.685889611636938) as f32;
        for value in &mut out.vocal_tract_db {
            *value += offset;
        }
    }
    for value in &mut out.vocal_tract_db {
        *value = value.max(-80.0);
    }
    for i in 0..256 {
        let a = (left.noise_psd_db[i] * f32::from_bits(0x3e13bb63)).exp2();
        let b = (right.noise_psd_db[i] * f32::from_bits(0x3e13bb63)).exp2();
        out.noise_psd_db[i] = ((b - a) * weight + a).log2() * f32::from_bits(0x40ddce9e);
    }
    for i in 0..4 {
        out.noise_bands[i] =
            left.noise_bands[i] + (right.noise_bands[i] - left.noise_bands[i]) * weight;
        out.noise_envelopes[i].amplitude = mix(
            &left.noise_envelopes[i].amplitude,
            &right.noise_envelopes[i].amplitude,
            weight,
            false,
            false,
        );
        out.noise_envelopes[i].phase = mix(
            &left.noise_envelopes[i].phase,
            &right.noise_envelopes[i].phase,
            weight,
            true,
            false,
        );
    }
    super::directional_model::finite(&out.noise_psd_db)?;
    super::directional_model::finite(&out.vocal_tract_db)?;
    Ok(out)
}
