use super::Editor;
use crate::model::*;

#[derive(Clone, Copy)]
pub enum GroupEdit {
    Move,
    Left,
    Right,
}

impl Editor {

    pub fn insert_group(&mut self, track_id: EntityId, start: i64, duration: i64) -> EntityId {
        let track = self
            .project
            .tracks
            .iter()
            .find(|t| t.id == track_id)
            .expect("target track exists");
        let group = Group::new(&track.name);
        let mut reference = track.main_ref.clone();
        reference.renew_identity();
        reference.group_id = group.uuid.clone();
        reference.blick_absolute_begin = start.max(0);
        reference.blick_absolute_end = start.max(0).saturating_add(duration.max(1));
        reference.blick_offset = start.max(0);
        reference.pitch_offset = 0;
        let id = reference.id;
        self.project.library.push(group);
        self.project
            .tracks
            .iter_mut()
            .find(|t| t.id == track_id)
            .unwrap()
            .groups
            .push(reference);
        self.track = track_id;
        self.reference = Some(id);
        self.track_focus=false;
        self.track_anchor=track_id;
        self.selected.clear();
        id
    }
    pub fn add_group_at(&mut self, track: EntityId, at: i64) {
        if self
            .project
            .tracks
            .iter()
            .find(|t| t.id == track)
            .is_none_or(Track::is_instrumental)
        {
            return;
        }
        self.begin("插入音符组");
        self.insert_group(track, at, 32 * QUARTER);
        self.finish();
    }
    pub fn add_group(&mut self) {
        self.add_group_at(self.track, self.transport.position);
    }

    pub fn group_at(&self, global: i64) -> Option<EntityId> {
        self.active_ref()
            .filter(|r| r.contains(global))
            .map(|r| r.id)
            .or_else(|| {
                self.current_track()
                    .visible_references()
                    .filter(|r| r.contains(global))
                    .last()
                    .map(|r| r.id)
            })
    }

    pub fn prepare_note_target(&mut self, global: i64) -> bool {
        if self.current_track().is_instrumental() {
            return false;
        }
        if let Some(id) = self.group_at(global) {
            if self.reference != Some(id) {
                self.reference = Some(id);
                self.selected.clear();
            }
            return true;
        }
        if self.current_track().visible_references().next().is_some() {
            return false;
        }
        let start = self
            .project
            .time
            .bars(beats(global))
            .last()
            .map_or(0, |b| ticks(b.0));
        self.insert_group(self.track, start, 32 * QUARTER);
        true
    }

    pub fn edit_group(
        &mut self,
        original: &GroupRef,
        destination: EntityId,
        kind: GroupEdit,
        delta: i64,
        minimum: i64,
    ) {
        let Some(source) = self
            .project
            .tracks
            .iter()
            .position(|t| t.groups.iter().any(|r| r.id == original.id))
        else {
            return;
        };
        let Some(target) = self.project.tracks.iter().position(|t| t.id == destination) else {
            return;
        };
        if self.project.tracks[target].is_instrumental() {
            return;
        }
        let Some(index) = self.project.tracks[source]
            .groups
            .iter()
            .position(|r| r.id == original.id)
        else {
            return;
        };
        let mut updated = original.clone();
        match kind {
            GroupEdit::Move => {
                let delta = delta.max(-original.blick_absolute_begin);
                updated.blick_absolute_begin += delta;
                if updated.blick_absolute_end >= 0 {
                    updated.blick_absolute_end += delta;
                }
                updated.blick_offset += delta;
            }
            GroupEdit::Left => {
                updated.blick_absolute_begin = (original.blick_absolute_begin + delta)
                    .clamp(0, original.visible_end().saturating_sub(minimum).max(0))
            }
            GroupEdit::Right => {
                updated.blick_absolute_end = (original.visible_end().saturating_add(delta))
                    .max(original.blick_absolute_begin.saturating_add(minimum))
            }
        }
        if source == target {
            self.project.tracks[source].groups[index] = updated;
        } else {
            self.project.tracks[source].groups.remove(index);
            self.project.tracks[target].groups.push(updated);
        }
        self.track = destination;
        self.reference = Some(original.id);
    }
    pub fn delete_group(&mut self, id: EntityId) {
        let Some(group_id) = self
            .project
            .tracks
            .iter()
            .flat_map(|t| &t.groups)
            .find(|r| r.id == id)
            .map(|r| r.group_id.clone())
        else {
            return;
        };
        self.begin("删除音符组");
        for track in &mut self.project.tracks {
            track.groups.retain(|r| r.id != id);
        }
        self.remove_unreferenced_groups(&[group_id]);
        if self.reference == Some(id) {
            self.reference = None;
            self.selected.clear();
        }
        self.finish();
    }
    pub fn rename_group(&mut self, id: EntityId, name: String) {
        let group_id = self
            .project
            .tracks
            .iter()
            .flat_map(Track::references)
            .find(|r| r.id == id)
            .map(|r| r.group_id.clone());
        if let Some(group_id) = group_id {
            self.begin("重命名音符组");
            self.project.group_mut(&group_id).unwrap().name = name;
            self.finish();
        }
    }
    pub fn rename_track(&mut self, id: EntityId, name: String) {
        self.begin("重命名音轨");
        if let Some(track) = self.project.tracks.iter_mut().find(|t| t.id == id) {
            track.name = name;
        }
        self.finish();
    }
    pub fn reorder_track(&mut self, id: EntityId, delta: isize) {
        let Some(index) = self.project.tracks.iter().position(|t| t.id == id) else {
            return;
        };
        let target =
            (index as isize + delta).clamp(0, self.project.tracks.len() as isize - 1) as usize;
        if index == target {
            return;
        }
        self.begin("调整音轨顺序");
        let track = self.project.tracks.remove(index);
        self.project.tracks.insert(target, track);
        self.update_track_order();
        self.finish();
    }
    pub(super) fn update_track_order(&mut self) {
        for (index, track) in self.project.tracks.iter_mut().enumerate() {
            track.disp_order = index as i32;
        }
    }
    pub(super) fn remove_unreferenced_groups(&mut self, affected: &[String]) {
        let ids: std::collections::BTreeSet<_> = self
            .project
            .tracks
            .iter()
            .flat_map(Track::references)
            .map(|r| r.group_id.clone())
            .collect();
        self.project
            .library
            .retain(|g| !affected.contains(&g.uuid) || ids.contains(&g.uuid));
    }
}
