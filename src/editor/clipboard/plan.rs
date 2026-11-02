use super::{Editor, NoteClipboard, curves};
use crate::model::{EntityId, Group, GroupRef, Note};

pub(super) struct PastePlan {
    track: EntityId,
    target: Target,
    notes: Vec<Note>,
    overlay: Box<Group>,
}

enum Target {
    Existing { reference: EntityId, group: String },
    New(Box<(Group, GroupRef)>),
}

impl PastePlan {
    pub(super) fn prepare(
        editor: &Editor,
        clipboard: &NoteClipboard,
        at: i64,
        duplicate: bool,
    ) -> Result<Self, String> {
        clipboard.validate()?;
        if at < 0 {
            return Err("粘贴位置不能早于项目起点。".into());
        }
        let track = editor.current_track();
        if track.is_instrumental() {
            return Err("请先选择歌声音轨再粘贴音符。".into());
        }
        let reference = if duplicate {
            editor.active_ref()
        } else {
            editor.group_at(at).and_then(|id| {
                track
                    .visible_references()
                    .find(|reference| reference.id == id)
            })
        };
        let offset = reference
            .map(|reference| reference.blick_offset)
            .unwrap_or(at);
        let mut notes = clipboard.notes.clone();
        let origin = clipboard.origin().ok_or("剪贴板数据无效。")?;
        let delta = at
            .checked_sub(offset)
            .and_then(|v| v.checked_sub(origin))
            .ok_or("粘贴位置超出可用范围。")?;
        let mut end = at;
        for note in &mut notes {
            let global = at
                .checked_add(
                    note.onset
                        .checked_sub(origin)
                        .ok_or("粘贴位置超出可用范围。")?,
                )
                .ok_or("粘贴后的音符时间超出可用范围。")?;
            let global_end = global
                .checked_add(note.duration)
                .ok_or("粘贴后的音符结束时间超出可用范围。")?;
            note.onset = global
                .checked_sub(offset)
                .ok_or("粘贴后的组内时间超出可用范围。")?;
            note.onset
                .checked_add(note.duration)
                .ok_or("粘贴后的组内结束时间超出可用范围。")?;
            note.renew_identity();
            end = end.max(global_end);
        }
        let target = if let Some(reference) = reference {
            editor
                .project
                .group(&reference.group_id)
                .ok_or("目标音符组不存在，无法粘贴。")?;
            Target::Existing {
                reference: reference.id,
                group: reference.group_id.clone(),
            }
        } else {
            let group = Group::new(&track.name);
            let mut reference = track.main_ref.clone();
            reference.renew_identity();
            reference.group_id = group.uuid.clone();
            reference.blick_absolute_begin = at;
            reference.blick_absolute_end = end;
            reference.blick_offset = at;
            reference.pitch_offset = 0;
            Target::New(Box::new((group, reference)))
        };
        let mut overlay = match &target {
            Target::Existing { group, .. } => editor.project.group(group).unwrap().clone(),
            Target::New(value) => value.0.clone(),
        };
        let minimum = offset.checked_neg().ok_or("粘贴位置超出可用范围。")?;
        curves::paste(&mut overlay, clipboard, delta, minimum)?;
        Ok(Self {
            track: track.id,
            target,
            notes,
            overlay: Box::new(overlay),
        })
    }

    pub(super) fn apply(self, editor: &mut Editor) -> usize {
        let count = self.notes.len();
        editor.selected = self.notes.iter().map(|note| note.id).collect();
        editor.track = self.track;
        match self.target {
            Target::Existing { reference, group } => {
                editor
                    .project
                    .group_mut(&group)
                    .unwrap()
                    .notes
                    .extend(self.notes);
                editor.reference = Some(reference);
            }
            Target::New(target) => {
                let (mut group, reference) = *target;
                group.notes = self.notes;
                editor.reference = Some(reference.id);
                editor.project.library.push(group);
                editor
                    .project
                    .tracks
                    .iter_mut()
                    .find(|track| track.id == self.track)
                    .unwrap()
                    .groups
                    .push(reference);
            }
        }
        let group = editor.group_mut().unwrap();
        group.parameters = self.overlay.parameters;
        group.vocal_modes = self.overlay.vocal_modes;
        group.pitch_controls = self.overlay.pitch_controls;
        count
    }
}
