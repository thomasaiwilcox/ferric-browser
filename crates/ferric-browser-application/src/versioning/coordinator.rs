use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    time::Instant,
};

use ferric_browser_core::{
    CapturedScope, NativeProfileOwner, OperationId, PrivacyKind, ProfileId, ScopeError,
    versioning::limits::{CONFIRMATION_PLAN_TTL_MS, MAX_OPERATION_QUEUE},
};

const MAX_RECENT_TERMINALS: usize = 32;
const MAX_IN_FLIGHT_OPERATIONS: usize = MAX_OPERATION_QUEUE + 1;
const PUBLICATION_CALLBACK_TIMEOUT_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationKind {
    ReadOnly,
    Mutating,
    Sensitive(SensitiveOperation),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SensitiveOperation {
    MergeApply,
    Restore,
    Drop,
    Stash,
    PageCapture,
}

impl OperationKind {
    const fn is_mutating(self) -> bool {
        !matches!(self, Self::ReadOnly)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivacyPolicy {
    NormalDurable,
    MemoryOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationState {
    Queued,
    Requested,
    Validating,
    AwaitingConsent,
    Preparing,
    Executing,
    Verifying,
    Publishing,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
    Partial,
    OutcomeUnknown,
}

impl OperationState {
    #[must_use]
    pub const fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::Partial | Self::OutcomeUnknown
        )
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct OperationEnvelope {
    id: OperationId,
    scope: CapturedScope,
    configuration_revision: u64,
    privacy: PrivacyPolicy,
    expected_logical_revision: u64,
    deadline_ms: u64,
    cancellation_generation: u64,
    kind: OperationKind,
}

impl fmt::Debug for OperationEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationEnvelope")
            .field("id", &"[redacted]")
            .field("scope", &self.scope)
            .field("configuration_revision", &"[redacted]")
            .field("privacy", &self.privacy)
            .field("expected_logical_revision", &"[redacted]")
            .field("deadline_ms", &"[redacted]")
            .field("cancellation_generation", &"[redacted]")
            .field("kind", &self.kind)
            .finish()
    }
}

impl OperationEnvelope {
    #[must_use]
    pub const fn id(&self) -> OperationId {
        self.id
    }
    #[must_use]
    pub const fn scope(&self) -> &CapturedScope {
        &self.scope
    }
    #[must_use]
    pub const fn owner(&self) -> NativeProfileOwner {
        self.scope.native()
    }
    #[must_use]
    pub const fn configuration_revision(&self) -> u64 {
        self.configuration_revision
    }
    #[must_use]
    pub const fn privacy(&self) -> PrivacyPolicy {
        self.privacy
    }
    #[must_use]
    pub const fn expected_logical_revision(&self) -> u64 {
        self.expected_logical_revision
    }
    #[must_use]
    pub const fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }
    #[must_use]
    pub const fn cancellation_generation(&self) -> u64 {
        self.cancellation_generation
    }
    #[must_use]
    pub const fn kind(&self) -> OperationKind {
        self.kind
    }
}

/// A ticket names exactly one staged resource. Fields are private so adapters
/// cannot retarget it to the active window or another operation.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StageTicket {
    operation: OperationId,
    generation: u64,
    stage: u64,
}

impl fmt::Debug for StageTicket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StageTicket([redacted])")
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum OperationEffect {
    Start(OperationId),
    Cleanup(StageTicket),
}

impl fmt::Debug for OperationEffect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(_) => formatter.write_str("OperationEffect::Start([redacted])"),
            Self::Cleanup(_) => formatter.write_str("OperationEffect::Cleanup([redacted])"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageResult {
    Success,
    FailedWithoutMutation,
    Partial,
    OutcomeUnknown,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum CallbackDisposition {
    Accepted,
    CleanupOwnStage(StageTicket),
    Ignored,
}

impl fmt::Debug for CallbackDisposition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accepted => formatter.write_str("CallbackDisposition::Accepted"),
            Self::CleanupOwnStage(_) => {
                formatter.write_str("CallbackDisposition::CleanupOwnStage([redacted])")
            }
            Self::Ignored => formatter.write_str("CallbackDisposition::Ignored"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationError {
    Scope(ScopeError),
    OwnerClosed,
    InvalidDeadline,
    QueueFull,
    DuplicateId,
    UnknownOperation,
    InvalidTransition,
    Expired,
    ConsentExpired,
    StaleCallback,
}

impl From<ScopeError> for OperationError {
    fn from(value: ScopeError) -> Self {
        Self::Scope(value)
    }
}

struct Operation {
    envelope: OperationEnvelope,
    state: OperationState,
    consent_deadline_ms: Option<u64>,
    ticket: Option<StageTicket>,
    callback_seen: bool,
    cleanup_issued: bool,
}

/// Owns admission and callback reconciliation. At most one mutating operation
/// executes per native owner; queued and recent terminal records are bounded.
pub struct VersioningCoordinator {
    clock: Box<dyn Fn() -> u64>,
    next_id: Box<dyn FnMut() -> OperationId>,
    owners: BTreeMap<ProfileId, NativeProfileOwner>,
    operations: BTreeMap<OperationId, Operation>,
    queue: VecDeque<OperationId>,
    terminal_order: VecDeque<OperationId>,
    next_stage: u64,
}

impl Default for VersioningCoordinator {
    fn default() -> Self {
        let start = Instant::now();
        Self::with_sources(
            move || start.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            OperationId::fresh,
        )
    }
}

impl VersioningCoordinator {
    /// Clock values are monotonic milliseconds in one coordinator lifetime.
    /// Callers can inject deterministic clocks and UUID sources for tests.
    #[must_use]
    pub fn with_sources(
        clock: impl Fn() -> u64 + 'static,
        ids: impl FnMut() -> OperationId + 'static,
    ) -> Self {
        Self {
            clock: Box::new(clock),
            next_id: Box::new(ids),
            owners: BTreeMap::new(),
            operations: BTreeMap::new(),
            queue: VecDeque::new(),
            terminal_order: VecDeque::new(),
            next_stage: 1,
        }
    }

    #[must_use]
    pub fn owner(&self, profile: ProfileId) -> Option<NativeProfileOwner> {
        self.owners.get(&profile).copied()
    }

    #[must_use]
    pub fn operation(&self, id: OperationId) -> Option<(&OperationEnvelope, OperationState)> {
        self.operations.get(&id).map(|op| (&op.envelope, op.state))
    }

    /// Reconstructs bounded start wakeups from coordinator state. A dropped
    /// adapter notification never becomes permission to invent an operation.
    #[must_use]
    pub fn requested_operations_for_owner(&self, owner: NativeProfileOwner) -> Vec<OperationId> {
        self.operations
            .iter()
            .filter_map(|(id, operation)| {
                (operation.state == OperationState::Requested
                    && operation.envelope.owner() == owner)
                    .then_some(*id)
            })
            .collect()
    }

    #[must_use]
    pub fn active_count(&self) -> u16 {
        u16::try_from(
            self.operations
                .values()
                .filter(|operation| !operation.state.terminal())
                .count(),
        )
        .unwrap_or(u16::MAX)
    }

    #[must_use]
    pub fn now_millis(&self) -> u64 {
        (self.clock)()
    }

    pub fn bind_owner(&mut self, owner: NativeProfileOwner) -> Vec<OperationEffect> {
        let profile = owner.runtime_profile();
        let effects = if self.owners.get(&profile).is_some_and(|old| *old != owner) {
            self.close_owner(profile)
        } else {
            Vec::new()
        };
        self.owners.insert(profile, owner);
        effects
    }

    pub fn close_owner(&mut self, profile: ProfileId) -> Vec<OperationEffect> {
        self.owners.remove(&profile);
        let ids: Vec<_> = self
            .operations
            .iter()
            .filter_map(|(id, op)| {
                (op.envelope.owner().runtime_profile() == profile && !op.state.terminal())
                    .then_some(*id)
            })
            .collect();
        let mut effects = Vec::new();
        for id in ids {
            effects.extend(self.cancel(id).unwrap_or_default());
        }
        effects
    }

    pub fn close_all(&mut self) -> Vec<OperationEffect> {
        let profiles: Vec<_> = self.owners.keys().copied().collect();
        profiles
            .into_iter()
            .flat_map(|profile| self.close_owner(profile))
            .collect()
    }

    /// Admits one exact captured scope. Private/ephemeral operations are
    /// memory-only; this protocol never grants them durable authority.
    ///
    /// # Errors
    /// Rejects stale bindings, invalid deadlines, duplicate IDs or queue overflow.
    pub fn admit(
        &mut self,
        scope: CapturedScope,
        configuration_revision: u64,
        privacy: PrivacyKind,
        kind: OperationKind,
        timeout_ms: u64,
    ) -> Result<(OperationId, Vec<OperationEffect>), OperationError> {
        if self.owner(scope.native().runtime_profile()) != Some(scope.native()) {
            return Err(OperationError::OwnerClosed);
        }
        if timeout_ms == 0 {
            return Err(OperationError::InvalidDeadline);
        }
        if self
            .operations
            .values()
            .filter(|op| !op.state.terminal())
            .count()
            >= MAX_IN_FLIGHT_OPERATIONS
        {
            return Err(OperationError::QueueFull);
        }
        let now = (self.clock)();
        let deadline_ms = now
            .checked_add(timeout_ms)
            .ok_or(OperationError::InvalidDeadline)?;
        let id = (self.next_id)();
        if self.operations.contains_key(&id) {
            return Err(OperationError::DuplicateId);
        }
        let queued = kind.is_mutating() && self.busy(scope.native());
        if queued && self.queue.len() >= MAX_OPERATION_QUEUE {
            return Err(OperationError::QueueFull);
        }
        let envelope = OperationEnvelope {
            id,
            expected_logical_revision: scope.revision(),
            scope,
            configuration_revision,
            privacy: match privacy {
                PrivacyKind::Normal => PrivacyPolicy::NormalDurable,
                PrivacyKind::Private | PrivacyKind::Ephemeral => PrivacyPolicy::MemoryOnly,
            },
            deadline_ms,
            cancellation_generation: 0,
            kind,
        };
        self.operations.insert(
            id,
            Operation {
                envelope,
                state: if queued {
                    OperationState::Queued
                } else {
                    OperationState::Requested
                },
                consent_deadline_ms: None,
                ticket: None,
                callback_seen: false,
                cleanup_issued: false,
            },
        );
        if queued {
            self.queue.push_back(id);
            Ok((id, Vec::new()))
        } else {
            Ok((id, vec![OperationEffect::Start(id)]))
        }
    }

    fn busy(&self, owner: NativeProfileOwner) -> bool {
        self.operations.values().any(|op| {
            op.envelope.kind.is_mutating()
                && op.envelope.owner() == owner
                && !op.state.terminal()
                && op.state != OperationState::Queued
        })
    }

    /// Advances only along the protocol's forward edges. `AwaitingConsent` is
    /// left through `consent`, not this general transition method.
    ///
    /// # Errors
    /// Rejects skipped stages, stale owners and expired operations.
    pub fn advance(&mut self, id: OperationId, next: OperationState) -> Result<(), OperationError> {
        self.check_live(id)?;
        let op = self
            .operations
            .get_mut(&id)
            .ok_or(OperationError::UnknownOperation)?;
        let valid = matches!(
            (op.state, next),
            (OperationState::Requested, OperationState::Validating)
                | (OperationState::Validating, OperationState::AwaitingConsent)
                | (OperationState::Preparing, OperationState::Executing)
                | (OperationState::Verifying, OperationState::Publishing)
        );
        if !valid {
            return Err(OperationError::InvalidTransition);
        }
        if next == OperationState::AwaitingConsent {
            op.consent_deadline_ms = Some(
                (self.clock)()
                    .saturating_add(CONFIRMATION_PLAN_TTL_MS)
                    .min(op.envelope.deadline_ms),
            );
        }
        if next == OperationState::Publishing {
            // Once durable publication is submitted, retain a bounded
            // callback window of its own. A lost worker callback becomes an
            // unknown outcome, never an unbounded live operation.
            op.envelope.deadline_ms =
                (self.clock)().saturating_add(PUBLICATION_CALLBACK_TIMEOUT_MS);
        }
        op.state = next;
        Ok(())
    }

    /// Consent is bound to the admitted owner, configuration and generation.
    /// The caller must supply the current configuration revision.
    ///
    /// # Errors
    /// Rejects stale or expired consent.
    pub(crate) fn consent(
        &mut self,
        id: OperationId,
        current_configuration_revision: u64,
    ) -> Result<(), OperationError> {
        self.check_live(id)?;
        let op = self
            .operations
            .get_mut(&id)
            .ok_or(OperationError::UnknownOperation)?;
        if op.state != OperationState::AwaitingConsent {
            return Err(OperationError::InvalidTransition);
        }
        if op.envelope.configuration_revision != current_configuration_revision
            || (self.clock)() >= op.consent_deadline_ms.unwrap_or(0)
        {
            self.finish(id, OperationState::Failed)?;
            return Err(OperationError::ConsentExpired);
        }
        op.consent_deadline_ms = None;
        op.state = OperationState::Preparing;
        Ok(())
    }

    /// Issues one callback ticket for this operation's own staged resource.
    ///
    /// # Errors
    /// Rejects duplicate tickets or a non-executing operation.
    pub fn stage_ticket(&mut self, id: OperationId) -> Result<StageTicket, OperationError> {
        self.check_live(id)?;
        let op = self
            .operations
            .get_mut(&id)
            .ok_or(OperationError::UnknownOperation)?;
        if op.state != OperationState::Executing || op.ticket.is_some() {
            return Err(OperationError::InvalidTransition);
        }
        let ticket = StageTicket {
            operation: id,
            generation: op.envelope.cancellation_generation,
            stage: self.next_stage,
        };
        self.next_stage = self
            .next_stage
            .checked_add(1)
            .ok_or(OperationError::InvalidTransition)?;
        op.ticket = Some(ticket);
        Ok(ticket)
    }

    /// Atomically enters execution and issues its one exact native ticket.
    /// A ticket-allocation error cannot leave an unticketed executing job.
    ///
    /// # Errors
    /// Rejects a stale owner, non-preparing state, duplicate ticket or stage overflow.
    pub fn begin_stage(&mut self, id: OperationId) -> Result<StageTicket, OperationError> {
        self.check_live(id)?;
        let op = self
            .operations
            .get_mut(&id)
            .ok_or(OperationError::UnknownOperation)?;
        if op.state != OperationState::Preparing || op.ticket.is_some() {
            return Err(OperationError::InvalidTransition);
        }
        let next_stage = self
            .next_stage
            .checked_add(1)
            .ok_or(OperationError::InvalidTransition)?;
        let ticket = StageTicket {
            operation: id,
            generation: op.envelope.cancellation_generation,
            stage: self.next_stage,
        };
        self.next_stage = next_stage;
        op.ticket = Some(ticket);
        op.state = OperationState::Executing;
        Ok(ticket)
    }

    /// Reconciles exactly one callback; cancelled/expired callbacks can only
    /// request cleanup of the ticket's own staging object.
    pub fn acknowledge(
        &mut self,
        ticket: StageTicket,
        result: StageResult,
    ) -> (CallbackDisposition, Vec<OperationEffect>) {
        let id = ticket.operation;
        let Some(op) = self.operations.get(&id) else {
            return (CallbackDisposition::Ignored, Vec::new());
        };
        if op.ticket != Some(ticket) || op.callback_seen {
            return (CallbackDisposition::Ignored, Vec::new());
        }
        if op.state == OperationState::Cancelled
            || (self.clock)() >= op.envelope.deadline_ms
            || self.owner(op.envelope.owner().runtime_profile()) != Some(op.envelope.owner())
        {
            if let Some(op) = self.operations.get_mut(&id) {
                op.callback_seen = true;
                op.cleanup_issued = true;
            }
            let effect = OperationEffect::Cleanup(ticket);
            return (CallbackDisposition::CleanupOwnStage(ticket), vec![effect]);
        }
        if op.state != OperationState::Executing {
            return (CallbackDisposition::Ignored, Vec::new());
        }
        let Some(op) = self.operations.get_mut(&id) else {
            return (CallbackDisposition::Ignored, Vec::new());
        };
        op.callback_seen = true;
        let mut effects = Vec::new();
        match result {
            StageResult::Success => {
                if let Some(op) = self.operations.get_mut(&id) {
                    op.state = OperationState::Verifying;
                }
            }
            StageResult::FailedWithoutMutation => {
                let _ = self.finish(id, OperationState::Failed);
                effects.push(OperationEffect::Cleanup(ticket));
            }
            StageResult::Partial => {
                let _ = self.finish(id, OperationState::Partial);
                effects.push(OperationEffect::Cleanup(ticket));
            }
            StageResult::OutcomeUnknown => {
                let _ = self.finish(id, OperationState::OutcomeUnknown);
                effects.push(OperationEffect::Cleanup(ticket));
            }
        }
        if !effects.is_empty() {
            if let Some(op) = self.operations.get_mut(&id) {
                op.cleanup_issued = true;
            }
        }
        effects.extend(self.promote());
        (CallbackDisposition::Accepted, effects)
    }

    /// Completes only a verified and published operation.
    ///
    /// # Errors
    /// Rejects double success, skipped verification and stale ownership.
    pub fn complete(&mut self, id: OperationId) -> Result<Vec<OperationEffect>, OperationError> {
        self.check_live(id)?;
        let op = self
            .operations
            .get(&id)
            .ok_or(OperationError::UnknownOperation)?;
        if op.state != OperationState::Publishing || !op.callback_seen {
            return Err(OperationError::InvalidTransition);
        }
        self.finish(id, OperationState::Completed)?;
        Ok(self.promote())
    }

    /// Cancellation increments the generation and invalidates consent and
    /// work before publication. Once a durable publication callback may be in
    /// flight, cancellation cannot retract it; the result is reported as
    /// uncertain instead of claiming that the write was canceled. Cleanup is
    /// idempotent for earlier stages: a late callback may require the same
    /// exact stage cleanup.
    ///
    /// # Errors
    /// Rejects unknown or already terminal operations.
    pub fn cancel(&mut self, id: OperationId) -> Result<Vec<OperationEffect>, OperationError> {
        let op = self
            .operations
            .get_mut(&id)
            .ok_or(OperationError::UnknownOperation)?;
        if op.state.terminal() {
            return Err(OperationError::InvalidTransition);
        }
        if op.state == OperationState::Publishing {
            op.envelope.cancellation_generation =
                op.envelope.cancellation_generation.saturating_add(1);
            self.finish(id, OperationState::OutcomeUnknown)?;
            return Ok(self.promote());
        }
        let cleanup = op
            .ticket
            .filter(|_| !op.cleanup_issued)
            .map(OperationEffect::Cleanup);
        if cleanup.is_some() {
            op.cleanup_issued = true;
        }
        op.state = OperationState::Cancelling;
        op.envelope.cancellation_generation = op.envelope.cancellation_generation.saturating_add(1);
        self.finish(id, OperationState::Cancelled)?;
        let mut effects = cleanup.into_iter().collect::<Vec<_>>();
        effects.extend(self.promote());
        Ok(effects)
    }

    /// Fails an operation only when the caller knows no external mutation was
    /// attempted. Uncertain writes must use `StageResult::Partial` or
    /// `StageResult::OutcomeUnknown` instead.
    ///
    /// # Errors
    /// Rejects a terminal operation or one already executing externally.
    pub fn fail_without_mutation(
        &mut self,
        id: OperationId,
    ) -> Result<Vec<OperationEffect>, OperationError> {
        self.check_live(id)?;
        let state = self
            .operation(id)
            .ok_or(OperationError::UnknownOperation)?
            .1;
        if matches!(
            state,
            OperationState::Executing | OperationState::Verifying | OperationState::Publishing
        ) {
            return Err(OperationError::InvalidTransition);
        }
        self.finish(id, OperationState::Failed)?;
        Ok(self.promote())
    }

    /// Deadline sweep; callers schedule this at the nearest envelope deadline.
    pub fn expire(&mut self) -> Vec<OperationEffect> {
        let now = (self.clock)();
        let mut effects = Vec::new();
        for op in self.operations.values_mut() {
            if op.state != OperationState::Publishing
                && now >= op.envelope.deadline_ms
                && !op.cleanup_issued
                && op.state != OperationState::Completed
            {
                if let Some(ticket) = op.ticket {
                    op.cleanup_issued = true;
                    effects.push(OperationEffect::Cleanup(ticket));
                }
            }
        }
        let ids: Vec<_> = self
            .operations
            .iter()
            .filter_map(|(id, op)| {
                (!op.state.terminal() && now >= op.envelope.deadline_ms).then_some(*id)
            })
            .collect();
        for id in ids {
            let terminal = if self
                .operation(id)
                .is_some_and(|(_, state)| state == OperationState::Publishing)
            {
                OperationState::OutcomeUnknown
            } else {
                OperationState::Failed
            };
            let _ = self.finish(id, terminal);
        }
        effects.extend(self.promote());
        effects
    }

    fn check_live(&self, id: OperationId) -> Result<(), OperationError> {
        let op = self
            .operations
            .get(&id)
            .ok_or(OperationError::UnknownOperation)?;
        if op.state.terminal() {
            return Err(OperationError::InvalidTransition);
        }
        if self.owner(op.envelope.owner().runtime_profile()) != Some(op.envelope.owner()) {
            return Err(OperationError::OwnerClosed);
        }
        if op.state != OperationState::Publishing && (self.clock)() >= op.envelope.deadline_ms {
            return Err(OperationError::Expired);
        }
        Ok(())
    }

    fn finish(&mut self, id: OperationId, state: OperationState) -> Result<(), OperationError> {
        if !state.terminal() {
            return Err(OperationError::InvalidTransition);
        }
        let op = self
            .operations
            .get_mut(&id)
            .ok_or(OperationError::UnknownOperation)?;
        if op.state.terminal() {
            return Err(OperationError::InvalidTransition);
        }
        op.state = state;
        self.queue.retain(|queued| *queued != id);
        self.terminal_order.push_back(id);
        while self.terminal_order.len() > MAX_RECENT_TERMINALS {
            if let Some(old) = self.terminal_order.pop_front() {
                self.operations.remove(&old);
            }
        }
        Ok(())
    }

    fn promote(&mut self) -> Vec<OperationEffect> {
        let mut effects = Vec::new();
        let mut remaining = VecDeque::new();
        while let Some(id) = self.queue.pop_front() {
            let Some(op) = self.operations.get(&id) else {
                continue;
            };
            let owner = op.envelope.owner();
            if op.state != OperationState::Queued {
                continue;
            }
            if self.owner(owner.runtime_profile()) != Some(owner)
                || (self.clock)() >= op.envelope.deadline_ms
            {
                let _ = self.finish(id, OperationState::Failed);
            } else if self.busy(owner) {
                remaining.push_back(id);
            } else {
                self.operations.get_mut(&id).expect("checked above").state =
                    OperationState::Requested;
                effects.push(OperationEffect::Start(id));
            }
        }
        self.queue = remaining;
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_core::{ApplicationOwnerId, ProfileInstanceId, ScopeSelector};
    use ferric_browser_runtime::BrowserRuntime;
    use std::{cell::Cell, rc::Rc};

    fn fixture() -> (VersioningCoordinator, CapturedScope, Rc<Cell<u64>>) {
        let (runtime, window, _tab) =
            BrowserRuntime::bootstrap(PrivacyKind::Normal, "test").unwrap();
        let profile = runtime.view().windows().get(&window).unwrap().profile;
        let native = NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            profile,
            ProfileInstanceId::fresh(),
        );
        let scope = runtime
            .view()
            .resolve_versioning_scope(ScopeSelector::Window { window }, native)
            .unwrap();
        let time = Rc::new(Cell::new(10));
        let clock = Rc::clone(&time);
        let mut coordinator =
            VersioningCoordinator::with_sources(move || clock.get(), OperationId::fresh);
        assert!(coordinator.bind_owner(native).is_empty());
        (coordinator, scope, time)
    }

    fn start(coordinator: &mut VersioningCoordinator, scope: &CapturedScope) -> OperationId {
        let (id, effects) = coordinator
            .admit(
                scope.clone(),
                4,
                PrivacyKind::Normal,
                OperationKind::Mutating,
                120_000,
            )
            .unwrap();
        assert_eq!(effects, vec![OperationEffect::Start(id)]);
        id
    }

    #[test]
    fn requested_owner_scan_recovers_only_live_start_wakeups() {
        let (mut coordinator, scope, _) = fixture();
        let owner = scope.native();
        let first = start(&mut coordinator, &scope);
        assert_eq!(coordinator.requested_operations_for_owner(owner), [first]);
        let (second, effects) = coordinator
            .admit(
                scope,
                4,
                PrivacyKind::Normal,
                OperationKind::Mutating,
                120_000,
            )
            .unwrap();
        assert!(effects.is_empty());
        assert_eq!(coordinator.requested_operations_for_owner(owner), [first]);
        assert_eq!(
            coordinator.cancel(first).unwrap(),
            [OperationEffect::Start(second)]
        );
        assert_eq!(coordinator.requested_operations_for_owner(owner), [second]);
        coordinator
            .advance(second, OperationState::Validating)
            .unwrap();
        assert!(coordinator.requested_operations_for_owner(owner).is_empty());
    }

    #[test]
    fn coordinator_debug_projections_redact_operation_and_stage_ids() {
        let (mut coordinator, scope, _) = fixture();
        let id = start(&mut coordinator, &scope);
        let id_text = id.to_string();
        let envelope = coordinator.operation(id).expect("admitted operation").0;
        assert!(!format!("{envelope:?}").contains(id_text.as_str()));

        let ticket = StageTicket {
            operation: id,
            generation: 7,
            stage: 9,
        };
        let projections = [
            format!("{ticket:?}"),
            format!("{:?}", OperationEffect::Start(id)),
            format!("{:?}", OperationEffect::Cleanup(ticket)),
            format!("{:?}", CallbackDisposition::CleanupOwnStage(ticket)),
        ];
        for projection in projections {
            assert!(!projection.contains(id_text.as_str()));
            assert!(!projection.contains("generation: 7"));
        }
    }

    #[test]
    fn validating_start_remains_active_until_deadline_expiry() {
        let (mut coordinator, scope, time) = fixture();
        let operation = start(&mut coordinator, &scope);
        coordinator
            .advance(operation, OperationState::Validating)
            .unwrap();
        assert_eq!(coordinator.active_count(), 1);
        time.set(120_010);
        assert!(coordinator.expire().is_empty());
        assert_eq!(
            coordinator.operation(operation).unwrap().1,
            OperationState::Failed
        );
        assert_eq!(coordinator.active_count(), 0);
    }

    fn reach(
        coordinator: &mut VersioningCoordinator,
        id: OperationId,
        state: OperationState,
    ) -> Option<StageTicket> {
        if state == OperationState::Requested {
            return None;
        }
        coordinator.advance(id, OperationState::Validating).unwrap();
        if state == OperationState::Validating {
            return None;
        }
        coordinator
            .advance(id, OperationState::AwaitingConsent)
            .unwrap();
        if state == OperationState::AwaitingConsent {
            return None;
        }
        coordinator.consent(id, 4).unwrap();
        if state == OperationState::Preparing {
            return None;
        }
        coordinator.advance(id, OperationState::Executing).unwrap();
        let ticket = coordinator.stage_ticket(id).unwrap();
        if state == OperationState::Executing {
            return Some(ticket);
        }
        assert_eq!(
            coordinator.acknowledge(ticket, StageResult::Success).0,
            CallbackDisposition::Accepted
        );
        if state == OperationState::Verifying {
            return Some(ticket);
        }
        coordinator.advance(id, OperationState::Publishing).unwrap();
        Some(ticket)
    }

    #[test]
    fn cancellation_is_possible_at_every_nonterminal_stage() {
        for state in [
            OperationState::Requested,
            OperationState::Validating,
            OperationState::AwaitingConsent,
            OperationState::Preparing,
            OperationState::Executing,
            OperationState::Verifying,
            OperationState::Publishing,
        ] {
            let (mut coordinator, scope, _) = fixture();
            let id = start(&mut coordinator, &scope);
            let ticket = reach(&mut coordinator, id, state);
            coordinator.cancel(id).unwrap();
            assert_eq!(
                coordinator.operation(id).unwrap().1,
                if state == OperationState::Publishing {
                    OperationState::OutcomeUnknown
                } else {
                    OperationState::Cancelled
                }
            );
            assert_eq!(
                coordinator.complete(id),
                Err(OperationError::InvalidTransition)
            );
            if state == OperationState::Executing {
                let ticket = ticket.unwrap();
                assert_eq!(
                    coordinator.acknowledge(ticket, StageResult::Success),
                    (
                        CallbackDisposition::CleanupOwnStage(ticket),
                        vec![OperationEffect::Cleanup(ticket)]
                    )
                );
                assert_eq!(
                    coordinator.acknowledge(ticket, StageResult::Success).0,
                    CallbackDisposition::Ignored
                );
            }
        }
    }

    #[test]
    fn lost_publication_callback_becomes_unknown_after_its_bounded_window() {
        let (mut coordinator, scope, time) = fixture();
        let id = start(&mut coordinator, &scope);
        reach(&mut coordinator, id, OperationState::Publishing);

        time.set(60_009);
        assert!(coordinator.expire().is_empty());
        assert_eq!(
            coordinator.operation(id).unwrap().1,
            OperationState::Publishing
        );
        time.set(60_010);
        assert!(coordinator.expire().is_empty());
        assert_eq!(
            coordinator.operation(id).unwrap().1,
            OperationState::OutcomeUnknown
        );
        assert_eq!(
            coordinator.complete(id),
            Err(OperationError::InvalidTransition)
        );
    }

    #[test]
    fn owner_replacement_invalidates_operation_and_its_consent() {
        let (mut coordinator, scope, _) = fixture();
        let id = start(&mut coordinator, &scope);
        let ticket = reach(&mut coordinator, id, OperationState::Executing).unwrap();
        let old = scope.native();
        coordinator.bind_owner(NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            old.runtime_profile(),
            ProfileInstanceId::fresh(),
        ));
        assert_eq!(
            coordinator.operation(id).unwrap().1,
            OperationState::Cancelled
        );
        assert_eq!(
            coordinator.acknowledge(ticket, StageResult::Success).0,
            CallbackDisposition::CleanupOwnStage(ticket)
        );
        assert_eq!(
            coordinator.admit(scope, 4, PrivacyKind::Normal, OperationKind::ReadOnly, 100),
            Err(OperationError::OwnerClosed)
        );
    }

    #[test]
    fn duplicate_ack_and_success_are_rejected() {
        let (mut coordinator, scope, _) = fixture();
        let id = start(&mut coordinator, &scope);
        let ticket = reach(&mut coordinator, id, OperationState::Executing).unwrap();
        assert_eq!(
            coordinator.acknowledge(ticket, StageResult::Success).0,
            CallbackDisposition::Accepted
        );
        assert_eq!(
            coordinator.acknowledge(ticket, StageResult::Success).0,
            CallbackDisposition::Ignored
        );
        coordinator.advance(id, OperationState::Publishing).unwrap();
        coordinator.complete(id).unwrap();
        assert_eq!(
            coordinator.complete(id),
            Err(OperationError::InvalidTransition)
        );
    }

    #[test]
    fn atomic_stage_start_never_leaves_an_unticketed_executing_operation() {
        let (mut coordinator, scope, _) = fixture();
        let id = start(&mut coordinator, &scope);
        reach(&mut coordinator, id, OperationState::Preparing);
        coordinator.next_stage = u64::MAX;
        assert_eq!(
            coordinator.begin_stage(id),
            Err(OperationError::InvalidTransition)
        );
        assert_eq!(
            coordinator.operation(id).unwrap().1,
            OperationState::Preparing
        );
        assert!(coordinator.operations[&id].ticket.is_none());
        coordinator.next_stage = 1;
        let ticket = coordinator.begin_stage(id).unwrap();
        assert_eq!(
            coordinator.operation(id).unwrap().1,
            OperationState::Executing
        );
        assert_eq!(coordinator.operations[&id].ticket, Some(ticket));
        assert_eq!(
            coordinator.begin_stage(id),
            Err(OperationError::InvalidTransition)
        );
    }

    #[test]
    fn consent_and_callback_deadlines_use_injected_clock() {
        let (mut coordinator, scope, time) = fixture();
        let id = start(&mut coordinator, &scope);
        reach(&mut coordinator, id, OperationState::AwaitingConsent);
        time.set(10 + CONFIRMATION_PLAN_TTL_MS);
        assert_eq!(
            coordinator.consent(id, 4),
            Err(OperationError::ConsentExpired)
        );
        assert_eq!(coordinator.operation(id).unwrap().1, OperationState::Failed);

        let (mut coordinator, scope, time) = fixture();
        let id = start(&mut coordinator, &scope);
        let ticket = reach(&mut coordinator, id, OperationState::Executing).unwrap();
        time.set(120_010);
        assert_eq!(
            coordinator.acknowledge(ticket, StageResult::Success).0,
            CallbackDisposition::CleanupOwnStage(ticket)
        );
        coordinator.expire();
        assert_eq!(coordinator.operation(id).unwrap().1, OperationState::Failed);
    }

    #[test]
    fn mutating_queue_is_bounded_and_promoted_after_cancellation() {
        let (mut coordinator, scope, _) = fixture();
        let first = start(&mut coordinator, &scope);
        let (second, effects) = coordinator
            .admit(
                scope.clone(),
                4,
                PrivacyKind::Normal,
                OperationKind::Mutating,
                120_000,
            )
            .unwrap();
        assert!(effects.is_empty());
        assert_eq!(
            coordinator.operation(second).unwrap().1,
            OperationState::Queued
        );
        assert_eq!(
            coordinator.cancel(first).unwrap(),
            vec![OperationEffect::Start(second)]
        );
        assert_eq!(
            coordinator.operation(second).unwrap().1,
            OperationState::Requested
        );
    }

    #[test]
    fn all_in_flight_operations_are_bounded() {
        let (mut coordinator, scope, _) = fixture();
        for _ in 0..MAX_IN_FLIGHT_OPERATIONS {
            coordinator
                .admit(
                    scope.clone(),
                    4,
                    PrivacyKind::Normal,
                    OperationKind::ReadOnly,
                    120_000,
                )
                .unwrap();
        }
        assert_eq!(
            coordinator.admit(
                scope,
                4,
                PrivacyKind::Normal,
                OperationKind::ReadOnly,
                120_000
            ),
            Err(OperationError::QueueFull)
        );
    }

    #[test]
    fn injected_duplicate_id_cannot_alias_an_existing_operation() {
        let (runtime, window, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "test").unwrap();
        let profile = runtime.view().windows()[&window].profile;
        let owner = NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            profile,
            ProfileInstanceId::fresh(),
        );
        let scope = runtime
            .view()
            .resolve_versioning_scope(ScopeSelector::Window { window }, owner)
            .unwrap();
        let fixed = OperationId::fresh();
        let mut coordinator = VersioningCoordinator::with_sources(|| 1, move || fixed);
        coordinator.bind_owner(owner);
        coordinator
            .admit(
                scope.clone(),
                1,
                PrivacyKind::Normal,
                OperationKind::ReadOnly,
                100,
            )
            .unwrap();
        assert_eq!(
            coordinator.admit(scope, 1, PrivacyKind::Normal, OperationKind::ReadOnly, 100),
            Err(OperationError::DuplicateId)
        );
    }

    #[test]
    fn late_callback_can_clean_only_its_own_stage() {
        let (mut coordinator, scope, _) = fixture();
        let first = start(&mut coordinator, &scope);
        let first_ticket = reach(&mut coordinator, first, OperationState::Executing).unwrap();
        let (second, _) = coordinator
            .admit(
                scope,
                4,
                PrivacyKind::Normal,
                OperationKind::ReadOnly,
                120_000,
            )
            .unwrap();
        let second_ticket = reach(&mut coordinator, second, OperationState::Executing).unwrap();
        coordinator.cancel(first).unwrap();
        assert_ne!(first_ticket, second_ticket);
        assert_eq!(
            coordinator.acknowledge(first_ticket, StageResult::Success),
            (
                CallbackDisposition::CleanupOwnStage(first_ticket),
                vec![OperationEffect::Cleanup(first_ticket)]
            )
        );
        assert_eq!(
            coordinator.operation(second).unwrap().1,
            OperationState::Executing
        );
        assert_eq!(
            coordinator
                .acknowledge(second_ticket, StageResult::Success)
                .0,
            CallbackDisposition::Accepted
        );
        assert_eq!(
            coordinator.operation(first).unwrap().1,
            OperationState::Cancelled
        );
    }

    #[test]
    fn deadline_sweep_cleans_unacknowledged_stage_once() {
        let (mut coordinator, scope, time) = fixture();
        let id = start(&mut coordinator, &scope);
        let ticket = reach(&mut coordinator, id, OperationState::Executing).unwrap();
        time.set(120_010);
        assert_eq!(coordinator.expire(), vec![OperationEffect::Cleanup(ticket)]);
        assert!(coordinator.expire().is_empty());
        assert_eq!(
            coordinator.acknowledge(ticket, StageResult::Success).0,
            CallbackDisposition::CleanupOwnStage(ticket)
        );
    }

    #[test]
    fn uncertain_writes_have_explicit_terminal_outcomes() {
        for (result, state) in [
            (StageResult::Partial, OperationState::Partial),
            (StageResult::OutcomeUnknown, OperationState::OutcomeUnknown),
            (StageResult::FailedWithoutMutation, OperationState::Failed),
        ] {
            let (mut coordinator, scope, _) = fixture();
            let id = start(&mut coordinator, &scope);
            let ticket = reach(&mut coordinator, id, OperationState::Executing).unwrap();
            assert_eq!(
                coordinator.acknowledge(ticket, result).0,
                CallbackDisposition::Accepted
            );
            assert_eq!(coordinator.operation(id).unwrap().1, state);
            assert_eq!(
                coordinator.complete(id),
                Err(OperationError::InvalidTransition)
            );
        }
    }
}
