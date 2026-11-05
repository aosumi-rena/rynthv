// SPDX-License-Identifier: MIT
use super::{
    Error, Result, StdBank, UnitCostModel,
    selection::{Handle, Target},
    warp,
};

pub(super) fn side(
    bank: &StdBank,
    model: &UnitCostModel,
    handle: Handle,
    target: &Target,
    requested: usize,
    output_hop: f64,
    current: bool,
    mut row: impl FnMut(usize) -> [f32; 72],
) -> Result<Vec<Option<f32>>> {
    let labels = &bank.units[handle.unit].timeline;
    let start = if !current {
        labels[handle.phone].end_seconds
    } else if handle.phone == 0 {
        0.0
    } else {
        labels[handle.phone - 1].end_seconds
    };
    let end = labels[(handle.phone + usize::from(!current)).min(labels.len() - 1)].end_seconds;
    let ct = model.category(if current {
        &target.current
    } else {
        &target.next
    });
    let nt = model.category(&target.next);
    if requested > 4096 {
        return Err(Error::Unsupported("source coordinate count"));
    }
    if requested == 0 {
        return Ok(Vec::new());
    }
    if matches!(model.category(&target.current), 10 | 11) && matches!(nt, 10 | 11) {
        return Ok(vec![None; requested]);
    }
    let duration = ((end - start) * (1.0 / output_hop)) as usize;
    let half = duration / 2;
    let (count, drop, padding, optimize) = if (ct <= 1 || ct == 10) && requested < half {
        (half, if current { half - requested } else { 0 }, 0, true)
    } else if ct == 10 && requested > duration {
        (duration, 0, requested - duration, false)
    } else {
        (requested, 0, 0, !matches!(ct, 2 | 3))
    };
    let hop = bank.profile.source_period;
    let begin = (start * (1.0 / hop)) as usize;
    let finish = (end * (1.0 / hop)) as usize;
    let m = finish.saturating_sub(begin);
    let mut coords = if optimize && count >= 5 && m >= 5 {
        let floor = bank
            .configuration
            .optional_first_number("ac_floor")?
            .unwrap_or(-1000.0) as f32;
        let lambda = bank
            .configuration
            .optional_first_number("concat_reg_warp")?
            .unwrap_or(1.0);
        let source: Vec<_> = (begin..finish).map(&mut row).collect();
        let mut curve = Vec::with_capacity(m);
        for i in 0..m {
            let a = &source[i.saturating_sub(2)];
            let b = &source[(i + 2).min(m - 1)];
            let mut total = 0.0;
            for j in 3..67 {
                let d = a[j].max(floor) - b[j].max(floor);
                total += f64::from(d * d);
            }
            curve.push((f64::from((total / 64.0) as f32) + 1e-5) as f32);
        }
        if end - start < count as f64 * output_hop
            && model.category(&target.context[if current { 2 } else { 3 }]) <= 1
            && model.category(&target.context[if current { 1 } else { 2 }]) > 1
        {
            for i in (0..m / 2).rev() {
                curve[i] = curve[i].max(curve[i + 1]);
            }
        }
        let mut q = m as f64 * hop / output_hop;
        if count > 128 {
            q *= 128.0 / count as f64;
        }
        let mut c = warp::optimize(&curve, count, q, lambda)?;
        if count > 128 {
            let inverse = 1.0 / count as f32;
            c = (0..count)
                .map(|i| {
                    let u = c.len() as f32 * (i as f32 * inverse);
                    let j = u as usize;
                    let a = c[j.min(c.len() - 1)];
                    let b = c[(j + 1).min(c.len() - 1)];
                    a + (b - a) * (u - j as f32)
                })
                .collect();
        }
        (0..count)
            .map(|i| (((f64::from(c[i]) * (1.0 / q)) * (end - start)) + start) as f32)
            .collect::<Vec<_>>()
    } else {
        (0..count)
            .map(|i| (((end - start) * i as f64) * (1.0 / count as f64) + start) as f32)
            .collect()
    };
    if drop > 0 {
        coords.drain(..drop);
    }
    if !current && coords.len() > requested {
        coords.truncate(requested);
    }
    let mut out = if current {
        vec![None; padding]
    } else {
        Vec::new()
    };
    out.extend(coords.into_iter().map(Some));
    if !current {
        out.extend(std::iter::repeat_n(None, padding));
    }
    if out.len() != requested {
        return Err(Error::Format("source coordinate assembly"));
    }
    Ok(out)
}
