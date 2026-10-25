//! Exactly-once terminal transitions for one browser-managed credential flow.
//!
//! Admission reserves a terminal outbox slot before capturing an owner. Every
//! accepted flow therefore has space for its terminal effect even if consumers
//! stop draining the outbox. RP validation and provider work remain elsewhere.

use std::collections::VecDeque;

use crate::{OriginBoundRpCandidate, RpValidationStatus};

/// Default bound for retained terminal effects while the browser is busy.
pub const DEFAULT_CREDENTIAL_TERMINAL_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CeremonyToken(u64);

impl CeremonyToken {
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CeremonyAdmissionError {
    Disabled,
    Unqualified,
    Busy,
    TerminalOutboxFull,
    InvalidDeadline,
    TokenExhausted,
    CandidateNotPending,
}

/// Pure actor phase. Phone-side phases remain unreachable in production until
/// the independent engine and helper authorities are connected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialCeremonyPhase {
    Received,
    Validating,
    AwaitingProvider,
    Starting,
    AwaitingPhone,
    Completing,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CredentialAvailability {
    Disabled,
    Unqualified,
    #[cfg(test)]
    SyntheticAvailable,
}

/// Typed source classification supplied by a future browser-owned mediator.
/// It is not deserializable and must never be copied from page data.
#[allow(dead_code)] // No production caller exists until P00 supplies that mediator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CredentialRequestIngress {
    PhoneShim,
    OutsidePhoneShim,
}

#[allow(dead_code)] // The policy result is exercised here before production wiring.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CredentialIngressDecision {
    ContinuePhoneAdmission,
    LeaveNative,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialTerminalReason {
    Aborted,
    DeadlineExpired,
    DocumentInvalidated,
    RendererLost,
    ProfileClosed,
    WindowClosed,
    Shutdown,
    HelperCompleted,
    HelperFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialTeardownReason {
    DocumentReplaced,
    RendererLost,
    ProfileClosed,
    WindowClosed,
    Shutdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialTerminalEffect {
    owner: crate::CredentialRequestOwner,
    reason: CredentialTerminalReason,
    phase: CredentialCeremonyPhase,
}

impl CredentialTerminalEffect {
    #[must_use]
    pub const fn owner(&self) -> &crate::CredentialRequestOwner {
        &self.owner
    }

    #[must_use]
    pub const fn reason(&self) -> CredentialTerminalReason {
        self.reason
    }

    #[must_use]
    pub const fn phase(&self) -> CredentialCeremonyPhase {
        self.phase
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelperCompletion {
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionOutcome {
    PhaseAdvanced,
    Unchanged,
    InvalidPhase,
    TerminalEmitted,
    StaleOrAlreadyTerminal,
    DeadlineNotReached,
}

struct ActiveCeremony {
    token: CeremonyToken,
    candidate: OriginBoundRpCandidate,
    deadline_ms: u64,
    phase: CredentialCeremonyPhase,
    completion: Option<HelperCompletion>,
}

/// Single-active-flow state with a bounded, lossless terminal-effect outbox.
///
/// `now_ms` is a caller-supplied monotonic timestamp so production can use its
/// monotonic source and tests can advance time deterministically.
pub struct CredentialCeremonyState {
    availability: CredentialAvailability,
    active: Option<ActiveCeremony>,
    terminal_outbox: VecDeque<CredentialTerminalEffect>,
    terminal_capacity: usize,
    next_token: u64,
}

impl CredentialCeremonyState {
    #[must_use]
    pub fn new() -> Self {
        Self::with_availability(
            CredentialAvailability::Unqualified,
            DEFAULT_CREDENTIAL_TERMINAL_CAPACITY,
        )
    }

    /// Constructs a phone-disabled state. No state constructor can establish
    /// production qualification for the phone boundary.
    #[must_use]
    pub fn disabled() -> Self {
        Self::with_availability(
            CredentialAvailability::Disabled,
            DEFAULT_CREDENTIAL_TERMINAL_CAPACITY,
        )
    }

    #[cfg(test)]
    pub(crate) fn synthetic_with_capacity(terminal_capacity: usize) -> Self {
        Self::with_availability(
            CredentialAvailability::SyntheticAvailable,
            terminal_capacity,
        )
    }

    #[cfg(test)]
    pub(crate) fn synthetic_disable(&mut self) {
        self.availability = CredentialAvailability::Disabled;
    }

    #[cfg(test)]
    pub(crate) fn synthetic_unqualify(&mut self) {
        self.availability = CredentialAvailability::Unqualified;
    }

    fn with_availability(availability: CredentialAvailability, terminal_capacity: usize) -> Self {
        Self {
            availability,
            active: None,
            terminal_outbox: VecDeque::with_capacity(terminal_capacity),
            terminal_capacity,
            next_token: 1,
        }
    }

    /// Pure routing decision only. `OutsidePhoneShim` is left to Qt's existing
    /// native handler; this method does not cancel or synthesize native work.
    #[allow(dead_code)] // Intentionally awaits the separately qualified caller.
    pub(crate) const fn classify_ingress(
        ingress: CredentialRequestIngress,
    ) -> CredentialIngressDecision {
        match ingress {
            CredentialRequestIngress::PhoneShim => {
                CredentialIngressDecision::ContinuePhoneAdmission
            }
            CredentialRequestIngress::OutsidePhoneShim => CredentialIngressDecision::LeaveNative,
        }
    }

    /// Captures ownership only after confirming both single-flow and outbox
    /// capacity. A successful return reserves one terminal outbox slot.
    ///
    /// # Errors
    ///
    /// Returns an error when phone use is disabled or unqualified, another
    /// flow is active, the terminal outbox is full, the deadline is invalid,
    /// the token space is exhausted, or the RP candidate is not pending.
    pub fn admit(
        &mut self,
        candidate: OriginBoundRpCandidate,
        now_ms: u64,
        timeout_ms: u64,
    ) -> Result<CeremonyToken, CeremonyAdmissionError> {
        let availability_result = match self.availability {
            CredentialAvailability::Disabled => Err(CeremonyAdmissionError::Disabled),
            CredentialAvailability::Unqualified => Err(CeremonyAdmissionError::Unqualified),
            #[cfg(test)]
            CredentialAvailability::SyntheticAvailable => Ok(()),
        };
        availability_result?;
        if self.active.is_some() {
            return Err(CeremonyAdmissionError::Busy);
        }
        if self.terminal_outbox.len() >= self.terminal_capacity {
            return Err(CeremonyAdmissionError::TerminalOutboxFull);
        }
        if candidate.status() != RpValidationStatus::Pending {
            return Err(CeremonyAdmissionError::CandidateNotPending);
        }
        let deadline_ms = now_ms
            .checked_add(timeout_ms)
            .filter(|deadline| *deadline > now_ms)
            .ok_or(CeremonyAdmissionError::InvalidDeadline)?;
        let token = CeremonyToken(self.next_token);
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or(CeremonyAdmissionError::TokenExhausted)?;

        // INVARIANT: this active flow owns one free outbox position until its
        // terminal effect is queued; no later admission can consume it.
        self.active = Some(ActiveCeremony {
            token,
            candidate,
            deadline_ms,
            phase: CredentialCeremonyPhase::Received,
            completion: None,
        });
        Ok(token)
    }

    /// Advance the accepted owner from request receipt into validation.
    pub fn begin_validation(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        now_ms: u64,
    ) -> TransitionOutcome {
        self.advance_phase(
            token,
            owner,
            CredentialCeremonyPhase::Received,
            CredentialCeremonyPhase::Validating,
            now_ms,
        )
    }

    /// Synthetic phase transitions exercise C08 ordering without supplying
    /// production RP/helper authority or enabling an actual phone operation.
    #[cfg(test)]
    pub(crate) fn synthetic_advance_phase(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        expected: CredentialCeremonyPhase,
        next: CredentialCeremonyPhase,
        now_ms: u64,
    ) -> TransitionOutcome {
        self.advance_phase(token, owner, expected, next, now_ms)
    }

    /// Focus changes never replace the captured host or end the phone actor.
    pub fn focus_changed(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        _is_foreground: bool,
        now_ms: u64,
    ) -> TransitionOutcome {
        if !self.active_matches(token, owner) {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        if self.expire_if_due(now_ms) {
            return TransitionOutcome::TerminalEmitted;
        }
        TransitionOutcome::Unchanged
    }

    pub fn abort(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        now_ms: u64,
    ) -> TransitionOutcome {
        self.terminate_for_event(token, owner, now_ms, CredentialTerminalReason::Aborted)
    }

    /// Invalidate only the captured document; an unrelated or stale navigation
    /// cannot retarget or cancel a different ceremony.
    pub fn invalidate_document(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        now_ms: u64,
    ) -> TransitionOutcome {
        if !self.active_matches(token, owner) {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        self.terminate_for_event(
            token,
            owner,
            now_ms,
            CredentialTerminalReason::DocumentInvalidated,
        )
    }

    /// Route browser-owned document, renderer, profile, window or shutdown
    /// teardown through the same exactly-once terminal projection.
    pub fn teardown(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        reason: CredentialTeardownReason,
        now_ms: u64,
    ) -> TransitionOutcome {
        let terminal_reason = match reason {
            CredentialTeardownReason::DocumentReplaced => {
                CredentialTerminalReason::DocumentInvalidated
            }
            CredentialTeardownReason::RendererLost => CredentialTerminalReason::RendererLost,
            CredentialTeardownReason::ProfileClosed => CredentialTerminalReason::ProfileClosed,
            CredentialTeardownReason::WindowClosed => CredentialTerminalReason::WindowClosed,
            CredentialTeardownReason::Shutdown => CredentialTerminalReason::Shutdown,
        };
        self.terminate_for_event(token, owner, now_ms, terminal_reason)
    }

    pub fn helper_result(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        result: HelperCompletion,
        now_ms: u64,
    ) -> TransitionOutcome {
        if !self.active_matches(token, owner) {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        if self.expire_if_due(now_ms) {
            return TransitionOutcome::TerminalEmitted;
        }
        let Some(active) = self.active.as_mut() else {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        };
        if active.phase != CredentialCeremonyPhase::AwaitingPhone {
            return TransitionOutcome::InvalidPhase;
        }
        active.phase = CredentialCeremonyPhase::Completing;
        active.completion = Some(result);
        TransitionOutcome::PhaseAdvanced
    }

    /// Finish an already-completing actor and project its one terminal effect.
    pub fn finish_completion(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        now_ms: u64,
    ) -> TransitionOutcome {
        if !self.active_matches(token, owner) {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        if self.expire_if_due(now_ms) {
            return TransitionOutcome::TerminalEmitted;
        }
        let Some(active) = self.active.as_ref() else {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        };
        if active.phase != CredentialCeremonyPhase::Completing {
            return TransitionOutcome::InvalidPhase;
        }
        let reason = match active.completion {
            Some(HelperCompletion::Success) => CredentialTerminalReason::HelperCompleted,
            Some(HelperCompletion::Failure) => CredentialTerminalReason::HelperFailed,
            None => return TransitionOutcome::InvalidPhase,
        };
        self.emit_terminal(reason);
        TransitionOutcome::TerminalEmitted
    }

    pub fn expire(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        now_ms: u64,
    ) -> TransitionOutcome {
        if !self.active_matches(token, owner) {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        if !self.expire_if_due(now_ms) {
            return TransitionOutcome::DeadlineNotReached;
        }
        TransitionOutcome::TerminalEmitted
    }

    /// Draining releases outbox capacity for future admissions.
    pub fn drain_one(&mut self) -> Option<CredentialTerminalEffect> {
        self.terminal_outbox.pop_front()
    }

    #[must_use]
    pub fn queued_terminal_count(&self) -> usize {
        self.terminal_outbox.len()
    }

    #[must_use]
    pub const fn has_active_ceremony(&self) -> bool {
        self.active.is_some()
    }

    #[must_use]
    pub const fn phase(&self) -> Option<CredentialCeremonyPhase> {
        match &self.active {
            Some(active) => Some(active.phase),
            None => None,
        }
    }

    #[must_use]
    pub fn active_owner(&self) -> Option<&crate::CredentialRequestOwner> {
        self.active.as_ref().map(|active| active.candidate.owner())
    }

    /// Exposes only the pending bounded page candidate for a later qualified
    /// helper; reading it does not authorize or prepare the RP.
    #[must_use]
    pub fn active_candidate(&self) -> Option<&OriginBoundRpCandidate> {
        self.active.as_ref().map(|active| &active.candidate)
    }

    fn terminate_for_event(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        now_ms: u64,
        requested_reason: CredentialTerminalReason,
    ) -> TransitionOutcome {
        if !self.active_matches(token, owner) {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        let expired = self.expire_if_due(now_ms);
        if expired {
            return TransitionOutcome::TerminalEmitted;
        }
        let reason = requested_reason;
        self.emit_terminal(reason);
        TransitionOutcome::TerminalEmitted
    }

    fn active_matches(&self, token: CeremonyToken, owner: &crate::CredentialRequestOwner) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.token == token && active.candidate.owner() == owner)
    }

    /// Shared production-compiled guard and mutation for all actor phase edges.
    /// Synthetic tests call this same implementation after supplying their
    /// explicitly test-only phase evidence.
    fn advance_phase(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        expected: CredentialCeremonyPhase,
        next: CredentialCeremonyPhase,
        now_ms: u64,
    ) -> TransitionOutcome {
        if !self.active_matches(token, owner) {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        if self.expire_if_due(now_ms) {
            return TransitionOutcome::TerminalEmitted;
        }
        let Some(active) = self.active.as_mut() else {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        };
        if active.phase != expected || !valid_phase_successor(expected, next) {
            return TransitionOutcome::InvalidPhase;
        }
        active.phase = next;
        TransitionOutcome::PhaseAdvanced
    }

    fn expire_if_due(&mut self, now_ms: u64) -> bool {
        if self
            .active
            .as_ref()
            .is_some_and(|active| now_ms >= active.deadline_ms)
        {
            self.emit_terminal(CredentialTerminalReason::DeadlineExpired);
            true
        } else {
            false
        }
    }

    fn emit_terminal(&mut self, reason: CredentialTerminalReason) {
        let active = self.active.take().expect("checked active ceremony");
        debug_assert!(self.terminal_outbox.len() < self.terminal_capacity);
        self.terminal_outbox.push_back(CredentialTerminalEffect {
            owner: active.candidate.into_owner(),
            reason,
            phase: CredentialCeremonyPhase::Terminal,
        });
    }
}

impl Default for CredentialCeremonyState {
    fn default() -> Self {
        Self::new()
    }
}

fn valid_phase_successor(current: CredentialCeremonyPhase, next: CredentialCeremonyPhase) -> bool {
    matches!(
        (current, next),
        (
            CredentialCeremonyPhase::Received,
            CredentialCeremonyPhase::Validating
        ) | (
            CredentialCeremonyPhase::Validating,
            CredentialCeremonyPhase::AwaitingProvider
        ) | (
            CredentialCeremonyPhase::AwaitingProvider,
            CredentialCeremonyPhase::Starting
        ) | (
            CredentialCeremonyPhase::Starting,
            CredentialCeremonyPhase::AwaitingPhone
        )
    )
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct SyntheticCredentialClock {
    now_ms: u64,
}

#[cfg(test)]
impl SyntheticCredentialClock {
    pub(crate) const fn now_ms(&self) -> u64 {
        self.now_ms
    }

    pub(crate) fn set(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }

    pub(crate) fn advance(&mut self, elapsed_ms: u64) {
        self.now_ms = self.now_ms.saturating_add(elapsed_ms);
    }
}
