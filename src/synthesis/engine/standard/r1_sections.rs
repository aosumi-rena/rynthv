//! Offline R1 phrase scheduling at real score rests; no sustained-phone cuts.
use std::sync::atomic::{AtomicBool, Ordering};
use crate::synthesis::{
    plan::{GlobalBlick, OwnerInput},
    r1_std::pipeline::StdBank,
    voice::{Result, VoiceError},
};
use super::NativeAudio;

const MINIMUM_REST: f64 = 0.25;
const MAX_AGGREGATE_SAMPLES: usize = 32 * 1024 * 1024;

struct ActiveNote {
    source: usize,
    start: f64,
    end: f64,
    continuation: bool,
    silence: bool,
}

fn cancelled(flag: &AtomicBool) -> Result<()> {
    if flag.load(Ordering::Acquire) {
        Err(VoiceError::new("R1 synthesis cancelled"))
    } else { Ok(()) }
}

fn origin(first: f64, available: f64) -> f64 {
    available.max(0.0).max(first - 0.5).min(first)
}

// TempoMapping::position_at truncates. Round a lower bound toward the future
// so conversion cannot place a new request before its actual silent interval.
fn lower_bound(input: &OwnerInput, seconds: f64) -> Result<GlobalBlick> {
    let mut position = input.tempo.position_at(seconds);
    if input.tempo.seconds_at(position) < seconds {
        position.0 = position.0.checked_add(1)
            .ok_or_else(|| VoiceError::new("R1 section boundary overflows"))?;
    }
    Ok(position)
}

/// Each clone keeps the complete global control context. Only its note slice
/// and publication section change. A short request is returned unchanged.
pub(super) fn sections(input: &OwnerInput, hop: f64) -> Result<Vec<OwnerInput>> {
    if !hop.is_finite() || !(0.001..=0.02).contains(&hop) || input.notes.len() > 8192 {
        return Err(VoiceError::new("Invalid R1 section clock or note count"));
    }
    let mut active: Vec<ActiveNote> = Vec::new();
    for (source, note) in input.notes.iter().enumerate() {
        if note.attributes.get("muted").and_then(serde_json::Value::as_bool) == Some(true) { continue; }
        let start = note.onset.0.checked_add(input.content_offset.0)
            .ok_or_else(|| VoiceError::new("R1 section onset overflows"))?;
        let end = start.checked_add(note.duration)
            .ok_or_else(|| VoiceError::new("R1 section end overflows"))?;
        let start = input.tempo.seconds_at(GlobalBlick(start));
        let end = input.tempo.seconds_at(GlobalBlick(end));
        if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start
            || active.last().is_some_and(|previous| start < previous.end) {
            return Err(VoiceError::new("Invalid R1 section note interval"));
        }
        let explicit = !note.phonemes.trim().is_empty();
        active.push(ActiveNote { source, start, end,
            continuation: !explicit && matches!(note.lyrics.as_str(), "+" | "-" | "!con"),
            silence: !explicit && note.lyrics == "!sil" });
    }
    if active.is_empty() { return Ok(vec![input.clone()]); }
    let budget = 24.0_f64.min(7000.0 * hop);
    let (publication_begin, publication_end) = input.publication_bounds();
    let available = input.tempo.seconds_at(publication_begin);
    if !available.is_finite() { return Err(VoiceError::new("Invalid R1 publication clock")); }
    let whole_origin = origin(active[0].start, available);
    let whole_duration = (active.last().expect("active notes").end-whole_origin) as f32;
    let whole_frames = (f64::from(whole_duration)/hop) as usize;
    let whole_records = active.len()+2+active.windows(2).filter(|pair|pair[1].start>pair[0].end).count();
    // Keep the established single-call path through its actual renderer caps.
    // The selected-guide frame extent is still checked by that renderer; the
    // smaller scheduling budget is not an added whole-request restriction.
    if whole_records<=4096 && (2..=8192).contains(&whole_frames) {
        return Ok(vec![input.clone()]);
    }
    let boundary = |at: usize| {
        let current = &active[at];
        !current.continuation && (current.start - active[at-1].end >= MINIMUM_REST
            || current.silence && current.end-current.start >= MINIMUM_REST
                && (at+1 == active.len() || !active[at+1].continuation))
    };
    let section_begin = |at: usize| -> Result<GlobalBlick> {
        if at == 0 { return Ok(publication_begin); }
        let current = &active[at];
        let begin = if current.silence { current.start } else {
            // Both endpoints lie in the real gap. This leaves room for the
            // previous renderer's final frame without consuming fake history.
            let gap = current.start-active[at-1].end;
            active[at-1].end + (2.0*hop).min(gap*0.5)
        };
        Ok(GlobalBlick(lower_bound(input, begin)?.0.max(publication_begin.0)))
    };
    let mut ranges = Vec::new();
    let mut first = 0;
    while first < active.len() {
        let begin = section_begin(first)?;
        let request_origin = origin(active[first].start, input.tempo.seconds_at(begin));
        let mut last_boundary = None;
        let mut end = first;
        let mut records = 2usize; // Existing constructed context, never audio warmup.
        while end < active.len() {
            if end > first && boundary(end) && active[end-1].end-request_origin <= budget {
                last_boundary = Some(end);
            }
            let cost = 1 + usize::from(end > first && active[end].start > active[end-1].end);
            if active[end].end-request_origin > budget || records+cost > 4096 { break; }
            records += cost;
            end += 1;
        }
        if end == active.len() { ranges.push((first, end, begin)); break; }
        let end = last_boundary.filter(|end| *end > first).ok_or_else(|| VoiceError::new(
            "This continuous R1 phrase exceeds the bounded renderer. Add a real rest of at least 0.25 seconds or split the part; sustained notes and continuation chains are not cut automatically"))?;
        ranges.push((first, end, begin));
        first = end;
    }
    if ranges.len() == 1 { return Ok(vec![input.clone()]); }
    let mut result = Vec::with_capacity(ranges.len());
    for (index, &(first, end, begin)) in ranges.iter().enumerate() {
        let mut section = input.clone();
        section.notes = input.notes[active[first].source..=active[end-1].source].to_vec();
        let limit = ranges.get(index+1).map(|next| next.2);
        let end = match (publication_end, limit) {
            (Some(a), Some(b)) => Some(GlobalBlick(a.0.min(b.0))),
            (a, b) => a.or(b),
        };
        section.section_bounds = Some((begin, end));
        result.push(section);
    }
    Ok(result)
}

/// Aggregate complete section renders on one absolute timeline. Missing spans
/// are real score rests; overlapping audio is refused, never silently replaced.
pub(super) fn render(bank: &StdBank, input: &OwnerInput, cancel: &AtomicBool, emit:&mut super::stream::NativeObserver<'_>) -> Result<NativeAudio> {
    cancelled(cancel)?;
    let sections = sections(input, bank.frame_seconds())?;
    if sections.len() == 1 { return super::r1::render(bank, input, cancel, emit); }
    let mut result: Option<NativeAudio> = None;
    let mut first_origin=None;
    for section in sections {
        cancelled(cancel)?;
        let mut raw_origin=0.0;let mut canonical_origin=0.0;
        let mut forward=|event:super::stream::NativeEvent<'_>| {
            use super::stream::NativeEvent as E;
            match event {
                E::Prepared {origin,rate,hop,phones,errors,pitch}=>{
                    raw_origin=origin;let base=*first_origin.get_or_insert(origin);
                    canonical_origin=base+((origin-base)*f64::from(rate)).round()/f64::from(rate);
                    emit(E::Prepared {origin:canonical_origin,rate,hop,phones,errors,pitch})
                },
                E::Pcm {origin,samples}=>emit(E::Pcm {origin:canonical_origin+(origin-raw_origin),samples}),
                other=>emit(other),
            }
        };
        let part = super::r1::render(bank, &section, cancel, &mut forward)?;
        cancelled(cancel)?;
        if part.samples.len()>MAX_AGGREGATE_SAMPLES {
            return Err(VoiceError::new("R1 whole-part audio is too large; split the part"));
        }
        if let Some(output) = &mut result {
            if part.rate != output.rate || part.pitch_hop != output.pitch_hop || !part.origin.is_finite() {
                return Err(VoiceError::new("R1 section output clocks disagree"));
            }
            let offset = ((part.origin-output.origin)*f64::from(output.rate)).round();
            if !offset.is_finite() || offset < output.samples.len() as f64 {
                return Err(VoiceError::new("R1 section audio overlaps; use a longer real rest"));
            }
            if offset > MAX_AGGREGATE_SAMPLES as f64 {
                return Err(VoiceError::new("R1 whole-part audio is too large; split the part"));
            }
            let offset = offset as usize;
            let end = offset.checked_add(part.samples.len())
                .filter(|end| *end <= MAX_AGGREGATE_SAMPLES)
                .ok_or_else(|| VoiceError::new("R1 whole-part audio is too large; split the part"))?;
            output.samples.try_reserve(end-output.samples.len())
                .map_err(|_| VoiceError::new("Cannot allocate R1 whole-part audio"))?;
            output.samples.resize(offset, 0.0);
            output.samples.extend(part.samples);
            output.pitch.extend(part.pitch);
            output.phones.extend(part.phones);
            output.note_errors.extend(part.note_errors);
        } else { result = Some(part); }
    }
    cancelled(cancel)?;
    result.ok_or_else(|| VoiceError::new("R1 phrase scheduling produced no audio"))
}
