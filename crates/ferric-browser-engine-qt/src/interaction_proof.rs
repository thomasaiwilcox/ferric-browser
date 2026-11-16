//! Versioned annotation facts, checked against an immutable browser request.
//! A checked annotation is NOT a live DOM proof, broker lease, current-session
//! freshness check or action permit. P05 must establish those independently.
//! Ordinary candidates have a separate decoder and retain their own limits.
mod bounded;
mod collections;
mod collector_request;
#[cfg(any(test, feature = "fuzzing"))]
pub mod fuzz_support;
mod merge;
mod receivers;
mod validation;
mod wire;

use bounded::Revision;
use ferric_browser_config::{EffectiveInteractionPolicy, InteractionFamily};
use ferric_browser_core::{HintCandidate, HintTarget, WindowId};
pub(crate) use merge::{MergeWorld, WorldProjections, merge_worlds};
pub use receivers::{AnnotationDocumentRoles, AnnotationFocusedReceiver};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

pub const INTERACTION_ANNOTATION_VERSION: u32 = 1;
/// Annotation-only UTF-8 bytes, checked before allocating the proof object.
pub const MAX_INTERACTION_ANNOTATION_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationError {
    TooLarge,
    InvalidSchema,
    WrongVersion,
    StaleRequest,
    InvalidInventory,
    InvalidDocument,
    InvalidRoot,
    InvalidAssociation,
    IncompleteEvidence,
    InvalidGeometry,
    WorkLimit,
}

impl fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "Interaction annotation byte limit exceeded",
            Self::InvalidSchema => "invalid Interaction annotation schema",
            Self::WrongVersion => "unsupported Interaction annotation version",
            Self::StaleRequest => "Interaction annotation does not match the captured request",
            Self::InvalidInventory => "Interaction annotation rule inventory mismatch",
            Self::InvalidDocument => "invalid Interaction annotation document graph",
            Self::InvalidRoot => "invalid Interaction annotation root path",
            Self::InvalidAssociation => "invalid Interaction annotation association",
            Self::IncompleteEvidence => "complete Interaction annotation lacks requested evidence",
            Self::InvalidGeometry => "invalid Interaction annotation geometry",
            Self::WorkLimit => "Interaction annotation shared work limit exceeded",
        })
    }
}
impl std::error::Error for AnnotationError {}

#[derive(Clone, Copy, Debug, Default)]
pub struct AnnotationGenerations {
    pub selector_revision: u64,
    pub view_generation: u64,
    pub viewport_generation: u64,
    pub collection_epoch: u64,
    pub provider_epoch: u64,
    pub request_id: u64,
}

/// Only capture from the browser's immutable snapshot and retained lease
/// catalogue. There is deliberately no Deserialize or from-renderer constructor.
#[derive(Clone, Debug)]
pub struct AnnotationRequest {
    policy: EffectiveInteractionPolicy,
    stamp: wire::Stamp,
    document_roles: AnnotationDocumentRoles,
}

impl AnnotationRequest {
    pub(crate) fn selected_collector(&self) -> Option<&AnnotationFocusedReceiver> {
        self.document_roles.focused()
    }

    /// Freeze a request using the browser's current snapshot and native roles.
    ///
    /// # Errors
    /// Returns `InvalidAssociation` for a zero request ID or an
    /// empty/oversized/unsafe scope lease.
    pub fn capture(
        policy: EffectiveInteractionPolicy,
        window: WindowId,
        target: HintTarget,
        family: InteractionFamily,
        generations: AnnotationGenerations,
        scope_lease: Option<&str>,
        document_roles: AnnotationDocumentRoles,
    ) -> Result<Self, AnnotationError> {
        if generations.request_id == 0
            || scope_lease.is_some_and(|scope| {
                scope.is_empty() || scope.len() > 128 || scope.chars().any(char::is_control)
            })
        {
            return Err(AnnotationError::InvalidAssociation);
        }
        let revision = policy.revision();
        let stamp = wire::Stamp {
            profile: revision.profile.to_string(),
            window: window.to_string(),
            tab: target.tab.to_string(),
            document: target.document.to_string(),
            profile_revision: Revision(revision.profile_revision),
            configuration_revision: Revision(revision.configuration_revision),
            policy_revision: Revision(revision.policy_revision),
            target_generation: Revision(target.generation),
            selector_revision: Revision(generations.selector_revision),
            view_generation: Revision(generations.view_generation),
            viewport_generation: Revision(generations.viewport_generation),
            collection_epoch: Revision(generations.collection_epoch),
            provider_epoch: Revision(generations.provider_epoch),
            request_id: Revision(generations.request_id),
            scope_lease: scope_lease.unwrap_or_default().into(),
            family,
        };
        Ok(Self {
            policy,
            stamp,
            document_roles,
        })
    }

    /// Request echo only; serializing it grants no new renderer authority.
    ///
    /// # Errors
    /// Returns `InvalidSchema` if the captured stamp cannot be serialized.
    pub fn stamp_json(&self) -> Result<String, AnnotationError> {
        serde_json::to_string(&self.stamp).map_err(|_| AnnotationError::InvalidSchema)
    }
}

/// Original structural facts awaiting native association and normalization.
/// This type cannot be serialized as completed evidence or change its request
/// or candidate inventory. The caller owns native sequencing and freshness.
pub(crate) struct CheckedAnnotationFacts {
    wire: wire::Envelope,
    request: AnnotationRequest,
    candidates: Arc<[HintCandidate]>,
    work: validation::NormalizationWork,
    association_focused_documents: BTreeSet<wire::Id>,
    original_annotation: String,
}

impl fmt::Debug for CheckedAnnotationFacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedAnnotationFacts")
            .field("rules", &self.wire.outcomes.len())
            .field("documents", &self.wire.documents.len())
            .field("roots", &self.wire.roots.len())
            .field("candidates", &self.candidates.len())
            .finish_non_exhaustive()
    }
}

impl CheckedAnnotationFacts {
    pub(crate) fn verification_steps(&self) -> usize {
        self.work.verification_steps()
    }
    pub(crate) fn discovery_work(&self) -> Option<(u32, u32)> {
        self.wire
            .discovery_work
            .map(|work| (work.inspections, work.predicate_steps))
    }

    /// A local graph cannot claim semantic evidence belonging to another
    /// collector. Rule source is derived from the original authored root and
    /// native roles, never from whichever world reports a positive result.
    pub(crate) fn belongs_to_world(&self, owner: &str) -> bool {
        use ferric_browser_config::InteractionRootStart;
        if self
            .wire
            .documents
            .iter()
            .any(|document| document.frame_owner != owner)
        {
            return false;
        }
        self.wire.outcomes.iter().all(|outcome| {
            if matches!(&outcome.result, wire::ResultFacts::Unavailable { .. }) {
                return true;
            }
            let root = match outcome.rule.kind {
                wire::RuleKind::Anchor => self
                    .request
                    .policy
                    .anchors()
                    .iter()
                    .find(|rule| rule.id == outcome.rule.id)
                    .and_then(|rule| rule.root.as_ref()),
                wire::RuleKind::Collection => self
                    .request
                    .policy
                    .collections()
                    .iter()
                    .find(|rule| rule.id == outcome.rule.id)
                    .and_then(|rule| rule.root.as_ref()),
            };
            if root.is_some_and(|root| root.start == InteractionRootStart::FocusedFrame) {
                self.request
                    .document_roles
                    .focused()
                    .is_some_and(|receiver| receiver.owner == owner)
            } else {
                owner.is_empty()
            }
        })
    }

    pub(crate) fn matches_local_viewport(&self, width: f64, height: f64) -> bool {
        let matches = |witness: &wire::Witness| {
            witness.top_viewport_css.width == width && witness.top_viewport_css.height == height
        };
        self.wire
            .outcomes
            .iter()
            .all(|outcome| match &outcome.result {
                wire::ResultFacts::AnchorObserved { targets, .. } => targets.iter().all(matches),
                wire::ResultFacts::AnchorComplete {
                    matches: targets, ..
                } => targets.iter().all(matches),
                wire::ResultFacts::CollectionObserved { items, .. } => {
                    items.iter().all(|item| match &item.primary {
                        wire::ObservedPrimary::TargetsComplete { targets, .. } => {
                            targets.iter().all(matches)
                        }
                        wire::ObservedPrimary::Unavailable { .. } => true,
                    })
                }
                wire::ResultFacts::CollectionComplete { items, .. } => {
                    items.iter().all(|item| matches(&item.primary))
                }
                wire::ResultFacts::SearchComplete { .. }
                | wire::ResultFacts::Unavailable { .. } => true,
            })
    }
    pub(crate) fn original_annotation(&self) -> &str {
        &self.original_annotation
    }
    pub(crate) fn policy_snapshot(&self) -> &EffectiveInteractionPolicy {
        &self.request.policy
    }
    pub(crate) fn association_documents_json(&self) -> Result<String, AnnotationError> {
        serde_json::to_string(&self.wire.documents).map_err(|_| AnnotationError::InvalidSchema)
    }
    pub(crate) fn association_focused_documents_json(&self) -> Result<String, AnnotationError> {
        serde_json::to_string(&self.association_focused_documents)
            .map_err(|_| AnnotationError::InvalidSchema)
    }
    /// Small native projection input from the retained ordinary inventory.
    /// Never re-encode names/hrefs or the large ordinary candidate payload.
    pub(crate) fn ordinary_geometry_json(&self) -> Result<String, AnnotationError> {
        #[derive(Serialize)]
        struct Geometry {
            x: f64,
            y: f64,
            width: f64,
            height: f64,
        }
        #[derive(Serialize)]
        struct Entry {
            index: usize,
            kind: &'static str,
            geometry: Geometry,
        }
        let entries: Vec<_> = self
            .candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| Entry {
                index,
                kind: crate::hint_payload::hint_kind_name(candidate.kind),
                geometry: Geometry {
                    x: candidate.geometry.x,
                    y: candidate.geometry.y,
                    width: candidate.geometry.width,
                    height: candidate.geometry.height,
                },
            })
            .collect();
        serde_json::to_string(&entries).map_err(|_| AnnotationError::InvalidSchema)
    }
    pub(crate) fn normalize(self) -> Result<CheckedAnnotations, AnnotationError> {
        finish_annotations(
            self.wire,
            &self.request,
            &self.candidates,
            self.work,
            self.association_focused_documents,
            self.original_annotation,
        )
    }
}

/// Typed, fully checked annotation structure. Current session/DOM authority
/// must still be verified before any special binding or action can be granted.
#[derive(Clone, Serialize)]
#[serde(transparent)]
pub struct CheckedAnnotations {
    pub(crate) wire: wire::Envelope,
    #[serde(skip)]
    policy: EffectiveInteractionPolicy,
    #[serde(skip)]
    candidates: BTreeMap<u16, HintCandidate>,
    #[serde(skip)]
    association_focused_documents: BTreeSet<wire::Id>,
    // Preserve the complete original graph for native resealing, including
    // documents and references excluded from the normalized public graph.
    #[serde(skip)]
    original_annotation: String,
}

impl fmt::Debug for CheckedAnnotations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedAnnotations")
            .field("version", &self.wire.version)
            .field("rules", &self.rule_count())
            .field("documents", &self.document_count())
            .field("roots", &self.root_count())
            .field("candidate_snapshots", &self.candidates.len())
            .finish_non_exhaustive()
    }
}

impl CheckedAnnotations {
    pub(crate) fn family(&self) -> InteractionFamily {
        self.wire.stamp.family
    }

    fn anchor_witness(
        &self,
        id: &str,
    ) -> Option<(&wire::Root, wire::Node, wire::RootKind, &wire::Witness)> {
        let outcome =
            self.wire.outcomes.iter().find(|outcome| {
                outcome.rule.kind == wire::RuleKind::Anchor && outcome.rule.id == id
            })?;
        let wire::ResultFacts::AnchorComplete {
            root,
            scope,
            scope_kind,
            matches,
        } = &outcome.result
        else {
            return None;
        };
        if matches.len() != 1 {
            return None;
        }
        Some((
            self.wire.roots.iter().find(|entry| entry.id == *root)?,
            *scope,
            *scope_kind,
            &matches[0],
        ))
    }

    /// Layout may change without changing a binding. Root objects, relevant
    /// predicate facts and physical target identity may not be substituted.
    pub(crate) fn same_anchor_binding(&self, current: &Self, id: &str) -> bool {
        let (
            Some((old_root, old_scope, old_kind, old)),
            Some((new_root, new_scope, new_kind, new)),
        ) = (self.anchor_witness(id), current.anchor_witness(id))
        else {
            return false;
        };
        old_kind == new_kind
            && self.same_node(current, old_scope, new_scope)
            && self.same_root(current, old_root, new_root)
            && self.same_witness(current, old, new)
    }

    fn same_node(&self, current: &Self, old: wire::Node, new: wire::Node) -> bool {
        let old_document = self
            .wire
            .documents
            .iter()
            .find(|doc| doc.id == old.document);
        let new_document = current
            .wire
            .documents
            .iter()
            .find(|doc| doc.id == new.document);
        old.node == new.node
            && old_document.zip(new_document).is_some_and(|(old, new)| {
                old.epoch == new.epoch
                    && old.origin == new.origin
                    && old.frame_owner == new.frame_owner
                    && old.frame_path == new.frame_path
            })
    }

    fn same_root(&self, current: &Self, old: &wire::Root, new: &wire::Root) -> bool {
        old.start_kind == new.start_kind
            && old.resolved_kind == new.resolved_kind
            && self.same_node(current, old.start, new.start)
            && self.same_node(current, old.resolved, new.resolved)
            && old.hops.len() == new.hops.len()
            && old.hops.iter().zip(new.hops.iter()).all(|(old, new)| {
                old.kind() == new.kind()
                    && self.same_node(current, old.host(), new.host())
                    && self.same_node(current, old.result().0, new.result().0)
            })
    }

    fn same_witness(&self, current: &Self, old: &wire::Witness, new: &wire::Witness) -> bool {
        old.candidate.frame_owner == new.candidate.frame_owner
            && old.candidate.frame_path == new.candidate.frame_path
            && old.candidate.element_id == new.candidate.element_id
            && old.candidate.related_element_id == new.candidate.related_element_id
            && old.name == new.name
            && old.href == new.href
            && old.role == new.role
            && old.attributes == new.attributes
            && self
                .candidate_snapshot(old.candidate.index)
                .zip(current.candidate_snapshot(new.candidate.index))
                .is_some_and(|(old, new)| old.kind == new.kind)
    }

    /// Only normalized, native-sealed evidence can supply initial bindings.
    /// Candidate filtering happens first, so policy cannot restore an excluded row.
    pub(crate) fn special_assignments(
        &self,
        candidates: &[HintCandidate],
        declarations: &[ferric_browser_core::HintCodeDeclaration],
        live_slots: Option<&BTreeSet<usize>>,
    ) -> Vec<ferric_browser_core::HintSpecialAssignment> {
        use ferric_browser_core::{
            HintReservationCategory, HintReservationReason as Reason, HintSpecialAssignment,
            HintSpecialBinding as Binding,
        };
        let mut assignments: Vec<_> = declarations
            .iter()
            .enumerate()
            .map(|(slot, declaration)| {
                let binding = if declaration.category == HintReservationCategory::Anchor
                    && live_slots.is_none_or(|slots| slots.contains(&slot))
                {
                    match self
                        .wire
                        .outcomes
                        .iter()
                        .find(|outcome| {
                            outcome.rule.kind == wire::RuleKind::Anchor
                                && outcome.rule.id == declaration.rule_id
                        })
                        .map(|outcome| &outcome.result)
                    {
                        Some(wire::ResultFacts::AnchorComplete { matches, .. })
                            if matches.len() == 1 =>
                        {
                            self.candidate_snapshot(matches[0].candidate.index)
                                .and_then(|candidate| {
                                    candidates.iter().find(|current| *current == candidate)
                                })
                                .map_or(Binding::Unavailable(Reason::Incomplete), |candidate| {
                                    Binding::Live(candidate.clone())
                                })
                        }
                        Some(wire::ResultFacts::AnchorComplete { matches, .. })
                            if matches.is_empty() =>
                        {
                            Binding::Unavailable(Reason::Missing)
                        }
                        Some(wire::ResultFacts::AnchorComplete { .. }) => {
                            Binding::Unavailable(Reason::Ambiguous)
                        }
                        Some(wire::ResultFacts::SearchComplete {
                            cardinality: wire::SearchCardinality::Absent,
                            ..
                        }) => Binding::Unavailable(Reason::Missing),
                        Some(wire::ResultFacts::SearchComplete {
                            cardinality: wire::SearchCardinality::Ambiguous,
                            ..
                        }) => Binding::Unavailable(Reason::Ambiguous),
                        _ => Binding::Unavailable(Reason::Incomplete),
                    }
                } else {
                    Binding::Unavailable(Reason::Incomplete)
                };
                HintSpecialAssignment { slot, binding }
            })
            .collect();
        // Neither independently unique anchor wins a shared physical target.
        // Leave that row eligible for ordinary fallback.
        let mut claims = BTreeMap::<_, Vec<usize>>::new();
        for assignment in &assignments {
            if let Binding::Live(candidate) = &assignment.binding {
                claims
                    .entry((
                        candidate.frame_owner.clone(),
                        candidate.frame_path.clone(),
                        candidate.element_id,
                        candidate.related_element_id,
                    ))
                    .or_default()
                    .push(assignment.slot);
            }
        }
        let anchor_targets = claims.keys().cloned().collect();
        for slots in claims.values().filter(|slots| slots.len() > 1) {
            for &slot in slots {
                assignments[slot].binding = Binding::Unavailable(Reason::Ambiguous);
            }
        }
        self.assign_collection_slots(
            candidates,
            declarations,
            live_slots,
            &anchor_targets,
            &mut assignments,
        );
        assignments
    }

    pub(crate) fn surviving_special_slots(
        &self,
        current: &Self,
        reservations: &[ferric_browser_core::HintReservation],
    ) -> BTreeSet<usize> {
        let collections: BTreeMap<_, _> = self
            .policy
            .collections()
            .iter()
            .map(|collection| {
                (
                    collection.id.as_str(),
                    self.same_collection_binding(current, &collection.id),
                )
            })
            .collect();
        reservations
            .iter()
            .enumerate()
            .filter(|(_, reservation)| {
                reservation.state() == ferric_browser_core::HintReservationState::Live
                    && match reservation.declaration().category {
                        ferric_browser_core::HintReservationCategory::Anchor => {
                            self.same_anchor_binding(current, &reservation.declaration().rule_id)
                        }
                        ferric_browser_core::HintReservationCategory::Collection => {
                            collections.get(reservation.declaration().rule_id.as_str())
                                == Some(&true)
                        }
                    }
            })
            .map(|(slot, _)| slot)
            .collect()
    }

    pub(crate) fn original_annotation(&self) -> &str {
        &self.original_annotation
    }
    pub fn rule_count(&self) -> usize {
        self.wire.outcomes.len()
    }
    pub fn document_count(&self) -> usize {
        self.wire.documents.len()
    }
    pub fn root_count(&self) -> usize {
        self.wire.roots.len()
    }
    /// Bounded local preview details, excluding DOM nodes and witness facts.
    pub(crate) fn preview_diagnostics(&self) -> serde_json::Value {
        let unavailable_rules = self
            .wire
            .outcomes
            .iter()
            .filter_map(|outcome| match &outcome.result {
                wire::ResultFacts::Unavailable { reason } => {
                    Some(serde_json::json!({"rule":outcome.rule,"reason":reason}))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "unavailable_rules":unavailable_rules,
            "discovery_work":self.wire.discovery_work
        })
    }
    /// Immutable browser meaning associated with this exact checked request.
    /// Raw policy names may differ from legacy fallback/display labels.
    pub fn candidate_snapshot(&self, index: u16) -> Option<&HintCandidate> {
        self.candidates.get(&index)
    }
    pub fn policy_snapshot(&self) -> &EffectiveInteractionPolicy {
        &self.policy
    }
    pub(crate) fn association_focused_documents_json(&self) -> Result<String, AnnotationError> {
        serde_json::to_string(&self.association_focused_documents)
            .map_err(|_| AnnotationError::InvalidSchema)
    }
    #[cfg(any(test, feature = "fuzzing"))]
    pub(crate) fn association_documents_json(&self) -> Result<String, AnnotationError> {
        let original: wire::Envelope = serde_json::from_str(&self.original_annotation)
            .map_err(|_| AnnotationError::InvalidSchema)?;
        serde_json::to_string(&original.documents).map_err(|_| AnnotationError::InvalidSchema)
    }
}

/// Atomic result: no partially decoded annotation escapes on any failure.
/// `candidates` must come from the existing ordinary/family decoder. They are
/// borrowed, never truncated/mutated or charged to the annotation byte budget.
/// Raw complete searches are classified with the captured browser policy:
/// deterministic nonmatches are removed and unprovable rules become unavailable.
/// Existing strict Complete claims must still satisfy every required predicate.
///
/// # Errors
/// Rejects oversized or malformed annotations, unsupported versions, request
/// mismatches, invalid rule inventories/graphs/associations, unsafe geometry and
/// exhausted structural work. Malformed references reject even excluded items;
/// missing facts or predicate exhaustion cannot make a raw positive match.
pub fn decode_annotations(
    raw: &str,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
) -> Result<CheckedAnnotations, AnnotationError> {
    let (wire, work, focused) = decode_structure(
        raw,
        request,
        candidates,
        ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS,
    )?;
    finish_annotations(wire, request, candidates, work, focused, raw.to_owned())
}

/// Validate the complete original graph without evaluating authored predicates.
/// Moves the separately decoded ordinary inventory into the native continuation.
#[cfg(any(test, feature = "fuzzing"))]
pub(crate) fn decode_annotation_facts(
    raw: &str,
    request: &AnnotationRequest,
    candidates: impl Into<Arc<[HintCandidate]>>,
) -> Result<CheckedAnnotationFacts, AnnotationError> {
    decode_annotation_facts_with_budget(
        raw,
        request,
        candidates,
        ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS,
    )
}

/// Limit structural work to the original request's remaining verification
/// allowance before decoding. This grants no additional semantic predicate work.
pub(crate) fn decode_annotation_facts_with_budget(
    raw: &str,
    request: &AnnotationRequest,
    candidates: impl Into<Arc<[HintCandidate]>>,
    verification_limit: usize,
) -> Result<CheckedAnnotationFacts, AnnotationError> {
    if verification_limit == 0 {
        return Err(AnnotationError::WorkLimit);
    }
    let candidates = candidates.into();
    let (wire, work, focused) = decode_structure(raw, request, &candidates, verification_limit)?;
    Ok(CheckedAnnotationFacts {
        wire,
        request: request.clone(),
        candidates,
        work,
        association_focused_documents: focused,
        original_annotation: raw.to_owned(),
    })
}

fn decode_structure(
    raw: &str,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
    verification_limit: usize,
) -> Result<
    (
        wire::Envelope,
        validation::NormalizationWork,
        BTreeSet<wire::Id>,
    ),
    AnnotationError,
> {
    if raw.len() > MAX_INTERACTION_ANNOTATION_BYTES {
        return Err(AnnotationError::TooLarge);
    }
    let wire: wire::Envelope =
        serde_json::from_str(raw).map_err(|_| AnnotationError::InvalidSchema)?;
    if wire.version != INTERACTION_ANNOTATION_VERSION {
        return Err(AnnotationError::WrongVersion);
    }
    if wire.stamp != request.stamp {
        return Err(AnnotationError::StaleRequest);
    }
    let (work, focused) =
        validation::validate_structure(&wire, request, candidates, verification_limit)?;
    Ok((wire, work, focused))
}

fn finish_annotations(
    mut wire: wire::Envelope,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
    work: validation::NormalizationWork,
    association_focused_documents: BTreeSet<wire::Id>,
    original_annotation: String,
) -> Result<CheckedAnnotations, AnnotationError> {
    let outcomes = validation::normalize(&wire, request, candidates, work)?;
    wire.outcomes = bounded::List::from_vec(outcomes).ok_or(AnnotationError::WorkLimit)?;
    validation::retain_normalized_graph(&mut wire)?;
    let mut snapshots = BTreeMap::new();
    for outcome in wire.outcomes.iter() {
        match &outcome.result {
            wire::ResultFacts::AnchorComplete { matches, .. } => {
                for witness in matches.iter() {
                    snapshots.entry(witness.candidate.index).or_insert_with(|| {
                        candidates[usize::from(witness.candidate.index)].clone()
                    });
                }
            }
            wire::ResultFacts::CollectionComplete { items, .. } => {
                for item in items.iter() {
                    snapshots
                        .entry(item.primary.candidate.index)
                        .or_insert_with(|| {
                            candidates[usize::from(item.primary.candidate.index)].clone()
                        });
                }
            }
            wire::ResultFacts::Unavailable { .. } | wire::ResultFacts::SearchComplete { .. } => {}
            wire::ResultFacts::AnchorObserved { .. }
            | wire::ResultFacts::CollectionObserved { .. } => {
                return Err(AnnotationError::InvalidSchema);
            }
        }
    }
    Ok(CheckedAnnotations {
        wire,
        policy: request.policy.clone(),
        candidates: snapshots,
        association_focused_documents,
        original_annotation,
    })
}

#[cfg(test)]
mod tests;
