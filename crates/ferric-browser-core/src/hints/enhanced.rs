//! Enhanced state uses the existing Hint input vocabulary. Names are checked
//! before discovery; initial target bindings remain browser-owned facts rather
//! than DOM proof or activation authority.
use super::{
    HintAutoFollow, HintBindingState, HintCandidate, HintError, HintIdentity, HintInputMode,
    HintInteraction, HintInteractionOutcome, HintKind, HintLabelBinding, HintMeaning, LabeledHint,
    MAX_HINT_CANDIDATES, compare_hint_candidates, hint_identity,
};
use crate::{
    CheckedHintCodebook, HintCodeDeclaration, HintCodeExclusion, HintCodebookError,
    HintLabelAllocator, MAX_HINT_CODE_BYTES,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintInteractionKind {
    Ordinary,
    Enhanced,
    UnavailableOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintReservationReason {
    Missing,
    Ambiguous,
    Incomplete,
    AnchorShadowed,
    Stale,
    MeaningChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintReservationState {
    PendingInitial,
    Live,
    Unavailable(HintReservationReason),
    Retired(HintReservationReason),
    Consumed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintRefreshIssue {
    NamespaceExhausted,
    RetainedStateExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintSelectionUnavailable {
    NoSelection,
    Unassigned,
    MissingTarget,
    InvalidCharacter(char),
    ExcludedCharacter {
        character: char,
        reason: HintCodeExclusion,
    },
    Reservation {
        slot: usize,
        state: HintReservationState,
    },
    RetiredLabel,
    ConsumedLabel,
    RefreshBlocked,
}

/// A caller-owned cap on retained identity/meaning payload, not a namespace
/// cap or process/allocator/working-set guarantee. No shipping default is
/// selected here. Production must resolve that policy before enabling it.
/// Charging includes each ledger key/binding's inline size and owned string
/// lengths (identity, code, URL and non-link action name), not `BTreeMap` overhead,
/// current candidate rows, the bounded trie or temporary refresh plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintRetentionBudget {
    Unlimited,
    Bytes(usize),
}

impl HintRetentionBudget {
    fn check(self, required: usize) -> Result<(), HintError> {
        if let Self::Bytes(limit) = self
            && required > limit
        {
            return Err(HintError::RetainedStateExhausted { required, limit });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HintSpecialBinding {
    Live(HintCandidate),
    Unavailable(HintReservationReason),
}

#[derive(Clone, Debug, PartialEq)]
pub struct HintSpecialAssignment {
    /// Index into `CheckedHintCodebook::declarations`, not the declaration's
    /// authored collection `slot` ordinal. Anchors also occupy one index.
    pub slot: usize,
    pub binding: HintSpecialBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintReservation {
    declaration: HintCodeDeclaration,
    pub(super) state: HintReservationState,
}

impl HintReservation {
    #[must_use]
    pub const fn declaration(&self) -> &HintCodeDeclaration {
        &self.declaration
    }
    #[must_use]
    pub const fn state(&self) -> HintReservationState {
        self.state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HintEnhancedError {
    Candidate(HintError),
    Codebook(HintCodebookError),
    Validation(super::HintValidationError),
    UnknownSlot(usize),
    DuplicateSlot(usize),
    UnknownCandidate(usize),
    CandidateSnapshotMismatch(usize),
    DuplicateSpecialTarget { first: usize, second: usize },
}

impl std::fmt::Display for HintEnhancedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Candidate(error) => error.fmt(f),
            Self::Codebook(error) => error.fmt(f),
            Self::Validation(error) => error.fmt(f),
            Self::UnknownSlot(slot) => write!(f, "Unknown Hint reservation slot {slot}"),
            Self::DuplicateSlot(slot) => write!(f, "Repeated initial Hint reservation slot {slot}"),
            Self::UnknownCandidate(slot) => write!(
                f,
                "Hint reservation {slot} names a candidate outside its initial snapshot"
            ),
            Self::CandidateSnapshotMismatch(slot) => write!(
                f,
                "Hint reservation {slot} does not match its canonical candidate facts"
            ),
            Self::DuplicateSpecialTarget { first, second } => write!(
                f,
                "Hint reservations {first} and {second} claim the same physical candidate; resolve ownership explicitly"
            ),
        }
    }
}
impl std::error::Error for HintEnhancedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Candidate(error) => Some(error),
            Self::Codebook(error) => Some(error),
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EnhancedHintState {
    allocator: HintLabelAllocator,
    pub(super) reservations: Vec<HintReservation>,
    budget: HintRetentionBudget,
    charged_bytes: usize,
    pub(super) unavailable: Option<HintSelectionUnavailable>,
    pub(super) issue: Option<HintRefreshIssue>,
    blocked: bool,
}

fn validate_candidates(candidates: &[HintCandidate]) -> Result<(), HintError> {
    if candidates.len() > MAX_HINT_CANDIDATES {
        return Err(HintError::TooManyCandidates {
            count: candidates.len(),
            maximum: MAX_HINT_CANDIDATES,
        });
    }
    let mut identities = BTreeSet::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if !candidate.geometry.is_valid() {
            return Err(HintError::InvalidGeometry { index });
        }
        if !identities.insert(hint_identity(candidate)) {
            return Err(HintError::DuplicateCandidate);
        }
    }
    Ok(())
}

fn charge(identity: &HintIdentity, binding: &HintLabelBinding) -> usize {
    std::mem::size_of::<(HintIdentity, HintLabelBinding)>()
        .saturating_add(identity.0.len())
        .saturating_add(identity.1.len())
        .saturating_add(binding.label.len())
        .saturating_add(binding.meaning.href.as_ref().map_or(0, String::len))
        .saturating_add(binding.meaning.action_name.as_ref().map_or(0, String::len))
}

pub(super) fn same_meaning(meaning: &HintMeaning, candidate: &HintCandidate) -> bool {
    meaning.kind == candidate.kind
        && meaning.href == candidate.href
        && (candidate.kind == HintKind::Link
            || meaning.action_name.as_deref() == Some(candidate.text.as_str()))
}

fn initial_slots(
    candidates: &[HintCandidate],
    codebook: &CheckedHintCodebook,
    assignments: Vec<HintSpecialAssignment>,
    pending_initial: bool,
) -> Result<(Vec<HintReservation>, BTreeMap<HintIdentity, usize>), HintEnhancedError> {
    let mut reservations: Vec<_> = codebook
        .declarations()
        .iter()
        .cloned()
        .map(|declaration| HintReservation {
            declaration,
            state: if pending_initial {
                HintReservationState::PendingInitial
            } else {
                HintReservationState::Unavailable(HintReservationReason::Missing)
            },
        })
        .collect();
    let known: BTreeMap<_, _> = candidates
        .iter()
        .map(|candidate| (hint_identity(candidate), candidate))
        .collect();
    let mut assigned = BTreeSet::new();
    let mut targets = BTreeMap::new();
    for assignment in assignments {
        let slot = assignment.slot;
        let reservation = reservations
            .get_mut(slot)
            .ok_or(HintEnhancedError::UnknownSlot(slot))?;
        if !assigned.insert(slot) {
            return Err(HintEnhancedError::DuplicateSlot(slot));
        }
        match assignment.binding {
            HintSpecialBinding::Unavailable(reason) => {
                reservation.state = HintReservationState::Unavailable(reason);
            }
            HintSpecialBinding::Live(candidate) => {
                let identity = hint_identity(&candidate);
                let canonical = known
                    .get(&identity)
                    .ok_or(HintEnhancedError::UnknownCandidate(slot))?;
                if **canonical != candidate {
                    return Err(HintEnhancedError::CandidateSnapshotMismatch(slot));
                }
                if let Some(first) = targets.insert(identity, slot) {
                    return Err(HintEnhancedError::DuplicateSpecialTarget {
                        first,
                        second: slot,
                    });
                }
                reservation.state = HintReservationState::Live;
            }
        }
    }
    Ok((reservations, targets))
}

impl HintInteraction {
    /// Creates one completed enhanced snapshot. Omitted slots stay missing;
    /// future refreshes cannot fill them. Initially no row is selected. Explicit
    /// navigation, query selection or label input can establish selection.
    /// # Errors
    /// Rejects invalid candidates, conflicting initial bindings, insufficient
    /// ordinary capacity or the caller's explicit retained-payload budget.
    pub fn new_enhanced(
        candidates: Vec<HintCandidate>,
        codebook: CheckedHintCodebook,
        assignments: Vec<HintSpecialAssignment>,
        budget: HintRetentionBudget,
    ) -> Result<Self, HintEnhancedError> {
        Self::enhanced_snapshot(candidates, codebook, assignments, budget, None)
    }

    /// Allocates a presentation-only initial snapshot. Omitted special slots
    /// remain pending, and the caller supplies complete ordinary capacity.
    /// # Errors
    /// Rejects invalid snapshots, insufficient capacity or retained-state budget.
    pub fn new_provisional_enhanced(
        candidates: Vec<HintCandidate>,
        codebook: CheckedHintCodebook,
        assignments: Vec<HintSpecialAssignment>,
        ordinary_capacity: usize,
        budget: HintRetentionBudget,
    ) -> Result<Self, HintEnhancedError> {
        let mut model = Self::enhanced_snapshot(
            candidates,
            codebook,
            assignments,
            budget,
            Some(ordinary_capacity),
        )?;
        model
            .begin_hint_validation()
            .map_err(HintEnhancedError::Validation)?;
        Ok(model)
    }

    fn enhanced_snapshot(
        mut candidates: Vec<HintCandidate>,
        codebook: CheckedHintCodebook,
        assignments: Vec<HintSpecialAssignment>,
        budget: HintRetentionBudget,
        ordinary_capacity: Option<usize>,
    ) -> Result<Self, HintEnhancedError> {
        validate_candidates(&candidates).map_err(HintEnhancedError::Candidate)?;
        candidates.sort_by(compare_hint_candidates);
        let (reservations, targets) = initial_slots(
            &candidates,
            &codebook,
            assignments,
            ordinary_capacity.is_some(),
        )?;
        let ordinary_count = candidates.len() - targets.len();
        let capacity = ordinary_capacity.unwrap_or(ordinary_count);
        if capacity < ordinary_count {
            return Err(HintEnhancedError::Codebook(
                HintCodebookError::InsufficientCapacity {
                    requested: ordinary_count,
                    available: capacity as u64,
                },
            ));
        }
        let mut allocator = codebook
            .allocator(capacity)
            .map_err(HintEnhancedError::Codebook)?;
        let mut hints = Vec::with_capacity(candidates.len());
        let mut bindings = BTreeMap::new();
        let mut charged_bytes = 0_usize;
        for candidate in candidates {
            let identity = hint_identity(&candidate);
            let special_slot = targets.get(&identity).copied();
            let label = if let Some(slot) = special_slot {
                reservations[slot].declaration.code.clone()
            } else {
                allocator.next().ok_or(HintEnhancedError::Codebook(
                    HintCodebookError::InsufficientCapacity {
                        requested: 1,
                        available: 0,
                    },
                ))?
            };
            let binding = HintLabelBinding {
                label: label.clone(),
                meaning: HintMeaning::of(&candidate),
                state: HintBindingState::Available,
                special_slot,
                provisional: false,
            };
            charged_bytes = charged_bytes.saturating_add(charge(&identity, &binding));
            budget
                .check(charged_bytes)
                .map_err(HintEnhancedError::Candidate)?;
            bindings.insert(identity, binding);
            hints.push(LabeledHint { label, candidate });
        }
        let mut model = Self::new(
            Vec::new(),
            allocator.codebook().effective_alphabet().to_owned(),
            allocator.codebook().auto_follow(),
        );
        model.matches = (0..hints.len()).collect();
        model.hints = hints;
        model.bindings = bindings;
        model.active = None;
        model.initial_activation_available = false;
        model.enhanced = Some(EnhancedHintState {
            allocator,
            reservations,
            budget,
            charged_bytes,
            unavailable: Some(HintSelectionUnavailable::NoSelection),
            issue: None,
            blocked: false,
        });
        Ok(model)
    }

    #[must_use]
    pub fn interaction_kind(&self) -> HintInteractionKind {
        match &self.enhanced {
            None => HintInteractionKind::Ordinary,
            Some(state) if self.hints.is_empty() && !state.reservations.is_empty() => {
                HintInteractionKind::UnavailableOnly
            }
            Some(_) => HintInteractionKind::Enhanced,
        }
    }
    #[must_use]
    pub fn reservations(&self) -> &[HintReservation] {
        self.enhanced
            .as_ref()
            .map_or(&[], |state| state.reservations.as_slice())
    }
    #[must_use]
    pub fn retained_payload_bytes(&self) -> Option<usize> {
        self.enhanced.as_ref().map(|state| state.charged_bytes)
    }
    #[must_use]
    pub fn retention_budget(&self) -> Option<HintRetentionBudget> {
        self.enhanced.as_ref().map(|state| state.budget)
    }
    #[must_use]
    pub fn ordinary_codes_remaining(&self) -> Option<u64> {
        self.enhanced
            .as_ref()
            .map(|state| state.allocator.remaining())
    }
    #[must_use]
    pub fn ordinary_suffix_width(&self) -> Option<u8> {
        self.enhanced
            .as_ref()
            .map(|state| state.allocator.suffix_width())
    }

    /// Settles omitted special declarations once during initial collection.
    /// Live additions are still provisional and cannot replace an ordinary row.
    /// # Errors
    /// Rejects closed/stale batches, already settled slots, duplicate targets,
    /// invalid geometry, candidate overflow or insufficient retained capacity.
    pub fn settle_initial_hint_reservations(
        &mut self,
        sequence: u64,
        assignments: Vec<HintSpecialAssignment>,
    ) -> Result<(), HintEnhancedError> {
        let progress = self
            .progressive
            .as_ref()
            .filter(|p| !p.closed && !p.complete)
            .ok_or(HintEnhancedError::Validation(
                super::HintValidationError::Closed,
            ))?;
        if sequence != progress.sequence || sequence == u64::MAX {
            return Err(HintEnhancedError::Validation(
                super::HintValidationError::InvalidSequence,
            ));
        }
        let state = self.enhanced.as_ref().ok_or(HintEnhancedError::Validation(
            super::HintValidationError::InvalidInitialState,
        ))?;
        if assignments.is_empty() || assignments.len() > state.reservations.len() {
            return Err(HintEnhancedError::Validation(
                super::HintValidationError::InvalidUpdate,
            ));
        }
        let mut slots = BTreeSet::new();
        let mut targets = BTreeSet::new();
        let mut additions = Vec::new();
        let mut charged = state.charged_bytes;
        for assignment in &assignments {
            let reservation = state
                .reservations
                .get(assignment.slot)
                .ok_or(HintEnhancedError::UnknownSlot(assignment.slot))?;
            if !slots.insert(assignment.slot) {
                return Err(HintEnhancedError::DuplicateSlot(assignment.slot));
            }
            if reservation.state != HintReservationState::PendingInitial {
                return Err(HintEnhancedError::Validation(
                    super::HintValidationError::InvalidUpdate,
                ));
            }
            if let HintSpecialBinding::Live(candidate) = &assignment.binding {
                let identity = hint_identity(candidate);
                if self.bindings.contains_key(&identity) || !targets.insert(identity.clone()) {
                    return Err(HintEnhancedError::Candidate(HintError::DuplicateCandidate));
                }
                if !candidate.geometry.is_valid() {
                    return Err(HintEnhancedError::Candidate(HintError::InvalidGeometry {
                        index: additions.len(),
                    }));
                }
                let binding = HintLabelBinding {
                    label: reservation.declaration.code.clone(),
                    meaning: HintMeaning::of(candidate),
                    state: HintBindingState::Available,
                    special_slot: Some(assignment.slot),
                    provisional: true,
                };
                charged = charged.saturating_add(charge(&identity, &binding));
                state
                    .budget
                    .check(charged)
                    .map_err(HintEnhancedError::Candidate)?;
                additions.push((identity, binding, candidate.clone()));
            }
        }
        if self.bindings.len().saturating_add(additions.len()) > MAX_HINT_CANDIDATES {
            return Err(HintEnhancedError::Candidate(HintError::TooManyCandidates {
                count: self.bindings.len().saturating_add(additions.len()),
                maximum: MAX_HINT_CANDIDATES,
            }));
        }
        self.commit_initial_hint_reservations(assignments, additions, charged);
        Ok(())
    }

    fn commit_initial_hint_reservations(
        &mut self,
        assignments: Vec<HintSpecialAssignment>,
        additions: Vec<(HintIdentity, HintLabelBinding, HintCandidate)>,
        charged: usize,
    ) {
        let selected = self.active_label().map(str::to_owned);
        let state = self.enhanced.as_mut().unwrap();
        for assignment in assignments {
            state.reservations[assignment.slot].state = match assignment.binding {
                HintSpecialBinding::Live(_) => HintReservationState::Live,
                HintSpecialBinding::Unavailable(reason) => {
                    HintReservationState::Unavailable(reason)
                }
            };
        }
        state.charged_bytes = charged;
        for (identity, binding, candidate) in additions {
            self.hints.push(LabeledHint {
                label: binding.label.clone(),
                candidate,
            });
            self.bindings.insert(identity, binding);
        }
        self.hints
            .sort_by(|a, b| compare_hint_candidates(&a.candidate, &b.candidate));
        self.rebuild_enhanced_matches(selected.as_deref());
        self.progressive.as_mut().unwrap().sequence += 1;
    }

    /// Revokes only a live initial slot. This never installs a new binding or
    /// turns an initial unavailable slot into a live one.
    pub fn retire_reservation(&mut self, slot: usize, reason: HintReservationReason) -> bool {
        let selected = self.active_label().map(str::to_owned);
        let Some(state) = &mut self.enhanced else {
            return false;
        };
        let Some(reservation) = state.reservations.get_mut(slot) else {
            return false;
        };
        if reservation.state != HintReservationState::Live {
            return false;
        }
        reservation.state = HintReservationState::Retired(reason);
        let code = reservation.declaration.code.clone();
        for binding in self
            .bindings
            .values_mut()
            .filter(|binding| binding.special_slot == Some(slot))
        {
            binding.state = HintBindingState::Retired;
        }
        self.hints.retain(|hint| hint.label != code);
        self.rebuild_enhanced_matches(selected.as_deref());
        if selected.as_deref() == Some(code.as_str()) {
            self.clear_enhanced_selection(HintSelectionUnavailable::Reservation {
                slot,
                state: HintReservationState::Retired(reason),
            });
        }
        true
    }

    pub(super) fn clear_enhanced_selection(&mut self, reason: HintSelectionUnavailable) {
        if let Some(state) = &mut self.enhanced {
            self.active = None;
            state.unavailable = Some(if state.blocked {
                HintSelectionUnavailable::RefreshBlocked
            } else {
                reason
            });
        }
    }

    pub(super) fn note_enhanced_selection(&mut self) {
        if let Some(state) = &mut self.enhanced {
            if state.blocked {
                self.active = None;
                state.unavailable = Some(HintSelectionUnavailable::RefreshBlocked);
            } else {
                state.unavailable = self
                    .active
                    .is_none()
                    .then_some(HintSelectionUnavailable::NoSelection);
            }
        }
    }

    pub(super) fn enhanced_label_input(&mut self, character: char) -> HintInteractionOutcome {
        if self.prefix.len().saturating_add(character.len_utf8()) > MAX_HINT_CODE_BYTES {
            self.clear_enhanced_selection(HintSelectionUnavailable::InvalidCharacter(character));
            return HintInteractionOutcome::NoMatches;
        }
        self.prefix.push(character);
        self.filter_label_matches();
        let exclusion = self
            .enhanced
            .as_ref()
            .and_then(|state| state.allocator.codebook().exclusion(character));
        if let Some(reason) = exclusion {
            self.clear_enhanced_selection(HintSelectionUnavailable::ExcludedCharacter {
                character,
                reason,
            });
            return HintInteractionOutcome::InvalidCharacter(character);
        }
        if self.matches.is_empty() {
            let reason = self.label_unavailable(&self.prefix);
            self.clear_enhanced_selection(reason);
            return HintInteractionOutcome::NoMatches;
        }
        self.note_enhanced_selection();
        if self.auto_follow == HintAutoFollow::FullMatch
            && let Some(label) = self.active_label()
            && label == self.prefix
        {
            return HintInteractionOutcome::Activate(label.to_owned());
        }
        HintInteractionOutcome::Updated
    }

    pub(super) fn label_unavailable(&self, label: &str) -> HintSelectionUnavailable {
        if let Some(state) = &self.enhanced
            && let Some((slot, reservation)) = state
                .reservations
                .iter()
                .enumerate()
                .find(|(_, r)| r.declaration.code == label)
        {
            return HintSelectionUnavailable::Reservation {
                slot,
                state: reservation.state,
            };
        }
        match self
            .bindings
            .values()
            .find(|binding| binding.label == label)
            .map(|binding| binding.state)
        {
            Some(HintBindingState::Available) => HintSelectionUnavailable::MissingTarget,
            Some(HintBindingState::Consumed) => HintSelectionUnavailable::ConsumedLabel,
            Some(HintBindingState::Retired) => HintSelectionUnavailable::RetiredLabel,
            _ if self
                .enhanced
                .as_ref()
                .is_some_and(|state| state.allocator.has_allocated(label)) =>
            {
                HintSelectionUnavailable::RetiredLabel
            }
            _ => HintSelectionUnavailable::Unassigned,
        }
    }

    fn rebuild_enhanced_matches(&mut self, selected: Option<&str>) {
        if self.mode == HintInputMode::Text {
            self.rank_text_matches();
        } else {
            self.filter_label_matches();
        }
        self.active = selected.and_then(|label| {
            self.matches
                .iter()
                .position(|index| self.hints[*index].label == label)
        });
        self.note_enhanced_selection();
        if self.active.is_none() {
            if let Some(label) = selected {
                let reason = self.label_unavailable(label);
                self.clear_enhanced_selection(reason);
            } else if !self.prefix.is_empty() && self.matches.is_empty() {
                let reason = self.label_unavailable(&self.prefix);
                self.clear_enhanced_selection(reason);
            }
        }
    }
}

struct RefreshPlan {
    hints: Vec<LabeledHint>,
    updates: Vec<(HintIdentity, HintLabelBinding)>,
    allocated: u64,
    charged: usize,
    exhausted: bool,
}

impl HintInteraction {
    fn observed_retirements(
        &self,
        candidates: &[HintCandidate],
        state: &mut EnhancedHintState,
    ) -> BTreeSet<HintIdentity> {
        let mut retired = BTreeSet::new();
        let incoming: BTreeMap<_, _> = candidates
            .iter()
            .map(|candidate| (hint_identity(candidate), candidate))
            .collect();
        for (identity, candidate) in &incoming {
            if let Some(binding) = self.bindings.get(identity)
                && binding.state != HintBindingState::Consumed
                && !same_meaning(&binding.meaning, candidate)
            {
                retired.insert(identity.clone());
                if let Some(slot) = binding.special_slot
                    && state.reservations[slot].state == HintReservationState::Live
                {
                    state.reservations[slot].state =
                        HintReservationState::Retired(HintReservationReason::MeaningChanged);
                }
            }
        }
        for hint in &self.hints {
            let identity = hint_identity(&hint.candidate);
            if !incoming.contains_key(&identity)
                && let Some(binding) = self.bindings.get(&identity)
                && let Some(slot) = binding.special_slot
                && state.reservations[slot].state == HintReservationState::Live
            {
                state.reservations[slot].state =
                    HintReservationState::Retired(HintReservationReason::Missing);
                retired.insert(identity);
            }
        }
        retired
    }

    fn plan_enhanced_refresh(
        &self,
        candidates: Vec<HintCandidate>,
        state: &EnhancedHintState,
        retired: &BTreeSet<HintIdentity>,
    ) -> Result<RefreshPlan, HintError> {
        let mut plan = RefreshPlan {
            hints: Vec::with_capacity(candidates.len()),
            updates: Vec::new(),
            allocated: 0,
            charged: state.charged_bytes,
            exhausted: false,
        };
        for candidate in candidates {
            let identity = hint_identity(&candidate);
            let previous = self.bindings.get(&identity);
            if previous.is_some_and(|binding| binding.state == HintBindingState::Consumed) {
                continue;
            }
            if let Some(binding) = previous.filter(|binding| {
                binding.state == HintBindingState::Available
                    && !retired.contains(&identity)
                    && binding.special_slot.is_none_or(|slot| {
                        state.reservations[slot].state == HintReservationState::Live
                    })
                    && same_meaning(&binding.meaning, &candidate)
            }) {
                plan.hints.push(LabeledHint {
                    label: binding.label.clone(),
                    candidate,
                });
                continue;
            }
            let Some(label) = state.allocator.pending_label(plan.allocated) else {
                plan.exhausted = true;
                continue;
            };
            let binding = HintLabelBinding {
                label: label.clone(),
                meaning: HintMeaning::of(&candidate),
                state: HintBindingState::Available,
                special_slot: None,
                provisional: false,
            };
            plan.charged = plan
                .charged
                .saturating_sub(previous.map_or(0, |old| charge(&identity, old)))
                .saturating_add(charge(&identity, &binding));
            state.budget.check(plan.charged)?;
            plan.updates.push((identity, binding));
            plan.allocated += 1;
            plan.hints.push(LabeledHint { label, candidate });
        }
        Ok(plan)
    }

    fn apply_observed_retirements(
        &mut self,
        retired: &BTreeSet<HintIdentity>,
        state: &EnhancedHintState,
    ) {
        for identity in retired {
            if let Some(binding) = self.bindings.get_mut(identity) {
                binding.state = HintBindingState::Retired;
            }
        }
        if let Some(current) = &mut self.enhanced {
            current.reservations.clone_from(&state.reservations);
        }
    }

    pub(super) fn refresh_enhanced(
        &mut self,
        mut candidates: Vec<HintCandidate>,
    ) -> Result<(), HintError> {
        validate_candidates(&candidates)?;
        candidates.sort_by(compare_hint_candidates);
        let Some(mut state) = self.enhanced.clone() else {
            return Ok(());
        };
        let selected = self.active_label().map(str::to_owned);
        let retired = self.observed_retirements(&candidates, &mut state);
        let plan = match self.plan_enhanced_refresh(candidates, &state, &retired) {
            Ok(plan) => plan,
            Err(error) => {
                // Failed positive admission cannot erase an observed revocation.
                // Preserve all old identity/meaning facts and the allocation
                // cursor, retain negative facts, and block stale confirmation.
                self.apply_observed_retirements(&retired, &state);
                self.hints
                    .retain(|hint| !retired.contains(&hint_identity(&hint.candidate)));
                self.rebuild_enhanced_matches(None);
                self.refresh_required = true;
                self.active = None;
                if let Some(current) = &mut self.enhanced {
                    current.blocked = true;
                    current.issue = Some(HintRefreshIssue::RetainedStateExhausted);
                    current.unavailable = Some(HintSelectionUnavailable::RefreshBlocked);
                }
                return Err(error);
            }
        };
        self.apply_observed_retirements(&retired, &state);
        for (identity, binding) in plan.updates {
            self.bindings.insert(identity, binding);
        }
        state.allocator.commit_labels(plan.allocated);
        state.charged_bytes = plan.charged;
        state.blocked = false;
        state.issue = plan
            .exhausted
            .then_some(HintRefreshIssue::NamespaceExhausted);
        self.refresh_required = plan.exhausted;
        self.initial_activation_available = false;
        self.hints = plan.hints;
        self.enhanced = Some(state);
        self.rebuild_enhanced_matches(selected.as_deref());
        Ok(())
    }
}
