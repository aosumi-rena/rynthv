use super::{VisualFlash, VisualPatch, VisualPiece, VisualSnapshot, intersection, matching_input};
use crate::synthesis::{plan::OwnerInput, rendered::RenderVisuals};
use std::{ops::Range, sync::Arc, time::Instant};

#[derive(Clone, Debug, Default)]
pub struct VisualTimeline {
    input: Option<Arc<OwnerInput>>,
    bounds: Range<i64>,
    pieces: Vec<VisualPiece>,
    flashes: Vec<VisualFlash>,
    completed: Option<Arc<RenderVisuals>>,
}

impl VisualTimeline {
    pub fn begin(&mut self, input: Arc<OwnerInput>) {
        if self
            .input
            .as_ref()
            .is_some_and(|old| matching_input(old, &input))
        {
            return;
        }
        let bounds = input.publication_range();
        self.pieces.retain_mut(|piece| {
            if let Some(range) = intersection(&piece.range, &bounds) {
                piece.range = range;
                true
            } else {
                false
            }
        });
        self.flashes.clear();
        self.completed = None;
        self.bounds = bounds;
        self.input = Some(input);
    }

    pub fn apply(&mut self, patch: VisualPatch, now: Instant) {
        if !self.accepts(&patch.input) {
            return;
        }
        let Some(range) = intersection(&patch.range, &self.bounds) else {
            return;
        };
        self.completed = None;
        let first = self.pieces.partition_point(|piece| piece.range.end <= range.start);
        let end = self.pieces.partition_point(|piece| piece.range.start < range.end);
        let covered = &self.pieces[first..end];
        let left = covered.first().filter(|piece| piece.range.start < range.start).map(|piece| VisualPiece {
            range: piece.range.start..range.start,
            snapshot: piece.snapshot.clone(),
        });
        let right = covered.last().filter(|piece| range.end < piece.range.end).map(|piece| VisualPiece {
            range: range.end..piece.range.end,
            snapshot: piece.snapshot.clone(),
        });
        let has_data = patch.data.is_some();
        let middle = patch.data.map(|data| VisualPiece {
            range: range.clone(),
            snapshot: VisualSnapshot { data, input: patch.input },
        });
        let replacement = [left, middle, right];
        let neighbors = first + replacement.iter().flatten().count();
        self.pieces.splice(first..end, replacement.into_iter().flatten());
        let mut index = first.saturating_sub(1);
        let mut limit = neighbors.min(self.pieces.len().saturating_sub(1));
        while index < limit {
            let previous = &self.pieces[index];
            let next = &self.pieces[index + 1];
            if previous.range.end == next.range.start
                && Arc::ptr_eq(&previous.snapshot.data, &next.snapshot.data)
                && matching_input(&previous.snapshot.input, &next.snapshot.input)
            {
                let end = next.range.end;
                self.pieces[index].range.end = end;
                self.pieces.remove(index + 1);
                limit -= 1;
            } else {
                index += 1;
            }
        }
        if patch.highlight || !has_data {
            self.replace_flashes(&range, now);
        } else {

            self.flashes.retain(|flash| flash.intensity(now) > 0.0);
        }
        if patch.highlight && has_data {
            self.flashes.push(VisualFlash {
                range,
                started: now,
            });
        }
    }

    pub fn complete(&mut self, snapshot: VisualSnapshot) {
        if !self.accepts(&snapshot.input)
            || self
                .completed
                .as_ref()
                .is_some_and(|data| Arc::ptr_eq(data, &snapshot.data))
        {
            return;
        }
        self.completed = Some(snapshot.data.clone());
        self.pieces.clear();
        let range = self.bounds.clone();
        if range.start < range.end {
            self.pieces.push(VisualPiece { range, snapshot });
        }
    }

    pub fn pieces(&self) -> &[VisualPiece] {
        &self.pieces
    }

    pub fn intensity(&self, position: i64, now: Instant) -> f32 {
        self.flashes
            .iter()
            .filter(|flash| flash.range.contains(&position))
            .map(|flash| flash.intensity(now))
            .fold(0.0, f32::max)
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.flashes.iter().any(|flash| flash.intensity(now) > 0.0)
    }

    pub fn animating_in(&self, range: Range<i64>, now: Instant) -> bool {
        self.flashes
            .iter()
            .any(|flash| intersection(&flash.range, &range).is_some() && flash.intensity(now) > 0.0)
    }

    fn accepts(&mut self, input: &Arc<OwnerInput>) -> bool {
        if self.input.is_none() {
            self.begin(input.clone());
        }
        self.input
            .as_ref()
            .is_some_and(|current| matching_input(current, input))
    }

    fn replace_flashes(&mut self, range: &Range<i64>, now: Instant) {
        let mut flashes = Vec::with_capacity(self.flashes.len() + 1);
        for flash in self.flashes.drain(..) {
            if flash.intensity(now) == 0.0 {
                continue;
            }
            if intersection(&flash.range, range).is_none() {
                flashes.push(flash);
                continue;
            }
            if flash.range.start < range.start {
                flashes.push(VisualFlash {
                    range: flash.range.start..range.start,
                    started: flash.started,
                });
            }
            if range.end < flash.range.end {
                flashes.push(VisualFlash {
                    range: range.end..flash.range.end,
                    started: flash.started,
                });
            }
        }
        self.flashes = flashes;
    }
}
