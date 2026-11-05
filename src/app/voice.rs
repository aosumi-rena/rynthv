use super::App;
use crate::synthesis::{runtime::VoiceState, settings::VoiceSelection, voice::VoiceInventory};

impl App {
    pub fn voice_name(&self) -> Option<&str> {
        if let Some(key)=self.voice_selection() && let Some(entry)=self.synthesis.library.entry(&key) {
            return Some(entry.localized_display_name());
        }
        let track = self.editor.current_track();
        let database = crate::model::singer_database(
            &track.main_ref.database,
            &self.editor.active_ref().unwrap_or(&track.main_ref).database,
        );
        database
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
    }
    pub fn has_voice(&self) -> bool {
        self.editor.current_track().references().any(|r| {
            ["name", "id"].iter().any(|key| {
                r.database
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|s| !s.is_empty())
            })
        })
    }
    pub fn voice_selection(&self) -> Option<VoiceSelection> {
        let track = self.editor.current_track();
        self.synthesis
            .resolve_voice(track, self.editor.active_ref().unwrap_or(&track.main_ref))
            .ok()
            .flatten()
    }
    pub fn voice_inventory(&self) -> Option<&VoiceInventory> {
        self.synthesis.inventory(&self.voice_selection()?)
    }
    pub fn voice_state(&self) -> &VoiceState {
        self.voice_selection()
            .as_ref()
            .map_or(&VoiceState::Unloaded, |key| self.synthesis.voice_state(key))
    }
    pub fn voice_error(&self) -> Option<&str> {
        self.synthesis.voice_error(&self.voice_selection()?)
    }
    pub fn select_voice(&mut self, selection: VoiceSelection) {
        let Some(entry) = self.synthesis.library.entry(&selection) else {
            return;
        };
        if !entry.availability.is_loadable() {
            return;
        }
        let Some(family) = entry.family else { return; };
        let name = entry.display_name.clone();
        self.commit_lyric();
        if let Err(error) =
            self.editor
                .set_track_voice(Some((&selection.database_id, &name, selection.revision, family)))
        {
            self.error = Some(error);
            return;
        }
        if self.synthesis.voice_error(&selection).is_some() {
            self.synthesis.reload_voice(&selection);
        }
    }
    pub fn unload_voice(&mut self) {
        self.commit_lyric();
        if let Err(error) = self.editor.set_track_voice(None) {
            self.error = Some(error);
        }
    }
    pub fn reload_voice(&mut self) {
        if let Some(key) = self.voice_selection() {
            self.synthesis.reload_voice(&key);
        }
    }
}
