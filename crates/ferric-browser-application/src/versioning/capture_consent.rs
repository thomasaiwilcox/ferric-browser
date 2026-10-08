//! One-use authorization for explicitly requested sensitive page capture.
//!
//! This protocol creates no archive bytes and has no persistence path. The
//! archive job must consume the returned exact scope in a later stage.

use std::{
    collections::BTreeSet,
    fmt,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ferric_browser_config::SensitiveCapturePolicy;
use ferric_browser_core::{
    CapturedScope, LogicalPageId, LogicalWindowId, OperationId, ScopeSelector, VersioningFailure,
    VersioningReason,
    versioning::{
        PAGE_STATE_HELPER_VERSION, PageStateSidecar, SensitiveCategory, SidecarCategory,
        SidecarSummary, limits::MAX_ARCHIVE_BYTES,
    },
};
use ferric_browser_storage::{
    StorageCommand, StorageCompletion, StorageOperation, StorageOperationError,
    versioning::{
        ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1, CaptureCategory, CaptureInterval, InstalledObject,
        MAX_CAPTURE_RESERVATION_BYTES, ManifestCategoryCoverage, ManifestCoverage,
        ManifestMutationEvidence, ManifestObjectRef, ManifestObjectRole,
        ManifestPageArchiveFailure, ManifestPageCapture, ManifestPageCategory, ManifestScope,
        VersionedCaptureManifest, VersioningObjectCommand,
    },
};
use uuid::Uuid;

use super::{
    AuthorizedPreview, CallbackDisposition, ConfirmationPermit, OperationEffect, OperationKind,
    OperationState, PageCaptureFailurePolicy, Preview, PreviewDraft, PreviewError,
    SensitiveOperation, StageResult, VersioningConsentError,
};
use crate::{BrowserApplication, StorageEffect, StorageTicket};

const PAGE_CAPTURE_OPERATION_TIMEOUT_MS: u64 = 6 * 60 * 1_000;
const PAGE_CAPTURE_PUBLISH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(60);

/// Exact operation and redacted preview to show in trusted native UI.
pub struct PageCaptureConsent {
    pub operation: OperationId,
    pub effects: Vec<OperationEffect>,
    pub preview: Preview,
}

/// Non-secret identity of the page-state helper and browser build that
/// produced one sidecar. Kept together so publication cannot misassociate
/// the helper and engine provenance fields.
#[derive(Clone, Copy)]
pub struct PageCaptureProvenance<'a> {
    pub helper_bundle_version: Option<&'a str>,
    pub engine_fingerprint: Option<&'a str>,
}

impl<'a> PageCaptureProvenance<'a> {
    #[must_use]
    pub const fn new(
        helper_bundle_version: Option<&'a str>,
        engine_fingerprint: Option<&'a str>,
    ) -> Self {
        Self {
            helper_bundle_version,
            engine_fingerprint,
        }
    }
}

impl fmt::Debug for PageCaptureConsent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PageCaptureConsent")
            .field("operation", &self.operation)
            .field("effects", &self.effects)
            .field("preview", &self.preview)
            .finish()
    }
}

/// Non-serializable authority for one accepted page-capture operation. It
/// exposes the frozen scope only to the application capture pipeline.
pub struct PageCaptureAuthorization {
    operation: OperationId,
    scope: CapturedScope,
    // A tab can be removed while a partial window capture is still advancing.
    // Preserve the consent-time identity so publication cannot rebind a
    // failed or already-archived page to whichever tab is live afterward.
    logical_bindings: Option<(LogicalWindowId, Box<[LogicalPageId]>)>,
    editable: bool,
    selection: bool,
    media: bool,
    failure_policy: PageCaptureFailurePolicy,
}

impl fmt::Debug for PageCaptureAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PageCaptureAuthorization")
            .field("operation", &self.operation)
            .field("editable", &self.editable)
            .field("selection", &self.selection)
            .field("media", &self.media)
            .field("failure_policy", &self.failure_policy)
            .field("scope", &"[captured scope]")
            .finish_non_exhaustive()
    }
}

impl PageCaptureAuthorization {
    #[must_use]
    pub const fn operation(&self) -> OperationId {
        self.operation
    }

    #[must_use]
    pub const fn scope(&self) -> &CapturedScope {
        &self.scope
    }

    #[must_use]
    pub const fn editable(&self) -> bool {
        self.editable
    }

    #[must_use]
    pub const fn selection(&self) -> bool {
        self.selection
    }

    #[must_use]
    pub const fn media(&self) -> bool {
        self.media
    }

    #[must_use]
    pub const fn failure_policy(&self) -> PageCaptureFailurePolicy {
        self.failure_policy
    }
}

impl BrowserApplication {
    /// Admits one native-requested page capture and prepares its fixed-copy
    /// consent preview. No page content, URL or title enters the preview.
    ///
    /// # Errors
    /// Refuses private/ephemeral owners, policy denial, unsupported context
    /// scope, busy owners, oversized captures, and stale runtime state.
    pub fn prepare_page_capture_consent(
        &mut self,
        selector: ScopeSelector,
        editable: bool,
    ) -> Result<PageCaptureConsent, VersioningFailure> {
        self.prepare_page_capture_consent_with_options(selector, editable, false)
    }

    /// Admits the same one-use page capture request with separately opted-in
    /// editable data and selection-position metadata.
    ///
    /// # Errors
    /// Refuses transient owners, policy denial, unsupported scopes, busy operations,
    /// oversized preview sets, and stale runtime state.
    pub fn prepare_page_capture_consent_with_options(
        &mut self,
        selector: ScopeSelector,
        editable: bool,
        selection: bool,
    ) -> Result<PageCaptureConsent, VersioningFailure> {
        self.prepare_page_capture_consent_with_failure_policy(
            selector,
            editable,
            selection,
            PageCaptureFailurePolicy::Strict,
        )
    }

    /// A partial window capture needs distinct, one-use disclosure. Tab and
    /// context captures cannot acquire this policy.
    ///
    /// # Errors
    /// Refuses unsupported scopes, one-page partial requests, policy denial,
    /// stale owners, busy operations and excessive preview sets.
    pub fn prepare_page_capture_consent_with_failure_policy(
        &mut self,
        selector: ScopeSelector,
        editable: bool,
        selection: bool,
        failure_policy: PageCaptureFailurePolicy,
    ) -> Result<PageCaptureConsent, VersioningFailure> {
        if failure_policy == PageCaptureFailurePolicy::AllowPartialPages
            && !matches!(selector, ScopeSelector::Window { .. })
        {
            return Err(capture_failure(
                VersioningReason::UnsupportedCapability,
                capture_category(editable),
            ));
        }
        let window = match selector {
            ScopeSelector::Tab { window, .. } | ScopeSelector::Window { window } => window,
            ScopeSelector::Context { .. } => {
                return Err(capture_failure(
                    VersioningReason::UnsupportedCapability,
                    capture_category(editable),
                ));
            }
        };
        let (_, native_owner) = self.structural_commit_owner(window)?;
        if self.configuration().versioning.page_capture == SensitiveCapturePolicy::Deny {
            return Err(capture_failure(
                VersioningReason::SensitiveCaptureDenied,
                SensitiveCategory::Page,
            ));
        }
        if editable
            && self.configuration().versioning.editable_capture == SensitiveCapturePolicy::Deny
        {
            return Err(capture_failure(
                VersioningReason::SensitiveCaptureDenied,
                SensitiveCategory::Editable,
            ));
        }

        let (operation, effects) = self
            .request_versioning_operation(
                selector,
                OperationKind::Sensitive(SensitiveOperation::PageCapture),
                PAGE_CAPTURE_OPERATION_TIMEOUT_MS,
            )
            .map_err(|error| {
                capture_failure(
                    match error {
                        super::OperationError::QueueFull => VersioningReason::Busy,
                        super::OperationError::Scope(scope) => scope.reason(),
                        _ => VersioningReason::StaleOwner,
                    },
                    capture_category(editable),
                )
            })?;
        if effects.as_slice() != [OperationEffect::Start(operation)] {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::Busy,
                capture_category(editable),
            ));
        }
        if self
            .versioning
            .operation(operation)
            .is_none_or(|(envelope, state)| {
                envelope.owner() != native_owner || state != OperationState::Requested
            })
        {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::StaleOwner,
                capture_category(editable),
            ));
        }
        let advanced = self
            .advance_versioning_operation(operation, OperationState::Validating)
            .and_then(|()| {
                self.advance_versioning_operation(operation, OperationState::AwaitingConsent)
            });
        if advanced.is_err() {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::StaleOwner,
                capture_category(editable),
            ));
        }

        let preview =
            self.prepare_page_capture_preview(operation, editable, selection, failure_policy)?;
        Ok(PageCaptureConsent {
            operation,
            effects,
            preview,
        })
    }

    fn prepare_page_capture_preview(
        &mut self,
        operation: OperationId,
        editable: bool,
        selection: bool,
        failure_policy: PageCaptureFailurePolicy,
    ) -> Result<Preview, VersioningFailure> {
        let category = capture_category(editable);
        let page_count = self
            .versioning
            .operation(operation)
            .and_then(|(envelope, _)| u16::try_from(envelope.scope().members().len()).ok())
            .filter(|count| *count > 0);
        let Some(page_count) = page_count else {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(VersioningReason::QuotaExceeded, category));
        };
        let Ok(draft) = PreviewDraft::page_capture_with_failure_policy(
            page_count,
            editable,
            selection,
            failure_policy,
        ) else {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                if failure_policy == PageCaptureFailurePolicy::AllowPartialPages && page_count < 2 {
                    VersioningReason::UnsupportedCapability
                } else {
                    VersioningReason::QuotaExceeded
                },
                category,
            ));
        };
        let Ok(preview) = self.prepare_versioning_preview(operation, draft) else {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::ConfirmationRequired,
                category,
            ));
        };
        Ok(preview)
    }

    /// Consumes one native decision for the exact preview and scope. The
    /// application revision check invalidates consent after any scope-changing
    /// reducer event, even if focus later returns to the same tab.
    ///
    /// # Errors
    /// Refuses cancellation, policy/configuration changes, scope changes,
    /// owner replacement, expiry and replay.
    pub fn confirm_page_capture_consent(
        &mut self,
        operation: OperationId,
        permit: ConfirmationPermit,
    ) -> Result<PageCaptureAuthorization, VersioningFailure> {
        let Some((envelope, state)) = self.versioning.operation(operation) else {
            return Err(capture_failure(
                VersioningReason::ConfirmationExpired,
                SensitiveCategory::Page,
            ));
        };
        let scope = envelope.scope().clone();
        let expected_revision = envelope.expected_logical_revision();
        let owner = envelope.owner();
        let kind = envelope.kind();
        if kind != OperationKind::Sensitive(SensitiveOperation::PageCapture)
            || state != OperationState::AwaitingConsent
            || scope.members().is_empty()
        {
            return Err(capture_failure(
                VersioningReason::ConfirmationRequired,
                SensitiveCategory::Page,
            ));
        }
        if self.runtime_view().revision() != expected_revision {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::DocumentChanged,
                SensitiveCategory::Page,
            ));
        }
        for member in scope.members() {
            let (_, current_owner) = self.structural_commit_owner(member.window())?;
            if current_owner != owner || member.native() != owner {
                let _ = self.cancel_versioning_operation(operation);
                return Err(capture_failure(
                    VersioningReason::StaleOwner,
                    SensitiveCategory::Page,
                ));
            }
        }
        if self.configuration().versioning.page_capture == SensitiveCapturePolicy::Deny {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::SensitiveCaptureDenied,
                SensitiveCategory::Page,
            ));
        }
        let logical_bindings =
            self.runtime_view()
                .logical_window(scope.window())
                .and_then(|window| {
                    scope
                        .members()
                        .iter()
                        .map(|member| self.runtime_view().logical_page(member.target().tab))
                        .collect::<Option<Vec<_>>>()
                        .map(|pages| (window, pages.into_boxed_slice()))
                });
        let authorization = match self.consent_versioning_operation(operation, permit) {
            Ok(AuthorizedPreview::PageCapture {
                page_count,
                editable,
                selection,
                failure_policy,
            }) if usize::from(page_count) == scope.members().len() => {
                PageCaptureAuthorization {
                    operation,
                    scope,
                    logical_bindings,
                    editable,
                    selection,
                    // Media position/paused metadata is part of the explicit
                    // page-capture disclosure, but never resumes playback.
                    media: true,
                    failure_policy,
                }
            }
            Ok(_) => {
                let _ = self.cancel_versioning_operation(operation);
                return Err(capture_failure(
                    VersioningReason::ConfirmationRequired,
                    SensitiveCategory::Page,
                ));
            }
            Err(error) => {
                let _ = self.cancel_versioning_operation(operation);
                return Err(capture_failure(
                    consent_failure_reason(error),
                    SensitiveCategory::Page,
                ));
            }
        };
        if self.configuration().versioning.page_capture == SensitiveCapturePolicy::Deny
            || (authorization.editable()
                && self.configuration().versioning.editable_capture == SensitiveCapturePolicy::Deny)
        {
            let category = capture_category(authorization.editable());
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::SensitiveCaptureDenied,
                category,
            ));
        }
        Ok(authorization)
    }

    /// Reserves the archive budget first, then creates the exact exclusive
    /// staging file on the file-only worker. The returned ticket completes
    /// only after both bounded steps have succeeded.
    ///
    /// # Errors
    /// Rejects changed authorization, non-durable owners, unsupported scope,
    /// policy denial, a full worker queue or failed profile recovery.
    pub fn request_page_capture_stage(
        &mut self,
        authorization: &PageCaptureAuthorization,
    ) -> Result<StorageTicket, VersioningFailure> {
        let (state_root, profile_uuid) = self.validate_page_capture_authorization(authorization)?;
        let page_count = authorization.scope().members().len();
        if page_count == 0 {
            return Err(capture_failure(
                VersioningReason::UnsupportedCapability,
                capture_category(authorization.editable()),
            ));
        }
        let reserved_bytes = MAX_CAPTURE_RESERVATION_BYTES
            .checked_mul(u64::try_from(page_count).map_err(|_| {
                capture_failure(
                    VersioningReason::QuotaExceeded,
                    capture_category(authorization.editable()),
                )
            })?)
            .ok_or_else(|| {
                capture_failure(
                    VersioningReason::QuotaExceeded,
                    capture_category(authorization.editable()),
                )
            })?;
        let operation = authorization.operation();
        if self.page_capture_job_is_active(operation) {
            return Err(capture_failure(
                VersioningReason::Busy,
                capture_category(authorization.editable()),
            ));
        }
        let Ok(ticket) = self.storage.submit(StorageCommand::VersioningBegin {
            operation,
            profile_uuid,
            reserved_bytes,
            state_root: Some(state_root.clone()),
        }) else {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::Busy,
                capture_category(authorization.editable()),
            ));
        };
        self.pending_page_capture_reservations.insert(
            ticket,
            (
                operation,
                profile_uuid,
                state_root,
                authorization.editable(),
            ),
        );
        self.page_capture_failure_policies
            .insert(operation, authorization.failure_policy());
        let Ok(stage_ticket) = self.versioning.begin_stage(operation) else {
            self.request_page_capture_reservation_release(operation);
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::StaleOwner,
                capture_category(authorization.editable()),
            ));
        };
        self.page_capture_stage_tickets
            .insert(operation, stage_ticket);
        Ok(ticket)
    }

    /// Prepares an exclusive target for one later member of the same
    /// consented window. The preceding page must have relinquished its
    /// target before another native save may be staged.
    ///
    /// # Errors
    /// Refuses a non-member, replaced document, overlapping page job, or
    /// missing durable window reservation.
    pub fn request_window_page_capture_stage(
        &mut self,
        authorization: &PageCaptureAuthorization,
        page_index: usize,
    ) -> Result<StorageTicket, VersioningFailure> {
        let (state_root, profile_uuid) = self.validate_page_capture_authorization(authorization)?;
        let operation = authorization.operation();
        let category = capture_category(authorization.editable());
        let page_index_u16 = u16::try_from(page_index)
            .map_err(|_| capture_failure(VersioningReason::UnsupportedCapability, category))?;
        if authorization.scope().members().len() < 2
            || page_index == 0
            || !self.page_capture_member_is_current(authorization, page_index)
            || !self.page_capture_reservations_held.contains_key(&operation)
            || self
                .page_capture_installed_archives
                .get(&operation)
                .is_some_and(|objects| objects.contains_key(&page_index))
            || self
                .page_capture_archive_failures
                .get(&operation)
                .is_some_and(|failures| failures.contains_key(&page_index))
            || (0..page_index).any(|preceding| {
                !self
                    .page_capture_installed_archives
                    .get(&operation)
                    .is_some_and(|objects| objects.contains_key(&preceding))
                    && !self
                        .page_capture_archive_failures
                        .get(&operation)
                        .is_some_and(|failures| failures.contains_key(&preceding))
            })
            || self.ready_page_capture_stages.contains_key(&operation)
            || self
                .pending_page_capture_stage_effects
                .values()
                .any(|(_, pending, ..)| *pending == operation)
            || self
                .pending_page_capture_install_effects
                .values()
                .any(|(pending, ..)| *pending == operation)
        {
            return Err(capture_failure(VersioningReason::DocumentChanged, category));
        }
        let ticket = self
            .storage
            .submit_versioning_object(VersioningObjectCommand::PrepareWindowPage {
                operation,
                profile_uuid,
                state_root: state_root.clone(),
                page_index: page_index_u16,
            })
            .map_err(|_| capture_failure(VersioningReason::Busy, category))?;
        self.pending_page_capture_stage_effects.insert(
            ticket,
            (
                ticket,
                operation,
                profile_uuid,
                state_root,
                authorization.editable(),
                page_index,
            ),
        );
        Ok(ticket)
    }

    fn page_capture_member_is_current(
        &self,
        authorization: &PageCaptureAuthorization,
        page_index: usize,
    ) -> bool {
        authorization
            .scope()
            .members()
            .get(page_index)
            .is_some_and(|member| self.capture_target(member.target().tab) == Some(member.target()))
    }

    /// Records one fixed, payload-free failed-page result only after that
    /// page's exact native stage has been claimed. Strict capture cannot use
    /// this path; its caller must cancel without publishing a partial commit.
    ///
    /// # Errors
    /// Refuses an unconsented policy, wrong page, or a page that already
    /// installed an immutable archive. A `DocumentChanged` result may describe
    /// a changed child frame even while the top-level document token is live.
    pub fn record_window_page_capture_failure(
        &mut self,
        authorization: &PageCaptureAuthorization,
        page_index: usize,
        reason: ManifestPageArchiveFailure,
    ) -> Result<(), VersioningFailure> {
        self.validate_page_capture_authorization(authorization)?;
        let operation = authorization.operation();
        let category = capture_category(authorization.editable());
        let member_current = self.page_capture_member_is_current(authorization, page_index);
        let reason_matches_document =
            reason == ManifestPageArchiveFailure::DocumentChanged || member_current;
        if authorization.failure_policy() != PageCaptureFailurePolicy::AllowPartialPages
            || authorization.scope().members().len() < 2
            || !reason_matches_document
            || self.ready_page_capture_stages.get(&operation) != Some(&page_index)
            || self
                .page_capture_installed_archives
                .get(&operation)
                .is_some_and(|objects| objects.contains_key(&page_index))
        {
            return Err(capture_failure(VersioningReason::DocumentChanged, category));
        }
        self.ready_page_capture_stages.remove(&operation);
        self.page_capture_archive_failures
            .entry(operation)
            .or_default()
            .insert(page_index, reason);
        Ok(())
    }

    /// Validates and installs the exact completed capture on the file-only
    /// worker. Metadata publication remains a later SQLite transaction.
    ///
    /// # Errors
    /// Rejects changed authorization, an empty/oversized result, policy
    /// denial, a full worker queue or failed profile recovery.
    pub fn request_page_capture_object_install(
        &mut self,
        authorization: &PageCaptureAuthorization,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
    ) -> Result<StorageTicket, VersioningFailure> {
        let (state_root, profile_uuid) = self.validate_page_capture_authorization(authorization)?;
        if authorization.scope().members().len() != 1
            || expected_size.is_some_and(|size| size == 0 || size > MAX_ARCHIVE_BYTES)
            || self
                .ready_page_capture_stages
                .get(&authorization.operation())
                .is_none_or(|index| *index != 0)
        {
            return Err(capture_failure(
                VersioningReason::QuotaExceeded,
                capture_category(authorization.editable()),
            ));
        }
        let operation = authorization.operation();
        let reservation_state_root = state_root.clone();
        let ticket = self
            .storage
            .submit_versioning_object(VersioningObjectCommand::InstallCapture {
                operation,
                profile_uuid,
                state_root,
                max_bytes: MAX_ARCHIVE_BYTES,
                expected_size,
                expected_sha256,
            })
            .map_err(|_| {
                capture_failure(
                    VersioningReason::Busy,
                    capture_category(authorization.editable()),
                )
            })?;
        self.ready_page_capture_stages.remove(&operation);
        self.pending_page_capture_install_effects.insert(
            ticket,
            (
                operation,
                profile_uuid,
                reservation_state_root,
                authorization.editable(),
                Some(0),
            ),
        );
        Ok(ticket)
    }

    /// Installs the archive from the exact indexed window-page target. A
    /// success returns an immutable object receipt; no commit is visible.
    ///
    /// # Errors
    /// Refuses a different page index, replaced document, invalid native
    /// byte count, absent reservation, or saturated file worker.
    pub fn request_window_page_capture_object_install(
        &mut self,
        authorization: &PageCaptureAuthorization,
        page_index: usize,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
    ) -> Result<StorageTicket, VersioningFailure> {
        let (state_root, profile_uuid) = self.validate_page_capture_authorization(authorization)?;
        let operation = authorization.operation();
        let category = capture_category(authorization.editable());
        let page_index_u16 = u16::try_from(page_index)
            .map_err(|_| capture_failure(VersioningReason::UnsupportedCapability, category))?;
        if authorization.scope().members().len() < 2
            || !self.page_capture_member_is_current(authorization, page_index)
            || self
                .page_capture_archive_failures
                .get(&operation)
                .is_some_and(|failures| failures.contains_key(&page_index))
            || expected_size.is_some_and(|size| size == 0 || size > MAX_ARCHIVE_BYTES)
            || self.ready_page_capture_stages.get(&operation) != Some(&page_index)
            || !self.page_capture_reservations_held.contains_key(&operation)
        {
            return Err(capture_failure(VersioningReason::DocumentChanged, category));
        }
        let command = if page_index == 0 {
            VersioningObjectCommand::InstallCapture {
                operation,
                profile_uuid,
                state_root: state_root.clone(),
                max_bytes: MAX_ARCHIVE_BYTES,
                expected_size,
                expected_sha256,
            }
        } else {
            VersioningObjectCommand::InstallWindowPage {
                operation,
                profile_uuid,
                state_root: state_root.clone(),
                page_index: page_index_u16,
                max_bytes: MAX_ARCHIVE_BYTES,
                expected_size,
                expected_sha256,
            }
        };
        let ticket = self
            .storage
            .submit_versioning_object(command)
            .map_err(|_| capture_failure(VersioningReason::Busy, category))?;
        self.ready_page_capture_stages.remove(&operation);
        self.pending_page_capture_install_effects.insert(
            ticket,
            (
                operation,
                profile_uuid,
                state_root,
                authorization.editable(),
                Some(page_index),
            ),
        );
        Ok(ticket)
    }

    /// Validates a bounded helper result against the exact Rust-issued
    /// operation challenge before handing its sensitive bytes to the
    /// file-only object worker.
    ///
    /// # Errors
    /// Rejects invalid consent, malformed or oversized sidecars, challenge mismatches,
    /// failed recovery, or a saturated worker queue.
    pub fn request_page_capture_sidecar_install(
        &mut self,
        authorization: &PageCaptureAuthorization,
        bytes: Vec<u8>,
        expected_origin: &str,
        expected_route_fingerprint: &str,
        expected_capture_fingerprint: &str,
        expected_nonce: &str,
        include_media: bool,
    ) -> Result<(StorageTicket, SidecarSummary), VersioningFailure> {
        self.request_page_capture_sidecar_install_for_index(
            authorization,
            None,
            bytes,
            expected_origin,
            expected_route_fingerprint,
            expected_capture_fingerprint,
            expected_nonce,
            include_media,
        )
    }

    /// Installs a window member's sidecar only after that same indexed member
    /// has an immutable archive receipt. The application retains its own
    /// indexed sidecar receipt for final publication.
    ///
    /// # Errors
    /// Refuses an uninstalled/wrong page, duplicate sidecar, invalid consent,
    /// invalid helper bytes, or unavailable storage.
    #[allow(clippy::too_many_arguments)]
    pub fn request_window_page_capture_sidecar_install(
        &mut self,
        authorization: &PageCaptureAuthorization,
        page_index: usize,
        bytes: Vec<u8>,
        expected_origin: &str,
        expected_route_fingerprint: &str,
        expected_capture_fingerprint: &str,
        expected_nonce: &str,
        include_media: bool,
    ) -> Result<(StorageTicket, SidecarSummary), VersioningFailure> {
        self.request_page_capture_sidecar_install_for_index(
            authorization,
            Some(page_index),
            bytes,
            expected_origin,
            expected_route_fingerprint,
            expected_capture_fingerprint,
            expected_nonce,
            include_media,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn request_page_capture_sidecar_install_for_index(
        &mut self,
        authorization: &PageCaptureAuthorization,
        page_index: Option<usize>,
        bytes: Vec<u8>,
        expected_origin: &str,
        expected_route_fingerprint: &str,
        expected_capture_fingerprint: &str,
        expected_nonce: &str,
        include_media: bool,
    ) -> Result<(StorageTicket, SidecarSummary), VersioningFailure> {
        let (state_root, profile_uuid) = self.validate_page_capture_authorization(authorization)?;
        let operation = authorization.operation();
        let member_eligible = match page_index {
            None => authorization.scope().members().len() == 1,
            Some(index) => self.window_page_sidecar_is_eligible(authorization, index),
        };
        if !member_eligible || !self.page_capture_reservations_held.contains_key(&operation) {
            return Err(capture_failure(
                VersioningReason::UnsupportedCapability,
                capture_category(authorization.editable()),
            ));
        }
        let sidecar = PageStateSidecar::parse_json(&bytes).map_err(|_| {
            capture_failure(
                VersioningReason::UnsupportedCapability,
                capture_category(authorization.editable()),
            )
        })?;
        if !sidecar.qualifies_for(
            expected_origin,
            expected_route_fingerprint,
            expected_capture_fingerprint,
            expected_nonce,
        ) {
            return Err(capture_failure(
                VersioningReason::DocumentChanged,
                capture_category(authorization.editable()),
            ));
        }
        let summary = sidecar.summary();
        if summary.helper_version != PAGE_STATE_HELPER_VERSION {
            return Err(capture_failure(
                VersioningReason::UnsupportedCapability,
                capture_category(authorization.editable()),
            ));
        }
        let unauthorized_category = summary.requested_categories.iter().any(|category| {
            use ferric_browser_core::versioning::SidecarCategory;
            match category {
                SidecarCategory::Editable => !authorization.editable(),
                SidecarCategory::Selection => !authorization.selection(),
                // Media has fixed native disclosure within the one-use page
                // archive consent; editable and selection remain separate.
                SidecarCategory::Media => !authorization.media(),
                SidecarCategory::Geometry | SidecarCategory::Controls => false,
            }
        });
        if unauthorized_category {
            return Err(capture_failure(
                VersioningReason::SensitiveCaptureDenied,
                capture_category(authorization.editable()),
            ));
        }
        if include_media && !authorization.media() {
            return Err(capture_failure(
                VersioningReason::SensitiveCaptureDenied,
                capture_category(authorization.editable()),
            ));
        }
        // INVARIANT: a renderer result cannot omit a Rust-authorized category
        // or silently add a category outside this exact native collector
        // request. Media may be omitted when its profile key is unavailable.
        if !sidecar_categories_match_options(
            &summary.requested_categories,
            authorization.editable(),
            authorization.selection(),
            include_media,
        ) {
            return Err(capture_failure(
                VersioningReason::UnsupportedCapability,
                capture_category(authorization.editable()),
            ));
        }
        let ticket = self
            .storage
            .submit_versioning_object(VersioningObjectCommand::InstallSidecarBytes {
                operation,
                profile_uuid,
                state_root: state_root.clone(),
                bytes,
            })
            .map_err(|_| {
                capture_failure(
                    VersioningReason::Busy,
                    capture_category(authorization.editable()),
                )
            })?;
        self.pending_page_capture_install_effects.insert(
            ticket,
            (
                operation,
                profile_uuid,
                state_root,
                authorization.editable(),
                None,
            ),
        );
        if let Some(index) = page_index {
            self.pending_window_page_sidecar_effects
                .insert(ticket, (operation, index, summary.clone()));
        }
        Ok((ticket, summary))
    }

    fn window_page_sidecar_is_eligible(
        &self,
        authorization: &PageCaptureAuthorization,
        index: usize,
    ) -> bool {
        let operation = authorization.operation();
        authorization.scope().members().len() > 1
            && self.page_capture_member_is_current(authorization, index)
            && self
                .page_capture_installed_archives
                .get(&operation)
                .is_some_and(|archives| archives.contains_key(&index))
            && !self
                .page_capture_installed_sidecars
                .get(&operation)
                .is_some_and(|sidecars| sidecars.contains_key(&index))
            && !self.pending_window_page_sidecar_effects.values().any(
                |(pending, pending_index, ..)| *pending == operation && *pending_index == index,
            )
    }

    /// Publishes one validated MHTML object into the exact consented
    /// single-page manifest. The storage transaction verifies the immutable
    /// object reference and atomically releases the capture reservation.
    ///
    /// # Errors
    /// Rejects stale authorization, unsupported scope, invalid capture intervals or objects,
    /// malformed manifests, and unavailable storage.
    pub fn request_page_capture_object_publish(
        &mut self,
        authorization: &PageCaptureAuthorization,
        mut manifest: VersionedCaptureManifest,
        archive: InstalledObject,
        sidecar: Option<InstalledObject>,
        page_capture: ManifestPageCapture,
        provenance: PageCaptureProvenance<'_>,
    ) -> Result<StorageTicket, VersioningFailure> {
        let (state_root, profile_uuid) = self.validate_page_capture_authorization(authorization)?;
        let operation = authorization.operation();
        let Some((envelope, _)) = self.versioning.operation(operation) else {
            return Err(capture_failure(
                VersioningReason::ConfirmationExpired,
                capture_category(authorization.editable()),
            ));
        };
        if authorization.scope().members().len() != 1
            || manifest.profile_uuid != profile_uuid
            || manifest.application_revision != envelope.expected_logical_revision()
            || manifest.scope != ManifestScope::Tab
            || manifest.windows.len() != 1
            || manifest.windows[0].pages.len() != 1
            || manifest.requested_capture.allow_partial_pages
                != (authorization.failure_policy() == PageCaptureFailurePolicy::AllowPartialPages)
            || !self.page_capture_manifest_matches_authorized_page(authorization, &manifest)
            || self
                .page_capture_installed_archives
                .get(&operation)
                .and_then(|objects| objects.get(&0))
                .copied()
                != Some(archive)
            || archive.bytes() == 0
            || archive.bytes() > MAX_ARCHIVE_BYTES
            || sidecar.is_some_and(|object| {
                object.bytes() == 0
                    || object.bytes()
                        > ferric_browser_core::versioning::MAX_PAGE_SIDECAR_BYTES as u64
            })
            || (sidecar.is_some() && page_capture.sidecar_interval.is_none())
        {
            return Err(capture_failure(
                VersioningReason::UnsupportedCapability,
                capture_category(authorization.editable()),
            ));
        }

        if let Err(reason) = finalize_page_capture_manifest(
            &mut manifest,
            archive,
            sidecar,
            page_capture,
            provenance.helper_bundle_version,
            provenance.engine_fingerprint,
        ) {
            return Err(capture_failure(
                reason,
                capture_category(authorization.editable()),
            ));
        }

        self.queue_page_capture_publish(authorization, state_root, profile_uuid, manifest)
    }

    /// Publishes a window only after every captured member has a separately
    /// installed archive or an expressly consented, fixed failed-page reason.
    /// For partial captures, a closed page can retain its frozen identity when
    /// it either has that fixed failure or an already-installed archive.
    /// Per-page archive receipts remain on the application side of this call;
    /// the caller supplies only bounded capture-interval evidence by ordinal.
    ///
    /// # Errors
    /// Refuses stale or reordered members, missing/contradictory page results,
    /// strict-policy failures, invalid evidence, or unavailable storage.
    #[allow(clippy::too_many_lines)] // One gate checks every captured member before the atomic submit.
    pub fn request_window_page_capture_object_publish(
        &mut self,
        authorization: &PageCaptureAuthorization,
        mut manifest: VersionedCaptureManifest,
        page_captures: Vec<Option<ManifestPageCapture>>,
        provenance: PageCaptureProvenance<'_>,
    ) -> Result<StorageTicket, VersioningFailure> {
        let (state_root, profile_uuid) = self.validate_page_capture_authorization(authorization)?;
        let operation = authorization.operation();
        let category = capture_category(authorization.editable());
        let Some((envelope, _)) = self.versioning.operation(operation) else {
            return Err(capture_failure(
                VersioningReason::ConfirmationExpired,
                category,
            ));
        };
        let members = authorization.scope().members();
        let failures = self.page_capture_archive_failures.get(&operation);
        let archives = self.page_capture_installed_archives.get(&operation);
        let Some((logical_window, logical_pages)) = authorization.logical_bindings.as_ref() else {
            return Err(capture_failure(VersioningReason::StaleOwner, category));
        };
        if members.len() < 2
            || manifest.profile_uuid != profile_uuid
            || manifest.application_revision != envelope.expected_logical_revision()
            || manifest.scope != ManifestScope::Window
            || manifest.windows.len() != 1
            || manifest.windows[0].pages.len() != members.len()
            || page_captures.len() != members.len()
            || manifest.requested_capture.allow_partial_pages
                != (authorization.failure_policy() == PageCaptureFailurePolicy::AllowPartialPages)
            || provenance.helper_bundle_version.is_some()
            || logical_pages.len() != members.len()
            || manifest.windows[0].logical_window_id != logical_window.to_string()
            || self
                .runtime_view()
                .logical_window(authorization.scope().window())
                != Some(*logical_window)
            || members.iter().enumerate().any(|(index, member)| {
                let expected = logical_pages[index];
                if manifest.windows[0].pages[index].logical_page_id != expected.to_string() {
                    return true;
                }
                match self.runtime_view().logical_page(member.target().tab) {
                    Some(current) => current != expected,
                    None => {
                        authorization.failure_policy()
                            != PageCaptureFailurePolicy::AllowPartialPages
                            || !(failures.is_some_and(|entries| entries.contains_key(&index))
                                || archives.is_some_and(|objects| objects.contains_key(&index)))
                    }
                }
            })
        {
            return Err(capture_failure(
                VersioningReason::UnsupportedCapability,
                category,
            ));
        }
        let sidecars = self.page_capture_installed_sidecars.get(&operation);
        let saved_count = members
            .iter()
            .enumerate()
            .filter(|(index, _)| archives.is_some_and(|objects| objects.contains_key(index)))
            .count();
        if saved_count == 0
            || (authorization.failure_policy() == PageCaptureFailurePolicy::Strict
                && failures.is_some_and(|entries| !entries.is_empty()))
        {
            return Err(capture_failure(
                VersioningReason::UnsupportedCapability,
                category,
            ));
        }
        for (index, capture) in page_captures.into_iter().enumerate() {
            let archive = archives.and_then(|objects| objects.get(&index)).copied();
            let sidecar = sidecars.and_then(|objects| objects.get(&index));
            let failure = failures.and_then(|entries| entries.get(&index)).copied();
            let page = &mut manifest.windows[0].pages[index];
            let sidecar_matches = match sidecar {
                Some((installed, summary)) => {
                    installed.bytes() > 0
                        && installed.bytes()
                            <= ferric_browser_core::versioning::MAX_PAGE_SIDECAR_BYTES as u64
                        && summary.helper_version == PAGE_STATE_HELPER_VERSION
                        && capture.as_ref().is_some_and(|evidence| {
                            evidence.sidecar_interval.is_some()
                                && evidence.sidecar_monotonic_interval.is_some()
                                && manifest_categories_match_sidecar_summary(evidence, summary)
                        })
                }
                None => capture.as_ref().is_none_or(|evidence| {
                    evidence.sidecar_interval.is_none()
                        && evidence.sidecar_monotonic_interval.is_none()
                        && evidence.categories.is_empty()
                }),
            };
            match (archive, failure, capture) {
                (Some(object), None, Some(capture))
                    if object.bytes() > 0
                        && object.bytes() <= MAX_ARCHIVE_BYTES
                        && sidecar_matches =>
                {
                    page.archive_object = Some(ManifestObjectRef {
                        role: ManifestObjectRole::Archive,
                        sha256: object.digest_hex(),
                        bytes: object.bytes(),
                        archive_backend: Some(ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned()),
                    });
                    page.archive_failure = None;
                    page.sidecar_object = sidecar.map(|(installed, _)| ManifestObjectRef {
                        role: ManifestObjectRole::Sidecar,
                        sha256: installed.digest_hex(),
                        bytes: installed.bytes(),
                        archive_backend: None,
                    });
                    page.fidelity.archive = ManifestCoverage::CompleteForRequest;
                    page.fidelity.page = page_capture_coverage(sidecar.is_some(), &capture);
                    page.page_capture = Some(capture);
                }
                (None, Some(reason), None)
                    if authorization.failure_policy()
                        == PageCaptureFailurePolicy::AllowPartialPages
                        && sidecar.is_none() =>
                {
                    page.archive_object = None;
                    page.archive_failure = Some(reason);
                    page.sidecar_object = None;
                    page.fidelity.archive = ManifestCoverage::Unavailable;
                    page.fidelity.page = ManifestCoverage::Unavailable;
                    page.page_capture = None;
                }
                _ => {
                    return Err(capture_failure(
                        VersioningReason::UnsupportedCapability,
                        category,
                    ));
                }
            }
        }
        let finished_at_ms = capture_unix_millis()
            .ok_or_else(|| capture_failure(VersioningReason::UnsupportedCapability, category))?;
        if finished_at_ms < manifest.capture_interval.started_at_ms
            || finished_at_ms - manifest.capture_interval.started_at_ms > 5 * 60 * 1_000
        {
            return Err(capture_failure(VersioningReason::DocumentChanged, category));
        }
        manifest.capture_interval.finished_at_ms = finished_at_ms;
        manifest.created_at_ms = finished_at_ms;
        manifest.windows[0].capture_interval = Some(manifest.capture_interval);
        manifest
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        manifest
            .requested_capture
            .categories
            .insert(CaptureCategory::Page);
        if sidecars.is_some_and(|objects| !objects.is_empty()) {
            PAGE_STATE_HELPER_VERSION.clone_into(&mut manifest.helper_bundle_version);
        }
        if let Some(engine_fingerprint) = provenance.engine_fingerprint {
            if !valid_capture_engine_fingerprint(engine_fingerprint) {
                return Err(capture_failure(
                    VersioningReason::UnqualifiedRuntime,
                    category,
                ));
            }
            engine_fingerprint.clone_into(&mut manifest.engine_fingerprint);
        }
        manifest
            .validate()
            .map_err(|_| capture_failure(VersioningReason::UnsupportedCapability, category))?;
        self.queue_page_capture_publish(authorization, state_root, profile_uuid, manifest)
    }

    fn queue_page_capture_publish(
        &mut self,
        authorization: &PageCaptureAuthorization,
        state_root: std::path::PathBuf,
        profile_uuid: Uuid,
        manifest: VersionedCaptureManifest,
    ) -> Result<StorageTicket, VersioningFailure> {
        let operation = authorization.operation();

        let Some(stage_ticket) = self.page_capture_stage_tickets.get(&operation).copied() else {
            return Err(capture_failure(
                VersioningReason::StaleOwner,
                capture_category(authorization.editable()),
            ));
        };
        let (disposition, stage_effects) = self
            .versioning
            .acknowledge(stage_ticket, StageResult::Success);
        if disposition != CallbackDisposition::Accepted || !stage_effects.is_empty() {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::StaleOwner,
                capture_category(authorization.editable()),
            ));
        }
        if self
            .versioning
            .advance(operation, OperationState::Publishing)
            .is_err()
        {
            let _ = self.cancel_versioning_operation(operation);
            return Err(capture_failure(
                VersioningReason::StaleOwner,
                capture_category(authorization.editable()),
            ));
        }

        if let Ok(ticket) = self
            .storage
            .submit(StorageCommand::VersioningObjectPublish {
                operation,
                profile_uuid,
                state_root,
                manifest,
            })
        {
            self.pending_page_capture_publications.insert(
                ticket,
                (
                    operation,
                    Instant::now() + PAGE_CAPTURE_PUBLISH_CALLBACK_TIMEOUT,
                ),
            );
            Ok(ticket)
        } else {
            let _ = self.cancel_versioning_operation(operation);
            Err(capture_failure(
                VersioningReason::Busy,
                capture_category(authorization.editable()),
            ))
        }
    }

    fn page_capture_manifest_matches_authorized_page(
        &self,
        authorization: &PageCaptureAuthorization,
        manifest: &VersionedCaptureManifest,
    ) -> bool {
        let Some(page) = manifest
            .windows
            .first()
            .and_then(|window| window.pages.first())
        else {
            return false;
        };
        let Some((logical_window, logical_pages)) = authorization.logical_bindings.as_ref() else {
            return false;
        };
        manifest.windows[0].logical_window_id == logical_window.to_string()
            && self
                .runtime_view()
                .logical_window(authorization.scope().window())
                == Some(*logical_window)
            && logical_pages.first().is_some_and(|logical| {
                page.logical_page_id == logical.to_string()
                    && authorization
                        .scope()
                        .members()
                        .first()
                        .is_some_and(|member| {
                            self.runtime_view().logical_page(member.target().tab) == Some(*logical)
                        })
            })
    }

    fn page_capture_job_is_active(&self, operation: OperationId) -> bool {
        self.page_capture_stage_tickets.contains_key(&operation)
            || self.page_capture_publications_pending(operation)
            || self.page_capture_reservations_held.contains_key(&operation)
            || self.ready_page_capture_stages.contains_key(&operation)
            || self
                .page_capture_installed_archives
                .contains_key(&operation)
            || self.page_capture_archive_failures.contains_key(&operation)
            || self
                .pending_page_capture_reservations
                .values()
                .any(|(pending, ..)| *pending == operation)
            || self
                .pending_page_capture_stage_effects
                .values()
                .any(|(_, pending, ..)| *pending == operation)
            || self
                .pending_page_capture_install_effects
                .values()
                .any(|(pending, ..)| *pending == operation)
    }

    pub(crate) fn route_page_capture_storage_effects(
        &mut self,
        effects: Vec<StorageEffect>,
    ) -> Vec<StorageEffect> {
        let mut forwarded = Vec::with_capacity(effects.len());
        for effect in effects {
            let ticket = effect.ticket;
            if self.pending_page_capture_publications.contains_key(&ticket) {
                self.route_page_capture_publication_effect(effect, &mut forwarded);
            } else if self
                .pending_page_capture_abort_effects
                .contains_key(&ticket)
            {
                self.route_page_capture_abort_effect(ticket, &effect.completion);
            } else if self.pending_page_capture_reservations.contains_key(&ticket) {
                self.route_page_capture_reservation_effect(
                    ticket,
                    &effect.completion,
                    &mut forwarded,
                );
            } else if self
                .pending_page_capture_stage_effects
                .contains_key(&ticket)
            {
                self.route_page_capture_stage_effect(effect, &mut forwarded);
            } else if self
                .pending_page_capture_install_effects
                .contains_key(&ticket)
            {
                self.route_page_capture_install_effect(effect, &mut forwarded);
            } else {
                forwarded.push(effect);
            }
        }
        forwarded
    }

    fn route_page_capture_publication_effect(
        &mut self,
        effect: StorageEffect,
        forwarded: &mut Vec<StorageEffect>,
    ) {
        let Some((operation, _deadline)) = self
            .pending_page_capture_publications
            .remove(&effect.ticket)
        else {
            return;
        };
        if matches!(
            &effect.completion,
            StorageCompletion::VersioningObjectPublish(Ok(_))
        ) {
            self.page_capture_reservations_held.remove(&operation);
            self.page_capture_abort_requested.remove(&operation);
            self.ready_page_capture_stages.remove(&operation);
        } else {
            self.cancel_page_capture_pipeline(operation);
        }
        forwarded.push(effect);
    }

    fn route_page_capture_abort_effect(
        &mut self,
        ticket: StorageTicket,
        completion: &StorageCompletion,
    ) {
        let Some((operation, _profile_uuid)) =
            self.pending_page_capture_abort_effects.remove(&ticket)
        else {
            return;
        };
        if matches!(completion, &StorageCompletion::VersioningAbort(Ok(()))) {
            self.page_capture_reservations_held.remove(&operation);
            self.page_capture_abort_requested.remove(&operation);
        }
    }

    fn route_page_capture_reservation_effect(
        &mut self,
        ticket: StorageTicket,
        completion: &StorageCompletion,
        forwarded: &mut Vec<StorageEffect>,
    ) {
        let Some((operation, profile_uuid, state_root, editable)) =
            self.pending_page_capture_reservations.remove(&ticket)
        else {
            return;
        };
        if !matches!(completion, &StorageCompletion::VersioningBegin(Ok(()))) {
            self.page_capture_abort_requested.remove(&operation);
            forwarded.push(page_capture_stage_failure(
                ticket,
                "page capture reservation could not be created",
            ));
            return;
        }
        self.page_capture_reservations_held
            .insert(operation, profile_uuid);
        if !self.page_capture_is_current(operation, profile_uuid, &state_root, editable) {
            self.request_page_capture_reservation_release(operation);
            forwarded.push(page_capture_stage_failure(
                ticket,
                "page capture authorization expired before staging",
            ));
            return;
        }
        let stage_state_root = state_root.clone();
        let command = VersioningObjectCommand::PrepareCapture {
            operation,
            profile_uuid,
            state_root,
        };
        if let Ok(stage_ticket) = self.storage.submit_versioning_object(command) {
            self.pending_page_capture_stage_effects.insert(
                stage_ticket,
                (
                    ticket,
                    operation,
                    profile_uuid,
                    stage_state_root,
                    editable,
                    0,
                ),
            );
        } else {
            self.request_page_capture_reservation_release(operation);
            forwarded.push(page_capture_stage_failure(
                ticket,
                "page capture staging could not be scheduled",
            ));
        }
    }

    fn route_page_capture_stage_effect(
        &mut self,
        effect: StorageEffect,
        forwarded: &mut Vec<StorageEffect>,
    ) {
        let Some((outer_ticket, operation, profile_uuid, state_root, editable, page_index)) = self
            .pending_page_capture_stage_effects
            .remove(&effect.ticket)
        else {
            return;
        };
        let still_current =
            self.page_capture_is_current(operation, profile_uuid, &state_root, editable);
        match effect.completion {
            StorageCompletion::VersioningCaptureStage(Ok(stage)) if still_current => {
                self.ready_page_capture_stages.insert(operation, page_index);
                forwarded.push(StorageEffect {
                    ticket: outer_ticket,
                    completion: StorageCompletion::VersioningCaptureStage(Ok(stage)),
                });
            }
            StorageCompletion::VersioningCaptureStage(Err(_)) => {
                self.ready_page_capture_stages.remove(&operation);
                self.request_page_capture_reservation_release(operation);
                forwarded.push(page_capture_stage_failure(
                    outer_ticket,
                    "page capture staging failed",
                ));
            }
            _ => {
                self.ready_page_capture_stages.remove(&operation);
                self.request_page_capture_reservation_release(operation);
                forwarded.push(page_capture_stage_failure(
                    outer_ticket,
                    if still_current {
                        "page capture staging returned an invalid result"
                    } else {
                        "page capture authorization expired during staging"
                    },
                ));
            }
        }
    }

    fn route_page_capture_install_effect(
        &mut self,
        effect: StorageEffect,
        forwarded: &mut Vec<StorageEffect>,
    ) {
        let Some((operation, profile_uuid, state_root, editable, stage_index)) = self
            .pending_page_capture_install_effects
            .remove(&effect.ticket)
        else {
            return;
        };
        let still_current =
            self.page_capture_is_current(operation, profile_uuid, &state_root, editable);
        let pending_sidecar = self
            .pending_window_page_sidecar_effects
            .remove(&effect.ticket);
        match &effect.completion {
            StorageCompletion::VersioningObjectInstall(Ok(object)) if still_current => {
                if let Some(page_index) = stage_index {
                    self.page_capture_installed_archives
                        .entry(operation)
                        .or_default()
                        .insert(page_index, *object);
                }
                if let Some((pending_operation, page_index, summary)) = pending_sidecar
                    && pending_operation == operation
                {
                    self.page_capture_installed_sidecars
                        .entry(operation)
                        .or_default()
                        .insert(page_index, (*object, summary));
                }
            }
            StorageCompletion::VersioningObjectInstall(Err(_)) if still_current => {
                if let Some(page_index) = stage_index {
                    self.ready_page_capture_stages.insert(operation, page_index);
                }
            }
            _ => self.request_page_capture_reservation_release(operation),
        }
        forwarded.push(effect);
    }

    fn page_capture_is_current(
        &self,
        operation: OperationId,
        profile_uuid: Uuid,
        state_root: &Path,
        editable: bool,
    ) -> bool {
        let Some((envelope, state)) = self.versioning.operation(operation) else {
            return false;
        };
        if state != OperationState::Executing {
            return false;
        }
        let Some(policy) = self.page_capture_failure_policies.get(&operation).copied() else {
            return false;
        };
        self.validate_page_capture_scope(operation, envelope.scope(), editable, policy)
            .is_ok_and(|(current_root, current_profile)| {
                current_root == state_root && current_profile == profile_uuid
            })
    }

    fn request_page_capture_reservation_release(&mut self, operation: OperationId) {
        self.page_capture_abort_requested.insert(operation);
        self.retry_page_capture_reservation_releases();
    }

    pub(crate) fn retry_page_capture_reservation_releases(&mut self) {
        let operations = self
            .page_capture_abort_requested
            .iter()
            .copied()
            .collect::<Vec<_>>();
        for operation in operations {
            if self
                .pending_page_capture_abort_effects
                .values()
                .any(|(pending, _)| *pending == operation)
                || self
                    .pending_page_capture_stage_effects
                    .values()
                    .any(|(_, pending, ..)| *pending == operation)
                || self
                    .pending_page_capture_install_effects
                    .values()
                    .any(|(pending, ..)| *pending == operation)
            {
                continue;
            }
            let Some(profile_uuid) = self.page_capture_reservations_held.get(&operation).copied()
            else {
                // A pending begin will request release after its completion.
                continue;
            };
            if let Ok(ticket) = self.storage.submit(StorageCommand::VersioningAbort {
                operation,
                profile_uuid,
            }) {
                self.pending_page_capture_abort_effects
                    .insert(ticket, (operation, profile_uuid));
            }
        }
    }

    pub(crate) fn cancel_page_capture_pipeline(&mut self, operation: OperationId) {
        if !self.page_capture_job_is_active(operation)
            && !self.page_capture_abort_requested.contains(&operation)
        {
            return;
        }
        self.storage.cancel_versioning_object(operation);
        self.ready_page_capture_stages.remove(&operation);
        self.page_capture_installed_archives.remove(&operation);
        self.page_capture_installed_sidecars.remove(&operation);
        self.pending_window_page_sidecar_effects
            .retain(|_, (pending, ..)| *pending != operation);
        self.page_capture_archive_failures.remove(&operation);
        self.page_capture_failure_policies.remove(&operation);
        self.request_page_capture_reservation_release(operation);
    }

    pub(crate) fn suppress_page_capture_cleanup_effect(
        &mut self,
        operation: OperationId,
        effects: &mut Vec<OperationEffect>,
    ) {
        if let Some(ticket) = self.page_capture_stage_tickets.remove(&operation) {
            effects.retain(|effect| *effect != OperationEffect::Cleanup(ticket));
        }
    }

    pub(crate) fn suppress_terminal_page_capture_cleanup_effects(
        &mut self,
        effects: &mut Vec<OperationEffect>,
    ) {
        let terminal = self
            .page_capture_stage_tickets
            .keys()
            .copied()
            .filter(|operation| {
                self.versioning
                    .operation(*operation)
                    .is_none_or(|(_, state)| state.terminal())
            })
            .collect::<Vec<_>>();
        for operation in terminal {
            self.suppress_page_capture_cleanup_effect(operation, effects);
        }
    }

    pub(crate) fn reap_page_capture_pipeline(&mut self) {
        let candidates = self
            .page_capture_reservations_held
            .keys()
            .copied()
            .chain(
                self.pending_page_capture_reservations
                    .values()
                    .map(|(operation, ..)| *operation),
            )
            .chain(
                self.pending_page_capture_stage_effects
                    .values()
                    .map(|(_, operation, ..)| *operation),
            )
            .chain(
                self.pending_page_capture_install_effects
                    .values()
                    .map(|(operation, ..)| *operation),
            )
            .collect::<BTreeSet<_>>();
        let stale = candidates
            .iter()
            .filter(|operation| {
                let callback_pending = self
                    .pending_page_capture_publications
                    .values()
                    .any(|(pending, until)| pending == *operation && *until > Instant::now());
                !callback_pending
                    && self
                        .versioning
                        .operation(**operation)
                        .is_none_or(|(envelope, state)| {
                            state.terminal()
                                || (state != OperationState::Publishing
                                    && self.versioning.now_millis() >= envelope.deadline_ms())
                        })
            })
            .copied()
            .collect::<Vec<_>>();
        for operation in stale {
            self.cancel_page_capture_pipeline(operation);
        }
    }

    pub(crate) fn cancel_page_capture_jobs_for_profile(
        &mut self,
        profile: ferric_browser_core::ProfileId,
    ) {
        let candidates = self
            .page_capture_reservations_held
            .keys()
            .copied()
            .chain(
                self.pending_page_capture_reservations
                    .values()
                    .map(|(operation, ..)| *operation),
            )
            .chain(
                self.pending_page_capture_stage_effects
                    .values()
                    .map(|(_, operation, ..)| *operation),
            )
            .chain(
                self.pending_page_capture_install_effects
                    .values()
                    .map(|(operation, ..)| *operation),
            )
            .collect::<BTreeSet<_>>();
        for operation in candidates {
            if self
                .versioning
                .operation(operation)
                .is_some_and(|(envelope, _)| envelope.owner().runtime_profile() == profile)
                && !self.page_capture_publications_pending(operation)
            {
                self.cancel_page_capture_pipeline(operation);
            }
        }
    }

    pub(crate) fn cancel_all_page_capture_jobs(&mut self) {
        let candidates = self
            .page_capture_reservations_held
            .keys()
            .copied()
            .chain(
                self.pending_page_capture_reservations
                    .values()
                    .map(|(operation, ..)| *operation),
            )
            .chain(
                self.pending_page_capture_stage_effects
                    .values()
                    .map(|(_, operation, ..)| *operation),
            )
            .chain(
                self.pending_page_capture_install_effects
                    .values()
                    .map(|(operation, ..)| *operation),
            )
            .collect::<BTreeSet<_>>();
        for operation in candidates {
            if !self.page_capture_publications_pending(operation) {
                self.cancel_page_capture_pipeline(operation);
            }
        }
    }

    fn page_capture_publications_pending(&self, operation: OperationId) -> bool {
        self.pending_page_capture_publications
            .values()
            .any(|(pending, until)| *pending == operation && *until > Instant::now())
    }

    fn validate_page_capture_authorization(
        &self,
        authorization: &PageCaptureAuthorization,
    ) -> Result<(std::path::PathBuf, uuid::Uuid), VersioningFailure> {
        self.validate_page_capture_scope(
            authorization.operation(),
            authorization.scope(),
            authorization.editable(),
            authorization.failure_policy(),
        )
    }

    fn validate_page_capture_scope(
        &self,
        operation: OperationId,
        scope: &CapturedScope,
        editable: bool,
        failure_policy: PageCaptureFailurePolicy,
    ) -> Result<(std::path::PathBuf, uuid::Uuid), VersioningFailure> {
        let category = capture_category(editable);
        let Some((envelope, state)) = self.versioning.operation(operation) else {
            return Err(capture_failure(
                VersioningReason::ConfirmationExpired,
                category,
            ));
        };
        if !matches!(state, OperationState::Preparing | OperationState::Executing)
            || (state == OperationState::Executing
                && !self.page_capture_stage_tickets.contains_key(&operation))
            || envelope.kind() != OperationKind::Sensitive(SensitiveOperation::PageCapture)
            || envelope.scope() != scope
            || (self.runtime_view().revision() != envelope.expected_logical_revision()
                && scope.members().len() == 1)
        {
            return Err(capture_failure(VersioningReason::DocumentChanged, category));
        }
        let profile = envelope.owner().runtime_profile();
        let (state_root, profile_uuid) = self
            .active_profile
            .as_ref()
            .filter(|_| self.active_profile_runtime == Some(profile))
            .and_then(|active| {
                active
                    .versioning_recovery_scope()
                    .map(|(root, owner)| (root.to_owned(), owner))
            })
            .ok_or_else(|| capture_failure(VersioningReason::PrivatePersistenceDenied, category))?;
        let (current_profile, current_owner) =
            self.structural_commit_owner(envelope.scope().window())?;
        if current_profile != profile_uuid || current_owner != envelope.owner() {
            return Err(capture_failure(VersioningReason::StaleOwner, category));
        }
        if scope.members().len() > 1
            && failure_policy == PageCaptureFailurePolicy::Strict
            && scope
                .members()
                .iter()
                .any(|member| self.capture_target(member.target().tab) != Some(member.target()))
        {
            return Err(capture_failure(VersioningReason::DocumentChanged, category));
        }
        if self.configuration().versioning.page_capture == SensitiveCapturePolicy::Deny
            || (editable
                && self.configuration().versioning.editable_capture == SensitiveCapturePolicy::Deny)
        {
            return Err(capture_failure(
                VersioningReason::SensitiveCaptureDenied,
                category,
            ));
        }
        Ok((state_root, profile_uuid))
    }
}

fn finalize_page_capture_manifest(
    manifest: &mut VersionedCaptureManifest,
    archive: InstalledObject,
    sidecar: Option<InstalledObject>,
    page_capture: ManifestPageCapture,
    helper_bundle_version: Option<&str>,
    engine_fingerprint: Option<&str>,
) -> Result<(), VersioningReason> {
    let finished_at_ms = capture_unix_millis().ok_or(VersioningReason::UnsupportedCapability)?;
    if finished_at_ms < manifest.capture_interval.started_at_ms
        || finished_at_ms - manifest.capture_interval.started_at_ms > 5 * 60 * 1_000
    {
        return Err(VersioningReason::DocumentChanged);
    }
    manifest.capture_interval = CaptureInterval {
        started_at_ms: manifest.capture_interval.started_at_ms,
        finished_at_ms,
    };
    manifest.created_at_ms = finished_at_ms;
    manifest
        .requested_capture
        .categories
        .insert(CaptureCategory::Archive);
    manifest
        .requested_capture
        .categories
        .insert(CaptureCategory::Page);
    let page = &mut manifest.windows[0].pages[0];
    page.archive_object = Some(ManifestObjectRef {
        role: ManifestObjectRole::Archive,
        sha256: archive.digest_hex(),
        bytes: archive.bytes(),
        archive_backend: Some(ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned()),
    });
    page.sidecar_object = sidecar.map(|object| ManifestObjectRef {
        role: ManifestObjectRole::Sidecar,
        sha256: object.digest_hex(),
        bytes: object.bytes(),
        archive_backend: None,
    });
    page.fidelity.archive = ManifestCoverage::CompleteForRequest;
    page.fidelity.page = page_capture_coverage(sidecar.is_some(), &page_capture);
    page.page_capture = Some(page_capture);
    if let Some(helper_version) = helper_bundle_version {
        helper_version.clone_into(&mut manifest.helper_bundle_version);
    }
    if let Some(engine_fingerprint) = engine_fingerprint {
        if !valid_capture_engine_fingerprint(engine_fingerprint) {
            return Err(VersioningReason::UnqualifiedRuntime);
        }
        engine_fingerprint.clone_into(&mut manifest.engine_fingerprint);
    }
    manifest.windows[0].capture_interval = Some(manifest.capture_interval);
    manifest
        .validate()
        .map_err(|_| VersioningReason::UnsupportedCapability)
}

fn valid_capture_engine_fingerprint(value: &str) -> bool {
    if value.is_empty() || value.len() > 256 {
        return false;
    }
    let mut fields = value.split(';');
    let Some(webengine) = fields.next() else {
        return false;
    };
    let Some(chromium) = fields.next() else {
        return false;
    };
    let Some(security_patch) = fields.next() else {
        return false;
    };
    let Some(platform) = fields.next() else {
        return false;
    };
    fields.next().is_none()
        && valid_capture_fingerprint_field(webengine, "qtwebengine", 48)
        && valid_capture_fingerprint_field(chromium, "chromium", 48)
        && valid_capture_fingerprint_field(security_patch, "security-patch", 48)
        && valid_capture_fingerprint_field(platform, "platform", 32)
}

fn valid_capture_fingerprint_field(field: &str, expected_name: &str, maximum_bytes: usize) -> bool {
    let Some((name, value)) = field.split_once('=') else {
        return false;
    };
    name == expected_name
        && !value.is_empty()
        && value.len() <= maximum_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn page_capture_coverage(
    sidecar_present: bool,
    evidence: &ManifestPageCapture,
) -> ManifestCoverage {
    if !sidecar_present {
        return ManifestCoverage::Unavailable;
    }
    if evidence.categories.is_empty()
        || evidence.mutation_evidence != ManifestMutationEvidence::ObservedStable
        || evidence
            .categories
            .iter()
            .any(|category| category.truncated)
    {
        ManifestCoverage::Partial
    } else {
        ManifestCoverage::CompleteForRequest
    }
}

fn manifest_categories_match_sidecar_summary(
    evidence: &ManifestPageCapture,
    summary: &SidecarSummary,
) -> bool {
    evidence.categories
        == summary
            .coverage
            .iter()
            .map(|coverage| ManifestCategoryCoverage {
                category: match coverage.category() {
                    SidecarCategory::Geometry => ManifestPageCategory::Geometry,
                    SidecarCategory::Controls => ManifestPageCategory::Controls,
                    SidecarCategory::Editable => ManifestPageCategory::Editable,
                    SidecarCategory::Selection => ManifestPageCategory::Selection,
                    SidecarCategory::Media => ManifestPageCategory::Media,
                },
                requested: coverage.requested(),
                captured: coverage.captured(),
                truncated: coverage.truncated(),
            })
            .collect::<Vec<_>>()
}

fn expected_page_sidecar_categories(
    editable: bool,
    selection: bool,
    media: bool,
) -> BTreeSet<SidecarCategory> {
    let mut categories = BTreeSet::from([SidecarCategory::Geometry, SidecarCategory::Controls]);
    if editable {
        categories.insert(SidecarCategory::Editable);
    }
    if selection {
        categories.insert(SidecarCategory::Selection);
    }
    if media {
        categories.insert(SidecarCategory::Media);
    }
    categories
}

fn sidecar_categories_match_options(
    actual: &[SidecarCategory],
    editable: bool,
    selection: bool,
    media: bool,
) -> bool {
    let expected = expected_page_sidecar_categories(editable, selection, media);
    actual.len() == expected.len() && actual.iter().copied().collect::<BTreeSet<_>>() == expected
}

fn capture_category(editable: bool) -> SensitiveCategory {
    if editable {
        SensitiveCategory::Editable
    } else {
        SensitiveCategory::Page
    }
}

fn capture_failure(reason: VersioningReason, category: SensitiveCategory) -> VersioningFailure {
    VersioningFailure::new(reason, category)
}

fn capture_unix_millis() -> Option<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
    .ok()
}

fn page_capture_stage_failure(ticket: StorageTicket, message: &'static str) -> StorageEffect {
    StorageEffect {
        ticket,
        completion: StorageCompletion::VersioningCaptureStage(Err(StorageOperationError {
            operation: StorageOperation::VersioningCaptureStage,
            kind: ferric_browser_storage::StorageFailureKind::Other,
            message: message.to_owned(),
        })),
    }
}

fn consent_failure_reason(error: VersioningConsentError) -> VersioningReason {
    match error {
        VersioningConsentError::Operation(super::OperationError::ConsentExpired)
        | VersioningConsentError::Preview(PreviewError::Expired) => {
            VersioningReason::ConfirmationExpired
        }
        VersioningConsentError::Operation(super::OperationError::OwnerClosed)
        | VersioningConsentError::Preview(PreviewError::WrongOwner) => VersioningReason::StaleOwner,
        VersioningConsentError::Operation(super::OperationError::Scope(scope)) => scope.reason(),
        _ => VersioningReason::ConfirmationRequired,
    }
}

#[cfg(test)]
mod page_capture_coverage_tests {
    use super::{
        expected_page_sidecar_categories, page_capture_coverage, sidecar_categories_match_options,
        valid_capture_engine_fingerprint,
    };
    use ferric_browser_core::versioning::SidecarCategory;
    use ferric_browser_storage::versioning::{
        ManifestCategoryCoverage, ManifestCoverage, ManifestMutationEvidence, ManifestPageCapture,
        ManifestPageCategory,
    };
    use std::collections::BTreeSet;

    fn evidence(
        mutation_evidence: ManifestMutationEvidence,
        truncated: bool,
    ) -> ManifestPageCapture {
        ManifestPageCapture {
            archive_interval: None,
            sidecar_interval: Some(ferric_browser_storage::versioning::CaptureInterval {
                started_at_ms: 1,
                finished_at_ms: 2,
            }),
            archive_monotonic_interval: None,
            sidecar_monotonic_interval: Some(
                ferric_browser_storage::versioning::CaptureMonotonicInterval {
                    started_offset_ms: 0,
                    finished_offset_ms: 1,
                },
            ),
            mutation_evidence,
            categories: vec![ManifestCategoryCoverage {
                category: ManifestPageCategory::Geometry,
                requested: 1,
                captured: 1,
                truncated,
            }],
        }
    }

    #[test]
    fn only_observed_stable_untruncated_sidecar_is_complete() {
        assert_eq!(
            page_capture_coverage(
                true,
                &evidence(ManifestMutationEvidence::ObservedStable, false)
            ),
            ManifestCoverage::CompleteForRequest
        );
        for mutation in [
            ManifestMutationEvidence::ObservedChanged,
            ManifestMutationEvidence::NotObserved,
        ] {
            assert_eq!(
                page_capture_coverage(true, &evidence(mutation, false)),
                ManifestCoverage::Partial
            );
        }
        assert_eq!(
            page_capture_coverage(
                true,
                &evidence(ManifestMutationEvidence::ObservedStable, true)
            ),
            ManifestCoverage::Partial
        );
        assert_eq!(
            page_capture_coverage(
                false,
                &evidence(ManifestMutationEvidence::ObservedStable, false)
            ),
            ManifestCoverage::Unavailable
        );
        let mut missing_categories = evidence(ManifestMutationEvidence::ObservedStable, false);
        missing_categories.categories.clear();
        assert_eq!(
            page_capture_coverage(true, &missing_categories),
            ManifestCoverage::Partial
        );
        let mut empty_page = evidence(ManifestMutationEvidence::ObservedStable, false);
        empty_page.categories[0].captured = 0;
        assert_eq!(
            page_capture_coverage(true, &empty_page),
            ManifestCoverage::CompleteForRequest,
            "the helper's synthetic minimum requested count is not a missing-record signal"
        );
    }

    #[test]
    fn requested_sidecar_categories_match_the_exact_consent_options() {
        let base = BTreeSet::from([SidecarCategory::Geometry, SidecarCategory::Controls]);
        assert_eq!(expected_page_sidecar_categories(false, false, false), base);
        let base_vec = vec![SidecarCategory::Geometry, SidecarCategory::Controls];
        assert!(sidecar_categories_match_options(
            &base_vec, false, false, false
        ));
        assert!(!sidecar_categories_match_options(
            &base_vec, true, false, false
        ));
        assert!(!sidecar_categories_match_options(
            &[SidecarCategory::Geometry],
            false,
            false,
            false
        ));
        assert!(!sidecar_categories_match_options(
            &[
                SidecarCategory::Geometry,
                SidecarCategory::Controls,
                SidecarCategory::Editable,
            ],
            false,
            false,
            false
        ));
        let all_optional = BTreeSet::from([
            SidecarCategory::Geometry,
            SidecarCategory::Controls,
            SidecarCategory::Editable,
            SidecarCategory::Selection,
            SidecarCategory::Media,
        ]);
        assert_eq!(
            expected_page_sidecar_categories(true, true, true),
            all_optional
        );
        let all_optional_vec = all_optional.iter().copied().collect::<Vec<_>>();
        assert!(sidecar_categories_match_options(
            &all_optional_vec,
            true,
            true,
            true
        ));
        assert_ne!(
            expected_page_sidecar_categories(true, false, false),
            expected_page_sidecar_categories(false, false, false)
        );
        assert_ne!(
            expected_page_sidecar_categories(false, true, false),
            expected_page_sidecar_categories(false, false, false)
        );
        assert_ne!(
            expected_page_sidecar_categories(false, false, true),
            expected_page_sidecar_categories(false, false, false)
        );
    }

    #[test]
    fn runtime_engine_fingerprint_is_bounded_and_plain() {
        assert!(valid_capture_engine_fingerprint(
            "qtwebengine=6.11.2;chromium=140.0.7339.225;security-patch=151.0.7922.71;platform=wayland"
        ));
        assert!(!valid_capture_engine_fingerprint(
            "qtwebengine=6.11.2\nsecret"
        ));
        assert!(!valid_capture_engine_fingerprint(&"a".repeat(257)));
        assert!(!valid_capture_engine_fingerprint(
            "qtwebengine=6.11.2;chromium=140;platform=wayland;security-patch=151"
        ));
        assert!(!valid_capture_engine_fingerprint(
            "qtwebengine=6.11.2;chromium=140;security-patch=151;platform=wayland;extra=true"
        ));
    }
}
