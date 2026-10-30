mod input;
mod publication;
mod units;
mod sections;

pub use input::*;
pub use units::*;

use crate::model::{Extra, Project};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum DependencyPolicy {

    ConservativeWholeOwner,
}

#[derive(Clone, Debug)]
pub struct OwnerRequest {
    pub owner: OwnerId,
    pub source_revision: ProjectRevision,
    pub epoch: RunEpoch,
    pub input: Arc<OwnerInput>,
    pub coverage: Coverage,
    pub dependency_policy: DependencyPolicy,
}

#[derive(Clone, Debug, Serialize)]
pub struct CoverageChange {
    pub owner: OwnerId,
    pub old: Option<Coverage>,
    pub new: Option<Coverage>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OwnerOutputControls {
    pub track_id: u64,
    pub reference_muted: bool,
    pub mixer: Extra,
}

#[derive(Clone, Default)]
pub struct RenderPlan {

    pub requests: BTreeMap<OwnerId, Arc<OwnerRequest>>,
    pub output_controls: BTreeMap<OwnerId, OwnerOutputControls>,

    pub changes: Vec<CoverageChange>,

    pub obsolete_clear: BTreeMap<OwnerId, Coverage>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OwnerPlanSummary {
    pub owner: OwnerId,
    pub source_revision: ProjectRevision,
    pub epoch: RunEpoch,
    pub note_count: usize,
    pub coverage: Coverage,
    pub dependency_policy: DependencyPolicy,
    pub voice_available: bool,
    pub reused_request: bool,
    pub output: OwnerOutputControls,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlanSummary {
    pub revision: ProjectRevision,
    pub pending_owners: usize,
    pub changed_owners: usize,
    pub removed_owners: usize,
    pub unchanged_owners: usize,
    pub output_changed_owners: usize,
    pub items: Vec<OwnerPlanSummary>,
    pub changes: Vec<CoverageChange>,
    pub obsolete_clear: BTreeMap<OwnerId, Coverage>,
}

pub struct Planner {
    pub experiments: crate::synthesis::experiments::ExperimentalContext,
    plan: RenderPlan,
    next_epoch: u64,
    revision: ProjectRevision,
    output_changed_owners: usize,
}

impl Default for Planner {
    fn default() -> Self {
        Self {
            plan: RenderPlan::default(),
            experiments: Default::default(),
            next_epoch: 0,
            revision: ProjectRevision(0),
            output_changed_owners: 0,
        }
    }
}

impl Planner {
    pub fn update(
        &mut self,
        project: &Project,
        revision: u64,
        voice_identity: impl Fn(OwnerId) -> Option<String>,
    ) -> &RenderPlan {
        self.revision = ProjectRevision(revision);
        self.plan.changes.clear();
        self.plan.obsolete_clear.clear();
        let mut previous = std::mem::take(&mut self.plan.requests);
        let previous_output = std::mem::take(&mut self.plan.output_controls);
        for track in &project.tracks {
            if track.is_instrumental() {
                continue;
            }
            for reference in track.references() {
                let Some(group) = project.group(&reference.group_id) else {
                    continue;
                };
                let identity = voice_identity(OwnerId::new(reference.id));
                let mut input = OwnerInput::new(project, track, reference, group, identity.as_deref());
                input.experiments.settings=self.experiments.settings;
                if self.experiments.settings.enabled {
                    if let Ok(controls)=crate::synthesis::experiments::ExperimentControls::from_extra(&reference.extra) {
                        input.experiments.sources=self.experiments.sources.iter().filter(|source|
                            (self.experiments.settings.voice_blending && controls.voices.iter().any(|v|v.voice==source.voice))
                            || (self.experiments.settings.dds_controls && controls.dds.vocoder.as_ref()==Some(&source.voice))).cloned().collect();
                    }
                }
                for (section, input) in input.sections() {
                let Some(coverage) = input.coverage() else {
                    continue;
                };
                let owner = OwnerId(reference.id, section);
                self.plan.output_controls.insert(
                    owner,
                    OwnerOutputControls {
                        track_id: track.id,
                        reference_muted: reference.mute,
                        mixer: track.mixer.clone(),
                    },
                );
                let old = previous.remove(&owner);
                if let Some(ref old) = old
                    && *old.input == input
                {
                    self.plan.requests.insert(owner, Arc::clone(old));
                    continue;
                }
                let input = Arc::new(input);
                let epoch = self.advance_epoch();
                let request = Arc::new(OwnerRequest {
                    owner,
                    source_revision: self.revision,
                    epoch,
                    input,
                    coverage,
                    dependency_policy: DependencyPolicy::ConservativeWholeOwner,
                });
                self.invalidate(owner, old.as_ref().map(|r| r.coverage), Some(coverage));
                self.plan.requests.insert(owner, request);
                }
            }
        }
        for (owner, old) in previous {
            self.invalidate(owner, Some(old.coverage), None);
        }
        self.output_changed_owners = self
            .plan
            .output_controls
            .iter()
            .filter(|(owner, controls)| previous_output.get(*owner) != Some(*controls))
            .count()
            + previous_output
                .keys()
                .filter(|owner| !self.plan.output_controls.contains_key(*owner))
                .count();
        &self.plan
    }

    pub fn reset(&mut self) {
        self.plan = RenderPlan::default();
        self.revision = ProjectRevision(0);
        self.output_changed_owners = 0;

        self.advance_epoch();
    }

    pub fn plan(&self) -> &RenderPlan {
        &self.plan
    }

    pub fn summary(&self) -> PlanSummary {
        let changed_owners = self.plan.changes.iter().filter(|c| c.new.is_some()).count();
        let removed_owners = self.plan.changes.iter().filter(|c| c.new.is_none()).count();
        PlanSummary {
            revision: self.revision,
            pending_owners: self.plan.requests.len(),
            changed_owners,
            removed_owners,
            unchanged_owners: self.plan.requests.len().saturating_sub(changed_owners),
            output_changed_owners: self.output_changed_owners,
            items: self
                .plan
                .requests
                .values()
                .map(|r| OwnerPlanSummary {
                    owner: r.owner,
                    source_revision: r.source_revision,
                    epoch: r.epoch,
                    note_count: r.input.notes.len(),
                    coverage: r.coverage,
                    dependency_policy: r.dependency_policy,
                    voice_available: r.input.voice.model_source_identity.is_some(),
                    reused_request: !self.plan.changes.iter().any(|c| c.owner == r.owner),
                    output: self.plan.output_controls[&r.owner].clone(),
                })
                .collect(),
            changes: self.plan.changes.clone(),
            obsolete_clear: self.plan.obsolete_clear.clone(),
        }
    }

    fn advance_epoch(&mut self) -> RunEpoch {
        self.next_epoch = self
            .next_epoch
            .checked_add(1)
            .expect("render epoch exhausted");
        RunEpoch(self.next_epoch)
    }

    fn invalidate(&mut self, owner: OwnerId, old: Option<Coverage>, new: Option<Coverage>) {
        if let Some(old) = old {
            self.plan.obsolete_clear.insert(owner, old);
        }
        self.plan.changes.push(CoverageChange { owner, old, new });
    }
}
