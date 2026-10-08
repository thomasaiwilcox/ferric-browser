//! Thin presentation of application-owned versioning responses.

use super::{
    BrowserUiRust, CxxQtType, Pin, ProfileInstanceId, QString, QStringList, StorageTicket,
    VersioningEngineEffect, qobject,
};
use ferric_browser_application::{
    BrowserApplication,
    versioning::{
        NativeDecisionSink, OperationEffect, OperationKind, OperationState,
        PageCaptureAuthorization, Preview, RedactedPreview, SensitiveOperation, StashError,
        StashObservation, StashPreparation, StashStage, VersioningEffects,
    },
};
use ferric_browser_core::{
    CapturedOwner, CommitId, NativeProfileOwner, OperationId, RetainedViewId, TabId, Target,
    versioning::{SidecarSummary, limits::MAX_OPERATION_QUEUE},
};
use ferric_browser_runtime::RuntimeEffect;
use ferric_browser_storage::versioning::{
    CaptureInterval, CaptureMonotonicInterval, CaptureStageTarget, InstalledObject,
    ManifestMutationEvidence, ManifestPageCapture, VersionedCaptureManifest,
};
use std::time::Instant;

mod branch;
#[cfg(all(test, feature = "unqualified-clean-branch-test"))]
mod branch_native_tests;
#[path = "browser_ui_versioning_cleanup.rs"]
mod cleanup;
#[path = "browser_ui_versioning_consent.rs"]
mod consent;
#[path = "browser_ui_versioning_stage.rs"]
mod stage;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativePageCaptureDownloadFacts {
    download_id: String,
    observed_path: String,
    is_save_page: bool,
    is_mhtml: bool,
    view_matches: bool,
    mutation_evidence: String,
    observed_url: String,
    terminal_state: String,
    received_bytes: i64,
}

fn parse_native_page_capture_download_facts(json: &str) -> Option<NativePageCaptureDownloadFacts> {
    const MAX_FACTS_JSON_BYTES: usize = 16 * 1024;
    if json.len() > MAX_FACTS_JSON_BYTES {
        return None;
    }
    let facts = serde_json::from_str::<NativePageCaptureDownloadFacts>(json).ok()?;
    (facts.download_id.len() <= 512
        && facts.observed_path.len() <= 4096
        && facts.observed_url.len() <= 8192
        && facts.terminal_state.len() <= 32
        && matches!(
            facts.mutation_evidence.as_str(),
            "not-observed" | "observed-stable" | "observed-changed" | "document-changed"
        ))
    .then_some(facts)
}

#[cfg(test)]
mod native_page_capture_download_facts_tests {
    use super::parse_native_page_capture_download_facts;
    use serde_json::json;

    #[test]
    fn parses_the_exact_bounded_public_download_record() {
        let encoded = json!({
            "download_id": "download-1",
            "observed_path": "/private/stage/page.mhtml",
            "is_save_page": true,
            "is_mhtml": true,
            "view_matches": true,
            "mutation_evidence": "observed-stable",
            "observed_url": "https://example.test/page",
            "terminal_state": "completed",
            "received_bytes": 2048
        })
        .to_string();

        let facts = parse_native_page_capture_download_facts(&encoded)
            .expect("exact bounded download facts should parse");
        assert_eq!(facts.download_id, "download-1");
        assert_eq!(facts.received_bytes, 2048);
        assert!(facts.is_save_page && facts.is_mhtml && facts.view_matches);
        assert_eq!(facts.mutation_evidence, "observed-stable");
    }

    #[test]
    fn rejects_unknown_fields_oversized_values_and_oversized_records() {
        let mut record = json!({
            "download_id": "download-1",
            "observed_path": "/private/stage/page.mhtml",
            "is_save_page": true,
            "is_mhtml": true,
            "view_matches": true,
            "mutation_evidence": "observed-stable",
            "observed_url": "https://example.test/page",
            "terminal_state": "completed",
            "received_bytes": 2048
        });
        record["unexpected"] = json!("value");
        assert!(parse_native_page_capture_download_facts(&record.to_string()).is_none());

        record.as_object_mut().expect("object").remove("unexpected");
        record["mutation_evidence"] = json!("mutation-details-leaked");
        assert!(parse_native_page_capture_download_facts(&record.to_string()).is_none());
        record["mutation_evidence"] = json!("observed-stable");
        record["observed_url"] = json!("x".repeat(8193));
        assert!(parse_native_page_capture_download_facts(&record.to_string()).is_none());
        assert!(parse_native_page_capture_download_facts(&"x".repeat(16 * 1024 + 1)).is_none());
    }
}

/// The logical fallback is allocated, but no native view has been hidden.
/// It stays in Rust until the exact native pair is checked or the application
/// cancels the reservation.
struct PendingNativeStash {
    operation: OperationId,
    preparation: StashPreparation,
    materialized: bool,
    native_held: bool,
}

struct PendingNativeDisclosure {
    preview: Preview,
    accept_attempted: bool,
}

struct PendingNativePageCaptureDisclosure {
    preview: Preview,
    manifest: VersionedCaptureManifest,
    editable: bool,
    accept_attempted: bool,
}

#[derive(Clone)]
struct PageCaptureCollectorRequest {
    expected_origin: String,
    route_fingerprint: String,
    capture_fingerprint: String,
    nonce: String,
    started_at_ms: u64,
    include_media: bool,
}

/// One native-only callback authority. QML receives the view ID for matching
/// its `QObject`; the coordinator ticket never crosses this adapter boundary.
#[derive(Default)]
pub(super) struct NativeStashBridge {
    start: Option<(OperationId, Target)>,
    disclosure: Option<PendingNativeDisclosure>,
    page_capture_disclosure: Option<PendingNativePageCaptureDisclosure>,
    page_capture_authorization: Option<PageCaptureAuthorization>,
    page_capture_manifest: Option<VersionedCaptureManifest>,
    page_capture_stage_ticket: Option<(OperationId, StorageTicket)>,
    page_capture_stage_target: Option<CaptureStageTarget>,
    page_capture_page_index: usize,
    page_capture_window_evidence: Vec<Option<ManifestPageCapture>>,
    page_capture_save_started: bool,
    page_capture_collector_request: Option<PageCaptureCollectorRequest>,
    page_capture_sidecar_json: Option<Vec<u8>>,
    page_capture_sidecar_summary: Option<SidecarSummary>,
    page_capture_sidecar_interval: Option<CaptureInterval>,
    page_capture_monotonic_started_at: Option<Instant>,
    page_capture_sidecar_monotonic_interval: Option<CaptureMonotonicInterval>,
    page_capture_sidecar_install_ticket: Option<(OperationId, StorageTicket)>,
    page_capture_sidecar_installed_object: Option<InstalledObject>,
    page_capture_archive_started_at_ms: Option<u64>,
    page_capture_archive_finished_at_ms: Option<u64>,
    page_capture_archive_monotonic_started_offset_ms: Option<u64>,
    page_capture_archive_monotonic_interval: Option<CaptureMonotonicInterval>,
    page_capture_mutation_evidence: Option<ManifestMutationEvidence>,
    page_capture_download_id: Option<String>,
    page_capture_install_ticket: Option<(OperationId, StorageTicket)>,
    page_capture_publish_ticket: Option<(OperationId, StorageTicket)>,
    page_capture_installed_object: Option<InstalledObject>,
    decision_sink: Option<NativeDecisionSink>,
    pending: Option<PendingNativeStash>,
    unstaged_cleanup: Option<(OperationId, RetainedViewId)>,
    stage: Option<StashStage>,
    cancel_effects: Option<VersioningEffects>,
}

pub(super) enum NativeStashCommit {
    Committed(VersioningEffects),
    Refused(StashError),
    OutcomeUnknown(StashError),
}

impl NativeStashBridge {
    fn start(&self) -> Option<(OperationId, Target)> {
        self.start
    }

    fn can_claim_start(&self) -> bool {
        self.start.is_none()
            && self.disclosure.is_none()
            && self.page_capture_disclosure.is_none()
            && self.page_capture_authorization.is_none()
            && self.page_capture_manifest.is_none()
            && self.page_capture_stage_ticket.is_none()
            && self.page_capture_stage_target.is_none()
            && self.page_capture_collector_request.is_none()
            && self.page_capture_sidecar_json.is_none()
            && self.page_capture_sidecar_summary.is_none()
            && self.page_capture_sidecar_interval.is_none()
            && self.page_capture_monotonic_started_at.is_none()
            && self.page_capture_sidecar_monotonic_interval.is_none()
            && self.page_capture_sidecar_install_ticket.is_none()
            && self.page_capture_sidecar_installed_object.is_none()
            && self.page_capture_archive_started_at_ms.is_none()
            && self.page_capture_archive_finished_at_ms.is_none()
            && self
                .page_capture_archive_monotonic_started_offset_ms
                .is_none()
            && self.page_capture_archive_monotonic_interval.is_none()
            && self.page_capture_mutation_evidence.is_none()
            && self.page_capture_download_id.is_none()
            && self.page_capture_install_ticket.is_none()
            && self.page_capture_publish_ticket.is_none()
            && self.page_capture_installed_object.is_none()
            && self.pending.is_none()
            && self.unstaged_cleanup.is_none()
            && self.stage.is_none()
    }

    fn pending_disclosure_operation(&self) -> Option<OperationId> {
        self.disclosure
            .as_ref()
            .map(|pending| pending.preview.operation())
    }

    fn clear_terminal_disclosure(&mut self, application: &BrowserApplication) {
        if self
            .pending_disclosure_operation()
            .is_some_and(|operation| {
                application
                    .versioning()
                    .operation(operation)
                    .is_none_or(|(_, state)| state != OperationState::AwaitingConsent)
            })
        {
            self.disclosure = None;
        }
    }

    fn hold_preparation(
        &mut self,
        operation: OperationId,
        mut preparation: StashPreparation,
    ) -> Result<Vec<RuntimeEffect>, Box<StashPreparation>> {
        if self.start.is_some()
            || self.disclosure.is_some()
            || self.page_capture_disclosure.is_some()
            || self.page_capture_authorization.is_some()
            || self.page_capture_manifest.is_some()
            || self.page_capture_stage_ticket.is_some()
            || self.page_capture_stage_target.is_some()
            || self.page_capture_collector_request.is_some()
            || self.page_capture_sidecar_json.is_some()
            || self.page_capture_sidecar_summary.is_some()
            || self.page_capture_sidecar_interval.is_some()
            || self.page_capture_monotonic_started_at.is_some()
            || self.page_capture_sidecar_monotonic_interval.is_some()
            || self.page_capture_sidecar_install_ticket.is_some()
            || self.page_capture_sidecar_installed_object.is_some()
            || self.page_capture_archive_started_at_ms.is_some()
            || self.page_capture_archive_finished_at_ms.is_some()
            || self
                .page_capture_archive_monotonic_started_offset_ms
                .is_some()
            || self.page_capture_archive_monotonic_interval.is_some()
            || self.page_capture_mutation_evidence.is_some()
            || self.page_capture_download_id.is_some()
            || self.page_capture_install_ticket.is_some()
            || self.page_capture_publish_ticket.is_some()
            || self.page_capture_installed_object.is_some()
            || self.pending.is_some()
            || self.unstaged_cleanup.is_some()
            || self.stage.is_some()
        {
            return Err(Box::new(preparation));
        }
        let runtime_effects = preparation.take_runtime_effects();
        self.pending = Some(PendingNativeStash {
            operation,
            preparation,
            materialized: false,
            native_held: false,
        });
        Ok(runtime_effects)
    }

    fn pending_ids(&self) -> Option<(RetainedViewId, TabId, TabId)> {
        let pending = self
            .pending
            .as_ref()
            .filter(|pending| !pending.materialized)?;
        Some((
            pending.preparation.view(),
            pending.preparation.owner().target().tab,
            pending.preparation.fallback().tab,
        ))
    }

    fn mark_materialized(
        &mut self,
        application: &BrowserApplication,
        view: RetainedViewId,
    ) -> bool {
        let Some(pending) = self
            .pending
            .as_mut()
            .filter(|pending| !pending.materialized && pending.preparation.view() == view)
        else {
            return false;
        };
        let preparation = &pending.preparation;
        let owner = preparation.owner();
        let fallback = preparation.fallback();
        if !application
            .versioning()
            .operation(pending.operation)
            .is_some_and(|(_, state)| state == OperationState::Preparing)
            || application.capture_target(owner.target().tab) != Some(owner.target())
            || application.capture_target(fallback.tab) != Some(fallback)
            || application
                .windows()
                .get(&owner.window())
                .is_none_or(|window| window.active_tab != Some(fallback.tab))
            || !application.tabs().get(&fallback.tab).is_some_and(|tab| {
                tab.url.is_none() && tab.profile == owner.native().runtime_profile()
            })
        {
            return false;
        }
        pending.materialized = true;
        true
    }

    fn pending_operation(&self) -> Option<OperationId> {
        self.pending.as_ref().map(|pending| pending.operation)
    }

    /// A later fresh eligibility check may consume this exact reservation to
    /// issue the application stage ticket. No QML callback can take it.
    fn take_materialized_preparation(&mut self) -> Option<(OperationId, StashPreparation)> {
        if self.stage.is_some()
            || self.unstaged_cleanup.is_some()
            || !self.pending.as_ref()?.native_held
        {
            return None;
        }
        let pending = self.pending.take()?;
        Some((pending.operation, pending.preparation))
    }

    fn clear_terminal_preparation(
        &mut self,
        application: &BrowserApplication,
        cleanup_failed: &[OperationId],
        native_cleanup_pending: &[OperationId],
    ) {
        if self.pending.as_ref().is_some_and(|pending| {
            !pending.materialized
                && !cleanup_failed.contains(&pending.operation)
                && !native_cleanup_pending.contains(&pending.operation)
                && application
                    .versioning()
                    .operation(pending.operation)
                    .is_none_or(|(_, state)| {
                        matches!(
                            state,
                            OperationState::Completed
                                | OperationState::Cancelled
                                | OperationState::Failed
                                | OperationState::Partial
                                | OperationState::OutcomeUnknown
                        )
                    })
        }) {
            self.pending = None;
        }
    }

    pub(super) fn hold(&mut self, stage: StashStage) -> Result<(), Box<StashStage>> {
        if self.start.is_some()
            || self.disclosure.is_some()
            || self.page_capture_disclosure.is_some()
            || self.page_capture_authorization.is_some()
            || self.page_capture_manifest.is_some()
            || self.page_capture_stage_ticket.is_some()
            || self.page_capture_stage_target.is_some()
            || self.page_capture_collector_request.is_some()
            || self.page_capture_sidecar_json.is_some()
            || self.page_capture_sidecar_summary.is_some()
            || self.page_capture_sidecar_interval.is_some()
            || self.page_capture_monotonic_started_at.is_some()
            || self.page_capture_sidecar_monotonic_interval.is_some()
            || self.page_capture_sidecar_install_ticket.is_some()
            || self.page_capture_sidecar_installed_object.is_some()
            || self.page_capture_archive_started_at_ms.is_some()
            || self.page_capture_archive_finished_at_ms.is_some()
            || self
                .page_capture_archive_monotonic_started_offset_ms
                .is_some()
            || self.page_capture_archive_monotonic_interval.is_some()
            || self.page_capture_mutation_evidence.is_some()
            || self.page_capture_download_id.is_some()
            || self.page_capture_install_ticket.is_some()
            || self.page_capture_publish_ticket.is_some()
            || self.page_capture_installed_object.is_some()
            || self.stage.is_some()
            || self.unstaged_cleanup.is_some()
            || self.pending.is_some()
        {
            return Err(Box::new(stage));
        }
        self.stage = Some(stage);
        Ok(())
    }

    pub(super) fn view(&self) -> Option<RetainedViewId> {
        self.stage.as_ref().map(StashStage::view)
    }

    /// Cancellation/expiry never drops the exact stage ticket before the
    /// native host has restored and acknowledged its original view.
    pub(super) fn cleanup_view(&self, application: &BrowserApplication) -> Option<RetainedViewId> {
        let stage = self.stage.as_ref()?;
        match application.versioning().operation(stage.operation()) {
            Some((_, operation_state))
                if !matches!(
                    operation_state,
                    OperationState::Cancelled
                        | OperationState::Failed
                        | OperationState::Partial
                        | OperationState::OutcomeUnknown
                ) =>
            {
                None
            }
            Some(_) | None => Some(stage.view()),
        }
    }

    /// Invoke only from the host's observed-Frozen callback. A known refusal
    /// keeps the stage for actual native restoration; uncertain reducer
    /// rollback keeps native ownership and must not report safe refusal.
    pub(super) fn confirm(
        &mut self,
        application: &mut BrowserApplication,
        view: RetainedViewId,
    ) -> NativeStashCommit {
        let Some(stage) = self.stage.as_ref().filter(|stage| stage.view() == view) else {
            return NativeStashCommit::Refused(StashError::StalePreparation);
        };
        match application.confirm_explicit_stash_frozen(stage) {
            Ok(effects) => {
                self.stage = None;
                NativeStashCommit::Committed(effects)
            }
            Err(error @ StashError::RollbackFailed) => NativeStashCommit::OutcomeUnknown(error),
            Err(error) => NativeStashCommit::Refused(error),
        }
    }

    /// Invoke only after the host has observed the same view restored to
    /// `Active`. The pending ticket and any cancellation effects survive a
    /// failed logical cleanup so the adapter can retry without losing work.
    pub(super) fn acknowledge_restored(
        &mut self,
        application: &mut BrowserApplication,
        view: RetainedViewId,
    ) -> Result<VersioningEffects, StashError> {
        let stage = self.stage.as_ref().ok_or(StashError::UnknownOperation)?;
        if stage.view() != view {
            return Err(StashError::StalePreparation);
        }
        let operation = stage.operation();
        if self.cancel_effects.is_none() {
            let effects = match application.versioning().operation(operation) {
                Some((
                    _,
                    OperationState::Cancelled
                    | OperationState::Failed
                    | OperationState::Partial
                    | OperationState::OutcomeUnknown,
                ))
                | None => VersioningEffects::default(),
                Some(_) => application
                    .cancel_versioning_operation(operation)
                    .map_err(StashError::Operation)?,
            };
            if !effects.cleanup_failed.is_empty() {
                return Err(StashError::RollbackFailed);
            }
            self.cancel_effects = Some(effects);
        }
        let runtime_effects = application.acknowledge_explicit_stash_rollback(stage)?;
        let mut effects = self.cancel_effects.take().unwrap_or_default();
        effects.runtime_effects.extend(runtime_effects);
        effects
            .native_cleanup_pending
            .retain(|pending| *pending != operation);
        effects
            .operation_effects
            .retain(|effect| *effect != OperationEffect::Cleanup(stage.ticket()));
        self.stage = None;
        Ok(effects)
    }
}

mod capture;
mod restore;

impl BrowserUiRust {
    pub(super) fn tab_for_index(&self, index: i32) -> Option<TabId> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.tab_ids.get(index).copied())
    }

    fn native_stash_disclosure_for_display(&mut self) -> Option<(OperationId, &'static str)> {
        let application = self.state.as_ref()?;
        self.native_stash_bridge
            .clear_terminal_disclosure(application);
        let pending = self.native_stash_bridge.disclosure.as_ref()?;
        let operation = pending.preview.operation();
        let (envelope, state) = application.versioning().operation(operation)?;
        let (profile, instance) = self.native_versioning_profile?;
        let window = self.window?;
        let owner = NativeProfileOwner::new(application.application_owner(), profile, instance);
        if state != OperationState::AwaitingConsent
            || envelope.kind() != OperationKind::Sensitive(SensitiveOperation::Stash)
            || envelope.owner() != owner
            || !matches!(envelope.scope().members(), [captured]
                if captured.window() == window && captured.native() == owner)
            || application.versioning().owner(profile) != Some(owner)
            || application
                .windows()
                .get(&window)
                .is_none_or(|current| current.profile != profile)
        {
            return None;
        }
        let redacted = pending.preview.redacted();
        if !redacted.approvable || !redacted.volatile_work_risk {
            return None;
        }
        Some((operation, redacted.kind.volatile_disclosure()?))
    }

    fn exact_requested_stash_target(
        application: &BrowserApplication,
        owner: NativeProfileOwner,
        window: ferric_browser_core::WindowId,
        operation: OperationId,
    ) -> Option<Target> {
        let (envelope, state) = application.versioning().operation(operation)?;
        if state != OperationState::Requested
            || envelope.kind() != OperationKind::Sensitive(SensitiveOperation::Stash)
            || envelope.owner() != owner
        {
            return None;
        }
        let [captured] = envelope.scope().members() else {
            return None;
        };
        (captured.window() == window && captured.native() == owner).then_some(captured.target())
    }

    /// Claims one exact admitted stash start. A full wakeup buffer is
    /// recovered from bounded coordinator state rather than losing authority.
    /// The claim is retained until a later native source check advances or
    /// fails the same operation; it does not authorize preparation or freeze.
    pub fn claim_native_stash_start(&mut self) -> Option<(OperationId, Target)> {
        let application = Option::as_ref(&self.state)?;
        let window = self.window?;
        let (profile, instance) = self.native_versioning_profile?;
        let owner = application.versioning().owner(profile)?;
        if owner != NativeProfileOwner::new(application.application_owner(), profile, instance)
            || application
                .windows()
                .get(&window)
                .is_none_or(|current| current.profile != profile)
        {
            return None;
        }
        if let Some((operation, target)) = self.native_stash_bridge.start() {
            if Self::exact_requested_stash_target(application, owner, window, operation)
                == Some(target)
            {
                return Some((operation, target));
            }
            self.native_stash_bridge.start = None;
        }
        if !self.native_stash_bridge.can_claim_start() {
            return None;
        }
        let mut selected = None;
        for _ in 0..self.pending_versioning_operation_effects.len() {
            let effect = self.pending_versioning_operation_effects.pop_front()?;
            match effect {
                OperationEffect::Start(operation) => {
                    if selected.is_none() {
                        if let Some(target) = Self::exact_requested_stash_target(
                            application,
                            owner,
                            window,
                            operation,
                        ) {
                            selected = Some((operation, target));
                            continue;
                        }
                    }
                    if application
                        .versioning()
                        .operation(operation)
                        .is_some_and(|(_, state)| state == OperationState::Requested)
                    {
                        self.pending_versioning_operation_effects
                            .push_back(OperationEffect::Start(operation));
                    }
                }
                cleanup @ OperationEffect::Cleanup(_) => {
                    self.pending_versioning_operation_effects.push_back(cleanup);
                }
            }
        }
        if selected.is_none() && self.pending_versioning_rescan_required {
            selected = application
                .versioning()
                .requested_operations_for_owner(owner)
                .into_iter()
                .find_map(|operation| {
                    Self::exact_requested_stash_target(application, owner, window, operation)
                        .map(|target| (operation, target))
                });
        }
        self.native_stash_bridge.start = selected;
        selected
    }

    /// Completes the start wakeup only after a trusted native source check.
    /// It advances protocol state but does not claim categorical eligibility.
    pub fn advance_native_stash_start(&mut self, operation: OperationId, target: Target) -> bool {
        if self.native_stash_bridge.start() != Some((operation, target)) {
            return false;
        }
        let Some(application) = Option::as_mut(&mut self.state) else {
            return false;
        };
        if application.capture_target(target.tab) != Some(target)
            || application
                .advance_versioning_operation(operation, OperationState::Validating)
                .is_err()
        {
            return false;
        }
        self.native_stash_bridge.start = None;
        true
    }

    /// Refuses an exact start when the native source is missing, stale or
    /// unqualified. The returned effects must be presented by the UI caller.
    ///
    /// # Errors
    /// Refuses an unrelated ID or a coordinator transition that is no longer
    /// eligible for fail-before-mutation cleanup.
    pub fn reject_native_stash_start(
        &mut self,
        operation: OperationId,
    ) -> Result<VersioningEffects, ferric_browser_application::versioning::OperationError> {
        if self.native_stash_bridge.start().map(|(id, _)| id) != Some(operation) {
            return Err(ferric_browser_application::versioning::OperationError::UnknownOperation);
        }
        let application = Option::as_mut(&mut self.state)
            .ok_or(ferric_browser_application::versioning::OperationError::UnknownOperation)?;
        let effects = application.fail_versioning_without_mutation(operation)?;
        self.native_stash_bridge.start = None;
        Ok(effects)
    }

    fn queue_native_operation_effects(&mut self, effects: Vec<OperationEffect>) -> bool {
        for effect in effects {
            if self.pending_versioning_operation_effects.len() < MAX_OPERATION_QUEUE {
                self.pending_versioning_operation_effects.push_back(effect);
            } else {
                // Coordinator state remains authoritative; a later scheduler
                // must rescan it instead of assuming a dropped wakeup ran.
                self.pending_versioning_rescan_required = true;
            }
        }
        self.pending_versioning_rescan_required
    }

    /// Holds an admitted logical reservation while the caller applies its
    /// returned runtime effects. QML may then verify the actual fallback,
    /// but no native view is hidden and no freeze ticket exists yet.
    ///
    /// # Errors
    /// Returns the untouched preparation for a stale owner, wrong window,
    /// wrong operation state, or occupied native bridge.
    pub fn register_native_stash_preparation(
        &mut self,
        operation: OperationId,
        preparation: StashPreparation,
    ) -> Result<Vec<RuntimeEffect>, Box<StashPreparation>> {
        let owner = preparation.owner();
        let profile = owner.native().runtime_profile();
        let valid = self.window == Some(owner.window())
            && self
                .native_versioning_profile
                .is_some_and(|(bound, instance)| {
                    Option::as_ref(&self.state).is_some_and(|application| {
                        bound == profile
                            && owner.native()
                                == NativeProfileOwner::new(
                                    application.application_owner(),
                                    bound,
                                    instance,
                                )
                    })
                })
            && Option::as_ref(&self.state).is_some_and(|application| {
                application.versioning().owner(profile) == Some(owner.native())
                    && application
                        .versioning()
                        .operation(operation)
                        .is_some_and(|(_, state)| state == OperationState::Preparing)
            });
        if !valid {
            return Err(Box::new(preparation));
        }
        self.native_stash_bridge
            .hold_preparation(operation, preparation)
    }

    /// The trusted adapter may install a ticket only after admitted consent,
    /// fallback materialization and successful native host preparation.
    /// The ticket stays in Rust and cannot be constructed by QML or IPC.
    ///
    /// # Errors
    /// Returns the uninstalled ticket if another native stage is still owned.
    pub fn register_native_stash_stage(
        &mut self,
        stage: StashStage,
    ) -> Result<(), Box<StashStage>> {
        self.native_stash_bridge.hold(stage)
    }
    fn bind_native_versioning_profile_owner(&mut self) -> bool {
        let Some(application) = Option::as_mut(&mut self.state) else {
            return false;
        };
        let Some(profile) = self
            .window
            .and_then(|window| application.windows().get(&window))
            .map(|window| window.profile)
        else {
            return false;
        };
        if let Some((bound_profile, _)) = self.native_versioning_profile {
            return bound_profile == profile;
        }
        if application.versioning().owner(profile).is_some() {
            return false;
        }
        let instance = ProfileInstanceId::fresh();
        match application.bind_native_profile(profile, instance) {
            Ok(effects) if effects.is_empty() => {
                self.native_versioning_profile = Some((profile, instance));
                true
            }
            Ok(_) | Err(_) => false,
        }
    }
}

fn stash_refusal_text(error: StashError) -> &'static str {
    match error {
        StashError::FallbackChanged => "Live stash refused: fallback changed",
        StashError::Expired => "Live stash refused: request expired",
        StashError::OwnerChanged => "Live stash refused: native owner changed",
        StashError::StaleObservation | StashError::StalePreparation => {
            "Live stash refused: source changed"
        }
        _ => "Live stash refused; restoring original view",
    }
}

fn stash_unknown_text(_error: StashError) -> &'static str {
    "Live stash outcome uncertain; native view remains retained"
}

pub(super) fn versioning_status_text(effect: &VersioningEngineEffect) -> String {
    match effect {
        VersioningEngineEffect::Status {
            production_enabled,
            active_operations,
        } => format!(
            "Versioning: {}; active operations: {active_operations}",
            if *production_enabled {
                "structural commits available; other features may be unavailable"
            } else {
                "unavailable"
            }
        ),
        VersioningEngineEffect::Request(_) => {
            "Versioning request was not admitted by the application".to_owned()
        }
        VersioningEngineEffect::CommitQueued { .. } => {
            "Versioned structural commit pending".to_owned()
        }
        VersioningEngineEffect::BranchQueued { .. } => {
            "Clean branch fork pending; source window remains active".to_owned()
        }
        VersioningEngineEffect::SwitchResolved { .. } => {
            "Live branch window selected; activation requested".to_owned()
        }
        VersioningEngineEffect::CommitListQueued { .. } => {
            "Versioned commit listing pending".to_owned()
        }
        VersioningEngineEffect::CommitInfoQueued { .. } => {
            "Versioned commit info pending".to_owned()
        }
        VersioningEngineEffect::CommitDeletePreviewQueued { .. } => {
            "Versioned commit deletion preview pending".to_owned()
        }
        VersioningEngineEffect::PolicyRestorePreviewQueued { .. } => {
            "Versioned configuration policy preview pending".to_owned()
        }
        VersioningEngineEffect::DiffQueued { .. } => "Versioned structural diff pending".to_owned(),
        VersioningEngineEffect::RestoreQueued { .. } => "Versioned restore plan pending".to_owned(),
    }
}

#[cfg(test)]
mod status_privacy_tests {
    use super::{stash_refusal_text, stash_unknown_text, versioning_status_text};
    use ferric_browser_application::versioning::{
        ActivityKind, OperationError, PreviewError, StashBlocker, StashError,
        VersioningConsentError,
    };
    use ferric_browser_core::{CommitId, OperationId, ScopeError, VersioningEngineEffect};

    #[test]
    fn commit_pending_status_does_not_expose_operation_or_commit_identity() {
        let operation = OperationId::fresh();
        let commit = CommitId::fresh();
        let status =
            versioning_status_text(&VersioningEngineEffect::CommitQueued { operation, commit });

        assert_eq!(status, "Versioned structural commit pending");
        assert!(!status.contains(&operation.to_string()));
        assert!(!status.contains(&commit.to_string()));
    }

    #[test]
    fn every_stash_error_projects_to_fixed_content_free_status() {
        let errors = [
            StashError::UnknownOperation,
            StashError::WrongKind,
            StashError::WrongScope,
            StashError::WrongStage,
            StashError::Expired,
            StashError::OwnerChanged,
            StashError::ConfigurationChanged,
            StashError::StaleObservation,
            StashError::Scope(ScopeError::ContextUnavailable),
            StashError::NotEligible(StashBlocker::ActivityNotClear(ActivityKind::LiveCall)),
            StashError::Operation(OperationError::QueueFull),
            StashError::Preview(PreviewError::StalePlan),
            StashError::Consent(VersioningConsentError::Preview(PreviewError::Expired)),
            StashError::InvalidAuthorization,
            StashError::DuplicatePreparation,
            StashError::FallbackAllocationFailed,
            StashError::RetentionRejected,
            StashError::RollbackFailed,
            StashError::StalePreparation,
            StashError::FallbackChanged,
            StashError::FallbackNotPresented,
        ];

        for error in errors {
            let expected = match error {
                StashError::FallbackChanged => "Live stash refused: fallback changed",
                StashError::Expired => "Live stash refused: request expired",
                StashError::OwnerChanged => "Live stash refused: native owner changed",
                StashError::StaleObservation | StashError::StalePreparation => {
                    "Live stash refused: source changed"
                }
                _ => "Live stash refused; restoring original view",
            };
            let refusal = stash_refusal_text(error);
            assert_eq!(refusal, expected);
            let diagnostic = format!("{error:?}");
            assert!(!refusal.contains(diagnostic.as_str()));
            assert_eq!(
                stash_unknown_text(error),
                "Live stash outcome uncertain; native view remains retained"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_application::versioning::{
        Activity, EditableState, OperationKind, SensitiveOperation, StashObservation,
    };
    use ferric_browser_core::{
        Event, NativeProfileOwner, OperationId, ScopeSelector, Target, ValidatedUrl,
        VersioningReason, ViewMembership,
    };
    use ferric_browser_runtime::RuntimeInput;

    fn modeled_authorized_preparation(
        adapter: &mut BrowserUiRust,
    ) -> (OperationId, StashPreparation, StashObservation) {
        assert!(adapter.bind_native_versioning_profile_owner());
        let window = adapter.window.unwrap();
        let tab = adapter.tab.unwrap();
        let application = Option::as_mut(&mut adapter.state).unwrap();
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
        let observation = StashObservation {
            target,
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
        };
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
        let prepared = application
            .prepare_explicit_stash_fallback(operation, observation)
            .unwrap();
        (operation, prepared, observation)
    }

    fn modeled_native_stage(adapter: &mut BrowserUiRust) -> (RetainedViewId, OperationId, Target) {
        let (operation, mut prepared, observation) = modeled_authorized_preparation(adapter);
        let view = prepared.view();
        let fallback = prepared.fallback();
        assert!(!prepared.take_runtime_effects().is_empty());
        let application = Option::as_mut(&mut adapter.state).unwrap();
        let stage = application
            .begin_explicit_stash_freeze(operation, prepared, observation)
            .unwrap();
        assert!(adapter.native_stash_bridge.hold(stage).is_ok());
        (view, operation, fallback)
    }

    #[test]
    fn native_profile_binding_is_owned_once_by_the_initialized_adapter() {
        let mut adapter = BrowserUiRust::default();
        assert!(adapter.bind_native_versioning_profile_owner());
        let (profile, instance) = adapter.native_versioning_profile.unwrap();
        assert_eq!(
            adapter.state.as_ref().unwrap().versioning().owner(profile),
            Some(NativeProfileOwner::new(
                adapter.state.as_ref().unwrap().application_owner(),
                profile,
                instance
            ))
        );
        assert!(adapter.bind_native_versioning_profile_owner());
        assert_eq!(adapter.native_versioning_profile, Some((profile, instance)));
    }

    #[test]
    fn logical_reservation_waits_for_exact_native_fallback_without_hiding_source() {
        let mut adapter = BrowserUiRust::default();
        let (operation, prepared, _) = modeled_authorized_preparation(&mut adapter);
        let view = prepared.view();
        let source = prepared.owner().target().tab;
        let fallback = prepared.fallback().tab;
        let effects = adapter
            .register_native_stash_preparation(operation, prepared)
            .ok()
            .unwrap();
        assert!(!effects.is_empty());
        assert_eq!(
            adapter.native_stash_bridge.pending_ids(),
            Some((view, source, fallback))
        );
        let application = Option::as_mut(&mut adapter.state).unwrap();
        assert!(
            !adapter
                .native_stash_bridge
                .mark_materialized(application, RetainedViewId::fresh())
        );
        assert!(
            adapter
                .native_stash_bridge
                .mark_materialized(application, view)
        );
        assert!(
            !adapter
                .native_stash_bridge
                .mark_materialized(application, view)
        );
        assert_eq!(adapter.native_stash_bridge.pending_ids(), None);
        assert_eq!(
            application.versioning_retention(view).unwrap().membership(),
            ViewMembership::Visible
        );
        assert_eq!(adapter.native_stash_bridge.view(), None);
        let aborted = application
            .abort_explicit_stash_preparation(operation)
            .unwrap();
        assert!(aborted.cleanup_failed.is_empty());
        adapter.native_stash_bridge.clear_terminal_preparation(
            application,
            &aborted.cleanup_failed,
            &aborted.native_cleanup_pending,
        );
        assert_eq!(
            adapter.native_stash_bridge.pending_operation(),
            Some(operation)
        );
        assert_eq!(
            adapter
                .native_stash_bridge
                .pending_cleanup_view(application),
            Some(view)
        );
        assert!(
            adapter
                .native_stash_bridge
                .acknowledge_held_restored(application, view)
                .unwrap()
                .is_empty()
        );
        assert_eq!(adapter.native_stash_bridge.pending_operation(), None);
    }

    #[test]
    fn changed_logical_fallback_refuses_materialization_without_native_stage() {
        let mut adapter = BrowserUiRust::default();
        let (operation, prepared, _) = modeled_authorized_preparation(&mut adapter);
        let view = prepared.view();
        let fallback = prepared.fallback();
        adapter
            .register_native_stash_preparation(operation, prepared)
            .ok()
            .unwrap();
        let application = Option::as_mut(&mut adapter.state).unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: fallback,
                url: ValidatedUrl::parse("https://example.test/changed").unwrap(),
            }))
            .unwrap();
        assert!(
            !adapter
                .native_stash_bridge
                .mark_materialized(application, view)
        );
        assert_eq!(adapter.native_stash_bridge.view(), None);
        let aborted = application
            .abort_explicit_stash_preparation(operation)
            .unwrap();
        assert!(aborted.cleanup_failed.is_empty());
        adapter.native_stash_bridge.clear_terminal_preparation(
            application,
            &aborted.cleanup_failed,
            &aborted.native_cleanup_pending,
        );
        assert_eq!(adapter.native_stash_bridge.pending_operation(), None);
        assert!(application.versioning_retention(view).is_none());
        assert!(application.tabs().contains_key(&fallback.tab));
    }

    #[test]
    fn materialized_reservation_holds_exact_ticket_or_cancels_on_fresh_refusal() {
        let mut adapter = BrowserUiRust::default();
        let (operation, prepared, observation) = modeled_authorized_preparation(&mut adapter);
        let view = prepared.view();
        adapter
            .register_native_stash_preparation(operation, prepared)
            .ok()
            .unwrap();
        assert!(
            adapter
                .native_stash_bridge
                .take_materialized_preparation()
                .is_none()
        );
        {
            let application = Option::as_mut(&mut adapter.state).unwrap();
            assert!(
                adapter
                    .native_stash_bridge
                    .mark_materialized(application, view)
            );
        }
        assert!(matches!(
            adapter.begin_materialized_native_stash_freeze(operation, observation),
            Err(StashError::WrongStage)
        ));
        assert!(
            !adapter
                .native_stash_bridge
                .mark_native_hold(RetainedViewId::fresh())
        );
        assert!(adapter.native_stash_bridge.mark_native_hold(view));
        assert!(!adapter.native_stash_bridge.mark_native_hold(view));
        assert!(matches!(
            adapter.begin_materialized_native_stash_freeze(OperationId::fresh(), observation),
            Err(StashError::WrongStage)
        ));
        assert_eq!(
            adapter.native_stash_bridge.pending_operation(),
            Some(operation)
        );
        let route = adapter
            .begin_materialized_native_stash_freeze(operation, observation)
            .unwrap();
        let stage::NativeStashFreezeStart::Ready {
            view: ready_view,
            source,
            fallback,
        } = route
        else {
            panic!("qualified materialized fallback must yield a native route");
        };
        assert_eq!(ready_view, view);
        assert_eq!(source, observation.target.tab);
        assert_ne!(fallback, source);
        assert_eq!(adapter.native_stash_bridge.view(), Some(view));
        assert_eq!(adapter.native_stash_bridge.pending_operation(), None);
        assert_eq!(
            adapter
                .state
                .as_ref()
                .unwrap()
                .versioning_retention(view)
                .unwrap()
                .membership(),
            ViewMembership::Visible
        );

        let mut refusing = BrowserUiRust::default();
        let (operation, prepared, observation) = modeled_authorized_preparation(&mut refusing);
        let view = prepared.view();
        refusing
            .register_native_stash_preparation(operation, prepared)
            .ok()
            .unwrap();
        assert!(
            refusing
                .native_stash_bridge
                .mark_materialized(refusing.state.as_ref().unwrap(), view)
        );
        assert!(refusing.native_stash_bridge.mark_native_hold(view));
        let blocked = StashObservation {
            live_call: Activity::Active,
            ..observation
        };
        let route = refusing
            .begin_materialized_native_stash_freeze(operation, blocked)
            .unwrap();
        let stage::NativeStashFreezeStart::Cancelled {
            view: cancelled_view,
            reason,
            effects,
            ..
        } = route
        else {
            panic!("new live activity must cancel before native freeze");
        };
        assert_eq!(cancelled_view, view);
        assert_eq!(
            reason,
            StashError::NotEligible(
                ferric_browser_application::versioning::StashBlocker::ActivityNotClear(
                    ferric_browser_application::versioning::ActivityKind::LiveCall
                )
            )
        );
        assert!(effects.native_cleanup_pending.is_empty());
        assert!(effects.cleanup_failed.is_empty());
        assert_eq!(refusing.native_stash_bridge.view(), None);
        assert_eq!(refusing.native_stash_bridge.pending_operation(), None);
        assert!(!refusing.native_stash_bridge.can_claim_start());
        assert_eq!(
            refusing
                .native_stash_bridge
                .pending_cleanup_view(refusing.state.as_ref().unwrap()),
            Some(view)
        );
        assert!(matches!(
            refusing.native_stash_bridge.acknowledge_held_restored(
                refusing.state.as_mut().unwrap(),
                RetainedViewId::fresh()
            ),
            Err(StashError::StalePreparation)
        ));
        assert!(
            refusing
                .native_stash_bridge
                .acknowledge_held_restored(refusing.state.as_mut().unwrap(), view)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            refusing
                .native_stash_bridge
                .pending_cleanup_view(refusing.state.as_ref().unwrap()),
            None
        );
        assert!(refusing.native_stash_bridge.can_claim_start());
        assert_eq!(
            refusing
                .state
                .as_ref()
                .unwrap()
                .versioning()
                .operation(operation)
                .unwrap()
                .1,
            OperationState::Cancelled
        );
    }

    #[test]
    fn terminal_materialized_reservation_keeps_native_hold_until_restored() {
        let mut adapter = BrowserUiRust::default();
        let (operation, prepared, _) = modeled_authorized_preparation(&mut adapter);
        let view = prepared.view();
        adapter
            .register_native_stash_preparation(operation, prepared)
            .ok()
            .unwrap();
        let application = Option::as_mut(&mut adapter.state).unwrap();
        assert!(
            adapter
                .native_stash_bridge
                .mark_materialized(application, view)
        );
        assert!(matches!(
            adapter
                .native_stash_bridge
                .acknowledge_held_restored(application, view),
            Err(StashError::WrongStage)
        ));
        let cancelled = application.cancel_versioning_operation(operation).unwrap();
        assert!(cancelled.cleanup_failed.is_empty());
        adapter.native_stash_bridge.clear_terminal_preparation(
            application,
            &cancelled.cleanup_failed,
            &cancelled.native_cleanup_pending,
        );
        assert_eq!(
            adapter.native_stash_bridge.pending_operation(),
            Some(operation)
        );
        assert_eq!(
            adapter
                .native_stash_bridge
                .pending_cleanup_view(application),
            Some(view)
        );
        assert!(matches!(
            adapter
                .native_stash_bridge
                .acknowledge_held_restored(application, RetainedViewId::fresh()),
            Err(StashError::StalePreparation)
        ));
        assert_eq!(
            adapter.native_stash_bridge.pending_operation(),
            Some(operation)
        );
        let effects = adapter
            .native_stash_bridge
            .acknowledge_held_restored(application, view)
            .unwrap();
        assert!(effects.is_empty());
        assert_eq!(adapter.native_stash_bridge.pending_operation(), None);
        assert!(application.versioning_retention(view).is_none());
    }

    #[test]
    fn native_stage_holder_rejects_wrong_view_and_commits_exact_modeled_callback() {
        let mut adapter = BrowserUiRust::default();
        let (view, operation, fallback) = modeled_native_stage(&mut adapter);
        assert_eq!(adapter.native_stash_bridge.view(), Some(view));
        let application = Option::as_mut(&mut adapter.state).unwrap();
        assert!(matches!(
            adapter
                .native_stash_bridge
                .confirm(application, RetainedViewId::fresh()),
            NativeStashCommit::Refused(StashError::StalePreparation)
        ));
        assert_eq!(adapter.native_stash_bridge.view(), Some(view));
        assert!(matches!(
            adapter.native_stash_bridge.confirm(application, view),
            NativeStashCommit::Committed(_)
        ));
        assert_eq!(adapter.native_stash_bridge.view(), None);
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Completed
        );
        assert_eq!(
            application.versioning_retention(view).unwrap().membership(),
            ViewMembership::Retained
        );
        assert_eq!(
            application.windows()[&adapter.window.unwrap()].active_tab,
            Some(fallback.tab)
        );
    }

    #[test]
    fn refused_modeled_callback_keeps_ticket_until_trusted_restoration_ack() {
        let mut adapter = BrowserUiRust::default();
        let (view, operation, fallback) = modeled_native_stage(&mut adapter);
        let application = Option::as_mut(&mut adapter.state).unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: fallback,
                url: ValidatedUrl::parse("https://example.test/changed").unwrap(),
            }))
            .unwrap();
        assert!(matches!(
            adapter.native_stash_bridge.confirm(application, view),
            NativeStashCommit::Refused(StashError::FallbackChanged)
        ));
        assert_eq!(adapter.native_stash_bridge.view(), Some(view));
        assert!(matches!(
            adapter
                .native_stash_bridge
                .acknowledge_restored(application, RetainedViewId::fresh()),
            Err(StashError::StalePreparation)
        ));
        assert!(application.versioning_retention(view).is_some());
        let effects = adapter
            .native_stash_bridge
            .acknowledge_restored(application, view)
            .unwrap();
        assert!(effects.native_cleanup_pending.is_empty());
        assert!(adapter.native_stash_bridge.view().is_none());
        assert!(application.versioning_retention(view).is_none());
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Cancelled
        );
        assert_eq!(
            application.tabs()[&fallback.tab].existence,
            ferric_browser_core::ExistenceState::Live
        );
    }

    #[test]
    fn adapter_operation_wakeup_buffer_is_bounded_and_requests_rescan() {
        let mut adapter = BrowserUiRust::default();
        let wakeups = (0..=MAX_OPERATION_QUEUE)
            .map(|_| OperationEffect::Start(OperationId::fresh()))
            .collect();
        assert!(adapter.queue_native_operation_effects(wakeups));
        assert_eq!(
            adapter.pending_versioning_operation_effects.len(),
            MAX_OPERATION_QUEUE
        );
        assert!(adapter.pending_versioning_rescan_required);
    }

    #[test]
    fn admitted_stash_start_is_claimed_once_for_exact_source_and_advanced() {
        let mut adapter = BrowserUiRust::default();
        assert!(adapter.bind_native_versioning_profile_owner());
        let window = adapter.window.unwrap();
        let tab = adapter.tab.unwrap();
        let application = Option::as_mut(&mut adapter.state).unwrap();
        let target = application.capture_target(tab).unwrap();
        let (operation, effects) = application
            .request_versioning_operation(
                ScopeSelector::Tab { window, target },
                OperationKind::Sensitive(SensitiveOperation::Stash),
                120_000,
            )
            .unwrap();
        assert!(!adapter.queue_native_operation_effects(effects));
        assert_eq!(
            adapter.claim_native_stash_start(),
            Some((operation, target))
        );
        assert_eq!(
            adapter.claim_native_stash_start(),
            Some((operation, target))
        );
        assert!(!adapter.advance_native_stash_start(
            operation,
            Target {
                generation: target.generation.saturating_add(1),
                ..target
            }
        ));
        assert!(adapter.advance_native_stash_start(operation, target));
        assert_eq!(adapter.claim_native_stash_start(), None);
        assert_eq!(
            Option::as_ref(&adapter.state)
                .unwrap()
                .versioning()
                .operation(operation)
                .unwrap()
                .1,
            OperationState::Validating
        );
    }

    #[test]
    fn rebound_native_profile_cannot_be_claimed_by_old_adapter() {
        let mut adapter = BrowserUiRust::default();
        assert!(adapter.bind_native_versioning_profile_owner());
        let (profile, _) = adapter.native_versioning_profile.unwrap();
        let window = adapter.window.unwrap();
        let tab = adapter.tab.unwrap();
        let application = Option::as_mut(&mut adapter.state).unwrap();
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .unwrap();
        let target = application.capture_target(tab).unwrap();
        let (_, effects) = application
            .request_versioning_operation(
                ScopeSelector::Tab { window, target },
                OperationKind::Sensitive(SensitiveOperation::Stash),
                120_000,
            )
            .unwrap();
        assert!(!adapter.queue_native_operation_effects(effects));
        assert_eq!(adapter.claim_native_stash_start(), None);
    }

    #[test]
    fn native_disclosure_holds_one_exact_preview_without_exporting_a_permit() {
        let mut adapter = BrowserUiRust::default();
        assert!(adapter.bind_native_versioning_profile_owner());
        let window = adapter.window.unwrap();
        let tab = adapter.tab.unwrap();
        let application = Option::as_mut(&mut adapter.state).unwrap();
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
        let observation = StashObservation {
            target,
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
        };
        let redacted = adapter
            .register_native_stash_disclosure(operation, observation)
            .unwrap();
        assert_eq!(
            redacted.kind,
            ferric_browser_application::versioning::PreviewKind::StashClean
        );
        assert!(redacted.volatile_work_risk);
        assert!(redacted.approvable);
        assert_eq!(
            adapter.native_stash_disclosure_for_display(),
            Some((
                operation,
                "This live stash is volatile. Closing the browser or a renderer crash may lose it. Nothing is saved to disk."
            ))
        );
        assert_eq!(
            adapter.native_stash_bridge.pending_disclosure_operation(),
            Some(operation)
        );
        assert!(
            adapter
                .register_native_stash_disclosure(operation, observation)
                .is_err()
        );
        assert_eq!(
            Option::as_ref(&adapter.state)
                .unwrap()
                .versioning()
                .operation(operation)
                .unwrap()
                .1,
            OperationState::AwaitingConsent
        );
        assert!(
            adapter
                .accept_native_stash_disclosure(OperationId::fresh(), observation)
                .is_err()
        );
        assert_eq!(
            adapter.native_stash_bridge.pending_disclosure_operation(),
            Some(operation)
        );
        let authorized = adapter
            .accept_native_stash_disclosure(operation, observation)
            .unwrap();
        assert_eq!(authorized.target(), target);
        assert_eq!(
            adapter.native_stash_bridge.pending_disclosure_operation(),
            None
        );
        assert_eq!(adapter.native_stash_disclosure_for_display(), None);
        assert!(
            adapter
                .accept_native_stash_disclosure(operation, observation)
                .is_err()
        );
        assert_eq!(
            Option::as_ref(&adapter.state)
                .unwrap()
                .versioning()
                .operation(operation)
                .unwrap()
                .1,
            OperationState::Preparing
        );
        let application = Option::as_mut(&mut adapter.state).unwrap();
        application.cancel_versioning_operation(operation).unwrap();
        let (second, _) = application
            .request_versioning_operation(
                ScopeSelector::Tab { window, target },
                OperationKind::Sensitive(SensitiveOperation::Stash),
                120_000,
            )
            .unwrap();
        application
            .advance_versioning_operation(second, OperationState::Validating)
            .unwrap();
        adapter
            .register_native_stash_disclosure(second, observation)
            .unwrap();
        assert!(adapter.decline_native_stash_disclosure(operation).is_err());
        assert_eq!(
            adapter.native_stash_disclosure_for_display().unwrap().0,
            second
        );
        assert_eq!(
            adapter.native_stash_bridge.pending_disclosure_operation(),
            Some(second)
        );
        adapter.decline_native_stash_disclosure(second).unwrap();
        assert_eq!(
            adapter.native_stash_bridge.pending_disclosure_operation(),
            None
        );
        assert_eq!(adapter.native_stash_disclosure_for_display(), None);
        assert_eq!(
            Option::as_ref(&adapter.state)
                .unwrap()
                .versioning()
                .operation(second)
                .unwrap()
                .1,
            OperationState::Cancelled
        );
        let application = Option::as_mut(&mut adapter.state).unwrap();
        let (third, _) = application
            .request_versioning_operation(
                ScopeSelector::Tab { window, target },
                OperationKind::Sensitive(SensitiveOperation::Stash),
                120_000,
            )
            .unwrap();
        application
            .advance_versioning_operation(third, OperationState::Validating)
            .unwrap();
        adapter
            .register_native_stash_disclosure(third, observation)
            .unwrap();
        let stale = StashObservation {
            target: Target {
                generation: target.generation.saturating_add(1),
                ..target
            },
            ..observation
        };
        assert!(
            adapter
                .accept_native_stash_disclosure(third, stale)
                .is_err()
        );
        assert_eq!(
            adapter.native_stash_bridge.pending_disclosure_operation(),
            Some(third)
        );
        assert!(
            adapter
                .accept_native_stash_disclosure(third, observation)
                .is_err()
        );
        // A failed one-use acceptance still leaves its exact disclosure
        // dismissible; it never retargets to the currently focused tab.
        assert_eq!(
            adapter.native_stash_disclosure_for_display().unwrap().0,
            third
        );
        adapter.decline_native_stash_disclosure(third).unwrap();
        assert_eq!(
            Option::as_ref(&adapter.state)
                .unwrap()
                .versioning()
                .operation(third)
                .unwrap()
                .1,
            OperationState::Cancelled
        );
    }

    #[test]
    fn native_disclosure_does_not_move_to_another_window_in_the_same_profile() {
        let mut adapter = BrowserUiRust::default();
        assert!(adapter.bind_native_versioning_profile_owner());
        let source_window = adapter.window.unwrap();
        let tab = adapter.tab.unwrap();
        let application = Option::as_mut(&mut adapter.state).unwrap();
        let profile = application.windows()[&source_window].profile;
        let target = application.capture_target(tab).unwrap();
        let (operation, _) = application
            .request_versioning_operation(
                ScopeSelector::Tab {
                    window: source_window,
                    target,
                },
                OperationKind::Sensitive(SensitiveOperation::Stash),
                120_000,
            )
            .unwrap();
        application
            .advance_versioning_operation(operation, OperationState::Validating)
            .unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .unwrap();
        let other_window = *application
            .windows()
            .keys()
            .find(|&&window| window != source_window)
            .unwrap();
        let observation = StashObservation {
            target,
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
        };

        adapter.window = Some(other_window);
        assert!(matches!(
            adapter.register_native_stash_disclosure(operation, observation),
            Err(StashError::OwnerChanged)
        ));
        assert_eq!(
            adapter.native_stash_bridge.pending_disclosure_operation(),
            None
        );

        adapter.window = Some(source_window);
        adapter
            .register_native_stash_disclosure(operation, observation)
            .unwrap();
        adapter.window = Some(other_window);
        assert_eq!(adapter.native_stash_disclosure_for_display(), None);
        assert!(
            adapter
                .accept_native_stash_disclosure(operation, observation)
                .is_err()
        );
        assert_eq!(
            adapter.native_stash_bridge.pending_disclosure_operation(),
            Some(operation)
        );
        adapter.window = Some(source_window);
        assert_eq!(
            adapter.native_stash_disclosure_for_display().unwrap().0,
            operation
        );
        assert_eq!(
            adapter
                .accept_native_stash_disclosure(operation, observation)
                .unwrap()
                .target(),
            target
        );
    }

    #[test]
    fn page_capture_disclosure_fails_closed_without_a_durable_active_profile() {
        let mut adapter = BrowserUiRust::default();
        assert!(adapter.bind_native_versioning_profile_owner());
        let tab = adapter.tab.unwrap();
        adapter.tab_ids = vec![tab];
        adapter.active_tab_index = 0;

        let failure = adapter
            .request_native_page_capture_disclosure(false, false)
            .expect_err("transient default runtime cannot persist a page archive");
        assert_eq!(failure.reason(), VersioningReason::WrongProfile);
        assert_eq!(adapter.native_page_capture_disclosure_for_display(), None);
        assert!(!adapter.has_native_page_capture_request());
    }

    #[test]
    fn native_consent_reserves_fallback_or_cancels_without_hiding_source() {
        fn validating_disclosure(adapter: &mut BrowserUiRust) -> (OperationId, StashObservation) {
            assert!(adapter.bind_native_versioning_profile_owner());
            let window = adapter.window.unwrap();
            let target = adapter
                .state
                .as_ref()
                .unwrap()
                .capture_target(adapter.tab.unwrap())
                .unwrap();
            let application = Option::as_mut(&mut adapter.state).unwrap();
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
            let observation = StashObservation {
                target,
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
            };
            adapter
                .register_native_stash_disclosure(operation, observation)
                .unwrap();
            (operation, observation)
        }

        let mut adapter = BrowserUiRust::default();
        let (operation, observation) = validating_disclosure(&mut adapter);
        let before = adapter.state.as_ref().unwrap().revision();
        let outcome = adapter
            .accept_and_prepare_native_stash_disclosure(operation, observation)
            .unwrap();
        let consent::NativeStashReservation::Prepared {
            previous_revision,
            runtime_effects,
        } = outcome
        else {
            panic!("safe consent should reserve a fallback");
        };
        assert_eq!(previous_revision, before);
        assert!(!runtime_effects.is_empty());
        let (view, source, fallback) = adapter.native_stash_bridge.pending_ids().unwrap();
        assert_eq!(source, observation.target.tab);
        assert_ne!(fallback, source);
        let application = adapter.state.as_ref().unwrap();
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Preparing
        );
        assert_eq!(
            application.versioning_retention(view).unwrap().membership(),
            ViewMembership::Visible
        );
        assert_eq!(adapter.native_stash_bridge.view(), None);

        let mut refusing = BrowserUiRust::default();
        let (operation, observation) = validating_disclosure(&mut refusing);
        refusing.native_stash_bridge.start = Some((OperationId::fresh(), observation.target));
        let outcome = refusing
            .accept_and_prepare_native_stash_disclosure(operation, observation)
            .unwrap();
        let consent::NativeStashReservation::Cancelled {
            reason, effects, ..
        } = outcome
        else {
            panic!("occupied bridge must cancel the logical reservation");
        };
        assert_eq!(reason, StashError::StalePreparation);
        assert!(effects.cleanup_failed.is_empty());
        assert!(effects.native_cleanup_pending.is_empty());
        assert_eq!(refusing.native_stash_bridge.pending_ids(), None);
        let application = refusing.state.as_ref().unwrap();
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            OperationState::Cancelled
        );
        assert!(
            application
                .visible_versioning_tabs_for_window(refusing.window.unwrap())
                .unwrap()
                .contains(&observation.target.tab)
        );
    }

    #[test]
    fn lost_start_wakeup_rescans_coordinator_but_stale_source_is_not_advanced() {
        let mut adapter = BrowserUiRust::default();
        assert!(adapter.bind_native_versioning_profile_owner());
        let window = adapter.window.unwrap();
        let tab = adapter.tab.unwrap();
        let application = Option::as_mut(&mut adapter.state).unwrap();
        let target = application.capture_target(tab).unwrap();
        let (operation, _) = application
            .request_versioning_operation(
                ScopeSelector::Tab { window, target },
                OperationKind::Sensitive(SensitiveOperation::Stash),
                120_000,
            )
            .unwrap();
        adapter.pending_versioning_rescan_required = true;
        assert_eq!(
            adapter.claim_native_stash_start(),
            Some((operation, target))
        );
        let application = Option::as_mut(&mut adapter.state).unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target,
                url: ValidatedUrl::parse("https://example.test/stale").unwrap(),
            }))
            .unwrap();
        assert!(!adapter.advance_native_stash_start(operation, target));
        assert_eq!(
            adapter.claim_native_stash_start(),
            Some((operation, target))
        );
        let effects = adapter.reject_native_stash_start(operation).unwrap();
        assert!(effects.runtime_effects.is_empty());
        assert!(effects.cleanup_failed.is_empty());
        assert_eq!(adapter.claim_native_stash_start(), None);
        assert_eq!(
            Option::as_ref(&adapter.state)
                .unwrap()
                .versioning()
                .operation(operation)
                .unwrap()
                .1,
            OperationState::Failed
        );
    }

    #[test]
    fn cancelled_stage_requests_exact_native_restoration_until_acknowledged() {
        let mut adapter = BrowserUiRust::default();
        let (view, operation, _) = modeled_native_stage(&mut adapter);
        let application = Option::as_mut(&mut adapter.state).unwrap();
        assert_eq!(adapter.native_stash_bridge.cleanup_view(application), None);
        let effects = application.cancel_versioning_operation(operation).unwrap();
        assert_eq!(effects.native_cleanup_pending, [operation]);
        assert_eq!(
            adapter.native_stash_bridge.cleanup_view(application),
            Some(view)
        );
        assert_eq!(
            adapter.native_stash_bridge.cleanup_view(application),
            Some(view)
        );
        adapter
            .native_stash_bridge
            .acknowledge_restored(application, view)
            .unwrap();
        assert_eq!(adapter.native_stash_bridge.cleanup_view(application), None);
    }
}
