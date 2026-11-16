//! Request-owned discovery sequencing and unverified local ordinary inventories.
//! Native receipts still have to seal every raw graph. These records cannot
//! become checked annotations, labels, a Hint session or an action permit.
use ferric_browser_core::{HintCandidate, MAX_HINT_CANDIDATES};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::interaction_proof::{
    AnnotationError, AnnotationFocusedReceiver, AnnotationRequest, CheckedAnnotationFacts,
    CheckedAnnotations, MAX_INTERACTION_ANNOTATION_BYTES, MergeWorld, WorldProjections,
    decode_annotation_facts_with_budget, merge_worlds,
};

mod ledger;
pub(super) use ledger::{
    Binding as NativeBinding, Ledger as CollectionLedger, NativeBudget, NativeOutcome, NativePhase,
    Operation as NativeOperation, Schedule as VerificationSchedule, Token as NativeToken,
};

pub(super) const MAX_METADATA_BYTES: usize = 4096;
const MAX_SAFE_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum World {
    Top,
    Selected,
}

#[cfg(test)]
mod tests;

impl World {
    const fn index(self) -> usize {
        match self {
            Self::Top => 0,
            Self::Selected => 1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Phase {
    Ordinary,
    Policy,
}

struct Step {
    world: World,
    phase: Phase,
}

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Viewport {
    width: f64,
    height: f64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OrdinaryMetadata {
    nonce: String,
    revision: u64,
    selector_revision: String,
    inspections: u32,
    predicate_steps: u32,
    truncated: bool,
    viewport: Viewport,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "annotation_debit"
    )]
    annotation_bytes: Option<u32>,
}

fn annotation_debit<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Option<u32>, D::Error> {
    u32::deserialize(decoder).map(Some)
}

struct OrdinarySnapshot {
    metadata: OrdinaryMetadata,
    budget: Budget,
    // The native catalogue supplies owner identity. Geometry remains local;
    // projection and semantic classification have not happened here.
    candidates: Arc<[HintCandidate]>,
}

struct PolicySnapshot {
    metadata: OrdinaryMetadata,
    facts: Box<CheckedAnnotationFacts>,
    sealed: bool,
    annotation_bytes: usize,
    annotation_limit: usize,
}

pub(super) struct PendingPolicy<'a> {
    pub(super) nonce: &'a str,
    pub(super) revision: u64,
    pub(super) facts: &'a CheckedAnnotationFacts,
    pub(super) annotation_bytes: usize,
    pub(super) annotation_limit: usize,
    pub(super) viewport: [f64; 2],
    pub(super) ordinary_budget: &'a Budget,
}

#[derive(Clone, Copy, Serialize)]
pub(super) struct Budget {
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<u8>,
    inspections: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidates: Option<usize>,
    predicate_steps: usize,
    annotation_bytes: usize,
}

#[derive(Serialize)]
struct OrdinarySummary<'a> {
    metadata: &'a OrdinaryMetadata,
    candidates: usize,
}

#[derive(Serialize)]
struct Plan<'a> {
    stage: String,
    world: World,
    phase: Phase,
    owner: &'a str,
    budget: Budget,
    #[serde(skip_serializing_if = "Option::is_none")]
    ordinary: Option<OrdinarySummary<'a>>,
}

pub(super) struct CollectionWork {
    selected: Option<AnnotationFocusedReceiver>,
    selector_revision: String,
    steps: Vec<Step>,
    position: usize,
    unavailable: [bool; 2],
    ordinary: [Option<OrdinarySnapshot>; 2],
    ordinary_grants: [Option<Budget>; 2],
    policy: [Option<PolicySnapshot>; 2],
    pub(super) ledger: Box<CollectionLedger>,
}

impl CollectionWork {
    pub(super) fn finish(
        mut self,
        request: &AnnotationRequest,
        projections: &WorldProjections,
    ) -> Result<(Vec<HintCandidate>, CheckedAnnotations), AnnotationError> {
        if self.position != self.steps.len() || self.ledger.has_pending() {
            return Err(AnnotationError::IncompleteEvidence);
        }
        let mut worlds = Vec::with_capacity(2);
        for world in [World::Top, World::Selected] {
            match (
                self.ordinary[world.index()].take(),
                self.policy[world.index()].take(),
            ) {
                (Some(ordinary), Some(policy)) if policy.sealed => worlds.push(MergeWorld {
                    owner: self.owner(world).to_owned(),
                    facts: policy.facts,
                    candidates: ordinary.candidates,
                }),
                (None, None) => {}
                _ => return Err(AnnotationError::IncompleteEvidence),
            }
        }
        merge_worlds(
            request,
            worlds,
            projections,
            self.ledger.used.inspections,
            self.ledger.used.predicate_steps,
            self.ledger.remaining_native().verification_steps,
        )
    }
    pub(super) fn projection_ready(&self, owner: &str) -> bool {
        self.position == self.steps.len()
            && self.steps.iter().any(|step| {
                self.owner(step.world) == owner
                    && self.policy[step.world.index()]
                        .as_ref()
                        .is_some_and(|policy| policy.sealed)
                    && self.ordinary[step.world.index()].is_some()
            })
    }
    /// The caller supplies the already retained native role catalogue. A
    /// required owner must be that exact selected foreign world, never Top or
    /// a structural-only ancestor. Same-origin focused Documents stay in Top.
    pub(super) fn capture(
        selected: Option<AnnotationFocusedReceiver>,
        required_owner: &str,
        selector_revision: u64,
        ledger: Box<CollectionLedger>,
    ) -> Result<Self, Box<CollectionLedger>> {
        if ledger.has_pending() {
            return Err(ledger);
        }
        let selected = selected.filter(|receiver| !receiver.owner.is_empty());
        if selected.as_ref().is_some_and(|receiver| {
            !(1..=8).contains(&receiver.depth)
                || receiver.composed_depth < receiver.depth
                || receiver.composed_depth > 8
        }) {
            return Err(ledger);
        }
        if !required_owner.is_empty()
            && selected
                .as_ref()
                .is_none_or(|receiver| receiver.owner != required_owner)
        {
            return Err(ledger);
        }
        let mut worlds = Vec::with_capacity(2);
        if required_owner.is_empty() {
            worlds.push(World::Top);
        }
        if selected.is_some() {
            worlds.push(World::Selected);
        }
        let mut steps = Vec::with_capacity(4);
        // Both ordinary stages precede either policy stage. A Top policy scan
        // cannot spend the work still needed by selected-world ordinary rows.
        for phase in [Phase::Ordinary, Phase::Policy] {
            for &world in &worlds {
                steps.push(Step { world, phase });
            }
        }
        let mut work = Self {
            selected,
            selector_revision: format!("{selector_revision:016x}"),
            steps,
            position: 0,
            unavailable: [false; 2],
            ordinary: [None, None],
            ordinary_grants: [None, None],
            policy: [None, None],
            ledger,
        };
        work.freeze_ordinary_grant();
        Ok(work)
    }

    fn owner(&self, world: World) -> &str {
        match world {
            World::Top => "",
            World::Selected => {
                &self
                    .selected
                    .as_ref()
                    .expect("captured selected step")
                    .owner
            }
        }
    }

    fn budget(&self, step: &Step) -> Budget {
        if step.phase == Phase::Ordinary {
            return self.ordinary_grants[step.world.index()].expect("issued ordinary grant");
        }
        self.remaining_budget(step)
    }

    fn remaining_budget(&self, step: &Step) -> Budget {
        Budget {
            depth: (step.phase == Phase::Ordinary).then(|| match step.world {
                World::Top => 8,
                World::Selected => {
                    8 - self
                        .selected
                        .as_ref()
                        .expect("captured selected step")
                        .composed_depth
                }
            }),
            inspections: self.ledger.remaining_producer_inspections(),
            candidates: (step.phase == Phase::Ordinary)
                .then(|| MAX_HINT_CANDIDATES - self.ledger.used.candidates),
            predicate_steps: ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS
                - self.ledger.used.predicate_steps,
            annotation_bytes: MAX_INTERACTION_ANNOTATION_BYTES - self.ledger.used.annotation_bytes,
        }
    }

    pub(super) fn plan_json(&self) -> Option<String> {
        let step = self.steps.get(self.position)?;
        serde_json::to_string(&Plan {
            stage: (self.position + 1).to_string(),
            world: step.world,
            phase: step.phase,
            owner: self.owner(step.world),
            budget: self.budget(step),
            ordinary: self.ordinary[step.world.index()]
                .as_ref()
                .map(|ordinary| OrdinarySummary {
                    metadata: &ordinary.metadata,
                    candidates: ordinary.candidates.len(),
                }),
        })
        .ok()
    }

    fn matches(&self, stage: &str, owner: &str) -> bool {
        self.steps.get(self.position).is_some_and(|step| {
            stage == (self.position + 1).to_string() && owner == self.owner(step.world)
        })
    }

    fn advance(&mut self) {
        self.position += 1;
        while self
            .steps
            .get(self.position)
            .is_some_and(|step| self.unavailable[step.world.index()])
        {
            self.position += 1;
        }
        self.freeze_ordinary_grant();
    }

    fn freeze_ordinary_grant(&mut self) {
        if let Some(step) = self.steps.get(self.position)
            && step.phase == Phase::Ordinary
            && self.ordinary_grants[step.world.index()].is_none()
        {
            self.ordinary_grants[step.world.index()] = Some(self.remaining_budget(step));
        }
    }

    /// Rejecting a matching operation reserves its entire remaining grant:
    /// unknown or malformed returned accounting never refunds work. A stale
    /// stage/owner cannot reject a newer operation or alter its budget.
    pub(super) fn reject(&mut self, stage: &str, owner: &str) -> bool {
        if !self.matches(stage, owner) {
            return false;
        }
        let step = &self.steps[self.position];
        self.unavailable[step.world.index()] = true;
        self.policy[step.world.index()] = None;
        self.ledger.burn_remaining(step.phase == Phase::Ordinary);
        self.advance();
        true
    }

    pub(super) fn accept_ordinary(
        &mut self,
        stage: &str,
        owner: &str,
        rows: &str,
        metadata: &str,
    ) -> bool {
        if !self.matches(stage, owner)
            || self.steps[self.position].phase != Phase::Ordinary
            || self.ledger.has_pending()
        {
            return false;
        }
        let step = &self.steps[self.position];
        let budget = self.budget(step);
        let ordinary = (|| {
            if metadata.len() > MAX_METADATA_BYTES {
                return None;
            }
            let metadata: OrdinaryMetadata = serde_json::from_str(metadata).ok()?;
            let nonce = uuid::Uuid::parse_str(&metadata.nonce).ok()?;
            if nonce.to_string() != metadata.nonce
                || nonce.get_version_num() != 4
                || metadata.revision > MAX_SAFE_REVISION
                || metadata.selector_revision != self.selector_revision
                || usize::try_from(metadata.inspections).ok()? > budget.inspections
                || usize::try_from(metadata.inspections).ok()?
                    > self.ledger.remaining_producer_inspections()
                || usize::try_from(metadata.predicate_steps).ok()? > budget.predicate_steps
                || usize::try_from(metadata.predicate_steps).ok()?
                    > ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS
                        - self.ledger.used.predicate_steps
                || !metadata.viewport.width.is_finite()
                || !metadata.viewport.height.is_finite()
                || metadata.viewport.width <= 0.
                || metadata.viewport.height <= 0.
                || metadata.annotation_bytes.is_some()
            {
                return None;
            }
            let mut candidates = crate::parse_hint_payload(rows).ok()?;
            if candidates.len() > budget.candidates.expect("ordinary candidate grant")
                || candidates.len() > MAX_HINT_CANDIDATES - self.ledger.used.candidates
                || candidates.iter().any(|candidate| {
                    !candidate.frame_owner.is_empty()
                        || candidate
                            .frame_path
                            .bytes()
                            .filter(|byte| *byte == b'.')
                            .count()
                            > usize::from(budget.depth.expect("ordinary depth grant"))
                })
            {
                return None;
            }
            if !owner.is_empty() {
                for candidate in &mut candidates {
                    candidate.frame_owner = owner.to_owned();
                }
            }
            Some(OrdinarySnapshot {
                metadata,
                budget,
                candidates: candidates.into(),
            })
        })();
        let Some(ordinary) = ordinary else {
            self.reject(stage, owner);
            return false;
        };
        self.ledger.used.inspections += usize::try_from(ordinary.metadata.inspections)
            .expect("bounded producer inspection count");
        self.ledger.used.predicate_steps += usize::try_from(ordinary.metadata.predicate_steps)
            .expect("bounded producer predicate count");
        self.ledger.used.candidates += ordinary.candidates.len();
        self.ordinary[step.world.index()] = Some(ordinary);
        self.advance();
        true
    }

    /// Stages the exact local raw graph against the already frozen ordinary
    /// inventory. This deliberately performs no canonical normalization.
    pub(super) fn can_prepare_policy(&self, stage: &str, owner: &str) -> bool {
        self.matches(stage, owner)
            && self.steps[self.position].phase == Phase::Policy
            && self.policy[self.steps[self.position].world.index()].is_none()
    }

    pub(super) fn prepare_policy(
        &mut self,
        stage: &str,
        owner: &str,
        raw: &str,
        metadata: &str,
        request: &AnnotationRequest,
    ) -> bool {
        if !self.can_prepare_policy(stage, owner) {
            return false;
        }
        let step = &self.steps[self.position];
        let index = step.world.index();
        let budget = self.budget(step);
        let verification_limit = self.ledger.remaining_native().verification_steps;
        let prepared = (|| {
            let ordinary = self.ordinary[index].as_ref()?;
            if metadata.len() > MAX_METADATA_BYTES || raw.len() > budget.annotation_bytes {
                return None;
            }
            let metadata: OrdinaryMetadata = serde_json::from_str(metadata).ok()?;
            let annotation_bytes = usize::try_from(metadata.annotation_bytes?).ok()?;
            if metadata.nonce != ordinary.metadata.nonce
                // A dirty renderer may repeat ordinary discovery within this
                // policy grant, preserving the exact retained candidate rows.
                // Its new snapshot must spend work and still pass native
                // batch/document/geometry validation before it can be sealed.
                || metadata.revision < ordinary.metadata.revision
                || (metadata.revision != ordinary.metadata.revision && metadata.inspections == 0)
                || metadata.selector_revision != ordinary.metadata.selector_revision
                || metadata.viewport != ordinary.metadata.viewport
                || ordinary.metadata.truncated && !metadata.truncated
                || usize::try_from(metadata.inspections).ok()? > budget.inspections
                || usize::try_from(metadata.predicate_steps).ok()? > budget.predicate_steps
                || annotation_bytes < raw.len()
                || annotation_bytes > budget.annotation_bytes
            {
                return None;
            }
            let facts = decode_annotation_facts_with_budget(
                raw,
                request,
                Arc::clone(&ordinary.candidates),
                verification_limit,
            )
            .ok()?;
            if !facts.belongs_to_world(owner)
                || !facts.matches_local_viewport(metadata.viewport.width, metadata.viewport.height)
                || facts.discovery_work()?
                    != (
                        ordinary
                            .metadata
                            .inspections
                            .checked_add(metadata.inspections)?,
                        ordinary
                            .metadata
                            .predicate_steps
                            .checked_add(metadata.predicate_steps)?,
                    )
            {
                return None;
            }
            Some(PolicySnapshot {
                metadata,
                facts: Box::new(facts),
                sealed: false,
                annotation_bytes,
                annotation_limit: budget.annotation_bytes,
            })
        })();
        let Some(prepared) = prepared else {
            self.reject(stage, owner);
            return false;
        };
        if !self
            .ledger
            .charge_verification(prepared.facts.verification_steps())
        {
            self.reject(stage, owner);
            return false;
        }
        self.ledger.used.inspections +=
            usize::try_from(prepared.metadata.inspections).expect("bounded policy inspections");
        self.ledger.used.predicate_steps +=
            usize::try_from(prepared.metadata.predicate_steps).expect("bounded policy predicates");
        // The native seal still has to corroborate this declared reservation
        // debit against the original private producer counter. No later world
        // can receive a new grant before that completion.
        self.ledger.used.annotation_bytes += prepared.annotation_bytes;
        self.policy[index] = Some(prepared);
        true
    }

    pub(super) fn pending_policy(&self, stage: &str, owner: &str) -> Option<PendingPolicy<'_>> {
        if !self.matches(stage, owner) || self.steps[self.position].phase != Phase::Policy {
            return None;
        }
        let policy = self.policy[self.steps[self.position].world.index()].as_ref()?;
        if policy.sealed {
            return None;
        }
        Some(PendingPolicy {
            nonce: &policy.metadata.nonce,
            revision: policy.metadata.revision,
            facts: &policy.facts,
            annotation_bytes: policy.annotation_bytes,
            annotation_limit: policy.annotation_limit,
            viewport: [
                policy.metadata.viewport.width,
                policy.metadata.viewport.height,
            ],
            ordinary_budget: &self.ordinary[self.steps[self.position].world.index()]
                .as_ref()?
                .budget,
        })
    }

    /// Native association only. Retain the original local bytes/tables for
    /// future projection and resealing; no checked annotation is published.
    pub(super) fn complete_policy(&mut self, stage: &str, owner: &str, valid: bool) -> bool {
        if self.pending_policy(stage, owner).is_none() {
            return false;
        }
        if self.ledger.has_pending() {
            return false;
        }
        if !valid {
            self.reject(stage, owner);
            return false;
        }
        self.policy[self.steps[self.position].world.index()]
            .as_mut()
            .expect("pending policy")
            .sealed = true;
        self.advance();
        true
    }
}
