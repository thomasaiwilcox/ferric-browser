//! Exact profile-local archive materialization for the restricted viewer.
//!
//! Manifest reads stay on the metadata worker. Archive bytes are copied and
//! digest-verified by the file-only object worker into one private staging
//! file; the normal profile and its network interceptor are never reused.

use std::path::PathBuf;

use ferric_browser_core::{
    CommitId, NativeProfileOwner, OperationId, ProfileId, VersioningFailure, VersioningReason,
    WindowId,
};
use ferric_browser_storage::{
    StorageCommand, StorageCompletion, StorageOperation, StorageOperationError,
    versioning::{ManifestCoverage, ManifestObjectRole, ManifestSafeUrl, VersionedCaptureManifest},
};
use uuid::Uuid;

use crate::{BrowserApplication, StorageEffect, StorageTicket};

// The native adapter owns one isolated archive surface per application
// instance. Keep admission aligned with that single-view UI and the file
// worker's one-at-a-time exact cleanup protocol.
const MAX_OPEN_ARCHIVE_VIEWS: usize = 1;

pub(crate) struct PendingArchiveViewManifest {
    operation: OperationId,
    window: WindowId,
    profile_uuid: Uuid,
    owner: NativeProfileOwner,
    state_root: PathBuf,
    commit: CommitId,
}

pub(crate) struct PendingArchiveViewObject {
    outer_ticket: StorageTicket,
    operation: OperationId,
    window: WindowId,
    profile_uuid: Uuid,
    owner: NativeProfileOwner,
    state_root: PathBuf,
    metadata: VersionedArchiveViewMetadata,
}

pub(crate) struct ActiveArchiveView {
    window: WindowId,
    profile_uuid: Uuid,
    owner: NativeProfileOwner,
    state_root: PathBuf,
    metadata: VersionedArchiveViewMetadata,
    presentation_handed_off: bool,
}

/// User-visible archive provenance returned only for an explicitly opened,
/// isolated archive view.
// INVARIANT: source_url is metadata only. It comes from the exact validated
// committed manifest, is returned only while its owner is current, and is
// never used to construct a live navigation target.
#[derive(Clone, Eq, PartialEq)]
pub struct VersionedArchiveViewMetadata {
    pub source_url: Option<String>,
    pub captured_at_ms: u64,
    pub fidelity_label: String,
}

impl std::fmt::Debug for VersionedArchiveViewMetadata {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VersionedArchiveViewMetadata")
            .field(
                "source_url",
                &self.source_url.as_ref().map(|_| "[redacted]"),
            )
            .field("captured_at_ms", &"[redacted]")
            .field("fidelity_label", &"[redacted]")
            .finish()
    }
}

impl BrowserApplication {
    /// Resolves the one archive object in a committed manifest and prepares
    /// an isolated-view file. A commit with zero or multiple archives fails
    /// closed; the method never substitutes the page's current live URL.
    ///
    /// # Errors
    /// Refuses a stale, private, unavailable, ambiguous, over-quota, or busy
    /// request. The archive remains untrusted input even after digest checks.
    pub fn request_versioned_archive_view(
        &mut self,
        window: WindowId,
        commit: CommitId,
    ) -> Result<StorageTicket, VersioningFailure> {
        if self.archive_view_count() >= MAX_OPEN_ARCHIVE_VIEWS {
            return Err(archive_failure(VersioningReason::Busy));
        }
        let (profile_uuid, owner) = self
            .structural_commit_owner(window)
            .map_err(|_| archive_failure(VersioningReason::StaleOwner))?;
        let state_root = self
            .profile_snapshot()
            .and_then(|profile| profile.roots)
            .map(|roots| roots.state)
            .ok_or_else(|| archive_failure(VersioningReason::PrivatePersistenceDenied))?;
        let operation = OperationId::fresh();
        let ticket = self
            .storage
            .submit(StorageCommand::VersioningManifest {
                profile_uuid,
                id: commit,
            })
            .map_err(|_| archive_failure(VersioningReason::Busy))?;
        self.pending_archive_view_manifests.insert(
            ticket,
            PendingArchiveViewManifest {
                operation,
                window,
                profile_uuid,
                owner,
                state_root,
                commit,
            },
        );
        Ok(ticket)
    }

    /// Removes one exact temporary viewer file after the isolated surface has
    /// stopped and navigated to blank.
    ///
    /// # Errors
    /// Refuses an unknown file, wrong window, or already-pending cleanup.
    pub fn close_versioned_archive_view(
        &mut self,
        window: WindowId,
        operation: OperationId,
    ) -> Result<StorageTicket, VersioningFailure> {
        let active = self
            .active_archive_views
            .get(&operation)
            .ok_or_else(|| archive_failure(VersioningReason::StaleOwner))?;
        if active.window != window
            || self
                .pending_archive_view_cleanups
                .values()
                .any(|pending| *pending == operation)
        {
            return Err(archive_failure(VersioningReason::StaleOwner));
        }
        let profile_uuid = active.profile_uuid;
        let state_root = active.state_root.clone();
        let ticket = self
            .storage
            .submit_versioning_object(
                ferric_browser_storage::versioning::VersioningObjectCommand::RemoveArchiveView {
                    operation,
                    profile_uuid,
                    state_root,
                },
            )
            .map_err(|_| archive_failure(VersioningReason::Busy))?;
        self.pending_archive_view_cleanups.insert(ticket, operation);
        Ok(ticket)
    }

    /// Returns inert provenance for one currently active isolated archive
    /// view. The caller must already own the exact window and operation.
    #[must_use]
    pub fn versioned_archive_view_metadata(
        &mut self,
        window: WindowId,
        operation: OperationId,
    ) -> Option<VersionedArchiveViewMetadata> {
        let (profile_uuid, owner) = self.structural_commit_owner(window).ok()?;
        let active = self.active_archive_views.get_mut(&operation)?;
        if active.window != window || active.profile_uuid != profile_uuid || active.owner != owner {
            return None;
        }
        // The caller may now publish the file URL to a live WebEngine view.
        // Owner invalidation must not delete the staged file before that view
        // has navigated to blank and called close_versioned_archive_view.
        active.presentation_handed_off = true;
        Some(active.metadata.clone())
    }

    fn archive_view_count(&self) -> usize {
        self.active_archive_views.len()
            + self.pending_archive_view_manifests.len()
            + self.pending_archive_view_objects.len()
    }

    pub(crate) fn schedule_archive_view_cleanup_for_profile(&mut self, profile: ProfileId) {
        self.schedule_archive_view_cleanup_where(|active| {
            active.owner.runtime_profile() == profile
        });
    }

    pub(crate) fn schedule_all_archive_view_cleanup(&mut self) {
        self.schedule_archive_view_cleanup_where(|_| true);
    }

    fn schedule_archive_view_cleanup_where(
        &mut self,
        mut selected: impl FnMut(&ActiveArchiveView) -> bool,
    ) {
        let targets = self
            .active_archive_views
            .iter()
            .filter_map(|(operation, active)| {
                (selected(active)
                    && !active.presentation_handed_off
                    && !self
                        .pending_archive_view_cleanups
                        .values()
                        .any(|pending| pending == operation))
                .then_some((*operation, active.profile_uuid, active.state_root.clone()))
            })
            .collect::<Vec<_>>();
        for (operation, profile_uuid, state_root) in targets {
            if let Ok(ticket) = self.storage.submit_versioning_object(
                ferric_browser_storage::versioning::VersioningObjectCommand::RemoveArchiveView {
                    operation,
                    profile_uuid,
                    state_root,
                },
            ) {
                self.pending_archive_view_cleanups.insert(ticket, operation);
            }
        }
    }

    pub(crate) fn route_archive_view_storage_effects(
        &mut self,
        effects: Vec<StorageEffect>,
    ) -> Vec<StorageEffect> {
        let mut forwarded = Vec::with_capacity(effects.len());
        for effect in effects {
            let ticket = effect.ticket;
            if self.pending_archive_view_manifests.contains_key(&ticket) {
                self.route_archive_manifest_effect(effect, &mut forwarded);
            } else if self.pending_archive_view_objects.contains_key(&ticket) {
                self.route_archive_object_effect(effect, &mut forwarded);
            } else if self.pending_archive_view_cleanups.contains_key(&ticket) {
                self.route_archive_cleanup_effect(effect, &mut forwarded);
            } else {
                forwarded.push(effect);
            }
        }
        forwarded
    }

    fn route_archive_manifest_effect(
        &mut self,
        effect: StorageEffect,
        forwarded: &mut Vec<StorageEffect>,
    ) {
        let Some(pending) = self.pending_archive_view_manifests.remove(&effect.ticket) else {
            return;
        };
        let manifest = match effect.completion {
            StorageCompletion::VersioningManifest(Ok(Some(manifest))) => Some(manifest),
            _ => None,
        };
        let archive = manifest
            .as_ref()
            .and_then(|manifest| self.archive_reference_for_manifest(manifest, &pending));
        let Some(archive) = archive else {
            forwarded.push(archive_failure_effect(
                effect.ticket,
                "stored archive reference is missing, ambiguous, or unavailable",
            ));
            return;
        };
        let command =
            ferric_browser_storage::versioning::VersioningObjectCommand::MaterializeArchiveView {
                operation: pending.operation,
                profile_uuid: pending.profile_uuid,
                state_root: pending.state_root.clone(),
                digest_hex: archive.sha256,
                expected_bytes: archive.bytes,
            };
        match self.storage.submit_versioning_object(command) {
            Ok(ticket) => {
                self.pending_archive_view_objects.insert(
                    ticket,
                    PendingArchiveViewObject {
                        outer_ticket: effect.ticket,
                        operation: pending.operation,
                        window: pending.window,
                        profile_uuid: pending.profile_uuid,
                        owner: pending.owner,
                        state_root: pending.state_root,
                        metadata: archive.metadata,
                    },
                );
            }
            Err(_) => forwarded.push(archive_failure_effect(
                effect.ticket,
                "isolated archive materialization could not be scheduled",
            )),
        }
    }

    fn route_archive_object_effect(
        &mut self,
        effect: StorageEffect,
        forwarded: &mut Vec<StorageEffect>,
    ) {
        let Some(pending) = self.pending_archive_view_objects.remove(&effect.ticket) else {
            return;
        };
        let current_owner =
            self.structural_commit_owner(pending.window)
                .is_ok_and(|(profile_uuid, owner)| {
                    profile_uuid == pending.profile_uuid && owner == pending.owner
                });
        match effect.completion {
            StorageCompletion::VersioningArchiveView(Ok(file)) if current_owner => {
                self.active_archive_views.insert(
                    pending.operation,
                    ActiveArchiveView {
                        window: pending.window,
                        profile_uuid: pending.profile_uuid,
                        owner: pending.owner,
                        state_root: pending.state_root,
                        metadata: pending.metadata,
                        presentation_handed_off: false,
                    },
                );
                forwarded.push(StorageEffect {
                    ticket: pending.outer_ticket,
                    completion: StorageCompletion::VersioningArchiveView(Ok(file)),
                });
            }
            StorageCompletion::VersioningArchiveView(Ok(_)) => {
                let command = ferric_browser_storage::versioning::VersioningObjectCommand::RemoveArchiveView {
                    operation: pending.operation,
                    profile_uuid: pending.profile_uuid,
                    state_root: pending.state_root,
                };
                let _ = self.storage.submit_versioning_object(command);
                forwarded.push(archive_failure_effect(
                    pending.outer_ticket,
                    "profile owner changed before archive view activation",
                ));
            }
            _ => forwarded.push(archive_failure_effect(
                pending.outer_ticket,
                "stored archive failed validation during materialization",
            )),
        }
    }

    fn route_archive_cleanup_effect(
        &mut self,
        effect: StorageEffect,
        forwarded: &mut Vec<StorageEffect>,
    ) {
        let Some(operation) = self.pending_archive_view_cleanups.remove(&effect.ticket) else {
            return;
        };
        if matches!(
            &effect.completion,
            StorageCompletion::VersioningArchiveViewCleanup(Ok(()))
        ) {
            self.active_archive_views.remove(&operation);
        }
        forwarded.push(effect);
    }

    fn archive_reference_for_manifest(
        &self,
        manifest: &VersionedCaptureManifest,
        pending: &PendingArchiveViewManifest,
    ) -> Option<ArchiveReference> {
        let owner_is_current =
            self.structural_commit_owner(pending.window)
                .is_ok_and(|(profile_uuid, owner)| {
                    profile_uuid == pending.profile_uuid && owner == pending.owner
                });
        if !owner_is_current
            || manifest.validate().is_err()
            || manifest.id != pending.commit.to_string()
            || manifest.profile_uuid != pending.profile_uuid
        {
            return None;
        }
        let mut archives = manifest
            .windows
            .iter()
            .flat_map(|window| window.pages.iter())
            .filter_map(|page| {
                page.archive_object
                    .as_ref()
                    .filter(|object| object.role == ManifestObjectRole::Archive)
                    .map(|object| (page, object))
            });
        let (page, object) = archives.next()?;
        if archives.next().is_some()
            || !object.supports_archive_view()
            || object.bytes == 0
            || object.bytes > ferric_browser_core::versioning::limits::MAX_ARCHIVE_BYTES
            || object.sha256.len() != 64
            || !object
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        let source_url = match &page.safe_url_or_placeholder {
            ManifestSafeUrl::Replayable(url) => Some(url.clone()),
            ManifestSafeUrl::Placeholder(_) => None,
        };
        let captured_at_ms = page
            .page_capture
            .as_ref()
            .and_then(|capture| capture.archive_interval)
            .map_or(manifest.created_at_ms, |interval| interval.finished_at_ms);
        let mutation_label = page
            .page_capture
            .as_ref()
            .map_or("not recorded", |capture| match capture.mutation_evidence {
                ferric_browser_storage::versioning::ManifestMutationEvidence::NotObserved => {
                    "unavailable"
                }
                ferric_browser_storage::versioning::ManifestMutationEvidence::ObservedStable => {
                    "no changes observed; not an atomicity guarantee"
                }
                ferric_browser_storage::versioning::ManifestMutationEvidence::ObservedChanged => {
                    "change observed during interval"
                }
            });
        Some(ArchiveReference {
            sha256: object.sha256.clone(),
            bytes: object.bytes,
            metadata: VersionedArchiveViewMetadata {
                source_url,
                captured_at_ms,
                fidelity_label: format!(
                    "Archive: {}; page state: {}; mutation evidence: {mutation_label}",
                    coverage_label(page.fidelity.archive),
                    coverage_label(page.fidelity.page)
                ),
            },
        })
    }
}

fn coverage_label(coverage: ManifestCoverage) -> &'static str {
    match coverage {
        ManifestCoverage::Unknown => "unknown",
        ManifestCoverage::NotRequested => "not requested",
        ManifestCoverage::Unavailable => "unavailable",
        ManifestCoverage::Partial => "partial",
        ManifestCoverage::CompleteForRequest => "complete for request",
    }
}

struct ArchiveReference {
    sha256: String,
    bytes: u64,
    metadata: VersionedArchiveViewMetadata,
}

fn archive_failure(reason: VersioningReason) -> VersioningFailure {
    VersioningFailure::new(
        reason,
        ferric_browser_core::versioning::SensitiveCategory::Page,
    )
}

fn archive_failure_effect(ticket: StorageTicket, message: &'static str) -> StorageEffect {
    StorageEffect {
        ticket,
        completion: StorageCompletion::VersioningArchiveView(Err(StorageOperationError {
            operation: StorageOperation::VersioningArchiveView,
            kind: ferric_browser_storage::StorageFailureKind::Other,
            message: message.to_owned(),
        })),
    }
}
