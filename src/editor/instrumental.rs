use super::Editor;
use crate::model::*;

impl Editor {
    pub fn add_instrumental_track(&mut self) -> EntityId {
        self.begin("创建伴奏轨");
        let mut track = Track::new(self.project.tracks.len());
        track.disp_color = self.project.next_track_color();
        track.name = format!("{} {}",crate::i18n::t("伴奏"),self.project.tracks.iter().filter(|t|t.is_instrumental()).count()+1);
        track.instrumental = Some(InstrumentalTrack::default());
        track
            .main_ref
            .extra
            .insert("isInstrumental".into(), true.into());
        let id = track.id;
        self.project.tracks.push(track);
        self.finish();
        id
    }

    pub fn set_audio_clip(&mut self, id: EntityId, clip: Option<AudioClip>) {
        if clip.as_ref().is_some_and(|c| !c.valid()) {
            return;
        }
        self.begin(if clip.is_some() {
            "导入伴奏"
        } else {
            "移除伴奏"
        });
        if let Some(track) = self.project.tracks.iter_mut().find(|t| t.id == id)
            && let Some(instrumental) = &mut track.instrumental
        {
            if instrumental.clip.is_none() || clip.is_none() {
                let position=clip.as_ref().map_or(0.0,|c|c.onset_seconds);
                track.main_ref.blick_offset=self.project.time.position_at(position);
                track.main_ref.blick_absolute_begin=track.main_ref.blick_offset;
                track.main_ref.blick_absolute_end=-1;
            }
            instrumental.clip = clip;
        }
        self.finish();
    }

    pub fn move_audio_clip(&mut self, id: EntityId, seconds: f64) {
        if !seconds.is_finite() {
            return;
        }
        let position = self.project.time.position_at(seconds);
        if let Some(track) = self
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == id)
            && let Some(clip) = track.instrumental.as_mut().and_then(|t|t.clip.as_mut())
        {
            let seconds_delta=seconds-clip.onset_seconds;
            if let Some(begin)=&mut clip.trim_begin_seconds { *begin+=seconds_delta; }
            if let Some(end)=&mut clip.trim_end_seconds { *end+=seconds_delta; }
            let delta = position.saturating_sub(track.main_ref.blick_offset);
            track.main_ref.blick_absolute_begin = track.main_ref.blick_absolute_begin.saturating_add(delta);
            if track.main_ref.blick_absolute_end != -1 {
                track.main_ref.blick_absolute_end = track.main_ref.blick_absolute_end.saturating_add(delta);
            }
            track.main_ref.blick_offset = position;
            clip.onset_seconds = seconds;
        }
    }
}
