use super::Runtime;
use crate::synthesis::{
    plan::OwnerId,
    rendered::{VisualPatch, VisualSnapshot},
};
use std::{collections::BTreeMap, sync::Arc, time::Instant};

impl Runtime {
    pub(super) fn sync_visuals(&mut self) {
        let plan = self.planner.plan();
        self.visuals
            .retain(|owner, _| plan.requests.contains_key(owner));
        self.phonemes.retain(|owner,snapshot|plan.requests.get(owner).is_some_and(|request| {
            let current=&request.input;let previous=&snapshot.input;
            current.group_id==previous.group_id && current.voice==previous.voice
                && current.content_offset==previous.content_offset && current.tempo==previous.tempo
                && current.pitch_offset==previous.pitch_offset
        }));
        for (&owner, request) in &plan.requests {

            self.visuals
                .entry(owner)
                .or_default()
                .begin(Arc::clone(&request.input));
        }
    }

    pub(super) fn prepared_phonemes(&mut self,owner:OwnerId,snapshot:crate::synthesis::rendered::PhonemeSnapshot) {
        if self.planner.plan().requests.get(&owner).is_some_and(|r|*r.input==*snapshot.input) {
            self.phonemes.insert(owner,snapshot);
        }
    }

    pub(super) fn patch_visuals(&mut self, patches: Vec<(OwnerId, VisualPatch)>) {
        let now = Instant::now();
        for (owner, patch) in patches {
            if self
                .planner
                .plan()
                .requests
                .get(&owner)
                .is_some_and(|request| *request.input == *patch.input)
            {
                self.visuals.entry(owner).or_default().apply(patch, now);
            }
        }
    }

    pub(super) fn complete_visuals(&mut self, snapshots: BTreeMap<OwnerId, VisualSnapshot>) {
        for (owner, snapshot) in snapshots {
            if self
                .planner
                .plan()
                .requests
                .get(&owner)
                .is_some_and(|request| *request.input == *snapshot.input)
            {

                self.phonemes.insert(owner,crate::synthesis::rendered::PhonemeSnapshot {
                    input:Arc::clone(&snapshot.input),phones:snapshot.data.phonemes.clone(),
                });
                self.visuals.entry(owner).or_default().complete(snapshot);
            }
        }
    }
}
