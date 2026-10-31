//! Native page operations can outlive their original window. Only fixed,
//! bounded receipts cross that lifetime boundary; no Ui/profile/view is held.

use super::{
    MAX_ACTIVE_OPERATIONS, MAX_PENDING_COMPLETIONS, MAX_TERMINAL_OPERATIONS, OperationCatalog,
    OperationCompletion, OperationRecord, OperationState, OperationUpdate,
};
use crate::{BrowserApplication, ProfileInstanceId};
use ferric_browser_core::{FailureKind, PrivacyKind, ShutdownState, WindowId};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Non-deserializable original ownership, captured from the actual application.
/// An observer cannot substitute its own window, profile or privacy for this.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeOperationScope {
    catalog: uuid::Uuid,
    window: WindowId,
    profile: ProfileInstanceId,
    privacy: PrivacyKind,
}

impl NativeOperationScope {
    #[must_use]
    pub fn capture(application: &BrowserApplication, window: WindowId) -> Option<Self> {
        if application.shutdown() != ShutdownState::Running {
            return None;
        }
        let owner = application.windows().get(&window)?;
        Some(Self {
            catalog: application.operations().owner,
            window,
            profile: application.profile_instance_id()?,
            privacy: application.profiles().get(&owner.profile)?.privacy,
        })
    }

    #[must_use]
    pub const fn window(self) -> WindowId {
        self.window
    }
    #[must_use]
    pub const fn profile(self) -> ProfileInstanceId {
        self.profile
    }
    #[must_use]
    pub const fn privacy(self) -> PrivacyKind {
        self.privacy
    }
}

/// Native facts only. Raw command, page, helper and credential errors cannot be
/// copied into the retained operation after its source privacy owner retires.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOperationOutcome {
    Applied,
    Activated,
    ActivationUnknown,
    Stale,
    Failed,
    Rejected,
    OwnerLost,
    ClosedPartially { closed: u16, requested: u16 },
}

impl NativeOperationOutcome {
    fn update(self) -> Option<OperationUpdate> {
        Some(match self {
            Self::Applied => OperationUpdate::completed("page state updated"),
            Self::Activated => OperationUpdate::completed("activated"),
            Self::ActivationUnknown => OperationUpdate::completed("activation outcome unknown"),
            Self::Stale => OperationUpdate::failed(FailureKind::StaleTarget, "page owner changed"),
            Self::Failed => {
                OperationUpdate::failed(FailureKind::Engine, "native page command failed")
            }
            Self::Rejected => OperationUpdate::cancelled(),
            Self::OwnerLost => OperationUpdate::failed(
                FailureKind::Engine,
                "native outcome unknown because its reporter was lost",
            ),
            Self::ClosedPartially { closed, requested }
                if requested <= 100 && closed < requested =>
            {
                OperationUpdate::failed(
                    FailureKind::Engine,
                    format!("Closed {closed} of {requested} pages; remaining pages were kept"),
                )
            }
            Self::ClosedPartially { .. } => return None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOperationReceipt {
    pub scope: NativeOperationScope,
    pub record: OperationRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedOperationCompletion {
    pub scope: NativeOperationScope,
    pub completion: OperationCompletion,
}

#[derive(Debug, Default)]
struct Ledger {
    active: usize,
    completions: VecDeque<PendingCompletion>,
    wake: Option<Wake>,
}

#[derive(Clone)]
struct Wake(Arc<dyn Fn() + Send + Sync>);
impl std::fmt::Debug for Wake {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("native operation wake")
    }
}

#[derive(Debug)]
struct PendingCompletion {
    value: ScopedOperationCompletion,
    identity: Arc<NativeOperationIdentity>,
}

// This reservation holds no record, ledger, Ui or profile resource. A queued
// notification can keep its identity reserved without forming an Arc cycle.
#[derive(Debug)]
pub(super) struct NativeOperationIdentity {
    id: String,
    sequence: u64,
    active: AtomicBool,
    notification_pending: AtomicBool,
}

#[derive(Debug)]
struct NativeOperationState {
    scope: NativeOperationScope,
    identity: Arc<NativeOperationIdentity>,
    retention_order: u64,
    record: Mutex<OperationRecord>,
    ledger: Arc<Mutex<Ledger>>,
}

impl NativeOperationState {
    fn is_active(&self) -> bool {
        self.record
            .lock()
            .map_or(true, |record| !record.state.is_terminal())
    }

    fn receipt(&self) -> Option<NativeOperationReceipt> {
        Some(NativeOperationReceipt {
            scope: self.scope,
            record: self.record.lock().ok()?.clone(),
        })
    }

    fn finish(&self, outcome: NativeOperationOutcome) -> bool {
        let Some(update) = outcome.update() else {
            return false;
        };
        let Ok(mut record) = self.record.lock() else {
            return false;
        };
        if record.state.is_terminal() {
            return false;
        }
        // No callbacks occur under either lock. The reserved active slot only
        // becomes free together with its terminal notification, so concurrent
        // completions cannot evade admission backpressure.
        let Ok(mut ledger) = self.ledger.lock() else {
            return false;
        };
        record.state = update.state;
        record.detail = update.detail;
        record.failure = update.failure;
        record.cancellable = false;
        ledger.active -= 1;
        self.identity.active.store(false, Ordering::Release);
        ledger.completions.push_back(PendingCompletion {
            identity: Arc::clone(&self.identity),
            value: ScopedOperationCompletion {
                scope: self.scope,
                completion: OperationCompletion {
                    id: self.identity.id.clone(),
                    sequence: self.identity.sequence,
                    state: record.state,
                    error: record.failure.as_ref().map(|failure| failure.kind),
                },
            },
        });
        let wake = ledger.wake.clone();
        drop(ledger);
        drop(record);
        if let Some(wake) = wake {
            wake.0();
        }
        true
    }
}

#[derive(Debug)]
struct ReporterAuthority {
    state: Arc<NativeOperationState>,
}

impl Drop for ReporterAuthority {
    fn drop(&mut self) {
        self.state.finish(NativeOperationOutcome::OwnerLost);
    }
}

/// Authority is counted independently of the registry's read-only retention.
/// Losing the last reporter ends pending work with an explicitly unknown
/// outcome; it never asserts that already-issued native effects rolled back.
#[derive(Clone, Debug)]
pub struct NativeOperationReporter(Arc<ReporterAuthority>);

impl NativeOperationReporter {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.0.state.identity.id
    }

    #[must_use]
    pub fn finish(&self, outcome: NativeOperationOutcome) -> bool {
        self.0.state.finish(outcome)
    }
}

/// One bounded registry per browser process. A native page handoff must reserve
/// its slot here before any native effect, then keep its reporter across hops.
#[derive(Debug, Default)]
pub struct NativeOperationRegistry {
    records: BTreeMap<String, Arc<NativeOperationState>>,
    ledger: Arc<Mutex<Ledger>>,
    next_retention_order: u64,
}

impl NativeOperationRegistry {
    /// Installs the process controller's asynchronous wake. No callback is
    /// invoked while the record or notification ledger is locked.
    pub fn install_waker(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut ledger) = self.ledger.lock() {
            ledger.wake = Some(Wake(wake));
        }
    }

    #[must_use]
    pub fn completions_pending(&self) -> bool {
        self.ledger
            .lock()
            .is_ok_and(|ledger| !ledger.completions.is_empty())
    }
    /// Explicit typed native-handoff admission; generic `begin_commit` does not
    /// move a record here. The original catalog and this scope must match.
    ///
    /// # Errors
    /// Leaves the source record unchanged on wrong scope, duplicate identity,
    /// terminal/output-bearing work or exhausted active/notification capacity.
    pub fn begin_handoff(
        &mut self,
        catalog: &mut OperationCatalog,
        scope: NativeOperationScope,
        id: &str,
    ) -> Result<NativeOperationReporter, FailureKind> {
        self.prune_terminal();
        let record = catalog.records.get(id).ok_or(FailureKind::NotFound)?;
        if scope.catalog != catalog.owner
            || self.records.contains_key(id)
            || record.state.is_terminal()
            || record.stderr.is_some()
        {
            return Err(FailureKind::InvalidArgument);
        }
        let retention_order = self
            .next_retention_order
            .checked_add(1)
            .ok_or(FailureKind::Busy)?;
        let mut ledger = self.ledger.lock().map_err(|_| FailureKind::Busy)?;
        if ledger
            .completions
            .iter()
            .any(|entry| entry.value.completion.id == id)
        {
            return Err(FailureKind::InvalidArgument);
        }
        if ledger.active >= MAX_ACTIVE_OPERATIONS
            || ledger.active + ledger.completions.len() >= MAX_PENDING_COMPLETIONS
        {
            return Err(FailureKind::Busy);
        }
        // All fallible admission is complete. There is no callback or external
        // publication between reservation, source removal and registry insert.
        let Some(mut record) = catalog.records.remove(id) else {
            return Err(FailureKind::NotFound);
        };
        ledger.active += 1;
        self.next_retention_order = retention_order;
        catalog.active -= 1;
        record.state = OperationState::Running;
        record.detail = "native handoff pending".into();
        record.failure = None;
        record.cancellable = false;
        let identity = Arc::new(NativeOperationIdentity {
            id: id.into(),
            sequence: record.sequence,
            active: AtomicBool::new(true),
            notification_pending: AtomicBool::new(true),
        });
        let state = Arc::new(NativeOperationState {
            scope,
            identity: Arc::clone(&identity),
            retention_order,
            record: Mutex::new(record),
            ledger: Arc::clone(&self.ledger),
        });
        catalog
            .native_handoffs
            .insert(id.into(), Arc::downgrade(&identity));
        catalog.changed();
        self.records.insert(id.into(), Arc::clone(&state));
        drop(ledger);
        Ok(NativeOperationReporter(Arc::new(ReporterAuthority {
            state,
        })))
    }

    #[must_use]
    pub fn get(&mut self, id: &str) -> Option<NativeOperationReceipt> {
        self.prune_terminal();
        self.records.get(id)?.receipt()
    }

    pub fn records(&mut self) -> impl Iterator<Item = NativeOperationReceipt> + '_ {
        self.prune_terminal();
        self.records.values().filter_map(|state| state.receipt())
    }

    /// Exactly-once notifications retain their ORIGINAL scope after retirement.
    /// Consumers must use that scope for privacy and routing, not their own Ui.
    pub fn drain_completions(&mut self) -> Vec<ScopedOperationCompletion> {
        self.ledger.lock().map_or_else(
            |_| Vec::new(),
            |mut ledger| {
                ledger
                    .completions
                    .drain(..)
                    .map(|entry| {
                        entry
                            .identity
                            .notification_pending
                            .store(false, Ordering::Release);
                        entry.value
                    })
                    .collect()
            },
        )
    }

    /// Called only after native lifetime proof that the final actual transient
    /// profile owner drained. Pending effects have an unknown outcome; only
    /// fixed terminal receipts remain for the adapter's short delivery grace.
    pub fn retire_transient_profile(&mut self, profile: ProfileInstanceId) {
        for state in self
            .records
            .values()
            .filter(|state| state.scope.profile == profile && state.scope.privacy.is_transient())
        {
            state.finish(NativeOperationOutcome::OwnerLost);
        }
    }

    /// End the adapter's bounded receipt grace after native transient teardown.
    /// Active records are retained; this never evicts in-flight authority.
    pub fn forget_transient_profile(&mut self, profile: ProfileInstanceId) {
        self.records.retain(|_, state| {
            state.scope.profile != profile
                || !state.scope.privacy.is_transient()
                || state.is_active()
        });
    }

    fn prune_terminal(&mut self) {
        let mut terminal = self
            .records
            .iter()
            .filter_map(|(id, state)| {
                (!state.is_active()).then_some((state.retention_order, id.clone()))
            })
            .collect::<Vec<_>>();
        terminal.sort_unstable();
        let excess = terminal.len().saturating_sub(MAX_TERMINAL_OPERATIONS);
        for (_, id) in terminal.into_iter().take(excess) {
            self.records.remove(&id);
        }
    }
}

impl OperationCatalog {
    pub(super) fn active_native_handoffs(&self) -> usize {
        self.native_handoffs
            .values()
            .filter_map(std::sync::Weak::upgrade)
            .filter(|identity| identity.active.load(Ordering::Acquire))
            .count()
    }

    pub(super) fn prune_native_handoffs(&mut self) {
        self.native_handoffs
            .retain(|_, state| state.strong_count() != 0);
        let mut terminal = self
            .native_handoffs
            .iter()
            .filter_map(|(id, weak)| {
                let state = weak.upgrade()?;
                (!state.active.load(Ordering::Acquire)
                    && !state.notification_pending.load(Ordering::Acquire))
                .then_some((state.sequence, id.clone()))
            })
            .collect::<Vec<_>>();
        terminal.sort_unstable();
        let excess = terminal.len().saturating_sub(MAX_TERMINAL_OPERATIONS);
        for (_, id) in terminal.into_iter().take(excess) {
            self.native_handoffs.remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_core::ProfileId;

    fn scope(
        catalog: &OperationCatalog,
        number: u64,
        privacy: PrivacyKind,
    ) -> NativeOperationScope {
        NativeOperationScope {
            catalog: catalog.owner,
            window: WindowId::from_display(&format!("windowid-{number}")).unwrap(),
            profile: ProfileInstanceId::synthetic_for_profile(
                ProfileId::from_display(&format!("profileid-{number}")).unwrap(),
            ),
            privacy,
        }
    }

    #[test]
    fn original_identity_scope_and_reporter_survive_source_retirement() {
        let mut catalog = OperationCatalog::default();
        let original = scope(&catalog, 1, PrivacyKind::Private);
        catalog.admit("native".into()).unwrap();
        let sequence = catalog.get("native").unwrap().sequence;
        let mut registry = NativeOperationRegistry::default();
        let reporter = registry
            .begin_handoff(&mut catalog, original, "native")
            .unwrap();
        assert_eq!(reporter.id(), "native");
        assert!(catalog.has_active());
        assert!(catalog.get("native").is_none());
        assert_eq!(
            catalog.admit("native".into()),
            Err(FailureKind::InvalidArgument)
        );
        assert!(!catalog.update("native", OperationUpdate::completed("late")));
        catalog.retire();
        drop(catalog);
        let pending = registry.get("native").unwrap();
        assert_eq!(pending.scope, original);
        assert_eq!(pending.record.state, OperationState::Running);
        assert_eq!(pending.record.sequence, sequence);
        assert!(!pending.record.cancellable);
        assert!(reporter.finish(NativeOperationOutcome::Activated));
        assert!(!reporter.finish(NativeOperationOutcome::Failed));
        let completion = registry.drain_completions();
        assert_eq!(completion.len(), 1);
        assert_eq!(completion[0].scope, original);
        assert_eq!(completion[0].completion.sequence, sequence);
        assert!(registry.drain_completions().is_empty());
    }

    #[test]
    fn actual_scope_capture_retains_no_profile_session_resources() {
        let (mut application, window, _) = BrowserApplication::bootstrap(
            ferric_browser_config::Config::default(),
            PrivacyKind::Private,
            "native source",
        )
        .unwrap();
        assert!(NativeOperationScope::capture(&application, window).is_none());
        application
            .activate_profile(crate::ProfileActivation {
                name: "native source".into(),
                id: None,
                privacy: PrivacyKind::Private,
                roots: None,
                lock: None,
                contexts: None,
                storage_path: None,
            })
            .unwrap();
        let original = NativeOperationScope::capture(&application, window).unwrap();
        assert_eq!(original.window(), window);
        assert_eq!(
            original.profile(),
            application.profile_instance_id().unwrap()
        );
        assert_eq!(original.privacy(), PrivacyKind::Private);
        let weak = Arc::downgrade(&application.active_profile.as_ref().unwrap().session.0);
        application.operations_mut().admit("native".into()).unwrap();
        let mut registry = NativeOperationRegistry::default();
        let reporter = registry
            .begin_handoff(application.operations_mut(), original, "native")
            .unwrap();
        application.close_profile();
        drop(application);
        assert!(weak.upgrade().is_none());
        assert!(reporter.finish(NativeOperationOutcome::Applied));
        assert_eq!(registry.get("native").unwrap().scope, original);
    }

    #[test]
    fn exhausted_registry_order_rejects_without_touching_source() {
        let mut catalog = OperationCatalog::default();
        catalog.admit("native".into()).unwrap();
        let original = scope(&catalog, 1, PrivacyKind::Normal);
        let before = catalog.get("native").unwrap().clone();
        let mut registry = NativeOperationRegistry {
            next_retention_order: u64::MAX,
            ..NativeOperationRegistry::default()
        };
        assert_eq!(
            registry
                .begin_handoff(&mut catalog, original, "native")
                .unwrap_err(),
            FailureKind::Busy
        );
        assert_eq!(catalog.get("native"), Some(&before));
        assert!(registry.records().next().is_none());
    }

    #[test]
    fn observer_retention_does_not_keep_reporter_authority_alive() {
        let mut catalog = OperationCatalog::default();
        let original = scope(&catalog, 1, PrivacyKind::Normal);
        catalog.admit("native".into()).unwrap();
        let mut registry = NativeOperationRegistry::default();
        let reporter = registry
            .begin_handoff(&mut catalog, original, "native")
            .unwrap();
        let second = reporter.clone();
        drop(reporter);
        assert_eq!(
            registry.get("native").unwrap().record.state,
            OperationState::Running
        );
        drop(second);
        let receipt = registry.get("native").unwrap();
        assert_eq!(receipt.record.state, OperationState::Failed);
        assert_eq!(
            receipt.record.detail,
            "native outcome unknown because its reporter was lost"
        );
        assert_eq!(registry.drain_completions().len(), 1);
        assert!(!catalog.has_active());
    }

    #[test]
    fn admission_failures_leave_original_records_and_capacity_unchanged() {
        let mut catalog = OperationCatalog::default();
        let original = scope(&catalog, 1, PrivacyKind::Normal);
        let another = OperationCatalog::default();
        let wrong = scope(&another, 2, PrivacyKind::Private);
        let mut registry = NativeOperationRegistry::default();
        for case in ["wrong-scope", "terminal", "output"] {
            catalog.admit(case.into()).unwrap();
            if case == "terminal" {
                catalog.update(case, OperationUpdate::completed("done"));
            }
            if case == "output" {
                catalog.remember_stderr(case, "private output".into());
            }
            let before = catalog.get(case).unwrap().clone();
            assert_eq!(
                registry
                    .begin_handoff(
                        &mut catalog,
                        if case == "wrong-scope" {
                            wrong
                        } else {
                            original
                        },
                        case
                    )
                    .unwrap_err(),
                FailureKind::InvalidArgument
            );
            assert_eq!(catalog.get(case), Some(&before));
        }
        assert!(registry.records().next().is_none());
        assert!(registry.drain_completions().is_empty());
    }

    #[test]
    fn duplicate_ids_cannot_alias_other_original_owners() {
        let mut first = OperationCatalog::default();
        let mut second = OperationCatalog::default();
        let left = scope(&first, 1, PrivacyKind::Normal);
        let right = scope(&second, 2, PrivacyKind::Private);
        first.admit("native".into()).unwrap();
        second.admit("native".into()).unwrap();
        let mut registry = NativeOperationRegistry::default();
        let reporter = registry.begin_handoff(&mut first, left, "native").unwrap();
        let before = second.get("native").unwrap().clone();
        assert_eq!(
            registry
                .begin_handoff(&mut second, right, "native")
                .unwrap_err(),
            FailureKind::InvalidArgument
        );
        assert_eq!(second.get("native"), Some(&before));
        assert!(reporter.finish(NativeOperationOutcome::Applied));
        assert_eq!(registry.get("native").unwrap().scope, left);
        assert_eq!(
            first.admit("native".into()),
            Err(FailureKind::InvalidArgument)
        );
    }

    #[test]
    fn last_reporter_and_concurrent_terminal_replies_have_one_winner() {
        let mut catalog = OperationCatalog::default();
        let original = scope(&catalog, 1, PrivacyKind::Normal);
        let mut registry = NativeOperationRegistry::default();
        catalog.admit("native".into()).unwrap();
        let reporter = registry
            .begin_handoff(&mut catalog, original, "native")
            .unwrap();
        let other = reporter.clone();
        let first = std::thread::spawn(move || reporter.finish(NativeOperationOutcome::Applied));
        let second = std::thread::spawn(move || other.finish(NativeOperationOutcome::Stale));
        assert_ne!(first.join().unwrap(), second.join().unwrap());
        assert_eq!(registry.drain_completions().len(), 1);
        assert!(registry.get("native").unwrap().record.state.is_terminal());
    }

    #[test]
    fn active_capacity_and_pending_notifications_remain_bounded() {
        let mut registry = NativeOperationRegistry::default();
        let mut catalog = OperationCatalog::default();
        let original = scope(&catalog, 1, PrivacyKind::Normal);
        let mut reporters = Vec::new();
        for index in 0..MAX_ACTIVE_OPERATIONS {
            let id = format!("native-{index}");
            catalog.admit(id.clone()).unwrap();
            reporters.push(registry.begin_handoff(&mut catalog, original, &id).unwrap());
        }
        assert_eq!(
            catalog.admit("local-overflow".into()),
            Err(FailureKind::Busy)
        );
        let mut other = OperationCatalog::default();
        let other_scope = scope(&other, 2, PrivacyKind::Normal);
        other.admit("global-overflow".into()).unwrap();
        assert_eq!(
            registry
                .begin_handoff(&mut other, other_scope, "global-overflow")
                .unwrap_err(),
            FailureKind::Busy
        );
        assert_eq!(registry.records().count(), MAX_ACTIVE_OPERATIONS);
        for reporter in reporters {
            assert!(reporter.finish(NativeOperationOutcome::Applied));
        }
        for index in MAX_ACTIVE_OPERATIONS..MAX_PENDING_COMPLETIONS {
            let id = format!("native-{index}");
            catalog.admit(id.clone()).unwrap();
            let reporter = registry.begin_handoff(&mut catalog, original, &id).unwrap();
            assert!(reporter.finish(NativeOperationOutcome::Applied));
        }
        assert_eq!(registry.records().count(), MAX_TERMINAL_OPERATIONS);
        // The oldest retained record has expired, but its undrained terminal
        // notification must still reserve the original identity everywhere.
        assert_eq!(
            catalog.admit("native-0".into()),
            Err(FailureKind::InvalidArgument)
        );
        other.admit("native-0".into()).unwrap();
        assert_eq!(
            registry
                .begin_handoff(&mut other, other_scope, "native-0")
                .unwrap_err(),
            FailureKind::InvalidArgument
        );
        assert_eq!(
            registry
                .begin_handoff(&mut other, other_scope, "global-overflow")
                .unwrap_err(),
            FailureKind::Busy
        );
        assert_eq!(registry.drain_completions().len(), MAX_PENDING_COMPLETIONS);
        catalog.admit("native-0".into()).unwrap();
        assert!(
            registry
                .begin_handoff(&mut other, other_scope, "global-overflow")
                .is_ok()
        );
    }

    #[test]
    fn partial_receipts_are_bounded_native_facts_without_external_payloads() {
        let mut catalog = OperationCatalog::default();
        let original = scope(&catalog, 1, PrivacyKind::Private);
        catalog.admit("native".into()).unwrap();
        let mut registry = NativeOperationRegistry::default();
        let reporter = registry
            .begin_handoff(&mut catalog, original, "native")
            .unwrap();
        assert!(!reporter.finish(NativeOperationOutcome::ClosedPartially {
            closed: 5,
            requested: 3
        }));
        assert!(!reporter.finish(NativeOperationOutcome::ClosedPartially {
            closed: 0,
            requested: 101
        }));
        assert!(reporter.finish(NativeOperationOutcome::ClosedPartially {
            closed: 1,
            requested: 3
        }));
        let receipt = registry.get("native").unwrap();
        assert_eq!(receipt.record.state, OperationState::Failed);
        assert_eq!(
            receipt.record.detail,
            "Closed 1 of 3 pages; remaining pages were kept"
        );
        assert!(receipt.record.stderr.is_none());
        assert_eq!(receipt.scope.privacy(), PrivacyKind::Private);
    }

    #[test]
    fn transient_retirement_ends_pending_authority_and_forgets_only_its_receipts() {
        let mut registry = NativeOperationRegistry::default();
        let mut source = OperationCatalog::default();
        let private = scope(&source, 1, PrivacyKind::Private);
        let normal = scope(&source, 1, PrivacyKind::Normal);
        let other = scope(&source, 2, PrivacyKind::Private);
        let mut reporters = Vec::new();
        for (id, scope) in [("private", private), ("normal", normal), ("other", other)] {
            source.admit(id.into()).unwrap();
            reporters.push(registry.begin_handoff(&mut source, scope, id).unwrap());
        }
        registry.retire_transient_profile(private.profile());
        assert_eq!(
            registry.get("private").unwrap().record.state,
            OperationState::Failed
        );
        assert_eq!(
            registry.get("normal").unwrap().record.state,
            OperationState::Running
        );
        assert_eq!(
            registry.get("other").unwrap().record.state,
            OperationState::Running
        );
        assert!(!reporters[0].finish(NativeOperationOutcome::Applied));
        registry.forget_transient_profile(private.profile());
        assert!(registry.get("private").is_none());
        assert!(registry.get("normal").is_some());
        assert!(registry.get("other").is_some());
        assert_eq!(registry.drain_completions().len(), 1);
    }

    #[test]
    fn terminal_and_lost_reporter_wakes_are_delivered_after_source_and_locks_are_gone() {
        let mut source = OperationCatalog::default();
        let owner = scope(&source, 1, PrivacyKind::Normal);
        source.admit("native".into()).unwrap();
        let mut registry = NativeOperationRegistry::default();
        let reporter = registry
            .begin_handoff(&mut source, owner, "native")
            .unwrap();
        let state = Arc::downgrade(registry.records.get("native").unwrap());
        let ledger = Arc::downgrade(&registry.ledger);
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = Arc::clone(&count);
        registry.install_waker(Arc::new(move || {
            // These reads would deadlock if the notification ran under locks.
            assert!(
                state
                    .upgrade()
                    .unwrap()
                    .receipt()
                    .unwrap()
                    .record
                    .state
                    .is_terminal()
            );
            assert_eq!(
                ledger.upgrade().unwrap().lock().unwrap().completions.len(),
                1
            );
            observed.fetch_add(1, Ordering::AcqRel);
        }));
        drop(source);
        drop(reporter);
        assert_eq!(count.load(Ordering::Acquire), 1);
        assert!(registry.completions_pending());
        assert_eq!(registry.drain_completions().len(), 1);
        assert!(!registry.completions_pending());
    }
}
