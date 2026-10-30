mod clipboard;
mod commands;
mod dictionary;
mod groups;
mod history;
mod instrumental;
mod loop_region;
mod lyrics;
mod midi;
mod project_import;
mod note_properties;
mod performance;
mod parameters;
mod experiments;
mod unison;
mod phonemes;
mod automation;
mod retakes;
mod time_events;
mod transport;
mod voice;
mod vocal_modes;
mod voice_presets;
pub use groups::GroupEdit;
pub use loop_region::LoopRegion;
pub use lyrics::{LyricSelection, split_lyrics};
pub use midi::ImportSummary;
pub use performance::{PerformanceParameter, PerformanceSelection};
pub use retakes::RetakeSelection;
pub use transport::Transport;
pub use vocal_modes::VocalModeEditCapability;

use crate::model::*;
use history::History;
use std::collections::BTreeSet;

#[derive(Clone)]
pub struct SelectionState {
    pub track: EntityId,
    pub reference: Option<EntityId>,
    pub notes: BTreeSet<EntityId>,
}
pub struct Editor {
    pub project: Project,
    pub track: EntityId,
    pub reference: Option<EntityId>,
    pub selected: BTreeSet<EntityId>,
    pub revision: u64,
    pub transport: Transport,
    history: History,
    performance_edit: Option<performance::PerformanceEdit>,
}
impl Editor {
    pub fn new(project: Project) -> Self {
        let track = project
            .tracks
            .iter()
            .find(|t| !t.is_instrumental())
            .unwrap_or(&project.tracks[0]);
        let reference = track.visible_references().next().map(|r| r.id);
        Self {
            track: track.id,
            reference,
            project,
            selected: BTreeSet::new(),
            revision: 0,
            transport: Transport::default(),
            history: History::default(),
            performance_edit: None,
        }
    }
    pub fn current_track(&self) -> &Track {
        self.project
            .tracks
            .iter()
            .find(|t| t.id == self.track)
            .expect("selected track exists")
    }
    pub fn active_ref(&self) -> Option<&GroupRef> {
        let id = self.reference?;
        self.current_track()
            .visible_references()
            .find(|r| r.id == id)
    }
    pub fn group(&self) -> Option<&Group> {
        self.project.group(&self.active_ref()?.group_id)
    }
    pub fn group_mut(&mut self) -> Option<&mut Group> {
        let id = self.active_ref()?.group_id.clone();
        self.project.group_mut(&id)
    }
    pub fn notes(&self) -> &[Note] {
        self.group().map_or(&[], |g| g.notes.as_slice())
    }
    pub fn select_all_notes(&mut self) {
        self.selected = self
            .active_ref()
            .map(|r| {
                self.notes()
                    .iter()
                    .filter(|n| r.note_range(n).is_some())
                    .map(|n| n.id)
                    .collect()
            })
            .unwrap_or_default();
    }
    fn selection(&self) -> SelectionState {
        SelectionState {
            track: self.track,
            reference: self.reference,
            notes: self.selected.clone(),
        }
    }
    fn restore_selection(&mut self, selection: SelectionState) {
        self.track = selection.track;
        self.reference = selection.reference;
        self.selected = selection.notes;
        self.reconcile();
    }
    pub fn begin(&mut self, label: &str) {
        self.history.begin(&self.project, self.selection(), label);
    }
    pub fn is_editing(&self) -> bool {
        self.history.is_editing()
    }
    pub fn finish(&mut self) {
        self.performance_edit = None;
        if self.history.finish(&self.project) {
            self.revision += 1;
            self.reconcile();
        }
    }
    pub fn cancel(&mut self) {
        self.performance_edit = None;
        if let Some(selection) = self.history.cancel(&mut self.project) {
            self.restore_selection(selection);
        }
    }
    pub fn dirty(&self) -> bool {
        self.history.dirty(&self.project)
    }
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    pub fn undo_label(&self) -> &str {
        self.history.undo_label()
    }
    pub fn undo(&mut self) {
        self.performance_edit = None;
        let selection = self.selection();
        if let Some(selection) = self.history.undo(&mut self.project, selection) {
            self.revision += 1;
            self.restore_selection(selection);
        }
    }
    pub fn redo(&mut self) {
        self.finish_performance_edit();
        let selection = self.selection();
        if let Some(selection) = self.history.redo(&mut self.project, selection) {
            self.revision += 1;
            self.restore_selection(selection);
        }
    }
    fn reconcile(&mut self) {
        if !self
            .project
            .tracks
            .iter()
            .any(|t| t.id == self.track && !t.is_instrumental())
        {

            self.track = self
                .project
                .tracks
                .iter()
                .find(|t| !t.is_instrumental())
                .unwrap_or(&self.project.tracks[0])
                .id;
            self.reference = None;
        }
        if self.active_ref().is_none() {
            self.reference = None;
        }
        let ids: BTreeSet<_> = self
            .active_ref()
            .map(|r| {
                self.notes()
                    .iter()
                    .filter(|n| r.note_range(n).is_some())
                    .map(|n| n.id)
                    .collect()
            })
            .unwrap_or_default();
        self.selected.retain(|id| ids.contains(id));
    }
    pub fn select_track(&mut self, track: EntityId) {
        if !self
            .project
            .tracks
            .iter()
            .any(|t| t.id == track && !t.is_instrumental())
        {
            return;
        }
        self.finish();
        self.track = track;
        self.reference = None;
        self.selected.clear();
    }
    pub fn select_reference(&mut self, track: EntityId, reference: EntityId) {
        if !self.project.tracks.iter().any(|t| {
            t.id == track
                && !t.is_instrumental()
                && t.visible_references().any(|r| r.id == reference)
        }) {
            return;
        }
        if self.track == track && self.reference == Some(reference) {
            return;
        }
        self.finish();
        self.track = track;
        self.reference = Some(reference);
        self.selected.clear();
        self.reconcile();
    }
}
