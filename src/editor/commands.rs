use super::Editor;
use crate::model::*;

impl Editor {

    pub fn add_note(&mut self, global: i64, duration: i64, pitch: i32) -> Option<EntityId> {
        let global = global.max(0);
        if !self.prepare_note_target(global) {
            return None;
        }
        let reference = self.active_ref()?;

        let note = Note::new(
            global - reference.blick_offset,
            duration.max(1),
            (pitch - reference.pitch_offset).clamp(0, 127),
        );
        let id = note.id;
        self.group_mut()?.notes.push(note);
        self.selected.clear();
        self.selected.insert(id);
        Some(id)
    }
    pub fn delete_selected(&mut self) {
        self.begin("删除音符");
        let selected = self.selected.clone();
        if let Some(group) = self.group_mut() {
            group.notes.retain(|n| !selected.contains(&n.id));
        }
        self.selected.clear();
        self.finish();
    }
    pub fn transpose(&mut self, amount: i32) {
        self.begin("移调");
        let selected = self.selected.clone();
        if let Some(group) = self.group_mut() {
            for n in &mut group.notes {
                if selected.contains(&n.id) {
                    n.pitch = (n.pitch + amount).clamp(0, 127);
                }
            }
        }
        self.finish();
    }
    pub fn mute_selected(&mut self) {
        self.begin("静音音符");
        let selected = self.selected.clone();
        let mute = self
            .notes()
            .iter()
            .filter(|n| selected.contains(&n.id))
            .any(|n| !n.muted());
        if let Some(group) = self.group_mut() {
            for n in &mut group.notes {
                if selected.contains(&n.id) {
                    n.attributes.insert("muted".into(), mute.into());
                }
            }
        }
        self.finish();
    }
    pub fn add_track(&mut self) {
        self.begin("创建音轨");
        let mut t = Track::new(self.project.tracks.len());
        t.name = format!("{} {}",crate::i18n::t("未命名音轨"),self.project.tracks.len()+1);
        t.disp_color = self.project.next_track_color();
        self.track = t.id;
        self.reference = None;
        self.project.tracks.push(t);
        self.selected.clear();
        self.selected_tracks=std::collections::BTreeSet::from([self.track]);
        self.track_anchor=self.track;
        self.track_focus=false;
        self.finish();
    }
    pub fn set_track_color(&mut self, id: EntityId, color: TrackColor) {
        let Some(index) = self.project.tracks.iter().position(|t| t.id == id) else {
            return;
        };
        if self.project.tracks[index].disp_color == color {
            return;
        }
        self.begin("修改音轨颜色");
        self.project.tracks[index].disp_color = color;
        self.finish();
    }
    pub fn set_lyric(&mut self, id: EntityId, lyric: String) {
        self.begin("填入歌词");
        if let Some(n) = self
            .group_mut()
            .and_then(|g| g.notes.iter_mut().find(|n| n.id == id))
        {
            if n.lyrics!=lyric {
                n.lyrics = lyric;
                n.phonemes.clear();
                n.attributes.remove("phonemes");
            }
        }
        self.finish();
    }
    pub fn set_phonemes(&mut self, id: EntityId, phonemes: String) {
        self.begin("编辑音素");
        if let Some(note)=self.group_mut().and_then(|g|g.notes.iter_mut().find(|n|n.id==id)) {
            if note.phonemes!=phonemes {
                note.phonemes=phonemes;
                note.attributes.remove("phonemes");
            }
        }
        self.finish();
    }
}
