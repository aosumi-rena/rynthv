// SPDX-License-Identifier: MIT
use super::{Error, Result};
pub(crate) const TICKS_PER_QUARTER: i64 = 705_600_000;

#[derive(Clone, Debug)]
pub(crate) struct Note {
    pub local_tick: i64,
    pub duration_ticks: i64,
    pub midi_pitch: u8,
    pub lyric: String,
    pub pronunciation: Option<String>,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tempo {
    pub tick: i64,
    pub bpm: f64,
}
pub(crate) struct Phrase {
    pub notes: Vec<Note>,
    pub tempos: Vec<Tempo>,
    pub anchor_tick: i64,
}
impl Phrase {
    pub fn validate(&self) -> Result<()> {
        if self.notes.is_empty()
            || self.notes.len() > 4096
            || self.anchor_tick < 0
            || self.tempos.is_empty()
            || self.tempos[0].tick != 0
        {
            return Err(Error::Format("phrase notes/tempo/anchor"));
        }
        let mut end = 0;
        for note in &self.notes {
            if note.local_tick < end
                || note.duration_ticks <= 0
                || !(1..=127).contains(&note.midi_pitch)
                || note.lyric.len() > 16384
                || note.pronunciation.as_ref().is_some_and(|p| p.len() > 16384)
            {
                return Err(Error::Format("ordered nonoverlapping phrase notes"));
            }
            end = note
                .local_tick
                .checked_add(note.duration_ticks)
                .ok_or(Error::Format("note tick overflow"))?;
            self.anchor_tick
                .checked_add(end)
                .ok_or(Error::Format("absolute note tick overflow"))?;
        }
        let mut previous = None;
        for tempo in &self.tempos {
            if !tempo.bpm.is_finite()
                || tempo.bpm <= 0.0
                || tempo.tick < 0
                || previous.is_some_and(|p| tempo.tick <= p)
            {
                return Err(Error::Format("ordered positive tempo map"));
            }
            previous = Some(tempo.tick);
        }
        Ok(())
    }
    pub fn seconds_at(&self, absolute_tick: i64) -> Result<f64> {
        self.validate()?;
        if absolute_tick < 0 {
            return Err(Error::Format("negative absolute musical tick"));
        }
        let mut seconds = 0.0;
        for (i, tempo) in self.tempos.iter().enumerate() {
            if tempo.tick >= absolute_tick {
                break;
            }
            let end = self
                .tempos
                .get(i + 1)
                .map_or(absolute_tick, |next| next.tick.min(absolute_tick));
            seconds += (end - tempo.tick) as f64 * (60.0 / tempo.bpm) / (TICKS_PER_QUARTER as f64);
            if end == absolute_tick {
                break;
            }
        }
        if !seconds.is_finite() {
            return Err(Error::Format("nonfinite phrase clock"));
        }
        Ok(seconds)
    }
}
