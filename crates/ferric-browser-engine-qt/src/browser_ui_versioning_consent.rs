//! Rust-held native volatile-work disclosure and one-use decision authority.

use super::{
    BrowserUiRust, CapturedOwner, NativeProfileOwner, OperationId, PendingNativeDisclosure,
    PendingNativePageCaptureDisclosure, RedactedPreview, RuntimeEffect, StashError,
    StashObservation, StorageTicket, VersioningEffects,
};
use ferric_browser_application::versioning::{PageCaptureAuthorization, PageCaptureFailurePolicy};
use ferric_browser_core::{
    ScopeSelector, VersioningFailure, VersioningReason, canonical_origin,
    versioning::{
        PageStateSidecar, SensitiveCategory, capture_fingerprint_for_request, page_route_for_url,
        route_fingerprint_for_url,
    },
};
use ferric_browser_storage::StorageOperationError;
use ferric_browser_storage::versioning::CaptureStageTarget;
use ferric_browser_storage::versioning::{
    CaptureInterval, CaptureMonotonicInterval, InstalledObject, ManifestMutationEvidence,
    ManifestPageCapture, ManifestPageCategory,
};
use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

/// Bounded challenge projected to the trusted QML page-state collector.
pub(super) struct PageCaptureSidecarRequest {
    pub expected_origin: String,
    pub expected_route: String,
    pub route_fingerprint: String,
    pub capture_fingerprint: String,
    pub nonce: String,
    pub include_editable: bool,
    pub include_selection: bool,
    pub include_media: bool,
}

/// Result of the consent-to-reservation seam. Native presentation effects
/// must be applied with the captured pre-mutation revision; a refused
/// reservation must apply its exact cancellation effects instead.
pub enum NativeStashReservation {
    Prepared {
        previous_revision: u64,
        runtime_effects: Vec<RuntimeEffect>,
    },
    Cancelled {
        previous_revision: u64,
        reason: StashError,
        effects: VersioningEffects,
    },
}

/// A refusal after structural preparation may still carry reducer effects
/// that the Qt adapter must checkpoint. The effects stay in Rust and are
/// deliberately redacted from diagnostics.
///
/// INVARIANT: after structural preparation succeeds, every later refusal
/// returns its exact effects to the Qt checkpoint/effect bridge.
#[must_use = "prepared runtime effects must reach the Qt checkpoint bridge"]
pub(super) struct NativePageCaptureDisclosureError {
    failure: VersioningFailure,
    prepared_runtime_effects: Option<(u64, Vec<RuntimeEffect>)>,
}

impl NativePageCaptureDisclosureError {
    fn before_structural_preparation(failure: VersioningFailure) -> Self {
        Self {
            failure,
            prepared_runtime_effects: None,
        }
    }

    fn after_structural_preparation(
        failure: VersioningFailure,
        previous_revision: u64,
        runtime_effects: Vec<RuntimeEffect>,
    ) -> Self {
        Self {
            failure,
            prepared_runtime_effects: Some((previous_revision, runtime_effects)),
        }
    }

    #[must_use]
    #[cfg(test)]
    pub(super) const fn reason(&self) -> VersioningReason {
        self.failure.reason()
    }

    #[must_use]
    pub(super) fn take_prepared_runtime_effects(&mut self) -> Option<(u64, Vec<RuntimeEffect>)> {
        self.prepared_runtime_effects.take()
    }
}

impl fmt::Debug for NativePageCaptureDisclosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativePageCaptureDisclosureError")
            .field("failure", &self.failure)
            .field(
                "prepared_runtime_effects",
                &self
                    .prepared_runtime_effects
                    .as_ref()
                    .map(|_| "[retained effects]"),
            )
            .finish()
    }
}

impl BrowserUiRust {
    /// Starts a page archive request only for the exact active tab owned by
    /// this native window. The fixed disclosure and its preview remain in
    /// Rust until the native consent callback consumes them.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal if ownership changed, another capture is
    /// active, or explicit disclosure cannot be prepared.
    pub(super) fn request_native_page_capture_disclosure(
        &mut self,
        editable: bool,
        selection: bool,
    ) -> Result<(RedactedPreview, u64, Vec<RuntimeEffect>), NativePageCaptureDisclosureError> {
        self.request_native_page_capture_disclosure_for_scope(editable, selection, None)
    }

    /// Prepares a native-window disclosure with the exact strict/partial
    /// choice bound before any page bytes are collected. No QML route exposes
    /// this until the indexed native callback path is qualified.
    pub(super) fn request_native_window_page_capture_disclosure(
        &mut self,
        editable: bool,
        selection: bool,
        failure_policy: PageCaptureFailurePolicy,
    ) -> Result<(RedactedPreview, u64, Vec<RuntimeEffect>), NativePageCaptureDisclosureError> {
        self.request_native_page_capture_disclosure_for_scope(
            editable,
            selection,
            Some(failure_policy),
        )
    }

    fn request_native_page_capture_disclosure_for_scope(
        &mut self,
        editable: bool,
        selection: bool,
        window_failure_policy: Option<PageCaptureFailurePolicy>,
    ) -> Result<(RedactedPreview, u64, Vec<RuntimeEffect>), NativePageCaptureDisclosureError> {
        if !self.native_stash_bridge.can_claim_start() {
            return Err(
                NativePageCaptureDisclosureError::before_structural_preparation(
                    page_capture_failure(VersioningReason::Busy, editable),
                ),
            );
        }
        let window = self.window.ok_or_else(|| {
            NativePageCaptureDisclosureError::before_structural_preparation(page_capture_failure(
                VersioningReason::StaleOwner,
                editable,
            ))
        })?;
        let (profile, instance) = self.native_versioning_profile.ok_or_else(|| {
            NativePageCaptureDisclosureError::before_structural_preparation(page_capture_failure(
                VersioningReason::StaleOwner,
                editable,
            ))
        })?;
        let active_tab = self.tab_for_index(self.active_tab_index);
        let application = self.state.as_mut().ok_or_else(|| {
            NativePageCaptureDisclosureError::before_structural_preparation(page_capture_failure(
                VersioningReason::StaleOwner,
                editable,
            ))
        })?;
        let owner = NativeProfileOwner::new(application.application_owner(), profile, instance);
        let tab = application
            .windows()
            .get(&window)
            .filter(|current| current.profile == profile)
            .and_then(|current| current.active_tab)
            .ok_or_else(|| {
                NativePageCaptureDisclosureError::before_structural_preparation(
                    page_capture_failure(VersioningReason::StaleOwner, editable),
                )
            })?;
        let target = application.capture_target(tab).ok_or_else(|| {
            NativePageCaptureDisclosureError::before_structural_preparation(page_capture_failure(
                VersioningReason::StaleOwner,
                editable,
            ))
        })?;
        if application.versioning().owner(profile) != Some(owner) || active_tab != Some(tab) {
            return Err(
                NativePageCaptureDisclosureError::before_structural_preparation(
                    page_capture_failure(VersioningReason::StaleOwner, editable),
                ),
            );
        }

        if self.native_stash_bridge.decision_sink.is_none() {
            self.native_stash_bridge.decision_sink =
                application.take_native_versioning_decision_sink();
        }
        if self.native_stash_bridge.decision_sink.is_none() {
            return Err(
                NativePageCaptureDisclosureError::before_structural_preparation(
                    page_capture_failure(VersioningReason::ConfirmationRequired, editable),
                ),
            );
        }

        let previous_revision = application.revision();
        let selector = if window_failure_policy.is_some() {
            ScopeSelector::Window { window }
        } else {
            ScopeSelector::Tab { window, target }
        };
        let mut prepared = application
            .prepare_structural_commit(selector, "Page archive".to_owned())
            .map_err(NativePageCaptureDisclosureError::before_structural_preparation)?;
        let current_target = match application.capture_target(tab) {
            Some(target) => target,
            None => {
                return Err(
                    NativePageCaptureDisclosureError::after_structural_preparation(
                        page_capture_failure(VersioningReason::StaleOwner, editable),
                        previous_revision,
                        prepared.runtime_effects,
                    ),
                );
            }
        };
        if current_target != target {
            return Err(
                NativePageCaptureDisclosureError::after_structural_preparation(
                    page_capture_failure(VersioningReason::DocumentChanged, editable),
                    previous_revision,
                    prepared.runtime_effects,
                ),
            );
        }
        let consent_selector = if window_failure_policy.is_some() {
            ScopeSelector::Window { window }
        } else {
            ScopeSelector::Tab {
                window,
                target: current_target,
            }
        };
        let consent = match application.prepare_page_capture_consent_with_failure_policy(
            consent_selector,
            editable,
            selection,
            window_failure_policy.unwrap_or(PageCaptureFailurePolicy::Strict),
        ) {
            Ok(consent) => consent,
            Err(failure) => {
                return Err(
                    NativePageCaptureDisclosureError::after_structural_preparation(
                        failure,
                        previous_revision,
                        prepared.runtime_effects,
                    ),
                );
            }
        };
        prepared.manifest.requested_capture.allow_partial_pages =
            window_failure_policy == Some(PageCaptureFailurePolicy::AllowPartialPages);
        let redacted = consent.preview.redacted();
        if !redacted.approvable || redacted.kind.page_capture_disclosure().is_none() {
            if let Ok(effects) = application.cancel_versioning_operation(consent.operation) {
                self.queue_native_operation_effects(effects.operation_effects);
            }
            return Err(
                NativePageCaptureDisclosureError::after_structural_preparation(
                    page_capture_failure(VersioningReason::ConfirmationRequired, editable),
                    previous_revision,
                    prepared.runtime_effects,
                ),
            );
        }
        self.native_stash_bridge.page_capture_disclosure =
            Some(PendingNativePageCaptureDisclosure {
                preview: consent.preview,
                manifest: prepared.manifest,
                editable,
                accept_attempted: false,
            });
        Ok((redacted, previous_revision, prepared.runtime_effects))
    }

    /// Returns only fixed application disclosure copy for the exact live
    /// native request; page-derived URL/title/content never reach QML.
    pub fn native_page_capture_disclosure_for_display(
        &mut self,
    ) -> Option<(OperationId, &'static str)> {
        self.clear_terminal_native_page_capture();
        let pending_operation = self
            .native_stash_bridge
            .page_capture_disclosure
            .as_ref()
            .map(|pending| pending.preview.operation());
        let projection = (|| {
            let application = self.state.as_ref()?;
            let pending = self.native_stash_bridge.page_capture_disclosure.as_ref()?;
            let operation = pending.preview.operation();
            let (envelope, state) = application.versioning().operation(operation)?;
            let (profile, instance) = self.native_versioning_profile?;
            let window = self.window?;
            let owner = NativeProfileOwner::new(application.application_owner(), profile, instance);
            let members = envelope.scope().members();
            if members.is_empty() {
                return None;
            }
            let active_tab = application
                .windows()
                .get(&window)
                .filter(|current| current.profile == profile)
                .and_then(|current| current.active_tab)?;
            if state != ferric_browser_application::versioning::OperationState::AwaitingConsent
                || envelope.kind()
                    != ferric_browser_application::versioning::OperationKind::Sensitive(
                        ferric_browser_application::versioning::SensitiveOperation::PageCapture,
                    )
                || envelope.owner() != owner
                || members.iter().any(|captured| {
                    captured.window() != window
                        || captured.native() != owner
                        || application.capture_target(captured.target().tab)
                            != Some(captured.target())
                })
                || application.versioning().owner(profile) != Some(owner)
                || !members
                    .iter()
                    .any(|captured| captured.target().tab == active_tab)
                || self.tab_for_index(self.active_tab_index) != Some(active_tab)
                || envelope.expected_logical_revision() != application.runtime_view().revision()
            {
                return None;
            }
            let redacted = pending.preview.redacted();
            if !redacted.approvable {
                return None;
            }
            Some((operation, redacted.kind.page_capture_disclosure()?))
        })();
        if projection.is_none() {
            if let Some(operation) = pending_operation {
                self.cancel_native_page_capture(operation);
            }
        }
        projection
    }

    /// Consumes the exact Rust-held archive preview on native acceptance.
    /// The resulting capability is retained in Rust and never crosses QML.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal if the preview expired, was already consumed,
    /// or the exact native operation cannot be authorized.
    pub fn accept_native_page_capture_disclosure(
        &mut self,
        operation: OperationId,
    ) -> Result<StorageTicket, VersioningFailure> {
        if self
            .native_page_capture_disclosure_for_display()
            .map(|(id, _)| id)
            != Some(operation)
        {
            return Err(page_capture_failure(
                VersioningReason::ConfirmationExpired,
                false,
            ));
        }
        let Some(pending) = self
            .native_stash_bridge
            .page_capture_disclosure
            .as_mut()
            .filter(|pending| pending.preview.operation() == operation)
        else {
            return Err(page_capture_failure(
                VersioningReason::ConfirmationExpired,
                false,
            ));
        };
        if pending.accept_attempted {
            return Err(page_capture_failure(
                VersioningReason::ConfirmationExpired,
                pending.editable,
            ));
        }
        pending.accept_attempted = true;
        let editable = pending.editable;
        let proof = self
            .native_stash_bridge
            .decision_sink
            .as_ref()
            .map(|sink| sink.accept(&pending.preview));
        let failure = page_capture_failure(VersioningReason::ConfirmationRequired, editable);
        let accepted = if let (Some(proof), Some(application)) = (proof, self.state.as_mut()) {
            application
                .capture_native_versioning_decision(operation, proof)
                .map_err(|_| failure)
                .and_then(|permit| {
                    application
                        .confirm_page_capture_consent(operation, permit)
                        .map_err(|failure| page_capture_failure(failure.reason(), editable))
                })
                .and_then(|authorization| {
                    application
                        .request_page_capture_stage(&authorization)
                        .map(|ticket| (authorization, ticket))
                        .map_err(|failure| page_capture_failure(failure.reason(), editable))
                })
        } else {
            Err(page_capture_failure(VersioningReason::StaleOwner, editable))
        };
        let (authorization, ticket) = match accepted {
            Ok(accepted) => accepted,
            Err(failure) => {
                let operation_effects = self
                    .state
                    .as_mut()
                    .and_then(|application| application.cancel_versioning_operation(operation).ok())
                    .map(|effects| effects.operation_effects);
                self.native_stash_bridge.page_capture_disclosure = None;
                if let Some(effects) = operation_effects {
                    self.queue_native_operation_effects(effects);
                }
                return Err(failure);
            }
        };
        let manifest = self
            .native_stash_bridge
            .page_capture_disclosure
            .take()
            .filter(|pending| pending.preview.operation() == operation)
            .map(|pending| pending.manifest)
            .ok_or_else(|| page_capture_failure(VersioningReason::ConfirmationExpired, editable))?;
        self.native_stash_bridge.page_capture_window_evidence =
            if authorization.scope().members().len() > 1 {
                vec![None; authorization.scope().members().len()]
            } else {
                Vec::new()
            };
        self.native_stash_bridge.page_capture_page_index = 0;
        self.native_stash_bridge.page_capture_authorization = Some(authorization);
        self.native_stash_bridge.page_capture_manifest = Some(manifest);
        self.native_stash_bridge.page_capture_stage_ticket = Some((operation, ticket));
        Ok(ticket)
    }

    /// A dismissal cancels only the operation for the preview being shown.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal if the exact pending disclosure is no longer
    /// current or its operation cannot be cancelled.
    pub fn decline_native_page_capture_disclosure(
        &mut self,
        operation: OperationId,
    ) -> Result<(), VersioningFailure> {
        if self
            .native_page_capture_disclosure_for_display()
            .map(|(id, _)| id)
            != Some(operation)
        {
            return Err(page_capture_failure(
                VersioningReason::ConfirmationExpired,
                false,
            ));
        }
        let effects = self
            .state
            .as_mut()
            .ok_or_else(|| page_capture_failure(VersioningReason::StaleOwner, false))?
            .cancel_versioning_operation(operation)
            .map_err(|_| page_capture_failure(VersioningReason::ConfirmationExpired, false))?;
        self.native_stash_bridge.page_capture_disclosure = None;
        self.queue_native_operation_effects(effects.operation_effects);
        Ok(())
    }

    /// Routes only the storage ticket created after native consent. The
    /// temporary path stays in Rust until the exact active tab claims the
    /// one-shot native save route.
    pub fn complete_native_page_capture_stage(
        &mut self,
        ticket: StorageTicket,
        result: Result<CaptureStageTarget, StorageOperationError>,
    ) -> bool {
        let Some((operation, expected_ticket)) = self.native_stash_bridge.page_capture_stage_ticket
        else {
            return false;
        };
        if ticket != expected_ticket {
            return false;
        }
        self.native_stash_bridge.page_capture_stage_ticket = None;
        match result {
            Ok(target)
                if target.operation() == operation
                    && self.page_capture_route_is_current(operation) =>
            {
                self.native_stash_bridge.page_capture_stage_target = Some(target);
                true
            }
            _ => {
                self.cancel_native_page_capture(operation);
                false
            }
        }
    }

    /// Projects the exact operation and captured tab, never its filesystem
    /// path, so trusted QML can locate the already-bound native view.
    pub fn pending_native_page_capture_save(
        &mut self,
    ) -> Option<(OperationId, ferric_browser_core::TabId)> {
        self.clear_terminal_native_page_capture();
        if self.native_stash_bridge.page_capture_save_started {
            return None;
        }
        let operation = self
            .native_stash_bridge
            .page_capture_stage_target
            .as_ref()?
            .operation();
        if !self.page_capture_route_is_current(operation) {
            return None;
        }
        let authorization = self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()?;
        let captured = authorization
            .scope()
            .members()
            .get(self.native_stash_bridge.page_capture_page_index)?;
        Some((operation, captured.target().tab))
    }
}

#[path = "browser_ui_versioning_consent/page_capture.rs"]
mod page_capture;

fn page_capture_failure(reason: VersioningReason, editable: bool) -> VersioningFailure {
    VersioningFailure::new(
        reason,
        if editable {
            SensitiveCategory::Editable
        } else {
            SensitiveCategory::Page
        },
    )
}

fn native_page_capture_unix_millis() -> Option<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
    .ok()
    .filter(|timestamp| *timestamp > 0)
}

#[cfg(test)]
mod disclosure_error_tests {
    use super::NativePageCaptureDisclosureError;
    use ferric_browser_core::{
        Event, ProfileId, VersioningFailure, VersioningReason, versioning::SensitiveCategory,
    };
    use ferric_browser_runtime::RuntimeEffect;

    #[test]
    fn refusal_retains_prepared_effects_once_and_redacts_them_from_debug() {
        let profile = ProfileId::from_display("profileid-41").expect("stable display identity");
        let mut error = NativePageCaptureDisclosureError::after_structural_preparation(
            VersioningFailure::new(VersioningReason::Busy, SensitiveCategory::Page),
            41,
            vec![RuntimeEffect::AppliedEvent(Event::CreateWindow { profile })],
        );

        let debug = format!("{error:?}");
        assert_eq!(error.reason(), VersioningReason::Busy);
        assert!(!debug.contains("CreateWindow"));
        assert!(!debug.contains(&profile.to_string()));

        let (previous_revision, effects) = error
            .take_prepared_runtime_effects()
            .expect("post-prepare refusal must return the checkpoint effects");
        assert_eq!(previous_revision, 41);
        assert_eq!(effects.len(), 1);
        assert!(error.take_prepared_runtime_effects().is_none());
    }
}
