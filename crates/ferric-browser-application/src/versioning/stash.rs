//! Fail-closed eligibility for explicit volatile view retention. This module
//! never reads editable contents or performs native lifecycle operations.

use ferric_browser_core::{
    CapturedOwner, Event, ExistenceState, LoadingState, OperationId, RendererState,
    ResourceLifecycle, RetainedLifecycle, RetainedViewId, ScopeError, ScopeSelector, StashId,
    Target, VersioningEvent, ViewMembership,
};
use ferric_browser_runtime::{RuntimeEffect, RuntimeError, RuntimeInput};

use crate::BrowserApplication;

use super::{
    AuthorizedPreview, CallbackDisposition, ConfirmationPermit, OperationError, OperationKind,
    OperationState, Preview, PreviewDraft, PreviewError, SensitiveOperation, StageResult,
    StageTicket, VersioningConsentError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreservationIntent {
    OrdinarySuspend,
    ExplicitStash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Activity {
    Clear,
    Active,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditableState {
    Clean,
    Dirty,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityKind {
    Navigation,
    Download,
    Prompt,
    Capture,
    LiveCall,
    DevTools,
    Audio,
    BrowserOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StashBlocker {
    RendererUnavailable,
    NavigationInFlight,
    NonActiveResources,
    LifecycleUnavailable,
    FreezePrimitiveUnqualified,
    ActivityNotClear(ActivityKind),
    EditableUnknown,
    DirtyOrdinarySuspend,
    DirtyEditorUnqualified,
}

impl StashBlocker {
    /// Safe, content-free status copy for the trusted native UI.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::RendererUnavailable => "The page renderer is unavailable",
            Self::NavigationInFlight => "A navigation is still in progress",
            Self::NonActiveResources => "The view is not in an active lifecycle state",
            Self::LifecycleUnavailable => "The native view is not active",
            Self::FreezePrimitiveUnqualified => "Live freeze is not qualified on this runtime",
            Self::ActivityNotClear(ActivityKind::Navigation) => {
                "Navigation activity is active or cannot be verified"
            }
            Self::ActivityNotClear(ActivityKind::Download) => {
                "A download is active or cannot be verified"
            }
            Self::ActivityNotClear(ActivityKind::Prompt) => {
                "A browser prompt is active or cannot be verified"
            }
            Self::ActivityNotClear(ActivityKind::Capture) => {
                "Media capture is active or cannot be verified"
            }
            Self::ActivityNotClear(ActivityKind::LiveCall) => {
                "A live call is active or cannot be verified"
            }
            Self::ActivityNotClear(ActivityKind::DevTools) => {
                "DevTools is attached or cannot be verified"
            }
            Self::ActivityNotClear(ActivityKind::Audio) => "Audio is active or cannot be verified",
            Self::ActivityNotClear(ActivityKind::BrowserOperation) => {
                "A browser operation is active or cannot be verified"
            }
            Self::EditableUnknown => "Editable page state could not be verified",
            Self::DirtyOrdinarySuspend => "Ordinary suspension refuses unsaved editable state",
            Self::DirtyEditorUnqualified => {
                "Dirty-editor live retention is not qualified on this runtime"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StashError {
    UnknownOperation,
    WrongKind,
    WrongScope,
    WrongStage,
    Expired,
    OwnerChanged,
    ConfigurationChanged,
    StaleObservation,
    Scope(ScopeError),
    NotEligible(StashBlocker),
    Operation(OperationError),
    Preview(PreviewError),
    Consent(VersioningConsentError),
    InvalidAuthorization,
    DuplicatePreparation,
    FallbackAllocationFailed,
    RetentionRejected,
    RollbackFailed,
    StalePreparation,
    FallbackChanged,
    FallbackNotPresented,
}

/// An application-reserved fallback and exact source lease. No native freeze
/// has been requested, so the source remains visible in the tab projection.
pub struct StashPreparation {
    owner: CapturedOwner,
    fallback: Target,
    stash: StashId,
    view: RetainedViewId,
    runtime_effects: Vec<RuntimeEffect>,
}

impl StashPreparation {
    #[must_use]
    pub const fn owner(&self) -> CapturedOwner {
        self.owner
    }
    #[must_use]
    pub const fn fallback(&self) -> Target {
        self.fallback
    }
    #[must_use]
    pub const fn stash(&self) -> StashId {
        self.stash
    }
    #[must_use]
    pub const fn view(&self) -> RetainedViewId {
        self.view
    }
    pub fn take_runtime_effects(&mut self) -> Vec<RuntimeEffect> {
        std::mem::take(&mut self.runtime_effects)
    }
}

/// Exact non-serializable callback authority for one native freeze attempt.
/// The native adapter keeps this away from page and IPC command surfaces.
pub struct StashStage {
    operation: OperationId,
    ticket: StageTicket,
    owner: CapturedOwner,
    fallback: Target,
    stash: StashId,
    view: RetainedViewId,
}

impl StashStage {
    #[must_use]
    pub const fn operation(&self) -> OperationId {
        self.operation
    }
    #[must_use]
    pub const fn ticket(&self) -> StageTicket {
        self.ticket
    }
    #[must_use]
    pub const fn owner(&self) -> CapturedOwner {
        self.owner
    }
    #[must_use]
    pub const fn fallback(&self) -> Target {
        self.fallback
    }
    #[must_use]
    pub const fn stash(&self) -> StashId {
        self.stash
    }
    #[must_use]
    pub const fn view(&self) -> RetainedViewId {
        self.view
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PendingStash {
    pub owner: CapturedOwner,
    pub fallback: Target,
    pub stash: StashId,
    pub view: RetainedViewId,
    pub prior_active: Option<ferric_browser_core::TabId>,
    pub ticket: Option<StageTicket>,
}

#[derive(Default)]
pub struct VersioningEffects {
    pub runtime_effects: Vec<RuntimeEffect>,
    pub operation_effects: Vec<super::OperationEffect>,
    /// Exact staged operations whose native hosts must restore the original
    /// view before their logical reservations can be released.
    pub native_cleanup_pending: Vec<OperationId>,
    /// Exact pending reservations that could not be released and must be
    /// retried before the native owner is destroyed.
    pub cleanup_failed: Vec<OperationId>,
}

impl VersioningEffects {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runtime_effects.is_empty()
            && self.operation_effects.is_empty()
            && self.native_cleanup_pending.is_empty()
            && self.cleanup_failed.is_empty()
    }
}

pub type StashAbort = VersioningEffects;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StashAssessment {
    ReadyToSuspend,
    NeedsVolatileDisclosure { dirty: bool },
    Refused(StashBlocker),
}

/// Native observations describe categories only; no field value, URL,
/// credential, or editable text crosses this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StashObservation {
    pub target: Target,
    pub view_active: bool,
    pub freeze_primitive_qualified: bool,
    pub navigation: Activity,
    pub download: Activity,
    pub prompt: Activity,
    pub capture: Activity,
    pub live_call: Activity,
    pub devtools: Activity,
    pub audio: Activity,
    pub browser_operation: Activity,
    pub editable: EditableState,
    pub dirty_editor_qualified: bool,
}

#[must_use]
pub fn assess_preservation(
    intent: PreservationIntent,
    observation: StashObservation,
) -> StashAssessment {
    if !observation.view_active {
        return StashAssessment::Refused(StashBlocker::LifecycleUnavailable);
    }
    if !observation.freeze_primitive_qualified {
        return StashAssessment::Refused(StashBlocker::FreezePrimitiveUnqualified);
    }
    for (kind, activity) in [
        (ActivityKind::Navigation, observation.navigation),
        (ActivityKind::Download, observation.download),
        (ActivityKind::Prompt, observation.prompt),
        (ActivityKind::Capture, observation.capture),
        (ActivityKind::LiveCall, observation.live_call),
        (ActivityKind::DevTools, observation.devtools),
        (ActivityKind::Audio, observation.audio),
        (
            ActivityKind::BrowserOperation,
            observation.browser_operation,
        ),
    ] {
        if activity != Activity::Clear {
            return StashAssessment::Refused(StashBlocker::ActivityNotClear(kind));
        }
    }
    match observation.editable {
        EditableState::Unknown => StashAssessment::Refused(StashBlocker::EditableUnknown),
        EditableState::Dirty if intent == PreservationIntent::OrdinarySuspend => {
            StashAssessment::Refused(StashBlocker::DirtyOrdinarySuspend)
        }
        EditableState::Dirty if !observation.dirty_editor_qualified => {
            StashAssessment::Refused(StashBlocker::DirtyEditorUnqualified)
        }
        EditableState::Clean if intent == PreservationIntent::OrdinarySuspend => {
            StashAssessment::ReadyToSuspend
        }
        EditableState::Clean | EditableState::Dirty => StashAssessment::NeedsVolatileDisclosure {
            dirty: observation.editable == EditableState::Dirty,
        },
    }
}

impl BrowserApplication {
    /// Reserves the exact source before allocating a blank fallback. This
    /// phase never hides the source or claims that native freeze succeeded.
    /// The caller must materialize the returned fallback in the same native
    /// profile before beginning the freeze stage.
    ///
    /// # Errors
    /// Refuses stale owners, changed activity, duplicate reservations and
    /// failed fallback allocation; releases a prepared record on failure.
    pub fn prepare_explicit_stash_fallback(
        &mut self,
        operation: OperationId,
        observation: StashObservation,
    ) -> Result<StashPreparation, StashError> {
        self.prepare_explicit_stash_fallback_with(operation, observation, |application, window| {
            application.dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
        })
    }

    fn prepare_explicit_stash_fallback_with(
        &mut self,
        operation: OperationId,
        observation: StashObservation,
        allocate: impl FnOnce(
            &mut BrowserApplication,
            ferric_browser_core::WindowId,
        ) -> Result<Vec<RuntimeEffect>, RuntimeError>,
    ) -> Result<StashPreparation, StashError> {
        match self.assess_explicit_stash(operation, observation)? {
            StashAssessment::NeedsVolatileDisclosure { .. } => {}
            StashAssessment::Refused(blocker) => return Err(StashError::NotEligible(blocker)),
            StashAssessment::ReadyToSuspend => return Err(StashError::WrongKind),
        }
        let (envelope, state) = self
            .versioning()
            .operation(operation)
            .ok_or(StashError::UnknownOperation)?;
        if state != OperationState::Preparing {
            return Err(StashError::WrongStage);
        }
        let [owner] = envelope.scope().members() else {
            return Err(StashError::WrongScope);
        };
        let owner = *owner;
        if self.pending_stashes.contains_key(&operation) {
            return Err(StashError::DuplicatePreparation);
        }
        let prior_active = self
            .windows()
            .get(&owner.window())
            .ok_or(StashError::StaleObservation)?
            .active_tab;
        let stash = StashId::fresh();
        let view = RetainedViewId::fresh();
        let mut runtime_effects = self
            .dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::PrepareStash { stash, view, owner },
            )))
            .map_err(|_| StashError::RetentionRejected)?;
        let Ok(fallback_effects) = allocate(self, owner.window()) else {
            self.dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::ReleaseRetained { view },
            )))
            .map_err(|_| StashError::RollbackFailed)?;
            return Err(StashError::FallbackAllocationFailed);
        };
        runtime_effects.extend(fallback_effects);
        let fallback = self
            .windows()
            .get(&owner.window())
            .and_then(|window| window.active_tab)
            .and_then(|tab| self.capture_target(tab));
        let fallback_is_blank = fallback.is_some_and(|fallback| {
            fallback.tab != owner.target().tab
                && self.tabs().get(&fallback.tab).is_some_and(|tab| {
                    tab.url.is_none() && tab.profile == owner.native().runtime_profile()
                })
        });
        if !fallback_is_blank {
            self.dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::ReleaseRetained { view },
            )))
            .map_err(|_| StashError::RollbackFailed)?;
            return Err(StashError::FallbackAllocationFailed);
        }
        let fallback = fallback.expect("validated above");
        self.pending_stashes.insert(
            operation,
            PendingStash {
                owner,
                fallback,
                stash,
                view,
                prior_active,
                ticket: None,
            },
        );
        Ok(StashPreparation {
            owner,
            fallback,
            stash,
            view,
            runtime_effects,
        })
    }

    /// Issues one exact callback ticket only after the trusted adapter has
    /// materialized and checked the distinct same-profile fallback host.
    /// This method rechecks logical ownership; it does not inspect Qt objects
    /// or claim that native freeze succeeded. The source remains visible in
    /// core until an actual frozen lifecycle observation is reconciled.
    ///
    /// # Errors
    /// Rejects changed source/fallback identity, duplicate staging, expired
    /// consent, wrong owner, or a missing prepared retention record.
    pub fn begin_explicit_stash_freeze(
        &mut self,
        operation: OperationId,
        preparation: StashPreparation,
        observation: StashObservation,
    ) -> Result<StashStage, StashError> {
        let StashPreparation {
            owner,
            fallback,
            stash,
            view,
            runtime_effects,
        } = preparation;
        if !runtime_effects.is_empty() {
            return Err(StashError::FallbackNotPresented);
        }
        match self.assess_explicit_stash(operation, observation)? {
            StashAssessment::NeedsVolatileDisclosure { .. } => {}
            StashAssessment::Refused(blocker) => return Err(StashError::NotEligible(blocker)),
            StashAssessment::ReadyToSuspend => return Err(StashError::WrongKind),
        }
        let pending = *self
            .pending_stashes
            .get(&operation)
            .ok_or(StashError::UnknownOperation)?;
        if pending.ticket.is_some() {
            return Err(StashError::WrongStage);
        }
        if pending.owner != owner
            || pending.fallback != fallback
            || pending.stash != stash
            || pending.view != view
        {
            return Err(StashError::StalePreparation);
        }
        if self
            .versioning_retention(pending.view)
            .is_none_or(|record| {
                record.stash() != Some(pending.stash)
                    || record.owner() != pending.owner
                    || record.membership() != ViewMembership::Visible
                    || record.lifecycle() != RetainedLifecycle::Active
                    || record.profile_lease_count() != 0
                    || record.view_lease_count() != 0
            })
        {
            return Err(StashError::RetentionRejected);
        }
        let fallback_is_current = self.capture_target(pending.fallback.tab)
            == Some(pending.fallback)
            && self.tabs().get(&pending.fallback.tab).is_some_and(|tab| {
                tab.window == pending.owner.window()
                    && tab.profile == pending.owner.native().runtime_profile()
                    && tab.existence == ExistenceState::Live
                    && tab.renderer == RendererState::Healthy
                    && tab.resources == ResourceLifecycle::Active
                    && tab.url.is_none()
            })
            && self
                .windows()
                .get(&pending.owner.window())
                .is_some_and(|window| window.active_tab == Some(pending.fallback.tab));
        if !fallback_is_current {
            return Err(StashError::FallbackChanged);
        }
        let entry = self
            .pending_stashes
            .get_mut(&operation)
            .ok_or(StashError::UnknownOperation)?;
        let ticket = self
            .versioning
            .begin_stage(operation)
            .map_err(StashError::Operation)?;
        entry.ticket = Some(ticket);
        Ok(StashStage {
            operation,
            ticket,
            owner: pending.owner,
            fallback: pending.fallback,
            stash: pending.stash,
            view: pending.view,
        })
    }

    /// Called only after the retaining native host has acknowledged the
    /// exact view's actual `Frozen` lifecycle. This is not a lifecycle probe:
    /// the trusted adapter must obtain that fact from its host first. The
    /// reducer then acquires both leases and records `Frozen` atomically.
    /// A rejected publication undoes that reducer transition; the caller
    /// must restore the exact native view and acknowledge rollback separately.
    ///
    /// # Errors
    /// Refuses a stale ticket, changed source/fallback, expired owner, or a
    /// missing prepared record. On failure the native host still owns the view.
    pub fn confirm_explicit_stash_frozen(
        &mut self,
        stage: &StashStage,
    ) -> Result<VersioningEffects, StashError> {
        let operation = stage.operation;
        let pending = self.validate_frozen_stash_commit(stage)?;
        let runtime_effects = self
            .dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::ConfirmFrozenStash {
                    stash: pending.stash,
                    view: pending.view,
                    owner: pending.owner,
                },
            )))
            .map_err(|_| StashError::RetentionRejected)?;
        let (disposition, mut operation_effects) = self
            .versioning
            .acknowledge(stage.ticket, StageResult::Success);
        if disposition != CallbackDisposition::Accepted {
            self.rollback_confirmed_stash(stage)?;
            return Err(StashError::WrongStage);
        }
        if let Err(error) = self
            .versioning
            .advance(operation, OperationState::Publishing)
        {
            self.rollback_confirmed_stash(stage)?;
            return Err(StashError::Operation(error));
        }
        match self.complete_versioning_operation(operation) {
            Ok(effects) => operation_effects.extend(effects),
            Err(error) => {
                self.rollback_confirmed_stash(stage)?;
                return Err(StashError::Operation(error));
            }
        }
        self.pending_stashes.remove(&operation);
        Ok(VersioningEffects {
            runtime_effects,
            operation_effects,
            native_cleanup_pending: Vec::new(),
            cleanup_failed: Vec::new(),
        })
    }

    fn validate_frozen_stash_commit(&self, stage: &StashStage) -> Result<PendingStash, StashError> {
        let operation = stage.operation;
        let pending = *self
            .pending_stashes
            .get(&operation)
            .ok_or(StashError::UnknownOperation)?;
        if pending.ticket != Some(stage.ticket)
            || pending.owner != stage.owner
            || pending.fallback != stage.fallback
            || pending.stash != stage.stash
            || pending.view != stage.view
        {
            return Err(StashError::StalePreparation);
        }
        let (envelope, operation_state) = self
            .versioning()
            .operation(operation)
            .ok_or(StashError::UnknownOperation)?;
        if operation_state != OperationState::Executing
            || envelope.kind() != OperationKind::Sensitive(SensitiveOperation::Stash)
            || envelope.scope().members() != [pending.owner]
        {
            return Err(StashError::WrongStage);
        }
        if self.versioning().now_millis() >= envelope.deadline_ms() {
            return Err(StashError::Expired);
        }
        if self
            .versioning()
            .owner(pending.owner.native().runtime_profile())
            != Some(pending.owner.native())
        {
            return Err(StashError::OwnerChanged);
        }
        if self.configuration_revision() != envelope.configuration_revision() {
            return Err(StashError::ConfigurationChanged);
        }
        if self.capture_target(pending.owner.target().tab) != Some(pending.owner.target())
            || self
                .tabs()
                .get(&pending.owner.target().tab)
                .is_none_or(|tab| {
                    tab.window != pending.owner.window()
                        || tab.profile != pending.owner.native().runtime_profile()
                        || tab.existence != ExistenceState::Live
                        || tab.renderer != RendererState::Healthy
                        || matches!(
                            tab.loading,
                            LoadingState::Provisional | LoadingState::Committed
                        )
                })
        {
            return Err(StashError::StaleObservation);
        }
        let fallback_is_current = self.capture_target(pending.fallback.tab)
            == Some(pending.fallback)
            && self.tabs().get(&pending.fallback.tab).is_some_and(|tab| {
                tab.window == pending.owner.window()
                    && tab.profile == pending.owner.native().runtime_profile()
                    && tab.existence == ExistenceState::Live
                    && tab.renderer == RendererState::Healthy
                    && tab.resources == ResourceLifecycle::Active
                    && tab.url.is_none()
            })
            && self
                .windows()
                .get(&pending.owner.window())
                .is_some_and(|window| window.active_tab == Some(pending.fallback.tab));
        if !fallback_is_current {
            return Err(StashError::FallbackChanged);
        }
        if self
            .versioning_retention(pending.view)
            .is_none_or(|record| {
                record.stash() != Some(pending.stash)
                    || record.owner() != pending.owner
                    || record.membership() != ViewMembership::Visible
                    || record.lifecycle() != RetainedLifecycle::Active
                    || record.profile_lease_count() != 0
                    || record.view_lease_count() != 0
            })
        {
            return Err(StashError::RetentionRejected);
        }
        Ok(pending)
    }

    fn rollback_confirmed_stash(&mut self, stage: &StashStage) -> Result<(), StashError> {
        self.dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
            VersioningEvent::RollbackFrozenStash {
                stash: stage.stash,
                view: stage.view,
                owner: stage.owner,
            },
        )))
        .map(|_| ())
        .map_err(|_| StashError::RollbackFailed)
    }

    /// Releases a cancelled native stage only after the trusted adapter has
    /// restored its exact original view through the retaining host. The stage
    /// token is not accepted from page content or generic IPC. No successful
    /// freeze claim is published by this rollback path.
    ///
    /// # Errors
    /// Refuses a live stage, a mismatched ticket, or a missing prepared record.
    pub fn acknowledge_explicit_stash_rollback(
        &mut self,
        stage: &StashStage,
    ) -> Result<Vec<RuntimeEffect>, StashError> {
        let (operation, pending) = self
            .pending_stashes
            .iter()
            .find_map(|(operation, pending)| {
                (pending.ticket == Some(stage.ticket)).then_some((*operation, *pending))
            })
            .ok_or(StashError::UnknownOperation)?;
        if pending.owner != stage.owner
            || pending.fallback != stage.fallback
            || pending.stash != stage.stash
            || pending.view != stage.view
        {
            return Err(StashError::StalePreparation);
        }
        // The coordinator bounds recent terminal history. A missing entry is
        // safe here: only terminal entries can be evicted, while this exact
        // pending stage ticket remains application-owned until native cleanup.
        if self
            .versioning()
            .operation(operation)
            .is_some_and(|(_, state)| {
                !matches!(
                    state,
                    OperationState::Cancelled
                        | OperationState::Failed
                        | OperationState::Partial
                        | OperationState::OutcomeUnknown
                )
            })
        {
            return Err(StashError::WrongStage);
        }
        if self
            .versioning_retention(pending.view)
            .is_none_or(|record| {
                record.stash() != Some(pending.stash) || record.owner() != pending.owner
            })
        {
            return Err(StashError::RetentionRejected);
        }
        if self
            .versioning_retention(pending.view)
            .is_some_and(|record| {
                record.membership() == ViewMembership::Retained
                    && record.lifecycle() == RetainedLifecycle::Frozen
                    && record.profile_lease_count() == 1
                    && record.view_lease_count() == 1
            })
        {
            self.rollback_confirmed_stash(stage)?;
        }
        if self
            .versioning_retention(pending.view)
            .is_none_or(|record| {
                record.membership() != ViewMembership::Visible
                    || record.lifecycle() != RetainedLifecycle::Active
                    || record.profile_lease_count() != 0
                    || record.view_lease_count() != 0
            })
        {
            return Err(StashError::RetentionRejected);
        }
        let entry = self
            .pending_stashes
            .get_mut(&operation)
            .ok_or(StashError::UnknownOperation)?;
        entry.ticket = None;
        match self.release_pending_stash_reservation(operation) {
            Ok(effects) => Ok(effects),
            Err(error) => {
                if let Some(pending) = self.pending_stashes.get_mut(&operation) {
                    pending.ticket = Some(stage.ticket);
                }
                Err(error)
            }
        }
    }

    /// Cancels a reservation before native staging. It releases only this
    /// source record and closes the exact untouched fallback when possible.
    /// A changed fallback is left available rather than deleting user work.
    ///
    /// # Errors
    /// Returns an error if the reservation or reducer-owned source is missing.
    pub fn abort_explicit_stash_preparation(
        &mut self,
        operation: OperationId,
    ) -> Result<StashAbort, StashError> {
        if !self.pending_stashes.contains_key(&operation) {
            return Err(StashError::UnknownOperation);
        }
        if self.pending_stashes[&operation].ticket.is_some() {
            return Err(StashError::WrongStage);
        }
        let effects = self
            .cancel_versioning_operation(operation)
            .map_err(StashError::Operation)?;
        if !effects.cleanup_failed.is_empty() {
            return Err(StashError::RollbackFailed);
        }
        Ok(effects)
    }

    /// Finishes a terminal reservation that never received a native freeze
    /// ticket. The trusted adapter calls this only after its exact source
    /// native view has been observed back at `Active`; an already-released
    /// logical reservation is an idempotent success for that held adapter
    /// identity. This never publishes a frozen stash.
    ///
    /// # Errors
    /// Refuses a live operation, a mismatched view, a stage ticket, or a
    /// failed reducer cleanup. The adapter must retain its native hold then.
    pub fn acknowledge_unstaged_stash_rollback(
        &mut self,
        operation: OperationId,
        view: RetainedViewId,
    ) -> Result<Vec<RuntimeEffect>, StashError> {
        if self
            .versioning()
            .operation(operation)
            .is_some_and(|(_, state)| {
                !matches!(
                    state,
                    OperationState::Cancelled
                        | OperationState::Failed
                        | OperationState::Partial
                        | OperationState::OutcomeUnknown
                )
            })
        {
            return Err(StashError::WrongStage);
        }
        let Some(pending) = self.pending_stashes.get(&operation) else {
            return Ok(Vec::new());
        };
        if pending.view != view {
            return Err(StashError::StalePreparation);
        }
        if pending.ticket.is_some() {
            return Err(StashError::WrongStage);
        }
        self.release_pending_stash_reservation(operation)
    }

    pub(crate) fn release_pending_stash_reservation(
        &mut self,
        operation: OperationId,
    ) -> Result<Vec<RuntimeEffect>, StashError> {
        let pending = *self
            .pending_stashes
            .get(&operation)
            .ok_or(StashError::UnknownOperation)?;
        if pending.ticket.is_some() {
            return Err(StashError::WrongStage);
        }
        if self
            .versioning_retention(pending.view)
            .is_none_or(|record| {
                record.stash() != Some(pending.stash) || record.owner() != pending.owner
            })
        {
            return Err(StashError::RollbackFailed);
        }
        let mut runtime_effects = self
            .dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::ReleaseRetained { view: pending.view },
            )))
            .map_err(|_| StashError::RollbackFailed)?;
        self.pending_stashes.remove(&operation);
        let fallback_is_untouched = self.capture_target(pending.fallback.tab)
            == Some(pending.fallback)
            && self
                .tabs()
                .get(&pending.fallback.tab)
                .is_some_and(|tab| tab.url.is_none());
        if fallback_is_untouched {
            let fallback_was_active = self
                .windows()
                .get(&pending.owner.window())
                .is_some_and(|window| window.active_tab == Some(pending.fallback.tab));
            if let Ok(effects) = self.dispatch_runtime(RuntimeInput::EngineFact(Event::CloseTab {
                tab: pending.fallback.tab,
            })) {
                runtime_effects.extend(effects);
                if fallback_was_active
                    && let Some(prior) = pending.prior_active
                    && self.tabs().get(&prior).is_some_and(|tab| {
                        tab.window == pending.owner.window()
                            && tab.existence == ferric_browser_core::ExistenceState::Live
                    })
                    && let Ok(effects) =
                        self.dispatch_runtime(RuntimeInput::EngineFact(Event::ActivateTab {
                            window: pending.owner.window(),
                            tab: prior,
                        }))
                {
                    runtime_effects.extend(effects);
                }
            }
        }
        Ok(runtime_effects)
    }

    pub(crate) fn release_pending_stashes_for_profile(
        &mut self,
        profile: ferric_browser_core::ProfileId,
    ) -> Result<Vec<RuntimeEffect>, StashError> {
        let ids = self
            .pending_stashes
            .iter()
            .filter_map(|(id, pending)| {
                (pending.owner.native().runtime_profile() == profile).then_some(*id)
            })
            .collect::<Vec<_>>();
        let mut effects = Vec::new();
        for id in ids {
            effects.extend(self.release_pending_stash_reservation(id)?);
        }
        Ok(effects)
    }

    /// Revalidates an admitted, exact tab/document before trusting categorical
    /// native observations. Focus changes do not retarget this operation.
    ///
    /// # Errors
    /// Rejects stale ownership, scope, stage, deadlines, and observations.
    pub fn assess_explicit_stash(
        &self,
        operation: OperationId,
        observation: StashObservation,
    ) -> Result<StashAssessment, StashError> {
        let (envelope, state) = self
            .versioning()
            .operation(operation)
            .ok_or(StashError::UnknownOperation)?;
        if envelope.kind() != OperationKind::Sensitive(SensitiveOperation::Stash) {
            return Err(StashError::WrongKind);
        }
        if !matches!(
            state,
            OperationState::Validating
                | OperationState::AwaitingConsent
                | OperationState::Preparing
        ) {
            return Err(StashError::WrongStage);
        }
        if self.versioning().now_millis() >= envelope.deadline_ms() {
            return Err(StashError::Expired);
        }
        if self.configuration_revision() != envelope.configuration_revision() {
            return Err(StashError::ConfigurationChanged);
        }
        let scope = envelope.scope();
        let [owner] = scope.members() else {
            return Err(StashError::WrongScope);
        };
        if self.versioning().owner(owner.native().runtime_profile()) != Some(owner.native()) {
            return Err(StashError::OwnerChanged);
        }
        if observation.target != owner.target() {
            return Err(StashError::StaleObservation);
        }
        let current = self
            .runtime_view()
            .resolve_versioning_scope(
                ScopeSelector::Tab {
                    window: owner.window(),
                    target: owner.target(),
                },
                owner.native(),
            )
            .map_err(StashError::Scope)?;
        if current.members() != scope.members() {
            return Err(StashError::StaleObservation);
        }
        let tab = self
            .tabs()
            .get(&owner.target().tab)
            .ok_or(StashError::StaleObservation)?;
        if tab.renderer != RendererState::Healthy {
            return Ok(StashAssessment::Refused(StashBlocker::RendererUnavailable));
        }
        if matches!(
            tab.loading,
            LoadingState::Provisional | LoadingState::Committed
        ) {
            return Ok(StashAssessment::Refused(StashBlocker::NavigationInFlight));
        }
        if tab.resources != ResourceLifecycle::Active {
            return Ok(StashAssessment::Refused(StashBlocker::NonActiveResources));
        }
        Ok(assess_preservation(
            PreservationIntent::ExplicitStash,
            observation,
        ))
    }

    /// Creates a fixed-copy volatile-work preview for one exact view. Clean
    /// work is disclosed too: no live stash is a durable save.
    ///
    /// # Errors
    /// Rejects unsafe/stale observations or a preview that cannot be stored.
    pub fn prepare_explicit_stash_disclosure(
        &mut self,
        operation: OperationId,
        observation: StashObservation,
    ) -> Result<Preview, StashError> {
        let dirty = match self.assess_explicit_stash(operation, observation)? {
            StashAssessment::NeedsVolatileDisclosure { dirty } => dirty,
            StashAssessment::Refused(blocker) => return Err(StashError::NotEligible(blocker)),
            StashAssessment::ReadyToSuspend => return Err(StashError::WrongKind),
        };
        let (envelope, state) = self
            .versioning()
            .operation(operation)
            .ok_or(StashError::UnknownOperation)?;
        if state != OperationState::Validating {
            return Err(StashError::WrongStage);
        }
        let [owner] = envelope.scope().members() else {
            return Err(StashError::WrongScope);
        };
        let owner = *owner;
        self.advance_versioning_operation(operation, OperationState::AwaitingConsent)
            .map_err(StashError::Operation)?;
        match self.prepare_versioning_preview(operation, PreviewDraft::stash(owner, dirty)) {
            Ok(preview) => Ok(preview),
            Err(error) => {
                let _ = self.cancel_versioning_operation(operation);
                Err(StashError::Preview(error))
            }
        }
    }

    /// Rechecks the exact view before consuming a one-use native decision.
    /// A page/IPC boolean is not a permit and cannot enter this method.
    ///
    /// # Errors
    /// Rejects changed activity, owner, document, disclosure or consent.
    pub fn authorize_explicit_stash(
        &mut self,
        operation: OperationId,
        observation: StashObservation,
        permit: ConfirmationPermit,
    ) -> Result<ferric_browser_core::CapturedOwner, StashError> {
        let dirty = match self.assess_explicit_stash(operation, observation)? {
            StashAssessment::NeedsVolatileDisclosure { dirty } => dirty,
            StashAssessment::Refused(blocker) => return Err(StashError::NotEligible(blocker)),
            StashAssessment::ReadyToSuspend => return Err(StashError::WrongKind),
        };
        let (envelope, state) = self
            .versioning()
            .operation(operation)
            .ok_or(StashError::UnknownOperation)?;
        if state != OperationState::AwaitingConsent {
            return Err(StashError::WrongStage);
        }
        let [owner] = envelope.scope().members() else {
            return Err(StashError::WrongScope);
        };
        let owner = *owner;
        let payload = self
            .consent_versioning_operation(operation, permit)
            .map_err(StashError::Consent)?;
        match payload {
            AuthorizedPreview::Stash {
                owner: authorized,
                dirty: authorized_dirty,
            } if authorized == owner && authorized_dirty == dirty => Ok(owner),
            _ => {
                let _ = self.cancel_versioning_operation(operation);
                Err(StashError::InvalidAuthorization)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::versioning::PreviewKind;
    use ferric_browser_config::Config;
    use ferric_browser_core::{Event, PrivacyKind, ProfileInstanceId, ValidatedUrl};
    use ferric_browser_runtime::BrowserRuntime;
    use ferric_browser_runtime::RuntimeInput;
    use std::{cell::Cell, rc::Rc};

    fn safe_observation() -> StashObservation {
        let (runtime, _, tab) =
            BrowserRuntime::bootstrap(PrivacyKind::Normal, "stash-test").unwrap();
        StashObservation {
            target: runtime.view().capture_target(tab).unwrap(),
            view_active: true,
            freeze_primitive_qualified: true,
            navigation: Activity::Clear,
            download: Activity::Clear,
            prompt: Activity::Clear,
            capture: Activity::Clear,
            live_call: Activity::Clear,
            devtools: Activity::Clear,
            audio: Activity::Clear,
            browser_operation: Activity::Clear,
            editable: EditableState::Clean,
            dirty_editor_qualified: false,
        }
    }

    fn application_fixture() -> (BrowserApplication, OperationId, StashObservation) {
        let (mut application, window, tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "stash-test")
                .unwrap();
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .unwrap();
        let target = application.capture_target(tab).unwrap();
        let (operation, _) = application
            .request_versioning_operation(
                ScopeSelector::Tab { window, target },
                OperationKind::Sensitive(SensitiveOperation::Stash),
                120_000,
            )
            .unwrap();
        application
            .advance_versioning_operation(operation, OperationState::Validating)
            .unwrap();
        let mut observation = safe_observation();
        observation.target = target;
        (application, operation, observation)
    }

    fn authorized_fixture() -> (BrowserApplication, OperationId, StashObservation) {
        let (mut application, operation, observation) = application_fixture();
        let native_sink = application.take_native_versioning_decision_sink().unwrap();
        let preview = application
            .prepare_explicit_stash_disclosure(operation, observation)
            .unwrap();
        let permit = application
            .capture_native_versioning_decision(operation, native_sink.accept(&preview))
            .unwrap();
        application
            .authorize_explicit_stash(operation, observation, permit)
            .unwrap();
        (application, operation, observation)
    }

    fn authorized_fixture_with_clock() -> (
        BrowserApplication,
        OperationId,
        StashObservation,
        Rc<Cell<u64>>,
    ) {
        let (mut application, window, tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "stash-test")
                .unwrap();
        let time = Rc::new(Cell::new(0));
        let clock = Rc::clone(&time);
        application.versioning = super::super::VersioningCoordinator::with_sources(
            move || clock.get(),
            OperationId::fresh,
        );
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .unwrap();
        let target = application.capture_target(tab).unwrap();
        let (operation, _) = application
            .request_versioning_operation(
                ScopeSelector::Tab { window, target },
                OperationKind::Sensitive(SensitiveOperation::Stash),
                100,
            )
            .unwrap();
        application
            .advance_versioning_operation(operation, OperationState::Validating)
            .unwrap();
        let mut observation = safe_observation();
        observation.target = target;
        let native_sink = application.take_native_versioning_decision_sink().unwrap();
        let preview = application
            .prepare_explicit_stash_disclosure(operation, observation)
            .unwrap();
        let permit = application
            .capture_native_versioning_decision(operation, native_sink.accept(&preview))
            .unwrap();
        application
            .authorize_explicit_stash(operation, observation, permit)
            .unwrap();
        (application, operation, observation, time)
    }

    #[test]
    fn authorized_stash_reserves_source_and_blank_fallback_without_hiding_it() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        assert_eq!(
            prepared.owner,
            application.pending_stashes[&operation].owner
        );
        assert_eq!(prepared.owner.target(), observation.target);
        assert_ne!(prepared.fallback.tab, observation.target.tab);
        assert_eq!(application.tabs()[&prepared.fallback.tab].url, None);
        assert_eq!(
            application
                .versioning_retention(prepared.view)
                .unwrap()
                .membership(),
            ferric_browser_core::ViewMembership::Visible
        );
        assert!(
            application
                .visible_versioning_tabs_for_window(prepared.owner.window())
                .unwrap()
                .contains(&observation.target.tab)
        );
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Preparing
        );
    }

    #[test]
    fn terminal_unstaged_cleanup_requires_exact_view_and_retries_logical_release() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let view = prepared.view();
        assert!(matches!(
            application.acknowledge_unstaged_stash_rollback(operation, view),
            Err(StashError::WrongStage)
        ));
        application.versioning.cancel(operation).unwrap();
        assert!(matches!(
            application.acknowledge_unstaged_stash_rollback(operation, RetainedViewId::fresh()),
            Err(StashError::StalePreparation)
        ));
        let effects = application
            .acknowledge_unstaged_stash_rollback(operation, view)
            .unwrap();
        assert!(!effects.is_empty());
        assert!(application.versioning_retention(view).is_none());
        assert!(
            application
                .acknowledge_unstaged_stash_rollback(operation, view)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn fallback_allocation_failure_releases_prepared_owner_without_hiding_source() {
        let (mut application, operation, observation) = authorized_fixture();
        let before_tabs = application.tabs().len();
        assert!(matches!(
            application.prepare_explicit_stash_fallback_with(operation, observation, |_, _| Err(
                ferric_browser_core::ReduceError::InvalidActivation.into()
            )),
            Err(StashError::FallbackAllocationFailed)
        ));
        assert_eq!(application.tabs().len(), before_tabs);
        assert!(application.pending_stashes.is_empty());
        let window = application
            .versioning()
            .operation(operation)
            .unwrap()
            .0
            .scope()
            .window();
        assert!(
            application
                .visible_versioning_tabs_for_window(window)
                .unwrap()
                .contains(&observation.target.tab)
        );
    }

    #[test]
    fn allocator_reporting_success_without_a_new_fallback_is_rolled_back() {
        let (mut application, operation, observation) = authorized_fixture();
        assert!(matches!(
            application.prepare_explicit_stash_fallback_with(operation, observation, |_, _| Ok(
                Vec::new()
            )),
            Err(StashError::FallbackAllocationFailed)
        ));
        assert!(application.pending_stashes.is_empty());
        let source = observation.target.tab;
        let window = application.tabs()[&source].window;
        assert_eq!(
            application.visible_versioning_tabs_for_window(window),
            Some(vec![source])
        );
    }

    #[test]
    fn aborting_a_prepared_stash_restores_source_and_closes_only_untouched_fallback() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let aborted = application
            .abort_explicit_stash_preparation(operation)
            .unwrap();
        assert!(!aborted.runtime_effects.is_empty());
        assert!(aborted.operation_effects.is_empty());
        assert!(application.pending_stashes.is_empty());
        assert!(application.versioning_retention(prepared.view).is_none());
        assert_eq!(
            application.visible_versioning_tabs_for_window(prepared.owner.window()),
            Some(vec![observation.target.tab])
        );
        assert_eq!(
            application.tabs()[&prepared.fallback.tab].existence,
            ferric_browser_core::ExistenceState::Closing
        );
        assert_eq!(
            application.windows()[&prepared.owner.window()].active_tab,
            Some(observation.target.tab)
        );
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Cancelled
        );
    }

    #[test]
    fn ordinary_operation_cancel_returns_exact_reservation_cleanup_effects() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let effects = application.cancel_versioning_operation(operation).unwrap();
        assert!(!effects.runtime_effects.is_empty());
        assert!(effects.operation_effects.is_empty());
        assert!(application.pending_stashes.is_empty());
        assert!(application.versioning_retention(prepared.view).is_none());
        assert_eq!(
            application.tabs()[&prepared.fallback.tab].existence,
            ferric_browser_core::ExistenceState::Closing
        );
    }

    #[test]
    fn replacing_native_owner_cleans_old_reservation_before_rebinding() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let effects = application
            .bind_native_profile(
                prepared.owner.native().runtime_profile(),
                ProfileInstanceId::fresh(),
            )
            .unwrap();
        assert!(!effects.runtime_effects.is_empty());
        assert!(application.pending_stashes.is_empty());
        assert!(application.versioning_retention(prepared.view).is_none());
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Cancelled
        );
    }

    #[test]
    fn expiry_releases_prepared_reservation_and_returns_runtime_cleanup() {
        let (mut application, operation, observation, time) = authorized_fixture_with_clock();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        time.set(100);
        let effects = application.expire_versioning_operations();
        assert!(!effects.runtime_effects.is_empty());
        assert!(effects.cleanup_failed.is_empty());
        assert!(application.pending_stashes.is_empty());
        assert!(application.versioning_retention(prepared.view).is_none());
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Failed
        );
    }

    #[test]
    fn native_profile_close_releases_pending_reservation_before_owner_close() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let profile = prepared.owner.native().runtime_profile();
        let effects = application.close_native_profile(profile);
        assert!(!effects.runtime_effects.is_empty());
        assert!(effects.cleanup_failed.is_empty());
        assert!(application.pending_stashes.is_empty());
        assert!(application.versioning_retention(prepared.view).is_none());
        assert!(application.versioning().owner(profile).is_none());
    }

    #[test]
    fn native_profile_close_refuses_to_drop_owner_when_reservation_cleanup_fails() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let profile = prepared.owner.native().runtime_profile();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::ReleaseRetained {
                    view: prepared.view,
                },
            )))
            .unwrap();
        let effects = application.close_native_profile(profile);
        assert_eq!(effects.cleanup_failed, vec![operation]);
        assert!(effects.runtime_effects.is_empty());
        assert!(effects.operation_effects.is_empty());
        assert!(application.pending_stashes.contains_key(&operation));
        assert_eq!(
            application.versioning().owner(profile),
            Some(prepared.owner.native())
        );
    }

    #[test]
    fn exact_preparation_enters_one_native_stage_without_hiding_source() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let view = prepared.view;
        assert!(!prepared.take_runtime_effects().is_empty());
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        assert_eq!(
            application.pending_stashes[&operation].ticket,
            Some(stage.ticket)
        );
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Executing
        );
        assert_eq!(
            application.versioning_retention(view).unwrap().membership(),
            ferric_browser_core::ViewMembership::Visible
        );
        assert_eq!(
            application.versioning_stage_ticket(operation),
            Err(OperationError::InvalidTransition)
        );
    }

    #[test]
    fn modeled_native_frozen_callback_commits_exact_stash_once() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let source = prepared.owner.target().tab;
        let fallback = prepared.fallback.tab;
        let view = prepared.view;
        prepared.take_runtime_effects();
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        // The trusted adapter must call this only after the Qt host has
        // acknowledged the real Frozen property; this unit test models that.
        application.confirm_explicit_stash_frozen(&stage).unwrap();
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Completed
        );
        let record = application.versioning_retention(view).unwrap();
        assert_eq!(record.membership(), ViewMembership::Retained);
        assert_eq!(record.lifecycle(), RetainedLifecycle::Frozen);
        assert_eq!(
            (record.profile_lease_count(), record.view_lease_count()),
            (1, 1)
        );
        assert_eq!(
            application.visible_versioning_tabs_for_window(stage.owner.window()),
            Some(vec![fallback])
        );
        assert_eq!(
            application.windows()[&stage.owner.window()].active_tab,
            Some(fallback)
        );
        assert!(application.tabs().contains_key(&source));
        assert!(!application.pending_stashes.contains_key(&operation));
        assert!(matches!(
            application.confirm_explicit_stash_frozen(&stage),
            Err(StashError::UnknownOperation)
        ));
    }

    #[test]
    fn changed_fallback_refuses_modeled_frozen_callback_without_acquiring_leases() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let fallback = prepared.fallback;
        let view = prepared.view;
        prepared.take_runtime_effects();
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: fallback,
                url: ValidatedUrl::parse("https://example.test/new").unwrap(),
            }))
            .unwrap();
        assert!(matches!(
            application.confirm_explicit_stash_frozen(&stage),
            Err(StashError::FallbackChanged)
        ));
        let record = application.versioning_retention(view).unwrap();
        assert_eq!(record.membership(), ViewMembership::Visible);
        assert_eq!(
            (record.profile_lease_count(), record.view_lease_count()),
            (0, 0)
        );
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Executing
        );
        let cancellation = application.cancel_versioning_operation(operation).unwrap();
        assert_eq!(cancellation.native_cleanup_pending, vec![operation]);
        // A native adapter would first restore the exact view, then signal
        // this acknowledgement; the changed fallback is left untouched.
        application
            .acknowledge_explicit_stash_rollback(&stage)
            .unwrap();
        assert!(application.versioning_retention(view).is_none());
        assert_eq!(
            application.tabs()[&fallback.tab].existence,
            ExistenceState::Live
        );
    }

    #[test]
    fn rollback_after_modeled_native_retention_releases_exact_record() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let view = prepared.view;
        let stash = prepared.stash;
        let owner = prepared.owner;
        prepared.take_runtime_effects();
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::ConfirmFrozenStash { stash, view, owner },
            )))
            .unwrap();
        assert_eq!(
            application.versioning_retention(view).unwrap().membership(),
            ViewMembership::Retained
        );
        application.cancel_versioning_operation(operation).unwrap();
        application
            .acknowledge_explicit_stash_rollback(&stage)
            .unwrap();
        assert!(application.versioning_retention(view).is_none());
        assert!(!application.pending_stashes.contains_key(&operation));
    }

    #[test]
    fn expired_frozen_callback_never_acquires_application_leases() {
        let (mut application, operation, observation, time) = authorized_fixture_with_clock();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let view = prepared.view;
        prepared.take_runtime_effects();
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        time.set(100);
        assert!(matches!(
            application.confirm_explicit_stash_frozen(&stage),
            Err(StashError::Expired)
        ));
        let record = application.versioning_retention(view).unwrap();
        assert_eq!(record.membership(), ViewMembership::Visible);
        assert_eq!(
            (record.profile_lease_count(), record.view_lease_count()),
            (0, 0)
        );
        let expired = application.expire_versioning_operations();
        assert_eq!(expired.native_cleanup_pending, vec![operation]);
        application
            .acknowledge_explicit_stash_rollback(&stage)
            .unwrap();
        assert!(application.versioning_retention(view).is_none());
    }

    #[test]
    fn unrelated_stage_callback_cannot_publish_a_modeled_stash() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let view = prepared.view;
        prepared.take_runtime_effects();
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        assert_eq!(
            application
                .acknowledge_versioning_stage(stage.ticket, StageResult::Success)
                .0,
            CallbackDisposition::Accepted
        );
        assert!(matches!(
            application.confirm_explicit_stash_frozen(&stage),
            Err(StashError::WrongStage)
        ));
        assert_eq!(
            application.versioning_retention(view).unwrap().membership(),
            ViewMembership::Visible
        );
        application.cancel_versioning_operation(operation).unwrap();
        application
            .acknowledge_explicit_stash_rollback(&stage)
            .unwrap();
    }

    #[test]
    fn native_stage_refuses_unpresented_fallback_effects() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        assert!(matches!(
            application.begin_explicit_stash_freeze(operation, prepared, observation),
            Err(StashError::FallbackNotPresented)
        ));
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Preparing
        );
        assert!(application.pending_stashes[&operation].ticket.is_none());
        application
            .abort_explicit_stash_preparation(operation)
            .unwrap();
    }

    #[test]
    fn changed_fallback_cannot_begin_native_freeze_stage() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let fallback = prepared.fallback;
        assert!(!prepared.take_runtime_effects().is_empty());
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: fallback,
                url: ValidatedUrl::parse("https://example.test/new").unwrap(),
            }))
            .unwrap();
        assert!(matches!(
            application.begin_explicit_stash_freeze(operation, prepared, observation),
            Err(StashError::FallbackChanged)
        ));
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Preparing
        );
        assert!(application.pending_stashes[&operation].ticket.is_none());
    }

    #[test]
    fn cancellation_after_stage_keeps_source_owned_until_native_cleanup() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let view = prepared.view;
        assert!(!prepared.take_runtime_effects().is_empty());
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        let effects = application.cancel_versioning_operation(operation).unwrap();
        assert_eq!(effects.native_cleanup_pending, vec![operation]);
        assert_eq!(
            effects.operation_effects,
            vec![super::super::OperationEffect::Cleanup(stage.ticket)]
        );
        assert!(effects.runtime_effects.is_empty());
        assert!(application.pending_stashes.contains_key(&operation));
        assert!(application.versioning_retention(view).is_some());
    }

    #[test]
    fn cancelled_stage_releases_source_only_after_exact_native_rollback_ack() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let view = prepared.view;
        let fallback = prepared.fallback;
        assert!(!prepared.take_runtime_effects().is_empty());
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        let cancellation = application.cancel_versioning_operation(operation).unwrap();
        assert_eq!(cancellation.native_cleanup_pending, vec![operation]);
        assert!(application.versioning_retention(view).is_some());
        let (late, effects) = application
            .acknowledge_versioning_stage(stage.ticket, super::super::StageResult::Success);
        assert_eq!(
            late,
            super::super::CallbackDisposition::CleanupOwnStage(stage.ticket)
        );
        assert_eq!(
            effects,
            vec![super::super::OperationEffect::Cleanup(stage.ticket)]
        );
        let runtime_effects = application
            .acknowledge_explicit_stash_rollback(&stage)
            .unwrap();
        assert!(!runtime_effects.is_empty());
        assert!(application.versioning_retention(view).is_none());
        assert!(!application.pending_stashes.contains_key(&operation));
        assert_eq!(
            application.tabs()[&fallback.tab].existence,
            ExistenceState::Closing
        );
        assert!(matches!(
            application.acknowledge_explicit_stash_rollback(&stage),
            Err(StashError::UnknownOperation)
        ));
        assert_eq!(
            application
                .acknowledge_versioning_stage(stage.ticket, super::super::StageResult::Success)
                .0,
            super::super::CallbackDisposition::Ignored
        );
    }

    #[test]
    fn rollback_ack_before_native_cleanup_is_rejected_without_releasing_source() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let view = prepared.view;
        assert!(!prepared.take_runtime_effects().is_empty());
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        assert!(matches!(
            application.acknowledge_explicit_stash_rollback(&stage),
            Err(StashError::WrongStage)
        ));
        assert!(application.versioning_retention(view).is_some());
        assert_eq!(
            application.pending_stashes[&operation].ticket,
            Some(stage.ticket)
        );
    }

    #[test]
    fn expired_native_stage_keeps_source_until_exact_rollback_ack() {
        let (mut application, operation, observation, time) = authorized_fixture_with_clock();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let view = prepared.view;
        assert!(!prepared.take_runtime_effects().is_empty());
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        time.set(100);
        let expired = application.expire_versioning_operations();
        assert_eq!(expired.native_cleanup_pending, vec![operation]);
        assert_eq!(
            expired.operation_effects,
            vec![super::super::OperationEffect::Cleanup(stage.ticket)]
        );
        assert!(application.versioning_retention(view).is_some());
        application
            .acknowledge_explicit_stash_rollback(&stage)
            .unwrap();
        assert!(application.versioning_retention(view).is_none());
    }

    #[test]
    fn profile_close_waits_for_native_restore_of_a_started_stage() {
        let (mut application, operation, observation) = authorized_fixture();
        let mut prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let profile = prepared.owner.native().runtime_profile();
        assert!(!prepared.take_runtime_effects().is_empty());
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        let close = application.close_native_profile(profile);
        assert_eq!(close.native_cleanup_pending, vec![operation]);
        assert!(close.cleanup_failed.is_empty());
        assert_eq!(
            application.versioning().owner(profile),
            Some(stage.owner.native())
        );
        assert!(matches!(
            application.bind_native_profile(profile, ProfileInstanceId::fresh()),
            Err(OperationError::InvalidTransition)
        ));
        let cancelled = application.cancel_versioning_operation(operation).unwrap();
        assert_eq!(cancelled.native_cleanup_pending, vec![operation]);
        application
            .acknowledge_explicit_stash_rollback(&stage)
            .unwrap();
        assert!(application.close_native_profile(profile).is_empty());
        assert!(application.versioning().owner(profile).is_none());
    }

    #[test]
    fn pre_execution_failure_releases_prepared_reservation() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let effects = application
            .fail_versioning_without_mutation(operation)
            .unwrap();
        assert!(!effects.runtime_effects.is_empty());
        assert!(application.pending_stashes.is_empty());
        assert!(application.versioning_retention(prepared.view).is_none());
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Failed
        );
    }

    #[test]
    fn application_profile_close_returns_preparation_cleanup() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        let effects = application.close_profile();
        assert!(!effects.runtime_effects.is_empty());
        assert!(effects.cleanup_failed.is_empty());
        assert!(application.pending_stashes.is_empty());
        assert!(application.versioning_retention(prepared.view).is_none());
        assert!(
            application
                .versioning()
                .owner(prepared.owner.native().runtime_profile())
                .is_none()
        );
    }

    #[test]
    fn abort_does_not_delete_a_fallback_that_navigated_after_reservation() {
        let (mut application, operation, observation) = authorized_fixture();
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: prepared.fallback,
                url: ValidatedUrl::parse("https://example.test/user-work").unwrap(),
            }))
            .unwrap();
        application
            .abort_explicit_stash_preparation(operation)
            .unwrap();
        assert!(application.versioning_retention(prepared.view).is_none());
        assert_eq!(
            application.tabs()[&prepared.fallback.tab].existence,
            ferric_browser_core::ExistenceState::Live
        );
        assert_eq!(
            application.windows()[&prepared.owner.window()].active_tab,
            Some(prepared.fallback.tab)
        );
    }

    #[test]
    fn ordinary_suspend_still_refuses_dirty_or_unknown_editors() {
        let mut observation = safe_observation();
        observation.editable = EditableState::Dirty;
        assert_eq!(
            assess_preservation(PreservationIntent::OrdinarySuspend, observation),
            StashAssessment::Refused(StashBlocker::DirtyOrdinarySuspend)
        );
        observation.editable = EditableState::Unknown;
        assert_eq!(
            assess_preservation(PreservationIntent::OrdinarySuspend, observation),
            StashAssessment::Refused(StashBlocker::EditableUnknown)
        );
    }

    #[test]
    fn dirty_explicit_stash_needs_qualification_and_disclosure() {
        let mut observation = safe_observation();
        observation.editable = EditableState::Dirty;
        assert_eq!(
            assess_preservation(PreservationIntent::ExplicitStash, observation),
            StashAssessment::Refused(StashBlocker::DirtyEditorUnqualified)
        );
        observation.dirty_editor_qualified = true;
        assert_eq!(
            assess_preservation(PreservationIntent::ExplicitStash, observation),
            StashAssessment::NeedsVolatileDisclosure { dirty: true }
        );
    }

    #[test]
    fn live_call_prompt_and_unknown_capture_never_hide() {
        let mut observation = safe_observation();
        observation.live_call = Activity::Active;
        assert_eq!(
            assess_preservation(PreservationIntent::ExplicitStash, observation),
            StashAssessment::Refused(StashBlocker::ActivityNotClear(ActivityKind::LiveCall))
        );
        observation.live_call = Activity::Clear;
        observation.prompt = Activity::Active;
        assert_eq!(
            assess_preservation(PreservationIntent::ExplicitStash, observation),
            StashAssessment::Refused(StashBlocker::ActivityNotClear(ActivityKind::Prompt))
        );
        observation.prompt = Activity::Clear;
        observation.live_call = Activity::Unknown;
        assert_eq!(
            assess_preservation(PreservationIntent::ExplicitStash, observation),
            StashAssessment::Refused(StashBlocker::ActivityNotClear(ActivityKind::LiveCall))
        );
        observation.live_call = Activity::Clear;
        observation.capture = Activity::Unknown;
        assert_eq!(
            assess_preservation(PreservationIntent::ExplicitStash, observation),
            StashAssessment::Refused(StashBlocker::ActivityNotClear(ActivityKind::Capture))
        );
    }

    #[test]
    fn every_unsafe_native_activity_category_fails_closed() {
        let observation = safe_observation();
        for (kind, change) in [
            (ActivityKind::Navigation, 0),
            (ActivityKind::Download, 1),
            (ActivityKind::Prompt, 2),
            (ActivityKind::Capture, 3),
            (ActivityKind::LiveCall, 4),
            (ActivityKind::DevTools, 5),
            (ActivityKind::Audio, 6),
            (ActivityKind::BrowserOperation, 7),
        ] {
            let mut blocked = observation;
            match change {
                0 => blocked.navigation = Activity::Active,
                1 => blocked.download = Activity::Active,
                2 => blocked.prompt = Activity::Active,
                3 => blocked.capture = Activity::Active,
                4 => blocked.live_call = Activity::Active,
                5 => blocked.devtools = Activity::Active,
                6 => blocked.audio = Activity::Active,
                _ => blocked.browser_operation = Activity::Active,
            }
            assert_eq!(
                assess_preservation(PreservationIntent::ExplicitStash, blocked),
                StashAssessment::Refused(StashBlocker::ActivityNotClear(kind))
            );
            assert!(!StashBlocker::ActivityNotClear(kind).message().is_empty());
        }
        let mut blocked = observation;
        blocked.view_active = false;
        assert_eq!(
            assess_preservation(PreservationIntent::ExplicitStash, blocked),
            StashAssessment::Refused(StashBlocker::LifecycleUnavailable)
        );
        blocked = observation;
        blocked.freeze_primitive_qualified = false;
        assert_eq!(
            assess_preservation(PreservationIntent::ExplicitStash, blocked),
            StashAssessment::Refused(StashBlocker::FreezePrimitiveUnqualified)
        );
    }

    #[test]
    fn dirty_stash_consent_is_exact_native_one_use_and_rechecks_activity() {
        let (mut application, operation, mut observation) = application_fixture();
        observation.editable = EditableState::Dirty;
        observation.dirty_editor_qualified = true;
        assert_eq!(
            application
                .assess_explicit_stash(operation, observation)
                .unwrap(),
            StashAssessment::NeedsVolatileDisclosure { dirty: true }
        );
        let native_sink = application.take_native_versioning_decision_sink().unwrap();
        let preview = application
            .prepare_explicit_stash_disclosure(operation, observation)
            .unwrap();
        assert_eq!(preview.redacted().kind, PreviewKind::StashDirty);
        assert!(preview.redacted().volatile_work_risk);
        assert!(!preview.redacted().authentication_risk);
        let ipc = serde_json::to_string(&preview.redacted()).unwrap();
        assert!(ipc.contains("\"volatile_work_risk\":true"));
        assert!(ipc.contains("\"authentication_risk\":false"));
        assert!(!ipc.contains("stash-test"));
        assert!(
            PreviewKind::StashDirty
                .volatile_disclosure()
                .unwrap()
                .contains("does not save")
        );
        assert!(!format!("{preview:?}").contains("stash-test"));
        let permit = application
            .capture_native_versioning_decision(operation, native_sink.accept(&preview))
            .unwrap();
        observation.capture = Activity::Active;
        assert_eq!(
            application.authorize_explicit_stash(operation, observation, permit),
            Err(StashError::NotEligible(StashBlocker::ActivityNotClear(
                ActivityKind::Capture
            )))
        );
    }

    #[test]
    fn clean_stash_still_requires_disclosure_and_same_owner() {
        let (mut application, operation, observation) = application_fixture();
        let native_sink = application.take_native_versioning_decision_sink().unwrap();
        let preview = application
            .prepare_explicit_stash_disclosure(operation, observation)
            .unwrap();
        assert_eq!(preview.redacted().kind, PreviewKind::StashClean);
        let permit = application
            .capture_native_versioning_decision(operation, native_sink.accept(&preview))
            .unwrap();
        let owner = application
            .authorize_explicit_stash(operation, observation, permit)
            .unwrap();
        assert_eq!(owner.target(), observation.target);
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Preparing
        );
        assert!(matches!(
            application.capture_native_versioning_decision(operation, native_sink.accept(&preview)),
            Err(PreviewError::StalePlan)
        ));
    }

    #[test]
    fn changed_dirty_classification_invalidates_accepted_preview() {
        let (mut application, operation, mut observation) = application_fixture();
        observation.editable = EditableState::Dirty;
        observation.dirty_editor_qualified = true;
        let native_sink = application.take_native_versioning_decision_sink().unwrap();
        let preview = application
            .prepare_explicit_stash_disclosure(operation, observation)
            .unwrap();
        let permit = application
            .capture_native_versioning_decision(operation, native_sink.accept(&preview))
            .unwrap();
        observation.editable = EditableState::Clean;
        assert_eq!(
            application.authorize_explicit_stash(operation, observation, permit),
            Err(StashError::InvalidAuthorization)
        );
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Cancelled
        );
    }

    #[test]
    fn stale_document_and_wrong_target_cannot_use_stash_observation() {
        let (mut application, operation, observation) = application_fixture();
        let mut wrong = observation;
        wrong.target.generation += 1;
        assert_eq!(
            application.assess_explicit_stash(operation, wrong),
            Err(StashError::StaleObservation)
        );
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: observation.target,
                url: ValidatedUrl::parse("https://example.test/next").unwrap(),
            }))
            .unwrap();
        assert!(matches!(
            application.assess_explicit_stash(operation, observation),
            Err(StashError::Scope(
                ScopeError::StaleTarget | ScopeError::DocumentChanged
            ))
        ));
    }
}
