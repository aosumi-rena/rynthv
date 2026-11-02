use super::Editor;
use crate::model::{EntityId, Group, Track, new_id};
use std::collections::{BTreeMap, BTreeSet};

impl Editor {
    pub fn select_track_header(&mut self, id: EntityId, toggle: bool, range: bool) {
        self.group_focus=false;
        let Some(index) = self.project.tracks.iter().position(|t| t.id == id) else {
            return;
        };
        self.finish();
        if range {
            let anchor = self
                .project
                .tracks
                .iter()
                .position(|t| t.id == self.track_anchor)
                .unwrap_or(index);
            if !toggle {
                self.selected_tracks.clear();
            }
            self.selected_tracks.extend(
                self.project.tracks[anchor.min(index)..=anchor.max(index)]
                    .iter()
                    .map(|t| t.id),
            );
        } else if toggle {
            if !self.selected_tracks.remove(&id) {
                self.selected_tracks.insert(id);
            }
        } else {
            self.selected_tracks = BTreeSet::from([id]);
        }
        if !range {
            self.track_anchor = id;
        }
        self.track = id;
        self.reference = None;
        self.selected.clear();
        self.track_focus = true;
    }

    pub fn duplicate_tracks(&mut self, unlinked: bool) {
        let selected: Vec<_> = self
            .project
            .tracks
            .iter()
            .filter(|t| self.selected_tracks.contains(&t.id))
            .cloned()
            .collect();
        if selected.is_empty() {
            return;
        }
        let originals: BTreeMap<_, _> = self
            .project
            .library
            .iter()
            .chain(self.project.tracks.iter().map(|t| &t.main_group))
            .map(|g| (g.uuid.clone(), g.clone()))
            .collect();
        self.begin(if unlinked {
            "Duplicate Track (Unlinked)"
        } else {
            "Duplicate Track"
        });
        let mut added = BTreeSet::new();
        for source in selected {
            let mut copy = source.clone();
            copy.id = new_id();
            copy.name = format!("{} - {}", source.name, crate::i18n::t("复制"));
            renew_group(&mut copy.main_group);
            copy.main_ref.renew_identity();
            copy.main_ref.group_id = copy.main_group.uuid.clone();
            let mut remap =
                BTreeMap::from([(source.main_group.uuid, copy.main_group.uuid.clone())]);
            for reference in &mut copy.groups {
                reference.renew_identity();
                if unlinked {
                    if !remap.contains_key(&reference.group_id)
                        && let Some(original) = originals.get(&reference.group_id)
                    {
                        let mut group = original.clone();
                        renew_group(&mut group);
                        remap.insert(reference.group_id.clone(), group.uuid.clone());
                        self.project.library.push(group);
                    }
                    if let Some(id) = remap.get(&reference.group_id) {
                        reference.group_id = id.clone();
                    }
                }
            }
            added.insert(copy.id);
            self.project.tracks.push(copy);
        }
        self.selected_tracks = added;
        self.track = *self.selected_tracks.first().unwrap();
        self.reference = None;
        self.track_anchor = self.track;
        self.selected.clear();
        self.track_focus = true;
        self.update_track_order();
        self.finish();
    }

    pub fn delete_tracks(&mut self) {
        if self.selected_tracks.is_empty() {
            return;
        }
        self.begin("删除音轨");
        let mut removed = Vec::new();
        self.project.tracks.retain(|track| {
            if self.selected_tracks.contains(&track.id) {
                removed.push(track.clone());
                false
            } else {
                true
            }
        });
        let affected: Vec<_> = removed
            .iter()
            .flat_map(|t| t.groups.iter().map(|r| r.group_id.clone()))
            .collect();
        for track in removed {
            if self
                .project
                .tracks
                .iter()
                .flat_map(Track::references)
                .any(|r| r.group_id == track.main_group.uuid)
            {
                self.project.library.push(track.main_group);
            }
        }
        if self.project.tracks.is_empty() {
            let mut track = Track::new(0);
            track.name = format!("{} 1", crate::i18n::t("未命名音轨"));
            self.project.tracks.push(track);
        }
        self.remove_unreferenced_groups(&affected);
        if !self.project.tracks.iter().any(|t| t.id == self.track) {
            self.track = self.project.tracks[0].id;
            self.reference = None;
        }
        self.selected_tracks = BTreeSet::from([self.track]);
        self.selected.clear();
        self.track_focus = true;
        self.track_anchor = self.track;
        self.update_track_order();
        self.finish();
    }
}

fn renew_group(group: &mut Group) {
    group.uuid = uuid::Uuid::new_v4().to_string();
    for note in &mut group.notes {
        note.renew_identity();
    }
}
