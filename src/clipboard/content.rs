use crate::model::{
    Automation, Extra, Group, GroupRef, Note, NotePerformance, PitchControl, TakeSet,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub library: Vec<Group>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<GroupRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<usize>,
    #[serde(default)]
    pub parameters: BTreeMap<String, Automation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pitch_controls: Vec<PitchControl>,
    #[serde(default, rename = "options.position")]
    pub position: i64,
    #[serde(default, rename = "options.positionSet")]
    pub position_set: bool,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Content {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > super::MAX_BYTES {
            return Err("剪贴板数据过大。".into());
        }
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        let value: Self = serde_json::from_slice(bytes).map_err(|_| "剪贴板数据无效。")?;
        value.validate()?;
        Ok(value)
    }
    pub fn summary(&self) -> super::Summary {
        super::Summary {
            notes: self.notes.len(),
            groups: self.groups.len(),
            has_parameters: self.parameters.values().any(|c| !c.points.is_empty()),
            pitch_controls: self.pitch_controls.len(),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.notes.len() > 100_000
            || self.groups.len() > 100_000
            || self.groups.len() != self.tracks.len()
            || self.groups.len() != self.library.len()
            || self.notes.is_empty()
                && self.groups.is_empty()
                && self.parameters.values().all(|c| c.points.is_empty())
                && self.pitch_controls.is_empty()
        {
            return Err("剪贴板数据无效。".into());
        }
        for note in self
            .notes
            .iter()
            .chain(self.library.iter().flat_map(|g| &g.notes))
        {
            if note.duration <= 0
                || !(0..=127).contains(&note.pitch)
                || note.onset.checked_add(note.duration).is_none()
            {
                return Err("音符剪贴板包含无效的起始时间或时长。".into());
            }
            NotePerformance::from_note(note)?;
            TakeSet::from_extra(&note.extra)?;
        }
        for curve in self.parameters.values().chain(
            self.library
                .iter()
                .flat_map(|g| g.parameters.values().chain(g.vocal_modes.values())),
        ) {
            curve.validate()?;
        }
        for control in self
            .pitch_controls
            .iter()
            .chain(self.library.iter().flat_map(|g| &g.pitch_controls))
        {
            match control {
                PitchControl::Point(p) if p.valid() => {}
                PitchControl::Curve(c) if c.valid() => {}
                _ => return Err("剪贴板音高控制点无效。".into()),
            }
        }
        // Clipboard groups and references are paired by position.
        Ok(())
    }
    pub fn origin(&self) -> Option<i64> {
        if let Some(origin) = self
            .notes
            .iter()
            .map(|n| n.onset)
            .chain(self.groups.iter().map(|r| r.blick_absolute_begin))
            .min()
        {
            return Some(origin);
        }
        let points = self.pitch_controls.iter().filter_map(|c| match c {
            PitchControl::Point(p) => Some(p.pos),
            PitchControl::Curve(c) => c.range().map(|r| r.0),
            _ => None,
        });
        points
            .chain(
                self.parameters
                    .values()
                    .flat_map(|c| c.points.iter().map(|p| p.0)),
            )
            .min()
    }
}
