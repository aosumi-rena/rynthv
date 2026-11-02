use super::*;
use std::collections::BTreeSet;

impl Editor {
    pub fn duplicate_clipboard_groups(&mut self) -> Result<usize, String> {
        let Some(data) = self.capture_groups()? else {
            return Ok(0);
        };
        let at = data
            .groups
            .iter()
            .filter(|r| r.blick_absolute_end >= 0)
            .map(|r| r.blick_absolute_end)
            .max()
            .unwrap_or(self.transport.position);
        self.paste_groups(&data, at)
    }
    pub fn select_group_clipboard(&mut self, track: EntityId, reference: EntityId, multiple: bool) {
        let previous = self.selected_groups.clone();
        if self
            .project
            .tracks
            .iter()
            .any(|t| t.id == track && t.is_instrumental() && t.main_ref.id == reference)
        {
            self.select_track(track);
        } else {
            self.select_reference(track, reference);
        }
        self.selected_groups = if multiple { previous } else { BTreeSet::new() };
        if !multiple || !self.selected_groups.remove(&(track, reference)) {
            self.selected_groups.insert((track, reference));
        }
        self.group_focus = true;
    }
    pub fn select_all_groups(&mut self) {
        let track = self.current_track();
        self.selected_groups = track
            .references()
            .filter(|r| {
                track.visible_references().any(|v| v.id == r.id)
                    || (r.id == track.main_ref.id && track.audio_clip().is_some())
            })
            .map(|r| (self.track, r.id))
            .collect();
    }
    pub fn delete_clipboard_groups(&mut self) {
        let selected = self.selected_groups.clone();
        self.begin("删除音符组");
        for track in &mut self.project.tracks {
            if selected.contains(&(track.id, track.main_ref.id)) {
                if let Some(audio) = &mut track.instrumental {
                    audio.clip = None;
                }
                track.main_group.notes.clear();
                track.main_group.pitch_controls.clear();
                for c in track
                    .main_group
                    .parameters
                    .values_mut()
                    .chain(track.main_group.vocal_modes.values_mut())
                {
                    c.points.clear();
                }
            }
            track
                .groups
                .retain(|r| !selected.contains(&(track.id, r.id)));
        }
        self.selected_groups.clear();
        self.finish();
    }
    pub(super) fn capture_groups(&self) -> Result<Option<Content>, String> {
        let mut data = Content {
            position_set: true,
            ..Default::default()
        };
        let mut snapshot = None;
        if self
            .project
            .tracks
            .iter()
            .any(|t| t.is_instrumental() && self.selected_groups.contains(&(t.id, t.main_ref.id)))
        {
            let mut project = self.project.clone();
            crate::storage::synchronize_audio_references(&mut project);
            snapshot = Some(project);
        }
        let project = snapshot.as_ref().unwrap_or(&self.project);
        for (index, track) in project.tracks.iter().enumerate() {
            for reference in track
                .references()
                .filter(|r| self.selected_groups.contains(&(track.id, r.id)))
            {
                data.tracks.push(index);
                data.groups.push(reference.clone());
                let mut group = project
                    .group(&reference.group_id)
                    .ok_or("音符组不存在。")?
                    .clone();
                group.uuid = uuid::Uuid::new_v4().to_string();
                data.library.push(group);
            }
        }
        if data.groups.is_empty() {
            return Ok(None);
        }
        data.validate()?;
        Ok(Some(data))
    }
    pub(super) fn paste_groups(&mut self, data: &Content, at: i64) -> Result<usize, String> {
        self.paste_groups_linked(data, at, false)
    }
    pub fn paste_arrangement_text(
        &mut self,
        text: &str,
        track: EntityId,
        at: i64,
        linked: bool,
    ) -> Result<usize, String> {
        let mut data = clipboard::read(text)?;
        if data.groups.is_empty() {
            return Ok(0);
        }
        if data.tracks.iter().all(|index| *index == data.tracks[0]) {
            let index = self
                .project
                .tracks
                .iter()
                .position(|t| t.id == track)
                .ok_or("音轨不存在。")?;
            data.tracks.fill(index);
        }
        self.paste_groups_linked(&data, at, linked)
    }
    fn paste_groups_linked(
        &mut self,
        data: &Content,
        at: i64,
        linked: bool,
    ) -> Result<usize, String> {
        if at < 0 {
            return Err("粘贴位置不能早于项目起点。".into());
        }
        let origin = data.origin().ok_or("剪贴板数据无效。")?;
        let delta = at.checked_sub(origin).ok_or("粘贴位置超出可用范围。")?;
        let mut tracks = self.project.tracks.clone();
        let mut library = Vec::new();
        let mut selected = BTreeSet::new();

        for ((reference, &offset), source) in
            data.groups.iter().zip(&data.tracks).zip(&data.library)
        {
            let target = offset;
            if target >= tracks.len() {
                continue;
            }
            // Preserve source track placement.
            if tracks[target].is_instrumental() {
                continue;
            }
            let audio = reference
                .extra
                .get("isInstrumental")
                .and_then(serde_json::Value::as_bool)
                == Some(true);
            // Unlinked copies receive a separate group and name.
            let existing = self.project.group(&reference.group_id);
            let mut copy = existing.unwrap_or(source).clone();
            if existing.is_some() && !linked {
                copy.name.push('*');
            }
            let copy_group = !linked || existing.is_none();
            if copy_group {
                copy.uuid = uuid::Uuid::new_v4().to_string();
                for note in &mut copy.notes {
                    note.id = new_id();
                }
                for control in &mut copy.pitch_controls {
                    curves::shift_control(control, 0, 0, true)?;
                }
            }
            let mut r = reference.clone();
            r.renew_identity();
            r.group_id = copy.uuid.clone();
            let duration = if audio && r.blick_absolute_end == -1 {
                reference
                    .extra
                    .get("audio")
                    .and_then(|a| a.get("duration"))
                    .and_then(serde_json::Value::as_f64)
                    .filter(|d| d.is_finite() && *d > 0.0)
                    .map(|d| {
                        self.project
                            .time
                            .position_at(self.project.time.seconds_at(r.blick_offset) + d)
                            - r.blick_absolute_begin
                    })
            } else if r.blick_absolute_end >= 0 {
                r.blick_absolute_end.checked_sub(r.blick_absolute_begin)
            } else {
                source
                    .notes
                    .iter()
                    .filter_map(|n| n.onset.checked_add(n.duration))
                    .max()
                    .and_then(|t| t.checked_add(r.blick_offset))
                    .and_then(|t| t.checked_sub(r.blick_absolute_begin))
            }
            .filter(|d| *d > 0)
            .ok_or("音符组时间范围无效。")?;
            let proposed = r
                .blick_absolute_begin
                .checked_add(delta)
                .ok_or("粘贴位置超出可用范围。")?;
            let mut begin = proposed;
            loop {
                let end = begin
                    .checked_add(duration)
                    .ok_or("粘贴位置超出可用范围。")?;
                let collision = tracks[target]
                    .visible_references()
                    .filter(|r| {
                        r.blick_absolute_end >= 0
                            && begin < r.visible_end()
                            && end > r.blick_absolute_begin
                    })
                    .map(|r| r.visible_end())
                    .max();
                if let Some(next) = collision {
                    if next == i64::MAX {
                        return Err("音符组时间范围无效。".into());
                    }
                    begin = next;
                } else {
                    break;
                }
            }
            let shift = begin
                .checked_sub(r.blick_absolute_begin)
                .ok_or("粘贴位置超出可用范围。")?;
            r.blick_offset = r
                .blick_offset
                .checked_add(shift)
                .ok_or("粘贴位置超出可用范围。")?;
            r.blick_absolute_begin = begin;
            if reference.blick_absolute_end >= 0 {
                r.blick_absolute_end = begin
                    .checked_add(duration)
                    .ok_or("粘贴位置超出可用范围。")?;
            }
            selected.insert((tracks[target].id, r.id));
            tracks[target].groups.push(r);
            if copy_group {
                library.push(copy);
            }
        }
        if selected.is_empty() {
            return Ok(0);
        }
        library.retain(|g| {
            tracks
                .iter()
                .flat_map(|t| &t.groups)
                .any(|r| r.group_id == g.uuid)
        });
        let count = selected.len();
        self.finish();
        self.begin("粘贴音符组");
        self.project.library.extend(library);
        self.project.tracks = tracks;
        self.selected_groups = selected;
        self.group_focus = true;
        self.selected.clear();
        if let Some(&(track, reference)) = self.selected_groups.first() {
            self.track = track;
            self.reference = Some(reference);
        }
        self.finish();
        Ok(count)
    }
}
