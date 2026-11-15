use super::Editor;
use crate::model::Project;
use std::collections::BTreeMap;

impl Editor {

    pub fn mark_imported(&mut self) {self.history.mark_unsaved();}

    pub fn import_project_tracks(&mut self,mut project:Project) {
        self.finish();self.begin("导入工程音轨");

        for (name,dictionary) in std::mem::take(&mut project.dictionaries) {
            let mut target=name.clone();let mut suffix=2;
            while self.project.dictionaries.get(&target).is_some_and(|d|d!=&dictionary) {
                target=format!("{name} ({suffix})");suffix+=1;
            }
            if target!=name {
                for track in &mut project.tracks {
                    for reference in std::iter::once(&mut track.main_ref).chain(track.groups.iter_mut()) {
                        if reference.extra.get("dictionary").and_then(serde_json::Value::as_str)==Some(name.as_str()) {
                            reference.extra.insert("dictionary".into(),target.clone().into());
                        }
                    }
                }
            }
            self.project.dictionaries.entry(target).or_insert(dictionary);
        }
        let mut ids=BTreeMap::new();
        for group in project.library.iter_mut().chain(project.tracks.iter_mut().map(|t|&mut t.main_group)) {
            let old=group.uuid.clone();group.uuid=uuid::Uuid::new_v4().to_string();
            ids.insert(old,group.uuid.clone());
            for note in &mut group.notes {note.renew_identity();}
        }
        let mut first=None;
        for mut track in project.tracks {
            track.id=crate::model::new_id();track.disp_order=self.project.tracks.len() as i32;
            for reference in std::iter::once(&mut track.main_ref).chain(track.groups.iter_mut()) {
                reference.renew_identity();
                if let Some(id)=ids.get(&reference.group_id){reference.group_id=id.clone();}
            }
            first.get_or_insert((track.id,track.visible_references().next().map(|r|r.id)));
            self.project.tracks.push(track);
        }
        self.project.library.extend(project.library);
        if let Some((track,reference))=first {
            self.track=track;self.reference=reference;self.selected.clear();
            self.selected_tracks=std::collections::BTreeSet::from([track]);self.track_anchor=track;
            self.selected_groups.clear();self.group_focus=false;self.track_focus=false;
        }
        self.finish();
    }
}
