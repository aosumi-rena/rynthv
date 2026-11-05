// SPDX-License-Identifier: MIT
use super::{
    Error, Result, StdBank, UnitCostModel,
    selection::{Handle, Target},
};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

const SENTINEL: f64 = 1e10;

pub(super) fn refine(
    bank: &StdBank,
    model: &UnitCostModel,
    targets: &[Target],
    row_durations: &[f64],
    candidates: &[Vec<Handle>],
    edges: &[Vec<Vec<f64>>],
    initial: &[Handle],
    cancel: &AtomicBool,
) -> Result<Vec<Handle>> {
    let n = targets.len();
    if n == 0
        || row_durations.len() != n
        || candidates.len() != n
        || edges.len() != n
        || initial.len() != n
    {
        return Err(Error::Format("selection refinement shape"));
    }
    let mut selected = Vec::with_capacity(n);
    for t in 0..n {
        if candidates[t].is_empty()
            || candidates[t].len() > 10
            || edges[t].len() != candidates[t].len()
            || edges[t].iter().any(|row| row.len() != if t == 0 { 1 } else { candidates[t - 1].len() })
        {
            return Err(Error::Format("selection refinement edge shape"));
        }
        selected.push(candidates[t].iter().position(|h| *h == initial[t])
            .ok_or(Error::Format("selection refinement initial membership"))?);
    }
    let mut grouped = HashMap::new();
    for t in (0..n).rev().chain(0..n) {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let before = selected[t];
        let left = if t == 0 { 0 } else { selected[t - 1] };
        let right = selected.get(t + 1).copied();
        let exact = exact_pair(bank, candidates[t][before], &targets[t]);
        let mut scores = Vec::with_capacity(candidates[t].len());
        for (c, &handle) in candidates[t].iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let value = if exact && !exact_pair(bank, handle, &targets[t]) {
                SENTINEL
            } else {
                let mut value = edges[t][c][left];
                value += duration_penalty(bank, model, &mut grouped, row_durations[t], handle)?;
                if let Some(right) = right {
                    value += edges[t + 1][right][c];
                    value += duration_penalty(bank, model, &mut grouped, row_durations[t + 1],
                        Handle { unit: handle.unit, phone: handle.phone + 1 })?;
                }
                value
            };
            scores.push((value, c));
        }
        for i in 1..scores.len() {
            let value = scores[i];
            let mut hole = i;
            while hole > 0 && !(value.0 >= scores[hole - 1].0) {
                scores[hole] = scores[hole - 1];
                hole -= 1;
            }
            scores[hole] = value;
        }
        selected[t] = if scores[0].0 < SENTINEL { scores[0].1 } else { before };
    }
    Ok(selected.iter().enumerate().map(|(t, &c)| candidates[t][c]).collect())
}

fn exact_pair(bank: &StdBank, handle: Handle, target: &Target) -> bool {
    let labels = &bank.units[handle.unit].timeline;
    labels[handle.phone].symbol == target.context[2]
        && labels.get(handle.phone + 1).map(|p| p.symbol.as_str()).unwrap_or("sil")
            == target.context[3]
}

fn duration_penalty(
    bank: &StdBank,
    model: &UnitCostModel,
    cache: &mut HashMap<usize, Vec<f64>>,
    prepared: f64,
    handle: Handle,
) -> Result<f64> {
    if !cache.contains_key(&handle.unit) {
        let labels = &bank.units[handle.unit].timeline;
        let mut values = vec![0.0; labels.len()];
        let mut start = 0;
        let mut total = 0.0;
        let mut previous = 0.0;
        for (i, phone) in labels.iter().enumerate() {
            if i != 0 && model.category(&phone.symbol) <= 1 {
                values[start..i].fill(total);
                start = i;
                total = 0.0;
            }
            let delta = phone.end_seconds - previous;
            total = delta + total;
            previous = phone.end_seconds;
        }
        values[start..].fill(total);
        cache.insert(handle.unit, values);
    }
    let source = cache[&handle.unit].get(handle.phone).copied().unwrap_or(0.0);
    if !prepared.is_finite() || prepared <= 0.0 || !source.is_finite() || source <= 0.0 {
        return Err(Error::Format("selection refinement reached log duration"));
    }
    Ok((prepared.ln() - source.ln()).abs())
}
