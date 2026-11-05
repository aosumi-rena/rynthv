// SPDX-License-Identifier: MIT
use super::{
    Error, Result, StdBank, UnitCostModel,
    target_rules::TargetRules,
    thumbnail::{Join, Thumbnail},
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Handle {
    pub unit: usize,
    pub phone: usize,
}
pub(super) struct Target {
    pub context: [String; 5],
    pub current: String,
    pub next: String,
}
impl Target {
    pub fn from_labels(labels: &[String]) -> Vec<Self> {
        (0..labels.len())
            .map(|i| {
                let context = std::array::from_fn(|lane| {
                    let p = i as isize + lane as isize - 2;
                    if p < 0 || p >= labels.len() as isize {
                        "sil".to_owned()
                    } else {
                        labels[p as usize].clone()
                    }
                });
                Self {
                    current: context[2].clone(),
                    next: context[3].clone(),
                    context,
                }
            })
            .collect()
    }
    fn refs(&self) -> [&str; 5] {
        self.context.each_ref().map(String::as_str)
    }
}
pub(super) struct Selection {
    pub handles: Vec<Handle>,
    pub total_cost: f64,
}
pub(super) struct Selector {
    buckets: HashMap<String, Vec<Handle>>,
    thumbnails: HashMap<usize, Arc<Thumbnail>>,
    join: Join,
    rules: TargetRules,
}
impl Selector {
    pub fn new(bank: &StdBank) -> Result<Self> {
        let mut buckets: HashMap<String, Vec<Handle>> = HashMap::new();
        let mut total = 0usize;
        for (unit, source) in bank.units.iter().enumerate() {
            for (phone, value) in source.timeline.iter().enumerate() {
                total += 1;
                if total > 1_048_576 {
                    return Err(Error::Unsupported("unit label catalogue extent"));
                }
                buckets
                    .entry(value.symbol.clone())
                    .or_default()
                    .push(Handle { unit, phone });
            }
        }
        Ok(Self {
            buckets,
            thumbnails: HashMap::new(),
            join: Join::new(bank)?,
            rules: TargetRules::read(bank)?,
        })
    }
    fn thumbnail(&mut self, bank: &StdBank, unit: usize) -> Result<Arc<Thumbnail>> {
        if let Some(value) = self.thumbnails.get(&unit) {
            return Ok(Arc::clone(value));
        }
        let value = Arc::new(Thumbnail::read(bank, unit)?);
        self.thumbnails.insert(unit, Arc::clone(&value));
        Ok(value)
    }
    pub fn select(
        &mut self,
        bank: &StdBank,
        model: &UnitCostModel,
        targets: &[Target],
        row_durations: &[f64],
        cancel: &AtomicBool,
    ) -> Result<Selection> {
        if targets.is_empty() || targets.len() > 4096 {
            return Err(Error::Unsupported("nonempty selection target extent"));
        }
        if row_durations.len() != targets.len() {
            return Err(Error::Format("selection prepared-row duration shape"));
        }
        let mut ranked = Vec::with_capacity(targets.len());
        let mut retained = 0usize;
        for (t, target) in targets.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let mut a = self
                .buckets
                .get(&target.current)
                .cloned()
                .unwrap_or_default();
            let mut write = 0;
            while write < a.len() && valid(bank, a[write]) {
                write += 1;
            }
            for read in write + 1..a.len() {
                if valid(bank, a[read]) {
                    a[write] = a[read];
                    write += 1;
                }
            }
            if let Some(b) = self.buckets.get(&target.next) {
                for handle in b {
                    if handle.phone == 0 {
                        continue;
                    }
                    let previous = Handle {
                        unit: handle.unit,
                        phone: handle.phone - 1,
                    };
                    let label = label(bank, previous);
                    if label != target.current
                        && (model.category(label) <= 1) == (model.category(&target.current) <= 1)
                    {
                        a.push(previous);
                    }
                }
            }
            if a.is_empty() {
                a = self.buckets.get("sil").cloned().unwrap_or_default();
            }
            retained = retained
                .checked_add(a.len())
                .ok_or(Error::Unsupported("candidate extent"))?;
            if retained > 1_048_576 {
                return Err(Error::Unsupported("retained candidate extent"));
            }
            let next = if let Some(next) = targets.get(t + 1) {
                next.refs()
            } else {
                [
                    target.context[1].as_str(),
                    target.context[2].as_str(),
                    target.context[3].as_str(),
                    target.context[4].as_str(),
                    "sil",
                ]
            };
            let mut row = Vec::new();
            for handle in a {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Error::Cancelled);
                }
                if !valid(bank, handle) {
                    continue;
                }
                let right = Handle {
                    unit: handle.unit,
                    phone: handle.phone + 1,
                };
                let cost =
                    model.cost(
                        target.refs(),
                        context(bank, handle),
                        model.category(target.context[2].as_str()) == 10,
                    )? + model.cost(next, context(bank, right), model.category(next[2]) == 10)?;
                if !cost.is_finite() {
                    return Err(Error::Format("nonfinite candidate rank"));
                }
                row.push((handle, cost));
            }
            row.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));
            row.truncate(10);
            if row.is_empty() {
                return Err(Error::Unsupported("no valid source unit for phone"));
            }
            ranked.push(row.into_iter().map(|x| x.0).collect::<Vec<_>>());
        }
        let mut costs: Vec<Vec<f64>> = Vec::with_capacity(targets.len());
        let mut back: Vec<Vec<usize>> = Vec::with_capacity(targets.len());
        let mut edges: Vec<Vec<Vec<f64>>> = Vec::with_capacity(targets.len());
        let mut join_cache = HashMap::new();
        for (t, target) in targets.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let current: Vec<f64> = ranked[t]
                .iter()
                .map(|&h| self.rules.cost(model, target.refs(), context(bank, h)))
                .collect::<Result<_>>()?;
            let previous: Vec<f64> = if t == 0 {
                Vec::new()
            } else {
                ranked[t - 1]
                    .iter()
                    .map(|h| {
                        self.rules.cost(
                            model,
                            target.refs(),
                            context(
                                bank,
                                Handle {
                                    unit: h.unit,
                                    phone: h.phone + 1,
                                },
                            ),
                        )
                    })
                    .collect::<Result<_>>()?
            };
            let kind = model.category(&target.context[2]);
            let boundary = kind > 1 && model.category(&target.context[3]) <= 1;
            let mut row = Vec::with_capacity(ranked[t].len());
            let mut pointers = Vec::with_capacity(ranked[t].len());
            let mut edge_rows = Vec::with_capacity(ranked[t].len());
            for (c, &a) in ranked[t].iter().enumerate() {
                if t == 0 {
                    row.push(current[c].max(0.0));
                    pointers.push(0);
                    edge_rows.push(vec![current[c].max(0.0)]);
                    continue;
                }
                let mut best = 1e8;
                let mut arg = 0;
                let mut edge_row = Vec::with_capacity(ranked[t - 1].len());
                for (p, h) in ranked[t - 1].iter().enumerate() {
                    let b = Handle {
                        unit: h.unit,
                        phone: h.phone + 1,
                    };
                    let am = label(bank, a) == target.context[2];
                    let bm = label(bank, b) == target.context[2];
                    let edge = if !am && !bm {
                        50.0
                    } else if boundary && !am {
                        10.0
                    } else {
                        let edge = if previous[p] > current[c] {
                            previous[p]
                        } else {
                            current[c]
                        };
                        if kind == 10 || a == b {
                            edge + 0.0
                        } else {
                            let key = (a, b);
                            let join = if let Some(value) = join_cache.get(&key) {
                                *value
                            } else {
                                let ta = self.thumbnail(bank, a.unit)?;
                                let tb = self.thumbnail(bank, b.unit)?;
                                let (sa, ea) = bounds(bank, a);
                                let (sb, eb) = bounds(bank, b);
                                let value =
                                    self.join.cost(&ta.window(sa, ea)?, &tb.window(sb, eb)?)?;
                                join_cache.insert(key, value);
                                value
                            };
                            edge + join
                        }
                    };
                    edge_row.push(edge);
                    let value = edge + costs[t - 1][p];
                    if best > value {
                        arg = p;
                    }
                    if value < best {
                        best = value;
                    }
                }
                row.push(best);
                pointers.push(arg);
                edge_rows.push(edge_row);
            }
            costs.push(row);
            back.push(pointers);
            edges.push(edge_rows);
        }
        let mut total = 1e8;
        let mut selected = 0;
        for (i, &value) in costs.last().unwrap().iter().enumerate() {
            if total > value {
                selected = i;
            }
            if value < total {
                total = value;
            }
        }
        if !total.is_finite() || total >= 1e8 {
            return Err(Error::Unsupported("no finite unit transition path"));
        }
        let mut handles = vec![ranked[0][0]; targets.len()];
        for t in (0..targets.len()).rev() {
            handles[t] = ranked[t][selected];
            selected = back[t][selected];
        }
        let handles = super::selection_refinement::refine(
            bank, model, targets, row_durations, &ranked, &edges, &handles, cancel,
        )?;
        Ok(Selection {
            handles,
            total_cost: total,
        })
    }
}
fn valid(bank: &StdBank, h: Handle) -> bool {
    let unit = &bank.units[h.unit];
    h.phone + 1 < unit.timeline.len()
        && unit.timeline[h.phone].raw_flags == 1
        && unit.timeline[h.phone + 1].raw_flags == 1
}
fn label(bank: &StdBank, h: Handle) -> &str {
    &bank.units[h.unit].timeline[h.phone].symbol
}
fn context(bank: &StdBank, h: Handle) -> [&str; 5] {
    let labels = &bank.units[h.unit].timeline;
    std::array::from_fn(|i| {
        let index = h.phone as isize + i as isize - 2;
        if index < 0 || index >= labels.len() as isize {
            "sil"
        } else {
            labels[index as usize].symbol.as_str()
        }
    })
}
pub(super) fn bounds(bank: &StdBank, h: Handle) -> (f64, f64) {
    let unit = &bank.units[h.unit];
    (
        if h.phone == 0 {
            0.0
        } else {
            unit.timeline[h.phone - 1].end_seconds
        },
        unit.timeline[h.phone].end_seconds,
    )
}
