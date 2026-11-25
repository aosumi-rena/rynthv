use super::SelectionState;
use crate::model::Project;

struct Entry {
    project: Project,
    selection: SelectionState,
    state: u64,
    label: String,
}
#[derive(Default)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    pending: Option<Entry>,
    state: u64,
    saved: u64,
    imported_unsaved: bool,
    next: u64,
}
impl History {
    pub fn is_editing(&self) -> bool {
        self.pending.is_some()
    }
    pub fn begin(&mut self, project: &Project, selection: SelectionState, label: &str) {
        if self.pending.is_none() {
            self.pending = Some(Entry {
                project: project.clone(),
                selection,
                state: self.state,
                label: label.into(),
            });
        }
    }
    pub fn finish(&mut self, project: &Project) -> bool {
        let Some(entry) = self.pending.take() else {
            return false;
        };
        if entry.project == *project {
            return false;
        }
        self.undo.push(entry);
        self.redo.clear();
        if self.undo.len() > 128 {
            self.undo.remove(0);
        }
        self.next += 1;
        self.state = self.next;
        true
    }
    pub fn cancel(&mut self, project: &mut Project) -> Option<SelectionState> {
        let entry = self.pending.take()?;
        *project = entry.project;
        Some(entry.selection)
    }
    pub fn coalesce_previous(&mut self) {
        if self.undo.len()<2{return;}
        let last=self.undo.last().unwrap();
        if last.state==self.saved{return;}
        self.undo.pop();
    }
    pub fn dirty(&self, project: &Project) -> bool {
        self.imported_unsaved || self.state != self.saved || self.pending.as_ref().is_some_and(|e| e.project != *project)
    }
    pub fn mark_saved(&mut self) {
        self.saved = self.state;
        self.imported_unsaved = false;
    }
    pub fn mark_unsaved(&mut self) {self.imported_unsaved=true;}
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_label(&self) -> &str {
        self.undo.last().map_or("", |e| e.label.as_str())
    }
    pub fn undo(
        &mut self,
        project: &mut Project,
        selection: SelectionState,
    ) -> Option<SelectionState> {
        self.finish(project);
        let e = self.undo.pop()?;
        self.redo.push(Entry {
            project: std::mem::replace(project, e.project),
            selection,
            state: self.state,
            label: e.label,
        });
        self.state = e.state;
        Some(e.selection)
    }
    pub fn redo(
        &mut self,
        project: &mut Project,
        selection: SelectionState,
    ) -> Option<SelectionState> {
        let e = self.redo.pop()?;
        self.undo.push(Entry {
            project: std::mem::replace(project, e.project),
            selection,
            state: self.state,
            label: e.label,
        });
        self.state = e.state;
        Some(e.selection)
    }
}
