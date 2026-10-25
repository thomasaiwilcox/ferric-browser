//! Bounded process-wide policy for one speculative blank page.
//!
//! This module owns no Qt objects. Callers ask for a preparation permit after
//! resolving a complete native environment key, then report readiness and use
//! `claim` on the ordinary open path. A pending preparation never delays open.

use std::sync::Mutex;
use std::time::Duration;

#[cfg(test)]
#[path = "tests/hot_page_policy.rs"]
mod tests;

pub(super) const PREPARATION_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const MAX_PREPARATION_RETRIES: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PresentationKind {
    Internal,
    Window,
}

/// Opaque native identities must be supplied by the Qt/profile layer. Names
/// alone are deliberately insufficient to prove that a spare is compatible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EnvironmentKey {
    pub(super) profile_identity: u64,
    pub(super) profile_generation: u64,
    pub(super) partition_identity: u64,
    pub(super) context_isolation: u64,
    pub(super) settings_generation: u64,
    pub(super) presentation: PresentationKind,
    /// Required for internal presentation; absent for window presentation.
    pub(super) host_window: Option<u64>,
}

impl EnvironmentKey {
    pub(super) fn is_well_formed(&self) -> bool {
        match self.presentation {
            PresentationKind::Internal => self.host_window.is_some(),
            PresentationKind::Window => self.host_window.is_none(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EvictionReason {
    Timeout,
    IncompatibleEnvironment,
    ProfileClosed,
    HostClosed,
    ConfigurationChanged,
    PrivacyChanged,
    PartitionChanged,
    RendererFailure,
    GraphicsFailure,
    MemoryPressure,
    Shutdown,
    Contaminated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PrepareDecision {
    Start { token: u64 },
    AlreadyPreparing,
    AlreadyReady,
    Ineligible,
    Suppressed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClaimResult {
    ReadyHit { token: u64 },
    ColdMiss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Disabled,
    Preparing { token: u64, started_ms: u64 },
    Ready { token: u64 },
    Claimed { token: u64 },
    Evicted { reason: EvictionReason },
}

struct Inner {
    state: State,
    key: Option<EnvironmentKey>,
    next_token: u64,
    retries: u8,
    requests_pending: bool,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            state: State::Disabled,
            key: None,
            next_token: 1,
            retries: 0,
            requests_pending: false,
        }
    }
}

/// Share one instance across all windows in an engine process.
pub(super) struct HotPageCoordinator(Mutex<Inner>);

impl Default for HotPageCoordinator {
    fn default() -> Self {
        Self(Mutex::new(Inner::default()))
    }
}

impl HotPageCoordinator {
    /// Coalesces requests and grants at most one speculative allocation.
    /// Private and ephemeral environments are always ineligible.
    pub(super) fn request_prepare(
        &self,
        key: EnvironmentKey,
        eligible: bool,
        now_ms: u64,
    ) -> PrepareDecision {
        let mut inner = self.0.lock().expect("hot-page policy mutex poisoned");
        Self::expire_if_needed(&mut inner, now_ms);
        if !eligible || !key.is_well_formed() {
            inner.requests_pending = false;
            if !matches!(inner.state, State::Disabled | State::Claimed { .. }) {
                inner.state = State::Evicted {
                    reason: EvictionReason::PrivacyChanged,
                };
            }
            return PrepareDecision::Ineligible;
        }
        match inner.state {
            State::Preparing { .. } => {
                if inner.key.as_ref() == Some(&key) {
                    inner.requests_pending = true;
                    return PrepareDecision::AlreadyPreparing;
                }
            }
            State::Ready { .. } => {
                if inner.key.as_ref() == Some(&key) {
                    inner.requests_pending = true;
                    return PrepareDecision::AlreadyReady;
                }
            }
            State::Claimed { .. } => return PrepareDecision::Suppressed,
            // `retries` counts failures after the initial attempt, so two
            // additional starts are allowed before the third failure stops it.
            State::Evicted { .. }
                if inner.key.as_ref() == Some(&key) && inner.retries > MAX_PREPARATION_RETRIES =>
            {
                return PrepareDecision::Suppressed;
            }
            _ => {}
        }

        if inner.key.as_ref() != Some(&key) {
            inner.key = Some(key);
            inner.retries = 0;
        }
        if matches!(inner.state, State::Preparing { .. } | State::Ready { .. }) {
            inner.state = State::Evicted {
                reason: EvictionReason::IncompatibleEnvironment,
            };
        }
        let token = inner.next_token;
        inner.next_token = inner.next_token.wrapping_add(1).max(1);
        inner.state = State::Preparing {
            token,
            started_ms: now_ms,
        };
        inner.requests_pending = false;
        PrepareDecision::Start { token }
    }

    pub(super) fn mark_ready(&self, token: u64, now_ms: u64) -> bool {
        let mut inner = self.0.lock().expect("hot-page policy mutex poisoned");
        if !matches!(inner.state, State::Preparing { token: current, .. } if current == token) {
            return false;
        }
        // Completion itself enforces the preparation deadline. A late
        // callback is a failed attempt even when no requester observed expiry.
        Self::expire_if_needed(&mut inner, now_ms);
        if matches!(inner.state, State::Preparing { token: current, .. } if current == token) {
            inner.state = State::Ready { token };
            true
        } else {
            false
        }
    }

    /// Returns immediately: only a ready matching spare is claimed; every
    /// other open follows its independent cold allocation path.
    pub(super) fn claim(&self, key: &EnvironmentKey, eligible: bool) -> ClaimResult {
        let mut inner = self.0.lock().expect("hot-page policy mutex poisoned");
        if eligible && inner.key.as_ref() == Some(key) {
            if let State::Ready { token } = inner.state {
                inner.state = State::Claimed { token };
                return ClaimResult::ReadyHit { token };
            }
        }
        ClaimResult::ColdMiss
    }

    /// Releases the process-wide permit after the claimed spare becomes a live
    /// page, allowing one later interaction-settled refill.
    pub(super) fn finish_claim(&self, token: u64) -> bool {
        let mut inner = self.0.lock().expect("hot-page policy mutex poisoned");
        if matches!(inner.state, State::Claimed { token: current } if current == token) {
            inner.state = State::Disabled;
            true
        } else {
            false
        }
    }

    /// Completes a failed preparation only for its current incarnation.
    /// Retryable failures count toward an initial attempt plus two retries.
    pub(super) fn fail_preparation(&self, token: u64, reason: EvictionReason) -> bool {
        if !matches!(
            reason,
            EvictionReason::Timeout
                | EvictionReason::RendererFailure
                | EvictionReason::GraphicsFailure
                | EvictionReason::Contaminated
        ) {
            return false;
        }
        let mut inner = self.0.lock().expect("hot-page policy mutex poisoned");
        Self::record_failure(&mut inner, token, reason)
    }

    pub(super) fn evict(&self, reason: EvictionReason) {
        let mut inner = self.0.lock().expect("hot-page policy mutex poisoned");
        inner.state = State::Evicted { reason };
        inner.requests_pending = false;
    }

    pub(super) fn take_replenish_request(&self) -> bool {
        let mut inner = self.0.lock().expect("hot-page policy mutex poisoned");
        std::mem::take(&mut inner.requests_pending)
    }

    fn expire_if_needed(inner: &mut Inner, now_ms: u64) {
        match inner.state {
            State::Preparing { token, started_ms }
                if now_ms.saturating_sub(started_ms) >= PREPARATION_TIMEOUT.as_millis() as u64 =>
            {
                Self::record_failure(inner, token, EvictionReason::Timeout);
            }
            _ => {}
        }
    }

    fn record_failure(inner: &mut Inner, token: u64, reason: EvictionReason) -> bool {
        if matches!(
            inner.state,
            State::Preparing { token: current, .. } | State::Ready { token: current }
                if current == token
        ) {
            inner.state = State::Evicted { reason };
            inner.retries = inner.retries.saturating_add(1);
            inner.requests_pending = false;
            true
        } else {
            false
        }
    }
}
