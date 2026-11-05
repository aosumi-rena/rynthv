// SPDX-License-Identifier: MIT
use super::{
    Error, NativeRenderer, Phrase, Result, NoteFailure,
    selection::{Selection, Target},
    timing::{Packet, PhoneTiming},
};
use std::sync::atomic::AtomicBool;

#[derive(Clone)]
pub(super) struct Phone {
    pub symbol: String,
    pub note: Option<usize>,
    pub token: Option<usize>,
}
struct Work {
    duration: f64,
    phones: Vec<Phone>,
    held: bool,
}
pub(super) struct Prepared {
    pub note_phones: Vec<Vec<String>>,
    pub origin: f64,
    pub phones: Vec<Phone>,
    pub selection: Selection,
    pub timing: Vec<PhoneTiming>,
    pub active_notes: Vec<bool>,
    pub note_failures: Vec<NoteFailure>,
}
impl NativeRenderer {
    pub(super) fn prepare(&mut self, phrase: &Phrase, cancel: &AtomicBool) -> Result<Prepared> {
        phrase.validate()?;
        let resolved = self.phonetics.prepare(phrase, &self.unit_cost)?;
        let active_notes = resolved.rows.iter().map(|row| !row.silent).collect();
        let note_phones = resolved.rows.iter().map(|row| row.phones.clone()).collect();
        let first = phrase.seconds_at(phrase.anchor_tick + phrase.notes[0].local_tick)?;
        let origin = first - 0.5;
        let silence = || Phone {
            symbol: "sil".to_owned(),
            note: None,
            token: None,
        };
        let mut work = vec![Work {
            duration: 0.5,
            phones: vec![silence()],
            held: false,
        }];
        let mut previous = first;
        for (i, note) in phrase.notes.iter().enumerate() {
            let start = phrase.seconds_at(phrase.anchor_tick + note.local_tick)?;
            let end =
                phrase.seconds_at(phrase.anchor_tick + note.local_tick + note.duration_ticks)?;
            if start > previous {
                work.push(Work {
                    duration: start - previous,
                    phones: vec![silence()],
                    held: false,
                });
            }
            let mut phones: Vec<_> = resolved.rows[i].phones
                .iter()
                .enumerate()
                .map(|(token, symbol)| Phone {
                    symbol: symbol.clone(),
                    note: Some(i),
                    token: Some(token),
                })
                .collect();
            if phones.is_empty() {
                phones.push(silence());
            }
            work.push(Work {
                duration: end - start,
                phones,
                held: resolved.rows[i].held,
            });
            previous = end;
        }
        work.push(Work {
            duration: 1.0,
            phones: vec![silence()],
            held: false,
        });
        work.push(Work {
            duration: 0.1,
            phones: vec![silence()],
            held: false,
        });
        let mut previous = 0;
        for i in 1..work.len() {
            if work[i]
                .phones
                .iter()
                .any(|p| self.unit_cost.category(&p.symbol) <= 1)
            {
                while self.unit_cost.category(&work[i].phones[0].symbol) > 1 {
                    let phone = work[i].phones.remove(0);
                    work[previous].phones.push(phone);
                }
            }
            if !work[i].held {
                previous = i;
            }
        }
        let mut rows: Vec<Work> = Vec::new();
        for row in work {
            if row.held {
                rows.last_mut()
                    .ok_or(Error::Format("hold predecessor"))?
                    .duration += row.duration;
                continue;
            }
            let mut starts = vec![0];
            let mut seen = false;
            for (i, phone) in row.phones.iter().enumerate() {
                if self.unit_cost.category(&phone.symbol) <= 1 {
                    if seen {
                        starts.push(i);
                    }
                    seen = true;
                }
            }
            starts.push(row.phones.len());
            let duration = row.duration / ((starts.len() - 1) as f64);
            for interval in starts.windows(2) {
                rows.push(Work {
                    duration,
                    phones: row.phones[interval[0]..interval[1]].to_vec(),
                    held: false,
                });
            }
        }
        let mut phones = Vec::new();
        let mut packets = Vec::new();
        let mut row_durations = Vec::new();
        for row in rows {
            packets.push(Packet {
                begin: phones.len(),
                count: row.phones.len(),
                duration: row.duration,
                weights: vec![1.0; row.phones.len()],
            });
            row_durations.extend(std::iter::repeat_n(row.duration, row.phones.len()));
            phones.extend(row.phones);
        }
        let targets =
            Target::from_labels(&phones.iter().map(|p| p.symbol.clone()).collect::<Vec<_>>());
        let selection = self
            .selector
            .select(&self.bank, &self.unit_cost, &targets, &row_durations, cancel)?;
        let timing =
            super::timing::allocate(&self.bank, &self.unit_cost, &targets, &selection, &packets)?;
        Ok(Prepared {
            note_phones,
            origin,
            phones,
            selection,
            timing,
            active_notes,
            note_failures: resolved.failures,
        })
    }
}
