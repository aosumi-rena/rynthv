// SPDX-License-Identifier: MIT
use super::{
    Error, Result, SourceDecoder, StdBank, UnitCostModel,
    overlap::{self, Row}, overlap_correction::Correction,
    pitch_transform::PitchTransform, selection::{Handle, Target},
    source_frames::SourceFrames, timing::PhoneTiming,
};
use std::{collections::VecDeque, sync::atomic::{AtomicBool, Ordering}};
pub(super) const MAX_ROWS: usize = 4096;

#[derive(Default)]
struct Clock { sum: f64, error: f64 }
impl Clock {
    fn advance(&mut self, duration: f64) -> Result<()> {
        let y = duration - self.error;
        let next = y + self.sum;
        self.error = (next - self.sum) - y;
        self.sum = next;
        if !self.sum.is_finite() || !self.error.is_finite() {
            return Err(Error::Format("retaining clock"));
        }
        Ok(())
    }
}

pub(super) struct Request<'a> {
    pub bank: &'a StdBank,
    pub decoder: &'a SourceDecoder,
    pub model: &'a UnitCostModel,
    pub transform: &'a PitchTransform,
    pub handles: &'a [Handle],
    pub targets: &'a [Target],
    pub timing: &'a [PhoneTiming],
    pub origin: f64,
    pub hop: f64,
}

pub(super) struct Retaining<'a> {
    request: Request<'a>,
    source: SourceFrames,
    retained: VecDeque<Row>,
    job_clock: Clock,
    release_clock: Clock,
    next_job: usize,
    previous: Option<usize>,
    outstanding: usize,
    threshold: usize,
}
impl<'a> Retaining<'a> {
    pub fn new(source: SourceFrames, request: Request<'a>) -> Self {
        Self { request, source, retained: VecDeque::new(),
            job_clock: Clock::default(), release_clock: Clock::default(),
            next_job: 0, previous: None, outstanding: 0, threshold: 15 }
    }
    pub fn next(
        &mut self, mut pitch: impl FnMut(f64) -> Result<f32>, cancel: &AtomicBool,
    ) -> Result<Option<Row>> {
        loop {
            if cancel.load(Ordering::Relaxed) { return Err(Error::Cancelled); }
            if self.outstanding > self.threshold {
                let row = self.retained.pop_front()
                    .ok_or(Error::Format("retaining release without row"))?;
                self.outstanding -= 1;
                self.release_clock.advance(self.request.hop)?;
                return Ok(Some(row));
            }
            if self.next_job < self.request.handles.len() {
                self.map_next(&mut pitch, cancel)?;
            } else if !self.retained.is_empty() {
                self.threshold = 0;
                self.outstanding = self.retained.len();
            } else { return Ok(None); }
        }
    }
    fn map_next(
        &mut self, pitch: &mut impl FnMut(f64) -> Result<f32>, cancel: &AtomicBool,
    ) -> Result<()> {
        let job = self.next_job;
        let r = &self.request;
        let duration = r.timing[job].duration;
        let q = ((self.job_clock.sum + duration) - self.release_clock.sum) / r.hop;
        if !q.is_finite() || q < i32::MIN as f64 || q >= 2147483648.0 {
            return Err(Error::Format("retaining raw frame count"));
        }
        let current = self.source.current_count(job);
        let total = self.source.pair_count(job);
        let trim = Trim::new(current, total, self.retained.len(), q as i32)?;
        let prospective = if trim.discard { self.retained.len() } else {
            self.retained.len() - trim.tail + total - trim.front - trim.u
        };
        if prospective > MAX_ROWS {
            return Err(Error::Unsupported("retaining prospective row extent"));
        }
        let handle = r.handles[job];
        let mut incoming = Vec::with_capacity(total);
        for i in 0..total {
            if cancel.load(Ordering::Relaxed) { return Err(Error::Cancelled); }
            let mut row = self.source.pair_row(r.bank, r.decoder, job, handle.unit, i)?;
            let time = r.origin + self.release_clock.sum +
                (self.outstanding + i) as f64 * r.hop;
            let hz = pitch(time)?;
            r.transform.apply(&r.bank.units[handle.unit].directory, &mut row.1, hz, &mut row.0)?;
            incoming.push(row);
        }
        self.job_clock.advance(duration)?;
        self.next_job += 1;
        if trim.discard { return Ok(()); }
        self.retained.truncate(self.retained.len() - trim.tail);
        incoming.drain(..trim.front);
        let mut phases: Vec<_> = incoming.iter().map(|row| row.0.harmonic_phase.clone()).collect();
        super::source_phase::repair(&mut phases)?;
        for (row, phase) in incoming.iter_mut().zip(phases) { row.0.harmonic_phase = phase; }
        let labels = &r.bank.units[handle.unit].timeline;
        let previous = self.previous.filter(|&j| r.timing[j].duration > 0.0)
            .map(|j| {
                let h = r.handles[j];
                let unit = &r.bank.units[h.unit];
                unit.timeline[(h.phone + 1).min(unit.timeline.len() - 1)].symbol.as_str()
            }).unwrap_or(labels[handle.phone].symbol.as_str());
        let roles = overlap::Roles {
            target: &r.targets[job].current,
            current: &labels[handle.phone].symbol,
            previous,
            category: r.model.category(&r.targets[job].current),
            start: if handle.phone == 0 { 0.0 } else { labels[handle.phone - 1].end_seconds },
            mid: labels[handle.phone].end_seconds,
            end: labels[(handle.phone + 1).min(labels.len() - 1)].end_seconds,
        };
        let overlap = self.retained.len().min(trim.u);
        if overlap > 0 {
            let old_start = self.retained.len() - overlap;
            let new_start = trim.u - overlap;
            let mut old_metadata: Vec<_> = self.retained.iter().map(|row| row.1).collect();
            let mut new_metadata: Vec<_> = incoming.iter().map(|row| row.1).collect();
            if roles.category <= 1 && roles.target == roles.current && roles.target == roles.previous {
                if r.bank.configuration.numbers("ac_order")? != [72.0]
                    || r.bank.configuration.numbers("voc_order_spec")? != [64.0]
                    || r.bank.configuration.numbers("ac_floor")? != [-6.0]
                    || !r.bank.configuration.flag("ac_enc")? {
                    return Err(Error::Unsupported("same-vowel spectral correction geometry"));
                }
                let correction = Correction::compute(
                    &mut old_metadata[old_start..], &mut new_metadata[..trim.u])?;
                for (i, row) in self.retained.iter_mut().skip(old_start).enumerate() {
                    correction.apply(row, i, true, r.decoder.nyquist)?;
                }
                for (i, row) in incoming.iter_mut().take(trim.u).enumerate() {
                    correction.apply(row, i, false, r.decoder.nyquist)?;
                }
            }
            let weights = overlap::weights(roles,
                &old_metadata[old_start..], &new_metadata[new_start..trim.u],
                r.bank.configuration.flag("concat_force_ct")?,
                r.bank.configuration.optional_first_number("concat_reg_fade")?
                    .ok_or(Error::Format("overlap regularization"))?, r.hop, trim.u)?;
            for i in 0..overlap {
                let row = overlap::blend(self.retained[old_start + i].clone(),
                    incoming[new_start + i].clone(), weights.first[i], weights.second[i], weights.third.as_ref().map(|w|w[i]))?;
                self.retained[old_start + i] = row;
            }
        }
        self.retained.extend(incoming.into_iter().skip(trim.u));
        if self.retained.len() > MAX_ROWS || trim.saved > self.retained.len() {
            return Err(Error::Unsupported("retaining row extent"));
        }
        self.outstanding = trim.saved;
        self.previous = Some(job);
        Ok(())
    }
}

struct Trim { discard: bool, front: usize, tail: usize, u: usize, saved: usize }
impl Trim {
    fn new(current: usize, incoming: usize, retained: usize, raw: i32) -> Result<Self> {
        if current > incoming || incoming > MAX_ROWS || retained > MAX_ROWS {
            return Err(Error::Format("retaining pair shape"));
        }
        let (left, total, e) = (current as i64, incoming as i64, retained as i64);
        let raw = i64::from(raw);
        if total + raw - left <= 0 {
            return Ok(Self { discard: true, front: 0, tail: 0, u: current, saved: 0 });
        }
        let (mut k, mut u, mut saved, mut front, mut tail) = (raw, left, raw, 0, 0);
        if k < 0 { front = left - k; k = 0; saved = 0; u = 0; }
        else if k < left { front = left - k; u = k; saved = k; }
        if e > k { tail = e - k; }
        else if e < k { u = (e - k + u).max(0); saved = e; }
        if front < 0 || front > total || u < 0 || u > total - front || saved < 0 {
            return Err(Error::Format("retaining trim geometry"));
        }
        Ok(Self { discard: false, front: front as usize, tail: tail as usize,
            u: u as usize, saved: saved as usize })
    }
}
