//! Presentation readiness for the existing interaction. This state is never a
//! DOM proof or action permit; adapters still own request freshness and expiry.
use super::{
    HintAutoFollow, HintBindingState, HintCandidate, HintInteraction, HintInteractionInput,
    HintInteractionOutcome, HintReservationReason, MAX_HINT_CANDIDATES, hint_identity,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintValidationStatus {
    Provisional,
    Validated,
    Withdrawn,
    Consumed,
}

/// A native-validated result for an already allocated presentation binding.
/// `None` withdraws it; `Some` must retain its identity and action meaning.
#[derive(Clone, Debug, PartialEq)]
pub struct HintValidationUpdate {
    pub label: String,
    pub candidate: Option<HintCandidate>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintValidationError {
    InvalidInitialState,
    InvalidSequence,
    InvalidUpdate,
    Incomplete,
    Closed,
}

impl std::fmt::Display for HintValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInitialState => "invalid provisional Hint snapshot",
            Self::InvalidSequence => "unexpected Hint validation sequence",
            Self::InvalidUpdate => "Hint validation changed or repeated a binding",
            Self::Incomplete => "Hint validation has unsettled bindings",
            Self::Closed => "Hint validation is closed",
        })
    }
}
impl std::error::Error for HintValidationError {}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProgressiveValidation {
    pub(super) sequence: u64,
    pub(super) complete: bool,
    pub(super) closed: bool,
    pending: Option<String>,
}

impl HintInteraction {
    /// Marks an allocated snapshot presentation-only. Ownership, width and
    /// codebook capacity must already have been resolved by the caller.
    /// # Errors
    /// Rejects repeated initialization, loading state or malformed bindings.
    pub fn begin_hint_validation(&mut self) -> Result<(), HintValidationError> {
        if self.progressive.is_some()
            || self.loading
            || self.hints.len() > MAX_HINT_CANDIDATES
            || !self.prefix.is_empty()
            || !self.query.is_empty()
            || self
                .bindings
                .values()
                .any(|b| b.state != HintBindingState::Available)
        {
            return Err(HintValidationError::InvalidInitialState);
        }
        let mut labels = BTreeSet::new();
        let mut identities = BTreeSet::new();
        for hint in &self.hints {
            if hint.label.is_empty()
                || hint.label.len() > 8
                || !hint.label.bytes().all(|c| c.is_ascii_graphic())
                || !hint.candidate.geometry.is_valid()
                || !labels.insert(hint.label.as_str())
                || !identities.insert(hint_identity(&hint.candidate))
            {
                return Err(HintValidationError::InvalidInitialState);
            }
        }
        let mut previous: Option<&str> = None;
        for label in labels {
            if previous.is_some_and(|prefix| label.starts_with(prefix)) {
                return Err(HintValidationError::InvalidInitialState);
            }
            previous = Some(label);
        }
        for binding in self.bindings.values_mut() {
            binding.provisional = true;
        }
        self.initial_activation_available = false;
        self.progressive = Some(ProgressiveValidation {
            sequence: 1,
            complete: false,
            closed: false,
            pending: None,
        });
        Ok(())
    }

    #[must_use]
    pub fn validation_in_progress(&self) -> bool {
        self.progressive.as_ref().is_some_and(|p| !p.complete)
    }

    /// Readiness barrier only; a true result is not an activation permit.
    #[must_use]
    pub fn hint_selection_is_validated(&self, label: &str) -> bool {
        self.progressive.as_ref().is_none_or(|p| {
            !p.closed && self.hint_validation_status(label) == Some(HintValidationStatus::Validated)
        })
    }

    #[must_use]
    pub fn hint_validation_status(&self, label: &str) -> Option<HintValidationStatus> {
        let binding = self.bindings.values().find(|b| b.label == label)?;
        Some(match binding.state {
            HintBindingState::Consumed => HintValidationStatus::Consumed,
            HintBindingState::Retired => HintValidationStatus::Withdrawn,
            HintBindingState::Available if binding.provisional => HintValidationStatus::Provisional,
            HintBindingState::Available => HintValidationStatus::Validated,
        })
    }

    #[must_use]
    pub fn pending_hint_selection(&self) -> Option<&str> {
        self.progressive.as_ref()?.pending.as_deref()
    }

    pub fn cancel_pending_hint_selection(&mut self) {
        if let Some(progressive) = &mut self.progressive {
            progressive.pending = None;
        }
    }

    /// Permanently fences callbacks when the owning request is retired.
    pub fn close_hint_validation(&mut self) {
        if let Some(progressive) = &mut self.progressive {
            progressive.closed = true;
            progressive.pending = None;
        }
    }

    /// Applies one atomic batch. A returned label is the one previously
    /// requested explicitly; it still needs the adapter's fresh action checks.
    /// # Errors
    /// Rejects stale/repeated batches, changed meanings, unknown rows, malformed
    /// geometry and premature completion without partially promoting any row.
    pub fn apply_hint_validation(
        &mut self,
        sequence: u64,
        updates: &[HintValidationUpdate],
        complete: bool,
    ) -> Result<Option<String>, HintValidationError> {
        let progress = self
            .progressive
            .as_ref()
            .ok_or(HintValidationError::Closed)?;
        if progress.closed || progress.complete {
            return Err(HintValidationError::Closed);
        }
        if sequence != progress.sequence || sequence == u64::MAX {
            return Err(HintValidationError::InvalidSequence);
        }
        if updates.is_empty() && !complete || updates.len() > MAX_HINT_CANDIDATES {
            return Err(HintValidationError::InvalidUpdate);
        }
        let current: BTreeMap<_, _> = self.hints.iter().map(|h| (h.label.as_str(), h)).collect();
        let mut labels = BTreeMap::new();
        for update in updates {
            if labels.insert(update.label.as_str(), update).is_some() {
                return Err(HintValidationError::InvalidUpdate);
            }
            let hint = current
                .get(update.label.as_str())
                .ok_or(HintValidationError::InvalidUpdate)?;
            let binding = &self.bindings[&hint_identity(&hint.candidate)];
            if !binding.provisional || binding.state != HintBindingState::Available {
                return Err(HintValidationError::InvalidUpdate);
            }
            if let Some(candidate) = &update.candidate
                && (hint_identity(candidate) != hint_identity(&hint.candidate)
                    || !super::enhanced::same_meaning(&binding.meaning, candidate)
                    || !candidate.geometry.is_valid())
            {
                return Err(HintValidationError::InvalidUpdate);
            }
        }
        if complete
            && (self
                .reservations()
                .iter()
                .any(|r| r.state() == super::HintReservationState::PendingInitial)
                || self.hints.iter().any(|hint| {
                    self.bindings[&hint_identity(&hint.candidate)].provisional
                        && !labels.contains_key(hint.label.as_str())
                }))
        {
            return Err(HintValidationError::Incomplete);
        }

        Ok(self.commit_hint_validation(&labels, complete))
    }

    // All labels, identities and the progressive owner were checked above.
    // No callback or external mutation occurs between checking and committing.
    fn commit_hint_validation(
        &mut self,
        labels: &BTreeMap<&str, &HintValidationUpdate>,
        complete: bool,
    ) -> Option<String> {
        let selected = self.active_label().map(str::to_owned);
        for hint in &mut self.hints {
            let Some(update) = labels.get(hint.label.as_str()) else {
                continue;
            };
            let identity = hint_identity(&hint.candidate);
            if let Some(candidate) = &update.candidate {
                hint.candidate.clone_from(candidate);
                self.bindings.get_mut(&identity).unwrap().provisional = false;
            } else {
                let slot = self.bindings[&identity].special_slot;
                if let Some(slot) = slot
                    && let Some(enhanced) = &mut self.enhanced
                {
                    enhanced.reservations[slot].state =
                        super::HintReservationState::Retired(HintReservationReason::Stale);
                }
                self.bindings.get_mut(&identity).unwrap().state = HintBindingState::Retired;
            }
        }
        self.hints.retain(|hint| {
            self.bindings[&hint_identity(&hint.candidate)].state == HintBindingState::Available
        });
        if self.mode == super::HintInputMode::Text {
            self.rank_text_matches();
        } else {
            self.filter_label_matches();
        }
        self.active = selected.and_then(|label| {
            self.matches
                .iter()
                .position(|index| self.hints[*index].label == label)
        });
        let pending = self.progressive.as_mut().unwrap().pending.take();
        let ready = pending.filter(|label| match self.hint_validation_status(label) {
            Some(HintValidationStatus::Validated) => true,
            Some(HintValidationStatus::Provisional) => {
                self.progressive.as_mut().unwrap().pending = Some(label.clone());
                false
            }
            _ => false,
        });
        let progress = self.progressive.as_mut().unwrap();
        progress.sequence += 1;
        progress.complete = complete;
        ready
    }

    pub fn handle(&mut self, input: HintInteractionInput) -> HintInteractionOutcome {
        if self.progressive.is_none() {
            return self.handle_ready_input(input);
        }
        if self.progressive.as_ref().is_some_and(|p| p.closed) {
            return if input == HintInteractionInput::Cancel {
                HintInteractionOutcome::Cancel
            } else {
                HintInteractionOutcome::NoMatches
            };
        }
        if input == HintInteractionInput::Activate && self.pending_hint_selection().is_some() {
            return HintInteractionOutcome::Updated;
        }
        self.cancel_pending_hint_selection();
        if input == HintInteractionInput::Cancel {
            self.close_hint_validation();
        }
        let follow = self.auto_follow;
        if self.validation_in_progress()
            && matches!(follow, HintAutoFollow::Always | HintAutoFollow::UniqueMatch)
        {
            self.auto_follow = HintAutoFollow::Never;
        }
        let outcome = self.handle_ready_input(input);
        self.auto_follow = follow;
        if self.progressive.is_some()
            && let HintInteractionOutcome::Activate(label) = &outcome
        {
            match self.hint_validation_status(label) {
                Some(HintValidationStatus::Provisional) => {
                    if let Some(progress) = &mut self.progressive {
                        progress.pending = Some(label.clone());
                    }
                    return HintInteractionOutcome::Updated;
                }
                Some(HintValidationStatus::Validated) => {}
                _ => return HintInteractionOutcome::NoMatches,
            }
        }
        outcome
    }
}
