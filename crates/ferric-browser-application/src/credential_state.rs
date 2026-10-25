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
    Busy,
    TerminalOutboxFull,
    InvalidDeadline,
    TokenExhausted,
    CandidateNotPending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialTerminalReason {
    Aborted,
    DeadlineExpired,
    DocumentInvalidated,
    HelperCompleted,
    HelperFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialTerminalEffect {
    owner: crate::CredentialRequestOwner,
    reason: CredentialTerminalReason,
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelperCompletion {
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionOutcome {
    TerminalEmitted,
    StaleOrAlreadyTerminal,
    DeadlineNotReached,
}

struct ActiveCeremony {
    token: CeremonyToken,
    owner: crate::CredentialRequestOwner,
    deadline_ms: u64,
}

/// Single-active-flow state with a bounded, lossless terminal-effect outbox.
///
/// `now_ms` is a caller-supplied monotonic timestamp so production can use its
/// monotonic source and tests can advance time deterministically.
pub struct CredentialCeremonyState {
    active: Option<ActiveCeremony>,
    terminal_outbox: VecDeque<CredentialTerminalEffect>,
    terminal_capacity: usize,
    next_token: u64,
}

impl CredentialCeremonyState {
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CREDENTIAL_TERMINAL_CAPACITY)
    }

    #[cfg(test)]
    pub(crate) fn synthetic_with_capacity(terminal_capacity: usize) -> Self {
        Self::with_capacity(terminal_capacity)
    }

    fn with_capacity(terminal_capacity: usize) -> Self {
        Self {
            active: None,
            terminal_outbox: VecDeque::with_capacity(terminal_capacity),
            terminal_capacity,
            next_token: 1,
        }
    }

    /// Captures ownership only after confirming both single-flow and outbox
    /// capacity. A successful return reserves one terminal outbox slot.
    ///
    /// # Errors
    ///
    /// Returns an error when another flow is active, the terminal outbox has
    /// no free reservation, the deadline is invalid, the token space is
    /// exhausted, or the RP candidate is not pending.
    pub fn admit(
        &mut self,
        candidate: OriginBoundRpCandidate,
        now_ms: u64,
        timeout_ms: u64,
    ) -> Result<CeremonyToken, CeremonyAdmissionError> {
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
        let owner = candidate.into_owner();
        self.active = Some(ActiveCeremony {
            token,
            owner,
            deadline_ms,
        });
        Ok(token)
    }

    pub fn abort(&mut self, token: CeremonyToken, now_ms: u64) -> TransitionOutcome {
        self.terminate_for_event(token, now_ms, CredentialTerminalReason::Aborted)
    }

    /// Invalidate only the captured document; an unrelated or stale navigation
    /// cannot retarget or cancel a different ceremony.
    pub fn invalidate_document(
        &mut self,
        token: CeremonyToken,
        owner: &crate::CredentialRequestOwner,
        now_ms: u64,
    ) -> TransitionOutcome {
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.token != token || active.owner != *owner)
        {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        self.terminate_for_event(token, now_ms, CredentialTerminalReason::DocumentInvalidated)
    }

    pub fn helper_result(
        &mut self,
        token: CeremonyToken,
        result: HelperCompletion,
        now_ms: u64,
    ) -> TransitionOutcome {
        let reason = match result {
            HelperCompletion::Success => CredentialTerminalReason::HelperCompleted,
            HelperCompletion::Failure => CredentialTerminalReason::HelperFailed,
        };
        self.terminate_for_event(token, now_ms, reason)
    }

    pub fn expire(&mut self, token: CeremonyToken, now_ms: u64) -> TransitionOutcome {
        let Some(active) = self.active.as_ref() else {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        };
        if active.token != token {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        if now_ms < active.deadline_ms {
            return TransitionOutcome::DeadlineNotReached;
        }
        self.emit_terminal(CredentialTerminalReason::DeadlineExpired);
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

    fn terminate_for_event(
        &mut self,
        token: CeremonyToken,
        now_ms: u64,
        requested_reason: CredentialTerminalReason,
    ) -> TransitionOutcome {
        let Some(active) = self.active.as_ref() else {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        };
        if active.token != token {
            return TransitionOutcome::StaleOrAlreadyTerminal;
        }
        let reason = if now_ms >= active.deadline_ms {
            CredentialTerminalReason::DeadlineExpired
        } else {
            requested_reason
        };
        self.emit_terminal(reason);
        TransitionOutcome::TerminalEmitted
    }

    fn emit_terminal(&mut self, reason: CredentialTerminalReason) {
        let active = self.active.take().expect("checked active ceremony");
        debug_assert!(self.terminal_outbox.len() < self.terminal_capacity);
        self.terminal_outbox.push_back(CredentialTerminalEffect {
            owner: active.owner,
            reason,
        });
    }
}

impl Default for CredentialCeremonyState {
    fn default() -> Self {
        Self::new()
    }
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
