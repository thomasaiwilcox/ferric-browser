//! Transactional publication on the normal profile metadata owner. Recovery
//! and the browser-facing acknowledgment remain separate, incomplete work.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use ferric_browser_core::{CommitId, OperationId, versioning::limits::MAX_LIST_ITEMS};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::manifest::MAX_MANIFEST_BYTES;
use super::retention::{
    MAX_PROFILE_VERSIONING_BYTES, MAX_RETENTION_ROWS, VersioningJournalEvent,
    append_bounded_journal, bounded_profile_versioning_bytes, check_profile_capture_slot_capacity,
    check_profile_versioning_budget, ensure_bounded_profile_object_link_rows,
};
use super::{
    CaptureCategory, CaptureRequest, ManifestCoverage, ManifestObjectRef,
    ManifestPageArchiveFailure, ManifestSafeUrl, ManifestScope, ManifestUnavailable,
    VersionedCaptureManifest,
    objects::{
        CaptureStageCleanupCheckpoint, ObjectPathError, RecoverySummary, VersioningObjectStore,
        sha256_hex,
    },
};
use crate::{ProfileStore, StoreError};

const MAX_COMMIT_OBJECT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_SINGLE_OBJECT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RECOVERY_REFERENCES: usize = 100_000;

/// Redacted metadata for one exact normal-profile commit. No message, URL,
/// title, page body or cookie value leaves the metadata worker in a listing.
#[derive(Clone, Eq, PartialEq)]
pub struct CommitSummary {
    pub id: CommitId,
    pub created_at_ms: u64,
    pub pinned: bool,
    pub object_links: u32,
}

impl fmt::Debug for CommitSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommitSummary")
            .field("id", &"[redacted]")
            .field("created_at_ms", &"[redacted]")
            .field("pinned", &self.pinned)
            .field("object_links", &self.object_links)
            .finish()
    }
}

/// Bounded, redacted detail for one exact commit. No message, raw URL, title,
/// page body, cookie or complete manifest bytes leave the metadata worker.
#[derive(Clone, Eq, PartialEq)]
pub struct CommitInfo {
    pub summary: CommitSummary,
    pub scope: ManifestScope,
    pub window_count: u16,
    pub page_count: u16,
    pub replayable_pages: u16,
    /// URL placeholders in `NotObserved`, `Unsupported`, `UnsafeNavigation` order.
    pub unavailable_urls: [u16; 3],
    pub archive_objects: u16,
    pub sidecar_objects: u16,
    pub missing_objects: u16,
    pub quarantined_objects: u16,
    pub requested_capture: Vec<CaptureCategory>,
    pub partial_pages_allowed: bool,
    pub archive_failures: Vec<CommitArchiveFailure>,
    pub archive_coverage: CoverageCounts,
    pub page_coverage: CoverageCounts,
    pub cookie_coverage: CoverageCounts,
    pub engine_fingerprint: String,
    pub helper_bundle_version: String,
    pub warning_count: u16,
    pub max_manifest_bytes: usize,
    pub max_single_object_bytes: u64,
    pub max_commit_object_bytes: u64,
}

impl fmt::Debug for CommitInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommitInfo")
            .field("summary", &self.summary)
            .field("scope", &self.scope)
            .field("window_count", &self.window_count)
            .field("page_count", &self.page_count)
            .field("replayable_pages", &self.replayable_pages)
            .field("unavailable_urls", &self.unavailable_urls)
            .field("archive_objects", &self.archive_objects)
            .field("sidecar_objects", &self.sidecar_objects)
            .field("missing_objects", &self.missing_objects)
            .field("quarantined_objects", &self.quarantined_objects)
            .field("requested_capture_count", &self.requested_capture.len())
            .field("partial_pages_allowed", &self.partial_pages_allowed)
            .field("archive_failure_count", &self.archive_failures.len())
            .field("archive_coverage", &self.archive_coverage)
            .field("page_coverage", &self.page_coverage)
            .field("cookie_coverage", &self.cookie_coverage)
            .field("engine_fingerprint", &"[redacted]")
            .field("helper_bundle_version", &"[redacted]")
            .field("warning_count", &self.warning_count)
            .finish_non_exhaustive()
    }
}

/// Opaque logical identity and payload-free failure for one requested page.
#[derive(Clone, Eq, PartialEq)]
pub struct CommitArchiveFailure {
    pub logical_page_id: String,
    pub reason: ManifestPageArchiveFailure,
}

impl fmt::Debug for CommitArchiveFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommitArchiveFailure")
            .field("logical_page_id", &"[redacted]")
            .field("reason", &self.reason)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoverageCounts {
    pub unknown: u16,
    pub not_requested: u16,
    pub unavailable: u16,
    pub partial: u16,
    pub complete_for_request: u16,
}

impl CoverageCounts {
    fn note(&mut self, coverage: ManifestCoverage) {
        match coverage {
            ManifestCoverage::Unknown => self.unknown += 1,
            ManifestCoverage::NotRequested => self.not_requested += 1,
            ManifestCoverage::Unavailable => self.unavailable += 1,
            ManifestCoverage::Partial => self.partial += 1,
            ManifestCoverage::CompleteForRequest => self.complete_for_request += 1,
        }
    }
}

impl CommitInfo {
    fn project(
        summary: CommitSummary,
        manifest: &VersionedCaptureManifest,
        missing_objects: u16,
        quarantined_objects: u16,
    ) -> Result<Self, StoreError> {
        let mut replayable_pages = 0_u16;
        let mut unavailable_urls = [0_u16; 3];
        let mut archive_objects = 0_u16;
        let mut sidecar_objects = 0_u16;
        let mut page_count = 0_u16;
        let mut archive_coverage = CoverageCounts::default();
        let mut page_coverage = CoverageCounts::default();
        let mut cookie_coverage = CoverageCounts::default();
        let mut archive_failures = Vec::new();
        for page in manifest.windows.iter().flat_map(|window| &window.pages) {
            page_count = page_count
                .checked_add(1)
                .ok_or(StoreError::InvalidInput("stored page count is invalid"))?;
            match &page.safe_url_or_placeholder {
                ManifestSafeUrl::Replayable(_) => replayable_pages += 1,
                ManifestSafeUrl::Placeholder(ManifestUnavailable::NotObserved) => {
                    unavailable_urls[0] += 1;
                }
                ManifestSafeUrl::Placeholder(ManifestUnavailable::Unsupported) => {
                    unavailable_urls[1] += 1;
                }
                ManifestSafeUrl::Placeholder(ManifestUnavailable::UnsafeNavigation) => {
                    unavailable_urls[2] += 1;
                }
            }
            archive_objects += u16::from(page.archive_object.is_some());
            if let Some(reason) = page.archive_failure {
                archive_failures.push(CommitArchiveFailure {
                    logical_page_id: page.logical_page_id.clone(),
                    reason,
                });
            }
            sidecar_objects += u16::from(page.sidecar_object.is_some());
            archive_coverage.note(page.fidelity.archive);
            page_coverage.note(page.fidelity.page);
            cookie_coverage.note(page.fidelity.cookies);
        }
        if summary.object_links != u32::from(archive_objects) + u32::from(sidecar_objects) {
            return Err(StoreError::InvalidInput(
                "stored object links disagree with manifest",
            ));
        }
        Ok(Self {
            summary,
            scope: manifest.scope,
            window_count: u16::try_from(manifest.windows.len())
                .map_err(|_| StoreError::InvalidInput("stored window count is invalid"))?,
            page_count,
            replayable_pages,
            unavailable_urls,
            archive_objects,
            sidecar_objects,
            missing_objects,
            quarantined_objects,
            requested_capture: manifest
                .requested_capture
                .categories
                .iter()
                .copied()
                .collect(),
            partial_pages_allowed: manifest.requested_capture.allow_partial_pages,
            archive_failures,
            archive_coverage,
            page_coverage,
            cookie_coverage,
            engine_fingerprint: manifest.engine_fingerprint.clone(),
            helper_bundle_version: manifest.helper_bundle_version.clone(),
            warning_count: u16::try_from(manifest.warnings.len())
                .map_err(|_| StoreError::InvalidInput("stored warning count is invalid"))?,
            max_manifest_bytes: MAX_MANIFEST_BYTES,
            max_single_object_bytes: MAX_SINGLE_OBJECT_BYTES,
            max_commit_object_bytes: MAX_COMMIT_OBJECT_BYTES,
        })
    }
}

struct ValidatedReference {
    hash: String,
    bytes: u64,
    role: &'static str,
    logical_page_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublishCheckpoint {
    BeforeTransaction,
    AfterOperationComplete,
    AfterCommitRow,
    AfterObjectLinks,
    BeforeTransactionCommit,
    AfterTransactionCommit,
    BeforeStageLeafRemoval(usize),
    AfterStageLeafRemoval(usize),
    BeforeStageOperationDirectorySync,
    AfterStageOperationDirectorySync,
    AfterStageOperationDirectoryRemoval,
    BeforeStageParentDirectorySync,
    AfterStageParentDirectorySync,
}

impl ProfileStore {
    /// Looks up one exact commit under the bound profile without returning
    /// manifest payloads or cross-profile matches.
    ///
    /// # Errors
    /// Rejects malformed stored metadata.
    pub(crate) fn versioning_commit_info(
        &self,
        profile_uuid: Uuid,
        id: CommitId,
    ) -> Result<Option<CommitInfo>, StoreError> {
        let Some((manifest, summary)) = self.checked_versioning_commit(profile_uuid, id)? else {
            return Ok(None);
        };
        let (missing, quarantined) = bounded_commit_object_state_counts(
            &self.connection,
            profile_uuid,
            id,
            MAX_RETENTION_ROWS,
        )?;
        CommitInfo::project(summary, &manifest, missing, quarantined).map(Some)
    }

    /// Returns a bounded, hash-checked current manifest (including explicit
    /// in-memory legacy migration) only to the trusted
    /// application coordinator for restore planning. Presentation and IPC
    /// callers must use redacted `versioning_commit_info` instead.
    ///
    /// # Errors
    /// Rejects malformed, oversized or cross-profile stored metadata.
    pub(crate) fn versioning_commit_manifest(
        &self,
        profile_uuid: Uuid,
        id: CommitId,
    ) -> Result<Option<VersionedCaptureManifest>, StoreError> {
        self.checked_versioning_commit(profile_uuid, id)
            .map(|result| result.map(|(manifest, _)| manifest))
    }

    fn checked_versioning_commit(
        &self,
        profile_uuid: Uuid,
        id: CommitId,
    ) -> Result<Option<(VersionedCaptureManifest, CommitSummary)>, StoreError> {
        let owner = profile_uuid.hyphenated().to_string();
        let commit_id = id.to_string();
        let metadata: Option<(i64, String)> = self
            .connection
            .query_row(
                "SELECT length(manifest_bytes),manifest_hash FROM vb_commits
             WHERE profile_uuid=?1 AND id=?2",
                params![owner, commit_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((length, hash)) = metadata else {
            return Ok(None);
        };
        let expected_len = usize::try_from(length)
            .ok()
            .filter(|length| *length <= MAX_MANIFEST_BYTES)
            .ok_or(StoreError::InvalidInput("stored manifest size is invalid"))?;
        let bytes: Vec<u8> = self.connection.query_row(
            "SELECT manifest_bytes FROM vb_commits WHERE profile_uuid=?1 AND id=?2",
            params![owner, commit_id],
            |row| row.get(0),
        )?;
        if bytes.len() != expected_len {
            return Err(StoreError::InvalidInput("stored manifest size changed"));
        }
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if sha256_hex(&digest) != hash {
            return Err(StoreError::InvalidInput("stored manifest hash is invalid"));
        }
        let manifest = VersionedCaptureManifest::decode(&bytes)
            .map_err(|_| StoreError::InvalidInput("stored manifest is invalid"))?;
        if manifest.id != commit_id || manifest.profile_uuid != profile_uuid {
            return Err(StoreError::InvalidInput("stored manifest owner is invalid"));
        }
        let object_links = bounded_commit_object_link_count(
            &self.connection,
            profile_uuid,
            id,
            MAX_RETENTION_ROWS,
        )?;
        let summary: Option<(String, i64, i64)> = self
            .connection
            .query_row(
                "SELECT c.id,c.created_at,c.pinned
             FROM vb_commits c
             WHERE c.profile_uuid=?1 AND c.id=?2",
                params![owner, commit_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (summary_id, created_at, pinned) =
            summary.ok_or(StoreError::InvalidInput("stored commit disappeared"))?;
        let summary = commit_summary_from_row((summary_id, created_at, pinned, object_links))?;
        if summary.created_at_ms != manifest.created_at_ms {
            return Err(StoreError::InvalidInput(
                "stored commit time disagrees with manifest",
            ));
        }
        Ok(Some((manifest, summary)))
    }

    /// Lists at most 1,000 redacted commits for one exact normal owner.
    ///
    /// # Errors
    /// Rejects invalid bounds and corrupt stored IDs/counts.
    pub(crate) fn list_versioning_commits(
        &self,
        profile_uuid: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<CommitSummary>, StoreError> {
        // The application reads at most one row beyond the requested page to
        // compute an exact continuation marker without an unbounded count.
        if !(1..=(MAX_LIST_ITEMS + 1)).contains(&limit) || offset > 100_000 {
            return Err(StoreError::InvalidInput(
                "commit listing bounds are invalid",
            ));
        }
        let limit = i64::try_from(limit)
            .map_err(|_| StoreError::InvalidInput("commit listing limit is invalid"))?;
        let offset = i64::try_from(offset)
            .map_err(|_| StoreError::InvalidInput("commit listing offset is invalid"))?;
        let mut statement = self.connection.prepare(
            "SELECT c.id,c.created_at,c.pinned
             FROM vb_commits c
             WHERE c.profile_uuid=?1
             ORDER BY c.created_at DESC,c.id DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let rows = statement.query_map(
            params![profile_uuid.hyphenated().to_string(), limit, offset],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?;
        let page_rows = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut link_counts = bounded_page_object_link_counts(
            &self.connection,
            profile_uuid,
            limit,
            offset,
            MAX_RETENTION_ROWS,
        )?;
        let mut summaries = Vec::with_capacity(page_rows.len());
        for (id, created_at, pinned) in page_rows {
            let object_links = link_counts.remove(&id).unwrap_or(0);
            summaries.push(commit_summary_from_row((
                id,
                created_at,
                pinned,
                object_links,
            ))?);
        }
        Ok(summaries)
    }

    /// Durably records a capture reservation before native staging begins.
    /// The metadata worker must have bound the exact normal profile.
    ///
    /// # Errors
    /// Rejects duplicate operations or a failed durable transaction.
    #[cfg(test)]
    pub(crate) fn begin_versioning_capture(
        &mut self,
        operation: OperationId,
        profile_uuid: Uuid,
    ) -> Result<(), StoreError> {
        self.begin_versioning_capture_reserved(operation, profile_uuid, 0, 0)
    }

    /// Durably admits one capture with an explicit archive-plus-manifest
    /// reservation before native save starts. Reservations live only in the
    /// bounded recovery metadata marker and are released atomically on publish
    /// or failure; no page-derived data is stored.
    pub(crate) fn begin_versioning_capture_reserved(
        &mut self,
        operation: OperationId,
        profile_uuid: Uuid,
        reserved_bytes: u64,
        unindexed_file_bytes: u64,
    ) -> Result<(), StoreError> {
        if reserved_bytes > super::MAX_WINDOW_CAPTURE_RESERVATION_BYTES {
            return Err(StoreError::InvalidInput(
                "capture reservation exceeds the bounded window limit",
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        reserve_profile_capture_budget(
            &transaction,
            profile_uuid,
            reserved_bytes,
            unindexed_file_bytes,
        )?;
        let recovery_marker = if reserved_bytes == 0 {
            "awaiting-native-capture".to_owned()
        } else {
            format!("capture-reserved-v1:{reserved_bytes}")
        };
        transaction.execute(
            "INSERT INTO vb_operations(id,profile_uuid,kind,state,safe_recovery_metadata)
             VALUES (?1,?2,'capture','staging',?3)",
            params![
                operation.to_string(),
                profile_uuid.hyphenated().to_string(),
                recovery_marker
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Identifies ready object rows already included in the durable byte sum
    /// so a filesystem admission scan counts only unindexed live objects.
    pub(crate) fn ready_versioning_object_sizes(
        &self,
    ) -> Result<BTreeMap<String, u64>, StoreError> {
        load_bounded_ready_object_sizes(&self.connection, MAX_RECOVERY_REFERENCES)
    }

    /// Returns the exact active capture reservations already included in the
    /// durable budget, keyed by their private staging operation directory.
    pub(crate) fn active_versioning_capture_reservations(
        &self,
        profile_uuid: Uuid,
    ) -> Result<BTreeMap<String, u64>, StoreError> {
        load_active_capture_reservations(&self.connection, profile_uuid, MAX_RECOVERY_REFERENCES)
    }

    /// Quarantines orphan files after all capture writers have stopped, then
    /// marks incomplete operation rows as interrupted. No bytes are deleted.
    ///
    /// # Errors
    /// Refuses wrong owners, oversized indexes or unsafe files. A partial
    /// quarantine remains recoverable on the next restart.
    pub(crate) fn recover_versioning_after_restart(
        &mut self,
        profile_uuid: Uuid,
        objects: &VersioningObjectStore,
    ) -> Result<RecoverySummary, StoreError> {
        self.recover_versioning_after_restart_with_limit(
            profile_uuid,
            objects,
            MAX_RECOVERY_REFERENCES,
        )
    }

    fn recover_versioning_after_restart_with_limit(
        &mut self,
        profile_uuid: Uuid,
        objects: &VersioningObjectStore,
        maximum_references: usize,
    ) -> Result<RecoverySummary, StoreError> {
        self.recover_versioning_after_restart_with_limits(
            profile_uuid,
            objects,
            maximum_references,
            MAX_RECOVERY_REFERENCES,
        )
    }

    fn recover_versioning_after_restart_with_limits(
        &mut self,
        profile_uuid: Uuid,
        objects: &VersioningObjectStore,
        maximum_references: usize,
        maximum_link_rows: usize,
    ) -> Result<RecoverySummary, StoreError> {
        if objects.profile_uuid() != profile_uuid {
            return Err(StoreError::InvalidInput(
                "recovery profile owner is invalid",
            ));
        }
        let profile = profile_uuid.hyphenated().to_string();
        let referenced = load_bounded_recovery_references_with_link_limit(
            &self.connection,
            &profile,
            maximum_references,
            maximum_link_rows,
        )?;
        let summary = objects
            .quarantine_after_restart(&referenced)
            .map_err(|_| StoreError::InvalidInput("versioning quarantine failed"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let interrupted = transaction.execute(
            "UPDATE vb_operations
             SET state='interrupted',safe_recovery_metadata='quarantined-after-restart'
             WHERE profile_uuid=?1
               AND state NOT IN (
                   'complete','failed','cancelled','interrupted','partial','outcome-unknown'
               )",
            [&profile],
        )?;
        if interrupted != 0 || summary != RecoverySummary::default() {
            append_bounded_journal(
                &transaction,
                profile_uuid,
                VersioningJournalEvent::RecoveryQuarantined,
            )?;
        }
        transaction.commit()?;
        Ok(summary)
    }

    /// Atomically publishes a validated, structural-only commit and its safe
    /// operation record. The caller must be the normal profile worker.
    ///
    /// # Errors
    /// Rejects a mismatched owner, object-bearing manifest, malformed metadata,
    /// duplicate IDs or a failed SQLite transaction without partial visibility.
    pub(crate) fn publish_structural_commit(
        &mut self,
        operation: OperationId,
        profile_uuid: Uuid,
        manifest: &VersionedCaptureManifest,
    ) -> Result<(), StoreError> {
        if manifest.profile_uuid != profile_uuid
            || manifest.requested_capture != CaptureRequest::structural_only()
            || manifest
                .windows
                .iter()
                .flat_map(|window| &window.pages)
                .any(|page| page.archive_object.is_some() || page.sidecar_object.is_some())
        {
            return Err(StoreError::InvalidInput(
                "structural publication owner or capture category is invalid",
            ));
        }
        self.publish_manifest_transaction(operation, profile_uuid, manifest, &[])
    }

    /// Revalidates all referenced objects before atomically publishing their
    /// manifest and link rows. The normal profile worker must supply the exact
    /// owner-bound object store; no UI or page path is accepted here.
    ///
    /// # Errors
    /// Rejects any missing/corrupt object, owner mismatch, over-budget
    /// manifest or failed SQLite transaction without a partial commit. The
    /// returned flag says whether exact-operation staging cleanup completed.
    pub(crate) fn publish_commit_with_objects(
        &mut self,
        operation: OperationId,
        profile_uuid: Uuid,
        manifest: &VersionedCaptureManifest,
        objects: &VersioningObjectStore,
    ) -> Result<bool, StoreError> {
        self.publish_commit_with_objects_with_checkpoint(
            operation,
            profile_uuid,
            manifest,
            objects,
            |_| Ok(()),
        )
    }

    fn publish_commit_with_objects_with_checkpoint(
        &mut self,
        operation: OperationId,
        profile_uuid: Uuid,
        manifest: &VersionedCaptureManifest,
        objects: &VersioningObjectStore,
        checkpoint: impl FnMut(PublishCheckpoint) -> Result<(), StoreError>,
    ) -> Result<bool, StoreError> {
        self.publish_commit_with_objects_with_checkpoint_and_sync(
            operation,
            profile_uuid,
            manifest,
            objects,
            checkpoint,
            |directory| directory.sync_all(),
        )
    }

    fn publish_commit_with_objects_with_checkpoint_and_sync(
        &mut self,
        operation: OperationId,
        profile_uuid: Uuid,
        manifest: &VersionedCaptureManifest,
        objects: &VersioningObjectStore,
        mut checkpoint: impl FnMut(PublishCheckpoint) -> Result<(), StoreError>,
        mut sync_directory: impl FnMut(&std::fs::File) -> std::io::Result<()>,
    ) -> Result<bool, StoreError> {
        if manifest.profile_uuid != profile_uuid || objects.profile_uuid() != profile_uuid {
            return Err(StoreError::InvalidInput("capture profile owner is invalid"));
        }
        manifest
            .encode()
            .map_err(|_| StoreError::InvalidInput("capture manifest is invalid"))?;
        let mut references = Vec::new();
        let mut unique = BTreeMap::<String, u64>::new();
        let mut total_unique_bytes = 0u64;
        for page in manifest.windows.iter().flat_map(|window| &window.pages) {
            if let Some(object) = &page.archive_object {
                append_reference(
                    &mut references,
                    &mut unique,
                    &mut total_unique_bytes,
                    object,
                    "archive",
                    &page.logical_page_id,
                )?;
            }
            if let Some(object) = &page.sidecar_object {
                append_reference(
                    &mut references,
                    &mut unique,
                    &mut total_unique_bytes,
                    object,
                    "sidecar",
                    &page.logical_page_id,
                )?;
            }
        }
        for (hash, bytes) in &unique {
            objects
                .verify_object_reference(hash, *bytes, MAX_SINGLE_OBJECT_BYTES)
                .map_err(|_| {
                    StoreError::InvalidInput("manifest object is unavailable or corrupt")
                })?;
        }
        self.publish_manifest_transaction_with_checkpoint(
            operation,
            profile_uuid,
            manifest,
            &references,
            &mut checkpoint,
        )?;
        // The immutable references are now durable. A failed exact cleanup
        // cannot undo that commit, so leave the private stage for conservative
        // accounting and restart quarantine instead of reporting false failure.
        let cleanup = objects.discard_published_capture_staging_with_checkpoint_and_sync(
            operation,
            |point| {
                let publication_point = match point {
                    CaptureStageCleanupCheckpoint::BeforeLeafRemoval(index) => {
                        PublishCheckpoint::BeforeStageLeafRemoval(index)
                    }
                    CaptureStageCleanupCheckpoint::AfterLeafRemoval(index) => {
                        PublishCheckpoint::AfterStageLeafRemoval(index)
                    }
                    CaptureStageCleanupCheckpoint::BeforeOperationDirectorySync => {
                        PublishCheckpoint::BeforeStageOperationDirectorySync
                    }
                    CaptureStageCleanupCheckpoint::AfterOperationDirectorySync => {
                        PublishCheckpoint::AfterStageOperationDirectorySync
                    }
                    CaptureStageCleanupCheckpoint::AfterOperationDirectoryRemoval => {
                        PublishCheckpoint::AfterStageOperationDirectoryRemoval
                    }
                    CaptureStageCleanupCheckpoint::BeforeStagingDirectorySync => {
                        PublishCheckpoint::BeforeStageParentDirectorySync
                    }
                    CaptureStageCleanupCheckpoint::AfterStagingDirectorySync => {
                        PublishCheckpoint::AfterStageParentDirectorySync
                    }
                };
                checkpoint(publication_point).map_err(|_| ObjectPathError::UnsafeEntry)
            },
            |directory| sync_directory(directory),
        );
        Ok(cleanup.is_ok())
    }

    fn publish_manifest_transaction(
        &mut self,
        operation: OperationId,
        profile_uuid: Uuid,
        manifest: &VersionedCaptureManifest,
        references: &[ValidatedReference],
    ) -> Result<(), StoreError> {
        self.publish_manifest_transaction_with_checkpoint(
            operation,
            profile_uuid,
            manifest,
            references,
            |_| Ok(()),
        )
    }

    fn publish_manifest_transaction_with_checkpoint(
        &mut self,
        operation: OperationId,
        profile_uuid: Uuid,
        manifest: &VersionedCaptureManifest,
        references: &[ValidatedReference],
        mut checkpoint: impl FnMut(PublishCheckpoint) -> Result<(), StoreError>,
    ) -> Result<(), StoreError> {
        let bytes = manifest
            .encode()
            .map_err(|_| StoreError::InvalidInput("capture manifest is invalid"))?;
        let hash: [u8; 32] = Sha256::digest(&bytes).into();
        let created_at = i64::try_from(manifest.created_at_ms)
            .map_err(|_| StoreError::InvalidInput("capture time is out of range"))?;
        let profile = profile_uuid.hyphenated().to_string();
        let operation_id = operation.to_string();
        checkpoint(PublishCheckpoint::BeforeTransaction)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        check_publish_budget(&transaction, profile_uuid, references, bytes.len())?;
        complete_capture_operation(&transaction, &operation_id, &profile)?;
        checkpoint(PublishCheckpoint::AfterOperationComplete)?;
        insert_published_commit_row(&transaction, manifest, &bytes, &hash, &profile, created_at)?;
        checkpoint(PublishCheckpoint::AfterCommitRow)?;
        insert_published_object_links(&transaction, manifest, references, created_at)?;
        checkpoint(PublishCheckpoint::AfterObjectLinks)?;
        append_bounded_journal(
            &transaction,
            profile_uuid,
            VersioningJournalEvent::CapturePublished {
                created_at_ms: created_at,
                operation,
            },
        )?;
        checkpoint(PublishCheckpoint::BeforeTransactionCommit)?;
        transaction.commit()?;
        checkpoint(PublishCheckpoint::AfterTransactionCommit)?;
        Ok(())
    }
}

fn load_bounded_ready_object_sizes(
    connection: &Connection,
    maximum: usize,
) -> Result<BTreeMap<String, u64>, StoreError> {
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("versioning object scan limit is invalid"))?;
    let mut objects = BTreeMap::new();
    let mut query = connection
        .prepare("SELECT hash,bytes FROM vb_objects WHERE state='ready' ORDER BY hash LIMIT ?1")?;
    let rows = query.query_map([scan_limit], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in rows {
        if objects.len() >= maximum {
            return Err(StoreError::InvalidInput(
                "versioning object index exceeds admission limit",
            ));
        }
        let (hash, bytes) = row?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StoreError::InvalidInput("stored object hash is invalid"));
        }
        let bytes = u64::try_from(bytes)
            .map_err(|_| StoreError::InvalidInput("stored object size is invalid"))?;
        objects.insert(hash, bytes);
    }
    Ok(objects)
}

fn load_bounded_recovery_references(
    connection: &Connection,
    profile: &str,
    maximum: usize,
) -> Result<BTreeSet<String>, StoreError> {
    load_bounded_recovery_references_with_link_limit(
        connection,
        profile,
        maximum,
        MAX_RECOVERY_REFERENCES,
    )
}

fn load_bounded_recovery_references_with_link_limit(
    connection: &Connection,
    profile: &str,
    maximum: usize,
    maximum_link_rows: usize,
) -> Result<BTreeSet<String>, StoreError> {
    ensure_bounded_profile_object_link_rows(connection, profile, maximum_link_rows)?;
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("recovery scan limit is invalid"))?;
    let mut referenced = BTreeSet::new();
    let mut query = connection.prepare(
        "SELECT DISTINCT co.object_hash
         FROM vb_commit_objects co
         JOIN vb_commits c ON c.id=co.commit_id
         WHERE c.profile_uuid=?1
         ORDER BY co.object_hash LIMIT ?2",
    )?;
    let rows = query.query_map(params![profile, scan_limit], |row| row.get::<_, String>(0))?;
    for row in rows {
        if referenced.len() >= maximum {
            return Err(StoreError::InvalidInput("recovery index exceeds limit"));
        }
        referenced.insert(row?);
    }
    Ok(referenced)
}

fn check_publish_budget(
    transaction: &Transaction<'_>,
    profile_uuid: Uuid,
    references: &[ValidatedReference],
    manifest_bytes: usize,
) -> Result<(), StoreError> {
    let mut incoming = BTreeMap::new();
    for reference in references {
        if let Some(bytes) = incoming.insert(reference.hash.clone(), reference.bytes)
            && bytes != reference.bytes
        {
            return Err(StoreError::InvalidInput(
                "object size conflicts with manifest",
            ));
        }
    }
    check_profile_versioning_budget(
        transaction,
        profile_uuid,
        &incoming,
        references.len(),
        u64::try_from(manifest_bytes)
            .map_err(|_| StoreError::InvalidInput("manifest size is invalid"))?,
    )
}

fn complete_capture_operation(
    transaction: &Transaction<'_>,
    operation_id: &str,
    profile: &str,
) -> Result<(), StoreError> {
    let existing: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT profile_uuid,kind,state FROM vb_operations WHERE id=?1",
            [operation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    match existing {
        Some((owner, kind, state))
            if owner == profile && kind == "capture" && state == "staging" =>
        {
            transaction.execute(
                "UPDATE vb_operations
                 SET state='complete',safe_recovery_metadata='manifest-v1'
                 WHERE id=?1 AND profile_uuid=?2",
                params![operation_id, profile],
            )?;
            Ok(())
        }
        None | Some(_) => Err(StoreError::InvalidInput("capture operation cannot publish")),
    }
}

fn insert_published_commit_row(
    transaction: &Transaction<'_>,
    manifest: &VersionedCaptureManifest,
    bytes: &[u8],
    hash: &[u8; 32],
    profile: &str,
    created_at: i64,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO vb_commits(id,profile_uuid,parent_id,created_at,message,manifest_schema,manifest_bytes,manifest_hash,pinned)
         VALUES (?1,?2,?3,?4,?5,1,?6,?7,0)",
        params![
            manifest.id,
            profile,
            manifest.parent_commit,
            created_at,
            manifest.message,
            bytes,
            sha256_hex(hash)
        ],
    )?;
    Ok(())
}

fn insert_published_object_links(
    transaction: &Transaction<'_>,
    manifest: &VersionedCaptureManifest,
    references: &[ValidatedReference],
    created_at: i64,
) -> Result<(), StoreError> {
    for reference in references {
        insert_commit_object_reference(transaction, manifest, reference, created_at)?;
    }
    Ok(())
}

fn insert_commit_object_reference(
    transaction: &Transaction<'_>,
    manifest: &VersionedCaptureManifest,
    reference: &ValidatedReference,
    created_at: i64,
) -> Result<(), StoreError> {
    let reference_bytes = i64::try_from(reference.bytes)
        .map_err(|_| StoreError::InvalidInput("object size is out of range"))?;
    transaction.execute(
        "INSERT OR IGNORE INTO vb_objects(hash,kind,bytes,state,created_at)
         VALUES (?1,'sha256',?2,'ready',?3)",
        params![reference.hash, reference_bytes, created_at],
    )?;
    let (kind, existing_bytes, state): (String, i64, String) = transaction.query_row(
        "SELECT kind,bytes,state FROM vb_objects WHERE hash=?1",
        [&reference.hash],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if kind != "sha256" || existing_bytes != reference_bytes || state != "ready" {
        return Err(StoreError::InvalidInput(
            "object index conflicts with manifest",
        ));
    }
    transaction.execute(
        "INSERT INTO vb_commit_objects(commit_id,object_hash,role,logical_page_id)
         VALUES (?1,?2,?3,?4)",
        params![
            manifest.id,
            reference.hash,
            reference.role,
            reference.logical_page_id
        ],
    )?;
    Ok(())
}

fn commit_summary_from_row(
    (id, created_at, pinned, object_links): (String, i64, i64, i64),
) -> Result<CommitSummary, StoreError> {
    let object_links = usize::try_from(object_links)
        .ok()
        .filter(|count| *count <= MAX_RETENTION_ROWS)
        .ok_or(StoreError::InvalidInput("stored object count is invalid"))?;
    Ok(CommitSummary {
        id: id
            .parse()
            .map_err(|_| StoreError::InvalidInput("stored commit id is invalid"))?,
        created_at_ms: u64::try_from(created_at)
            .map_err(|_| StoreError::InvalidInput("stored capture time is invalid"))?,
        pinned: match pinned {
            0 => false,
            1 => true,
            _ => return Err(StoreError::InvalidInput("stored pin state is invalid")),
        },
        object_links: u32::try_from(object_links)
            .map_err(|_| StoreError::InvalidInput("stored object count is invalid"))?,
    })
}

fn bounded_commit_object_link_count(
    connection: &Connection,
    profile_uuid: Uuid,
    id: CommitId,
    maximum: usize,
) -> Result<i64, StoreError> {
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("commit object link limit is invalid"))?;
    let count: i64 = connection.query_row(
        "SELECT count(*) FROM (
            SELECT co.object_hash
            FROM vb_commit_objects co
            JOIN vb_commits c ON c.id=co.commit_id
            WHERE c.profile_uuid=?1 AND c.id=?2
            LIMIT ?3
         ) AS bounded_links",
        params![
            profile_uuid.hyphenated().to_string(),
            id.to_string(),
            scan_limit
        ],
        |row| row.get(0),
    )?;
    let count = usize::try_from(count)
        .map_err(|_| StoreError::InvalidInput("stored object count is invalid"))?;
    if count > maximum {
        return Err(StoreError::InvalidInput(
            "commit object link index exceeds limit",
        ));
    }
    i64::try_from(count).map_err(|_| StoreError::InvalidInput("stored object count is invalid"))
}

fn bounded_commit_object_state_counts(
    connection: &Connection,
    profile_uuid: Uuid,
    id: CommitId,
    maximum: usize,
) -> Result<(u16, u16), StoreError> {
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("commit object link limit is invalid"))?;
    let mut missing = BTreeSet::new();
    let mut quarantined = BTreeSet::new();
    let mut statement = connection.prepare(
        "SELECT o.hash,o.state
         FROM vb_commit_objects co
         JOIN vb_commits c ON c.id=co.commit_id
         JOIN vb_objects o ON o.hash=co.object_hash
         WHERE c.profile_uuid=?1 AND c.id=?2
         LIMIT ?3",
    )?;
    let rows = statement.query_map(
        params![
            profile_uuid.hyphenated().to_string(),
            id.to_string(),
            scan_limit
        ],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    let mut scanned = 0_usize;
    for row in rows {
        if scanned >= maximum {
            return Err(StoreError::InvalidInput(
                "commit object state index exceeds limit",
            ));
        }
        scanned += 1;
        let (hash, state) = row?;
        match state.as_str() {
            "ready" => {}
            "missing" => {
                missing.insert(hash);
            }
            "quarantined" => {
                quarantined.insert(hash);
            }
            _ => return Err(StoreError::InvalidInput("stored object state is invalid")),
        }
    }
    let missing = u16::try_from(missing.len())
        .map_err(|_| StoreError::InvalidInput("missing object count is invalid"))?;
    let quarantined = u16::try_from(quarantined.len())
        .map_err(|_| StoreError::InvalidInput("quarantined object count is invalid"))?;
    Ok((missing, quarantined))
}

fn bounded_page_object_link_counts(
    connection: &Connection,
    profile_uuid: Uuid,
    limit: i64,
    offset: i64,
    maximum: usize,
) -> Result<BTreeMap<String, i64>, StoreError> {
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("commit object link limit is invalid"))?;
    let mut statement = connection.prepare(
        "WITH page_commits AS (
             SELECT c.id
             FROM vb_commits c
             WHERE c.profile_uuid=?1
             ORDER BY c.created_at DESC,c.id DESC
             LIMIT ?2 OFFSET ?3
         ), bounded_links AS (
             SELECT co.commit_id
             FROM vb_commit_objects co
             JOIN page_commits p ON p.id=co.commit_id
             LIMIT ?4
         )
         SELECT commit_id,count(*) FROM bounded_links GROUP BY commit_id",
    )?;
    let rows = statement.query_map(
        params![
            profile_uuid.hyphenated().to_string(),
            limit,
            offset,
            scan_limit
        ],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )?;
    let mut counts = BTreeMap::new();
    let mut total_links = 0_usize;
    for row in rows {
        let (commit_id, count) = row?;
        let count = usize::try_from(count)
            .map_err(|_| StoreError::InvalidInput("stored object count is invalid"))?;
        total_links = total_links
            .checked_add(count)
            .ok_or(StoreError::InvalidInput("stored object count is invalid"))?;
        if total_links > maximum {
            return Err(StoreError::InvalidInput(
                "commit listing object-link index exceeds limit",
            ));
        }
        counts.insert(
            commit_id,
            i64::try_from(count)
                .map_err(|_| StoreError::InvalidInput("stored object count is invalid"))?,
        );
    }
    Ok(counts)
}

fn reserve_profile_capture_budget(
    connection: &Connection,
    profile_uuid: Uuid,
    incoming_reservation: u64,
    unindexed_file_bytes: u64,
) -> Result<(), StoreError> {
    check_profile_capture_slot_capacity(connection, profile_uuid)?;
    let used = bounded_profile_versioning_bytes(connection, profile_uuid, MAX_RETENTION_ROWS)?;
    let active_reservations =
        load_active_capture_reservations(connection, profile_uuid, MAX_RECOVERY_REFERENCES)?
            .values()
            .try_fold(0u64, |total, reserved| {
                total.checked_add(*reserved).ok_or(StoreError::InvalidInput(
                    "capture reservation total is invalid",
                ))
            })?;
    if used
        .checked_add(unindexed_file_bytes)
        .and_then(|total| total.checked_add(active_reservations))
        .and_then(|total| total.checked_add(incoming_reservation))
        .is_none_or(|total| total > MAX_PROFILE_VERSIONING_BYTES)
    {
        return Err(StoreError::InvalidInput(
            "versioning profile disk budget cannot reserve this capture",
        ));
    }
    Ok(())
}

fn load_active_capture_reservations(
    connection: &Connection,
    profile_uuid: Uuid,
    maximum: usize,
) -> Result<BTreeMap<String, u64>, StoreError> {
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("capture reservation scan limit is invalid"))?;
    let mut reservations = BTreeMap::new();
    let mut query = connection.prepare(
        "SELECT id,safe_recovery_metadata FROM vb_operations
         WHERE profile_uuid=?1 AND kind='capture' AND state='staging'
         ORDER BY id LIMIT ?2",
    )?;
    let rows = query.query_map(
        params![profile_uuid.hyphenated().to_string(), scan_limit],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    for row in rows {
        if reservations.len() >= maximum {
            return Err(StoreError::InvalidInput(
                "active capture reservation index exceeds limit",
            ));
        }
        let (operation, marker) = row?;
        let operation = operation
            .parse::<OperationId>()
            .map_err(|_| StoreError::InvalidInput("active capture operation id is invalid"))?;
        let reserved = if marker == "awaiting-native-capture" {
            0
        } else if let Some(encoded) = marker.strip_prefix("capture-reserved-v1:") {
            if encoded.is_empty() || !encoded.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(StoreError::InvalidInput(
                    "capture reservation metadata is invalid",
                ));
            }
            encoded
                .parse::<u64>()
                .ok()
                .filter(|bytes| *bytes <= super::MAX_WINDOW_CAPTURE_RESERVATION_BYTES)
                .ok_or(StoreError::InvalidInput(
                    "capture reservation metadata is invalid",
                ))?
        } else {
            return Err(StoreError::InvalidInput(
                "active capture reservation metadata is unknown",
            ));
        };
        reservations.insert(operation.as_uuid().hyphenated().to_string(), reserved);
    }
    Ok(reservations)
}

fn append_reference(
    references: &mut Vec<ValidatedReference>,
    unique: &mut BTreeMap<String, u64>,
    total_unique_bytes: &mut u64,
    object: &ManifestObjectRef,
    role: &'static str,
    logical_page_id: &str,
) -> Result<(), StoreError> {
    match unique.get(&object.sha256) {
        Some(bytes) if *bytes != object.bytes => {
            return Err(StoreError::InvalidInput(
                "object size conflicts with manifest",
            ));
        }
        Some(_) => {}
        None => {
            *total_unique_bytes = total_unique_bytes
                .checked_add(object.bytes)
                .filter(|total| *total <= MAX_COMMIT_OBJECT_BYTES)
                .ok_or(StoreError::InvalidInput("commit object budget exceeded"))?;
            unique.insert(object.sha256.clone(), object.bytes);
        }
    }
    references.push(ValidatedReference {
        hash: object.sha256.clone(),
        bytes: object.bytes,
        role,
        logical_page_id: logical_page_id.to_owned(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::retention::MAX_RETENTION_ROWS;
    use super::*;
    use crate::StoreMode;
    use crate::versioning::{
        CaptureCategory, CaptureMonotonicInterval, ManifestCoverage, ManifestMutationEvidence,
        ManifestObjectRef, ManifestObjectRole, ManifestPageCapture, objects::VersioningObjectStore,
    };
    use ferric_browser_core::{
        CaptureField, CommitId, LogicalPageId, LogicalWindowId, SafePageUrl, ScrollPoint,
        StructuralPage, StructuralSnapshot, StructuralWindow, versioning::Coverage,
    };

    #[test]
    fn commit_metadata_debug_redacts_identity_and_environment_values() {
        let id = CommitId::fresh();
        let id_text = id.to_string();
        let summary = CommitSummary {
            id,
            created_at_ms: 1_234_567,
            pinned: false,
            object_links: 1,
        };
        let info = CommitInfo {
            summary: summary.clone(),
            scope: ManifestScope::Window,
            window_count: 1,
            page_count: 1,
            replayable_pages: 1,
            unavailable_urls: [0; 3],
            archive_objects: 0,
            sidecar_objects: 0,
            missing_objects: 0,
            quarantined_objects: 0,
            requested_capture: Vec::new(),
            partial_pages_allowed: false,
            archive_failures: vec![CommitArchiveFailure {
                logical_page_id: "private-logical-page-sentinel".into(),
                reason: ManifestPageArchiveFailure::Timeout,
            }],
            archive_coverage: CoverageCounts::default(),
            page_coverage: CoverageCounts::default(),
            cookie_coverage: CoverageCounts::default(),
            engine_fingerprint: "private-engine-fingerprint".into(),
            helper_bundle_version: "private-helper-version".into(),
            warning_count: 1,
            max_manifest_bytes: 1,
            max_single_object_bytes: 1,
            max_commit_object_bytes: 1,
        };
        let debug = format!("{summary:?} {info:?}");
        for secret in [
            id_text.as_str(),
            "1234567",
            "private-logical-page-sentinel",
            "private-engine-fingerprint",
            "private-helper-version",
        ] {
            assert!(!debug.contains(secret));
        }
    }

    fn manifest(profile_uuid: Uuid) -> VersionedCaptureManifest {
        let page = LogicalPageId::fresh();
        let structural = StructuralSnapshot {
            application_revision: 7,
            windows: vec![StructuralWindow {
                logical_id: LogicalWindowId::fresh(),
                context_ref: None,
                selected_page: Some(page),
                pages: vec![StructuralPage {
                    logical_id: page,
                    lineage_parent: None,
                    url: SafePageUrl::from_observed(Some("https://example.test/"), true),
                    pinned: false,
                    muted: false,
                    zoom_hundredths: 100,
                    scroll: CaptureField::Known(ScrollPoint::new(0.0, 0.0).unwrap()),
                    page_coverage: Coverage::from_counts(Some(1), Some(1)).unwrap(),
                    cookie_coverage: Coverage::from_counts(Some(1), Some(1)).unwrap(),
                }],
            }],
        };
        VersionedCaptureManifest::from_structural(
            super::super::ManifestHeader {
                id: CommitId::fresh(),
                profile_uuid,
                created_at_ms: 102,
                message: "before trial".into(),
                parent_commit: None,
                configuration_revision: 9,
                scope: super::super::ManifestScope::Tab,
                capture_interval: super::super::CaptureInterval {
                    started_at_ms: 100,
                    finished_at_ms: 101,
                },
                engine_fingerprint: "qt-structural-test".into(),
                helper_bundle_version: "structural-1".into(),
            },
            &structural,
        )
        .expect("valid structural manifest")
    }

    #[test]
    fn restart_and_admission_indexes_stop_at_the_first_excess_row() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_objects(hash TEXT,bytes INTEGER,state TEXT);
                 CREATE TABLE vb_commits(id TEXT,profile_uuid TEXT);
                 CREATE TABLE vb_commit_objects(commit_id TEXT,object_hash TEXT);
                 CREATE TABLE vb_operations(
                     id TEXT,profile_uuid TEXT,kind TEXT,state TEXT,safe_recovery_metadata TEXT
                 );",
            )
            .expect("bounded index schema");
        let profile = Uuid::new_v4();
        let other_profile = Uuid::new_v4();
        let profile_text = profile.hyphenated().to_string();
        let hash_a = format!("{:064x}", 1);
        let hash_b = format!("{:064x}", 2);
        for (hash, bytes) in [(&hash_a, 17_i64), (&hash_b, 23_i64)] {
            connection
                .execute(
                    "INSERT INTO vb_objects(hash,bytes,state) VALUES (?1,?2,'ready')",
                    params![hash, bytes],
                )
                .expect("ready object row");
        }
        connection
            .execute_batch(
                "INSERT INTO vb_commits(id,profile_uuid) VALUES
                    ('commit-a','current-profile'),('commit-b','current-profile'),
                    ('commit-other','other-profile');
                 INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES
                    ('commit-a','0000000000000000000000000000000000000000000000000000000000000001'),
                    ('commit-b','0000000000000000000000000000000000000000000000000000000000000001'),
                    ('commit-b','0000000000000000000000000000000000000000000000000000000000000002'),
                    ('commit-other','0000000000000000000000000000000000000000000000000000000000000002');",
            )
            .expect("commit reference rows");
        connection
            .execute(
                "UPDATE vb_commits SET profile_uuid=?1 WHERE profile_uuid='current-profile'",
                [&profile_text],
            )
            .expect("current profile fixture identity");
        connection
            .execute(
                "UPDATE vb_commits SET profile_uuid=?1 WHERE profile_uuid='other-profile'",
                [other_profile.hyphenated().to_string()],
            )
            .expect("other profile fixture identity");
        for _ in 0..2 {
            connection
                .execute(
                    "INSERT INTO vb_operations(id,profile_uuid,kind,state,safe_recovery_metadata)
                     VALUES (?1,?2,'capture','staging','capture-reserved-v1:10')",
                    params![OperationId::fresh().to_string(), profile_text],
                )
                .expect("active capture reservation");
        }
        connection
            .execute(
                "INSERT INTO vb_operations(id,profile_uuid,kind,state,safe_recovery_metadata)
                 VALUES (?1,?2,'capture','staging','capture-reserved-v1:10')",
                params![
                    OperationId::fresh().to_string(),
                    other_profile.hyphenated().to_string()
                ],
            )
            .expect("other profile reservation");

        let exact_objects =
            load_bounded_ready_object_sizes(&connection, 2).expect("exact ready-object bound");
        assert_eq!(exact_objects.len(), 2);
        assert!(load_bounded_ready_object_sizes(&connection, 1).is_err());

        let exact_references = load_bounded_recovery_references(&connection, &profile_text, 2)
            .expect("exact distinct-reference bound");
        assert_eq!(exact_references, BTreeSet::from([hash_a, hash_b]));
        assert!(load_bounded_recovery_references(&connection, &profile_text, 1).is_err());
        let link_limit_error =
            load_bounded_recovery_references_with_link_limit(&connection, &profile_text, 2, 2)
                .expect_err("duplicate/shared profile links exceed the raw recovery bound");
        assert!(
            link_limit_error
                .to_string()
                .contains("commit-object-link index exceeds limit")
        );

        let exact_reservations = load_active_capture_reservations(&connection, profile, 2)
            .expect("exact active-reservation bound");
        assert_eq!(exact_reservations.len(), 2);
        assert!(load_active_capture_reservations(&connection, profile, 1).is_err());
    }

    #[test]
    fn duplicate_heavy_restart_link_index_refuses_before_quarantine() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let orphan_stage = objects.begin(OperationId::fresh()).expect("orphan stage");
        std::fs::write(orphan_stage.capture_path(), b"unpublished").expect("staged orphan bytes");
        let stage_directory = orphan_stage.directory().to_owned();
        let stage_file = orphan_stage.capture_path().to_owned();
        drop(orphan_stage);

        let database = roots.data.join("restart-reference-limit.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata store");
        let capture = manifest(profile);
        let bytes = capture.encode().expect("valid capture manifest");
        let manifest_digest: [u8; 32] = Sha256::digest(&bytes).into();
        let profile_text = profile.hyphenated().to_string();
        let transaction = store.connection.transaction().expect("fixture transaction");
        insert_published_commit_row(
            &transaction,
            &capture,
            &bytes,
            &manifest_digest,
            &profile_text,
            102,
        )
        .expect("commit row");
        let hash = format!("{:064x}", 1);
        transaction
            .execute(
                "INSERT INTO vb_objects(hash,kind,bytes,state,created_at)
                 VALUES (?1,'archive',0,'ready',0)",
                [&hash],
            )
            .expect("damaged-index object row");
        for (role, page) in [
            ("archive", None),
            ("sidecar", None),
            ("archive", Some("page-two")),
        ] {
            transaction
                .execute(
                    "INSERT INTO vb_commit_objects(commit_id,object_hash,role,logical_page_id)
                     VALUES (?1,?2,?3,?4)",
                    params![capture.id.to_string(), hash, role, page],
                )
                .expect("duplicate-root damaged-index link");
        }
        transaction.commit().expect("fixture commit");

        let error = store
            .recover_versioning_after_restart_with_limits(profile, &objects, 1, 2)
            .expect_err("raw duplicate links must refuse before recovery moves files");
        assert!(
            error
                .to_string()
                .contains("commit-object-link index exceeds limit")
        );
        assert!(stage_directory.is_dir(), "preflight must not move staging");
        assert!(stage_file.is_file(), "preflight must retain staged bytes");
        let quarantine = roots
            .state
            .join("versioning")
            .join(profile.hyphenated().to_string())
            .join("quarantine");
        assert_eq!(
            std::fs::read_dir(quarantine)
                .expect("quarantine directory")
                .count(),
            0,
            "preflight must not create a quarantine batch"
        );

        drop(store);
        drop(objects);
        roots.cleanup().expect("cleanup");
    }

    fn archive_manifest(profile_uuid: Uuid, object: ManifestObjectRef) -> VersionedCaptureManifest {
        let mut capture = manifest(profile_uuid);
        capture
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        capture.windows[0].pages[0].archive_object = Some(object);
        capture.windows[0].pages[0].fidelity.archive = ManifestCoverage::CompleteForRequest;
        mark_archive_capture_interval(&mut capture);
        capture
    }

    fn mark_archive_capture_interval(capture: &mut VersionedCaptureManifest) {
        capture.windows[0].pages[0].page_capture = Some(ManifestPageCapture {
            archive_interval: Some(capture.capture_interval),
            sidecar_interval: None,
            archive_monotonic_interval: Some(CaptureMonotonicInterval {
                started_offset_ms: 0,
                finished_offset_ms: 1,
            }),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        });
    }

    fn install_archive_object(
        objects: &VersioningObjectStore,
        bytes: &[u8],
    ) -> (ManifestObjectRef, std::path::PathBuf) {
        let stage = objects.begin(OperationId::fresh()).expect("object stage");
        std::fs::write(stage.capture_path(), bytes).expect("object fixture");
        let stored = stage
            .install_object(64 * 1024 * 1024, Some(bytes.len() as u64), None)
            .expect("installed object");
        let reference = ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: stored.digest_hex(),
            bytes: stored.bytes(),
            archive_backend: Some(
                crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
            ),
        };
        let path = stored.path().to_owned();
        drop(stage);
        (reference, path)
    }

    #[test]
    fn partial_window_archive_publishes_only_successful_object_and_reports_failed_page() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let database = roots.data.join("partial-window-archive.sqlite");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let operation = OperationId::fresh();
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        store
            .begin_versioning_capture(operation, profile)
            .expect("reservation");

        let first_stage = objects.begin(operation).expect("first-page stage");
        let first_bytes = b"one page archive";
        std::fs::write(first_stage.capture_path(), first_bytes).expect("first archive");
        let installed = first_stage
            .install_object(64 * 1024 * 1024, Some(first_bytes.len() as u64), None)
            .expect("first-page immutable object");
        let archive_path = installed.path().to_owned();
        let archive = ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: installed.digest_hex(),
            bytes: installed.bytes(),
            archive_backend: Some(
                crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
            ),
        };
        let failed_stage = objects
            .prepare_window_page(operation, 1)
            .expect("second-page stage");
        std::fs::write(failed_stage.capture_path(), b"retained failed-page source")
            .expect("failed page source");
        let stage_directory = first_stage.directory().to_owned();

        let mut capture = archive_manifest(profile, archive);
        let source_url = "https://private.example.test/archive-url-secret";
        let private_title = "archive-title-secret";
        capture.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable(source_url.to_owned());
        capture.windows[0].pages[0].title_redacted = Some(private_title.to_owned());
        capture.message = "capture-message-secret".to_owned();
        capture.scope = ManifestScope::Window;
        capture.requested_capture.allow_partial_pages = true;
        let mut failed_page = capture.windows[0].pages[0].clone();
        failed_page.logical_page_id = LogicalPageId::fresh().to_string();
        failed_page.archive_object = None;
        failed_page.page_capture = None;
        failed_page.fidelity.archive = ManifestCoverage::Unavailable;
        failed_page.archive_failure = Some(ManifestPageArchiveFailure::Timeout);
        let failed_page_id = failed_page.logical_page_id.clone();
        capture.windows[0].pages.push(failed_page);
        let commit = capture.id.parse::<CommitId>().expect("commit id");
        assert!(
            store
                .publish_commit_with_objects(operation, profile, &capture, &objects)
                .expect("one successful object and one explicit failure publish atomically")
        );
        assert!(
            !stage_directory.exists(),
            "successful partial publication retires both page source leaves"
        );
        assert_eq!(
            std::fs::read(&archive_path).expect("committed immutable archive"),
            first_bytes.to_vec()
        );
        let info = store
            .versioning_commit_info(profile, commit)
            .expect("detail")
            .expect("partial commit");
        assert_eq!(info.page_count, 2);
        assert_eq!(info.summary.object_links, 1);
        let projected_debug = format!("{info:?}");
        assert!(!projected_debug.contains(source_url));
        assert!(!projected_debug.contains("archive-url-secret"));
        assert!(!projected_debug.contains(private_title));
        assert!(!projected_debug.contains("capture-message-secret"));
        assert_eq!(info.archive_coverage.complete_for_request, 1);
        assert_eq!(info.archive_coverage.unavailable, 1);
        assert!(info.partial_pages_allowed);
        assert_eq!(
            info.archive_failures,
            vec![CommitArchiveFailure {
                logical_page_id: failed_page_id,
                reason: ManifestPageArchiveFailure::Timeout,
            }]
        );
        drop(store);
        let reopened = ProfileStore::open(&database, StoreMode::Normal).expect("reopen metadata");
        assert_eq!(
            reopened
                .versioning_commit_info(profile, commit)
                .expect("reopened detail")
                .expect("durable partial commit"),
            info
        );
        drop(reopened);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn structural_publication_is_atomic_and_owner_scoped() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let database = roots.data.join("publication.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("store");
        let profile = Uuid::new_v4();
        let capture = manifest(profile);
        let operation = OperationId::fresh();
        assert!(matches!(
            store.publish_structural_commit(operation, Uuid::new_v4(), &capture),
            Err(StoreError::InvalidInput(_))
        ));
        assert_eq!(commit_count(&store), 0);
        assert!(matches!(
            store.publish_structural_commit(operation, profile, &capture),
            Err(StoreError::InvalidInput(_))
        ));
        assert_eq!(commit_count(&store), 0);
        store
            .begin_versioning_capture(operation, profile)
            .expect("durable reservation");
        assert!(store.begin_versioning_capture(operation, profile).is_err());
        let mut missing_parent = manifest(profile);
        missing_parent.parent_commit = Some(CommitId::fresh().to_string());
        assert!(
            store
                .publish_structural_commit(operation, profile, &missing_parent)
                .is_err()
        );
        assert_eq!(commit_count(&store), 0);
        assert_eq!(operation_count(&store), 1);
        store
            .publish_structural_commit(operation, profile, &capture)
            .expect("transactional commit");
        assert_eq!(commit_count(&store), 1);
        assert!(
            store
                .publish_structural_commit(operation, profile, &manifest(profile))
                .is_err()
        );
        assert_eq!(commit_count(&store), 1);
        let (operations, journals): (i64, i64) = (
            operation_count(&store),
            store
                .connection
                .query_row("SELECT count(*) FROM vb_journal", [], |row| row.get(0))
                .unwrap(),
        );
        assert_eq!((operations, journals), (1, 1));
        let (kind, summary, journal_operation): (String, String, Option<String>) = store
            .connection
            .query_row(
                "SELECT kind,safe_summary,operation_id FROM vb_journal",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("fixed publication journal event");
        let expected_operation = operation.to_string();
        assert_eq!(kind, "capture");
        assert_eq!(summary, "commit published");
        assert_eq!(
            journal_operation.as_deref(),
            Some(expected_operation.as_str())
        );
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn capture_slots_and_publication_share_the_gc_commit_bound() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let database = roots.data.join("capture-capacity.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("store");
        let profile = Uuid::new_v4();
        let profile_text = profile.hyphenated().to_string();
        let other_profile = Uuid::new_v4();
        let other_profile_text = other_profile.hyphenated().to_string();
        let manifest_bytes: &[u8] = b"x";
        let manifest_hash = "0".repeat(64);

        let transaction = store.connection.transaction().expect("fixture transaction");
        {
            let mut insert = transaction
                .prepare_cached(
                    "INSERT INTO vb_commits(
                         id,profile_uuid,parent_id,created_at,message,manifest_schema,
                         manifest_bytes,manifest_hash,pinned
                     ) VALUES (?1,?2,NULL,0,'',1,?3,?4,0)",
                )
                .expect("commit-row fixture insert");
            for index in 0..(MAX_RETENTION_ROWS - 1) {
                let id = format!("fixture-commit-{index:06}");
                insert
                    .execute(params![
                        id,
                        profile_text.as_str(),
                        manifest_bytes,
                        manifest_hash.as_str()
                    ])
                    .expect("bounded commit fixture row");
            }
        }
        transaction.commit().expect("commit fixture rows");
        assert_eq!(
            commit_count(&store),
            i64::try_from(MAX_RETENTION_ROWS - 1).expect("row bound fits SQLite count")
        );

        store
            .connection
            .execute(
                "INSERT INTO vb_operations(id,profile_uuid,kind,state,safe_recovery_metadata)
                 VALUES (?1,?2,'capture','staging','awaiting-native-capture')",
                params![OperationId::fresh().to_string(), other_profile_text],
            )
            .expect("other profile active capture");

        let reserved_operation = OperationId::fresh();
        store
            .begin_versioning_capture_reserved(reserved_operation, profile, 0, 0)
            .expect("the final bounded commit slot can be reserved");
        let reservation_error = store
            .begin_versioning_capture_reserved(OperationId::fresh(), profile, 0, 0)
            .expect_err("active captures reserve the remaining GC commit slot");
        assert!(reservation_error.to_string().contains("commit-row limit"));
        assert_eq!(operation_count(&store), 2);
        let journal_count: i64 = store
            .connection
            .query_row("SELECT count(*) FROM vb_journal", [], |row| row.get(0))
            .expect("journal count");
        assert_eq!(journal_count, 0);

        // A pre-existing active operation may have been admitted before
        // reservation-slot accounting existed. It must remain bounded by the
        // publication backstop after the final slot is consumed.
        let legacy_operation = OperationId::fresh();
        store
            .connection
            .execute(
                "INSERT INTO vb_operations(id,profile_uuid,kind,state,safe_recovery_metadata)
                 VALUES (?1,?2,'capture','staging','awaiting-native-capture')",
                params![legacy_operation.to_string(), profile_text],
            )
            .expect("simulate a pre-existing capture reservation");

        store
            .publish_structural_commit(reserved_operation, profile, &manifest(profile))
            .expect("the admitted operation publishes into its reserved slot");
        assert_eq!(
            commit_count(&store),
            i64::try_from(MAX_RETENTION_ROWS).expect("row bound fits SQLite count")
        );

        // The pre-existing active operation cannot publish beyond the GC
        // ceiling even after the previously reserved final slot is consumed.
        let publish_error = store
            .publish_structural_commit(legacy_operation, profile, &manifest(profile))
            .expect_err("publication backstop must preserve the bounded GC row set");
        assert!(publish_error.to_string().contains("commit-row limit"));
        assert_eq!(
            commit_count(&store),
            i64::try_from(MAX_RETENTION_ROWS).expect("row bound fits SQLite count")
        );
        assert_eq!(operation_count(&store), 3);
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM vb_journal", [], |row| row
                    .get::<_, i64>(0))
                .expect("journal count after publication refusal"),
            1
        );
        let full_capacity_error = store
            .begin_versioning_capture_reserved(OperationId::fresh(), profile, 0, 0)
            .expect_err("a full durable commit set has no remaining capture slot");
        assert!(full_capacity_error.to_string().contains("commit-row limit"));
        assert_eq!(operation_count(&store), 3);
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn fault_before_sql_is_invisible_and_after_commit_is_not_acknowledged_as_success() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let database = roots.data.join("publication-faults.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata store");
        let profile = Uuid::new_v4();
        let before = manifest(profile);
        assert!(matches!(
            store.publish_manifest_transaction_with_checkpoint(
                OperationId::fresh(),
                profile,
                &before,
                &[],
                |point| {
                    if point == PublishCheckpoint::BeforeTransaction {
                        Err(StoreError::InvalidInput("injected interruption"))
                    } else {
                        Ok(())
                    }
                }
            ),
            Err(StoreError::InvalidInput(_))
        ));
        assert_eq!(commit_count(&store), 0);
        let after = manifest(profile);
        let after_operation = OperationId::fresh();
        store
            .begin_versioning_capture(after_operation, profile)
            .expect("durable reservation");
        assert!(matches!(
            store.publish_manifest_transaction_with_checkpoint(
                after_operation,
                profile,
                &after,
                &[],
                |point| {
                    if point == PublishCheckpoint::AfterTransactionCommit {
                        Err(StoreError::InvalidInput("injected lost acknowledgment"))
                    } else {
                        Ok(())
                    }
                }
            ),
            Err(StoreError::InvalidInput(_))
        ));
        assert_eq!(commit_count(&store), 1);
        let stored_id: String = store
            .connection
            .query_row("SELECT id FROM vb_commits", [], |row| row.get(0))
            .expect("durable committed id");
        assert_eq!(stored_id, after.id);
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn sqlite_page_limit_rolls_back_structural_publication() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let database = roots.data.join("publication-page-limit.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata store");
        let profile = Uuid::new_v4();
        let operation = OperationId::fresh();
        let mut capture = manifest(profile);
        capture.scope = super::super::ManifestScope::Window;
        for _ in 1..128 {
            let mut page = capture.windows[0].pages[0].clone();
            page.logical_page_id = LogicalPageId::fresh().to_string();
            capture.windows[0].pages.push(page);
        }
        assert!(
            capture.encode().expect("bounded manifest").len() > 4096,
            "fixture must require new SQLite pages"
        );
        store
            .begin_versioning_capture(operation, profile)
            .expect("durable reservation");
        let page_count: i64 = store
            .connection
            .query_row("PRAGMA page_count", [], |row| row.get(0))
            .expect("current database pages");
        let capped: i64 = store
            .connection
            .query_row(&format!("PRAGMA max_page_count={page_count}"), [], |row| {
                row.get(0)
            })
            .expect("cap database growth");
        assert_eq!(capped, page_count);
        assert!(matches!(
            store.publish_structural_commit(operation, profile, &capture),
            Err(StoreError::DiskFull)
        ));
        assert_eq!(commit_count(&store), 0);
        let state: String = store
            .connection
            .query_row(
                "SELECT state FROM vb_operations WHERE id=?1",
                [operation.to_string()],
                |row| row.get(0),
            )
            .expect("reserved operation");
        assert_eq!(state, "staging");
        drop(store);

        let mut reopened = ProfileStore::open(&database, StoreMode::Normal).expect("reopen");
        assert_eq!(commit_count(&reopened), 0);
        let capture_events: i64 = reopened
            .connection
            .query_row(
                "SELECT count(*) FROM vb_journal WHERE kind='capture'",
                [],
                |row| row.get(0),
            )
            .expect("publication journal");
        assert_eq!(capture_events, 0, "failed publication cannot be journaled");
        let enlarged: i64 = reopened
            .connection
            .query_row(
                &format!("PRAGMA max_page_count={}", page_count + 256),
                [],
                |row| row.get(0),
            )
            .expect("restore database growth");
        assert!(enlarged > page_count);
        reopened
            .publish_structural_commit(operation, profile, &capture)
            .expect("retry the same reserved operation");
        assert_eq!(commit_count(&reopened), 1);
        let capture_events: i64 = reopened
            .connection
            .query_row(
                "SELECT count(*) FROM vb_journal WHERE kind='capture'",
                [],
                |row| row.get(0),
            )
            .expect("publication journal");
        assert_eq!(capture_events, 1, "retry writes one capture event");
        let state: String = reopened
            .connection
            .query_row(
                "SELECT state FROM vb_operations WHERE id=?1",
                [operation.to_string()],
                |row| row.get(0),
            )
            .expect("completed operation");
        assert_eq!(state, "complete");
        drop(reopened);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    #[allow(clippy::too_many_lines)] // One integration journey spans SQLite refusal and restart recovery.
    fn sqlite_page_limit_rolls_back_object_publication_and_recovery_quarantines_orphan() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let archive = b"archive retained after SQLite publication refusal";
        let database = roots.data.join("object-publication-page-limit.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata store");
        let operation = OperationId::fresh();
        let stage = objects.begin(operation).expect("capture stage");
        std::fs::write(stage.capture_path(), archive).expect("archive source");
        let installed = stage
            .install_object(64 * 1024 * 1024, Some(archive.len() as u64), None)
            .expect("installed archive object");
        let stage_directory = stage.directory().to_owned();
        let object_path = installed.path().to_owned();
        let object = ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: installed.digest_hex(),
            bytes: installed.bytes(),
            archive_backend: Some(
                crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
            ),
        };
        let mut capture = archive_manifest(profile, object.clone());
        capture.scope = super::super::ManifestScope::Window;
        for _ in 1..128 {
            let mut page = capture.windows[0].pages[0].clone();
            page.logical_page_id = LogicalPageId::fresh().to_string();
            capture.windows[0].pages.push(page);
        }
        assert!(capture.encode().expect("bounded manifest").len() > 4096);
        store
            .begin_versioning_capture(operation, profile)
            .expect("durable reservation");
        let page_count: i64 = store
            .connection
            .query_row("PRAGMA page_count", [], |row| row.get(0))
            .expect("current database pages");
        let capped: i64 = store
            .connection
            .query_row(&format!("PRAGMA max_page_count={page_count}"), [], |row| {
                row.get(0)
            })
            .expect("cap database growth");
        assert_eq!(capped, page_count);
        assert!(matches!(
            store.publish_commit_with_objects(operation, profile, &capture, &objects),
            Err(StoreError::DiskFull)
        ));
        assert_eq!(commit_count(&store), 0);
        let indexed_objects: i64 = store
            .connection
            .query_row("SELECT count(*) FROM vb_objects", [], |row| row.get(0))
            .expect("object index");
        assert_eq!(
            indexed_objects, 0,
            "failed transaction leaves no object index"
        );
        assert!(object_path.exists(), "installed bytes remain recoverable");
        assert!(
            stage_directory.exists(),
            "failed publication retains its source stage"
        );
        drop(store);

        let mut reopened = ProfileStore::open(&database, StoreMode::Normal).expect("reopen");
        assert_eq!(commit_count(&reopened), 0);
        let recovered = reopened
            .recover_versioning_after_restart(profile, &objects)
            .expect("quarantine unpublished object");
        assert_eq!(recovered.orphan_objects, 1);
        assert_eq!(recovered.staging_operations, 1);
        assert!(
            !stage_directory.exists(),
            "recovery moves the retained source stage"
        );
        assert!(!object_path.exists(), "orphan is no longer loadable");
        let quarantine = roots
            .state
            .join("versioning")
            .join(profile.hyphenated().to_string())
            .join("quarantine");
        let mut retained = Vec::new();
        for batch in std::fs::read_dir(quarantine).expect("quarantine batches") {
            let batch = batch.expect("batch");
            let candidate = batch.path().join(format!("object-{}", object.sha256));
            if candidate.exists() {
                retained.push(std::fs::read(candidate).expect("quarantined archive"));
            }
        }
        assert_eq!(retained, vec![archive.to_vec()]);
        let operation_state: String = reopened
            .connection
            .query_row(
                "SELECT state FROM vb_operations WHERE id=?1",
                [operation.to_string()],
                |row| row.get(0),
            )
            .expect("recovered operation");
        assert_eq!(operation_state, "interrupted");
        let capture_events: i64 = reopened
            .connection
            .query_row(
                "SELECT count(*) FROM vb_journal WHERE kind='capture'",
                [],
                |row| row.get(0),
            )
            .expect("capture journal");
        assert_eq!(capture_events, 0, "failed publication has no capture event");
        drop(reopened);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn killed_publication_is_atomic_through_staging_retirement() {
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
            sync::mpsc,
            thread,
            time::Duration,
        };

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let root = roots.temporary_root.as_ref().expect("temporary root");
        for (index, checkpoint) in [
            PublishCheckpoint::BeforeTransaction,
            PublishCheckpoint::AfterOperationComplete,
            PublishCheckpoint::AfterCommitRow,
            PublishCheckpoint::AfterObjectLinks,
            PublishCheckpoint::BeforeTransactionCommit,
            PublishCheckpoint::AfterTransactionCommit,
            PublishCheckpoint::BeforeStageLeafRemoval(0),
            PublishCheckpoint::AfterStageLeafRemoval(0),
            PublishCheckpoint::BeforeStageOperationDirectorySync,
            PublishCheckpoint::AfterStageOperationDirectorySync,
            PublishCheckpoint::AfterStageOperationDirectoryRemoval,
            PublishCheckpoint::BeforeStageParentDirectorySync,
            PublishCheckpoint::AfterStageParentDirectorySync,
        ]
        .into_iter()
        .enumerate()
        {
            let profile = Uuid::new_v4();
            let objects = VersioningObjectStore::open(&roots.state, profile).expect("objects");
            let database = roots.data.join(format!("publication-crash-{index}.sqlite"));
            let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
            let operation = OperationId::fresh();
            store
                .begin_versioning_capture(operation, profile)
                .expect("durable reservation");
            drop(store);
            let stage = objects.begin(operation).expect("same-operation stage");
            let archive = b"publication crash";
            std::fs::write(stage.capture_path(), archive).expect("staged archive");
            let installed = stage
                .install_object(64 * 1024, Some(archive.len() as u64), None)
                .expect("installed archive object");
            let object_path = installed.path().to_owned();
            let stage_directory = stage.directory().to_owned();
            drop(stage);

            let mut child = Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "versioning::journal::tests::crash_child_during_commit_publication",
                    "--nocapture",
                ])
                .env("FERRIC_PUBLISH_CRASH_ROOT", root)
                .env("FERRIC_PUBLISH_CRASH_DATABASE", &database)
                .env("FERRIC_PUBLISH_CRASH_PROFILE", profile.to_string())
                .env("FERRIC_PUBLISH_CRASH_OPERATION", operation.to_string())
                .env("FERRIC_PUBLISH_CRASH_OBJECT", installed.digest_hex())
                .env("FERRIC_PUBLISH_CRASH_POINT", format!("{checkpoint:?}"))
                .stdout(Stdio::piped())
                .spawn()
                .expect("spawn disposable publication child");
            let stdout = child.stdout.take().expect("child stdout");
            let (sender, receiver) = mpsc::channel();
            let reader = thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    if line.is_ok_and(|line| line.contains("FERRIC_PUBLISH_CRASH_BOUNDARY")) {
                        let _ = sender.send(true);
                        return;
                    }
                }
                let _ = sender.send(false);
            });
            let reached = receiver
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or(false);
            let _ = child.kill();
            let status = child.wait().expect("reap disposable child");
            reader.join().expect("reader joined");
            assert!(reached, "child must reach {checkpoint:?}");
            assert!(!status.success(), "disposable child was killed");

            let mut reopened = ProfileStore::open(&database, StoreMode::Normal).expect("reopen");
            let committed = matches!(
                checkpoint,
                PublishCheckpoint::AfterTransactionCommit
                    | PublishCheckpoint::BeforeStageLeafRemoval(_)
                    | PublishCheckpoint::AfterStageLeafRemoval(_)
                    | PublishCheckpoint::BeforeStageOperationDirectorySync
                    | PublishCheckpoint::AfterStageOperationDirectorySync
                    | PublishCheckpoint::AfterStageOperationDirectoryRemoval
                    | PublishCheckpoint::BeforeStageParentDirectorySync
                    | PublishCheckpoint::AfterStageParentDirectorySync
            );
            let counts: (i64, i64, i64, i64) = reopened
                .connection
                .query_row(
                    "SELECT (SELECT count(*) FROM vb_commits),
                            (SELECT count(*) FROM vb_commit_objects),
                            (SELECT count(*) FROM vb_objects),
                            (SELECT count(*) FROM vb_journal WHERE kind='capture')",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("publication row counts");
            assert_eq!(
                counts,
                (
                    i64::from(committed),
                    i64::from(committed),
                    i64::from(committed),
                    i64::from(committed)
                ),
                "{checkpoint:?}"
            );
            let operation_state: String = reopened
                .connection
                .query_row(
                    "SELECT state FROM vb_operations WHERE id=?1",
                    [operation.to_string()],
                    |row| row.get(0),
                )
                .expect("operation state");
            assert_eq!(
                operation_state,
                if committed { "complete" } else { "staging" },
                "{checkpoint:?}"
            );
            assert_eq!(
                std::fs::read(&object_path).expect("object remains"),
                b"publication crash"
            );
            let stage_present_before_recovery = !matches!(
                checkpoint,
                PublishCheckpoint::AfterStageOperationDirectoryRemoval
                    | PublishCheckpoint::BeforeStageParentDirectorySync
                    | PublishCheckpoint::AfterStageParentDirectorySync
            );
            assert_eq!(
                stage_directory.exists(),
                stage_present_before_recovery,
                "stage presence after process death at {checkpoint:?}"
            );
            let recovery = reopened
                .recover_versioning_after_restart(profile, &objects)
                .expect("recovery reconciles the interrupted publication/cleanup");
            assert_eq!(
                recovery.staging_operations,
                usize::from(stage_present_before_recovery),
                "{checkpoint:?}"
            );
            assert_eq!(
                recovery.orphan_objects,
                usize::from(!committed),
                "{checkpoint:?}"
            );
            assert!(!stage_directory.exists(), "recovery leaves no live stage");
            assert_eq!(
                object_path.exists(),
                committed,
                "only the committed object remains live after recovery"
            );
        }
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn crash_child_during_commit_publication() {
        use std::{io::Write, thread, time::Duration};

        let Ok(root) = std::env::var("FERRIC_PUBLISH_CRASH_ROOT") else {
            return;
        };
        let database = std::env::var("FERRIC_PUBLISH_CRASH_DATABASE").expect("database");
        let profile =
            Uuid::parse_str(&std::env::var("FERRIC_PUBLISH_CRASH_PROFILE").expect("profile UUID"))
                .expect("typed profile");
        let operation: OperationId = std::env::var("FERRIC_PUBLISH_CRASH_OPERATION")
            .expect("operation ID")
            .parse()
            .expect("typed operation");
        let digest = std::env::var("FERRIC_PUBLISH_CRASH_OBJECT").expect("object digest");
        let checkpoint = std::env::var("FERRIC_PUBLISH_CRASH_POINT").expect("checkpoint");
        let roots =
            crate::StorageRoots::resolve(crate::RootSpec::Base(root.into())).expect("child roots");
        roots.ensure().expect("private child roots");
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("child objects");
        let object_bytes = b"publication crash".len() as u64;
        let capture = archive_manifest(
            profile,
            ManifestObjectRef {
                role: ManifestObjectRole::Archive,
                sha256: digest.clone(),
                bytes: object_bytes,
                archive_backend: Some(
                    crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
                ),
            },
        );
        let mut store = ProfileStore::open(database, StoreMode::Normal).expect("child metadata");
        store
            .publish_commit_with_objects_with_checkpoint(
                operation,
                profile,
                &capture,
                &objects,
                |point| {
                    if format!("{point:?}") == checkpoint {
                        println!("FERRIC_PUBLISH_CRASH_BOUNDARY");
                        std::io::stdout().flush().expect("flush boundary marker");
                        loop {
                            thread::park_timeout(Duration::from_secs(60));
                        }
                    }
                    Ok(())
                },
            )
            .expect("publication should reach checkpoint");
        panic!("child must be killed at the requested boundary");
    }

    #[test]
    fn commit_link_projections_refuse_beyond_the_shared_row_limit() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_commits(
                    id TEXT PRIMARY KEY, profile_uuid TEXT NOT NULL,
                    created_at INTEGER NOT NULL, pinned INTEGER NOT NULL
                 );
                 CREATE TABLE vb_commit_objects(commit_id TEXT NOT NULL, object_hash TEXT NOT NULL);
                 CREATE TABLE vb_objects(hash TEXT PRIMARY KEY, state TEXT NOT NULL);",
            )
            .expect("link projection schema");
        let profile = Uuid::new_v4();
        let target = CommitId::fresh();
        let other = CommitId::fresh();
        for (id, created_at) in [(target, 2_i64), (other, 1_i64)] {
            connection
                .execute(
                    "INSERT INTO vb_commits(id,profile_uuid,created_at,pinned)
                     VALUES (?1,?2,?3,0)",
                    params![id.to_string(), profile.hyphenated().to_string(), created_at],
                )
                .expect("commit row");
        }
        connection
            .execute_batch(
                "INSERT INTO vb_objects(hash,state) VALUES
                    ('missing-object','missing'),('quarantined-object','quarantined'),
                    ('ready-object','ready');",
            )
            .expect("object states");
        for hash in ["missing-object", "quarantined-object"] {
            connection
                .execute(
                    "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES (?1,?2)",
                    params![target.to_string(), hash],
                )
                .expect("target commit link");
        }
        connection
            .execute(
                "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES (?1,'ready-object')",
                [other.to_string()],
            )
            .expect("other commit link");

        assert_eq!(
            bounded_commit_object_link_count(&connection, profile, target, 2)
                .expect("exact per-commit link bound"),
            2
        );
        assert_eq!(
            bounded_commit_object_state_counts(&connection, profile, target, 2)
                .expect("exact object-state link bound"),
            (1, 1)
        );
        let page_counts = bounded_page_object_link_counts(&connection, profile, 10, 0, 3)
            .expect("exact page-wide link bound");
        assert_eq!(page_counts.get(&target.to_string()), Some(&2));
        assert_eq!(page_counts.get(&other.to_string()), Some(&1));
        assert!(bounded_page_object_link_counts(&connection, profile, 10, 0, 2).is_err());

        connection
            .execute(
                "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES (?1,'ready-object')",
                [target.to_string()],
            )
            .expect("over-limit target link");
        assert!(bounded_commit_object_link_count(&connection, profile, target, 2).is_err());
        assert!(bounded_commit_object_state_counts(&connection, profile, target, 2).is_err());
    }

    #[test]
    fn redacted_commit_listing_is_bounded_and_survives_reopen() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let database = roots.data.join("commit-list.sqlite");
        let profile = Uuid::new_v4();
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        for _ in 0..3 {
            let operation = OperationId::fresh();
            store
                .begin_versioning_capture(operation, profile)
                .expect("reservation");
            store
                .publish_structural_commit(operation, profile, &manifest(profile))
                .expect("commit");
        }
        let first = store
            .list_versioning_commits(profile, 2, 0)
            .expect("first page");
        let second = store
            .list_versioning_commits(profile, 2, 2)
            .expect("second page");
        assert_eq!((first.len(), second.len()), (2, 1));
        assert!(first.iter().all(|summary| summary.object_links == 0));
        let info = store
            .versioning_commit_info(profile, first[0].id)
            .unwrap()
            .unwrap();
        assert_eq!(info.summary, first[0]);
        assert_eq!(info.page_count, 1);
        assert_eq!(info.replayable_pages, 1);
        assert_eq!(info.window_count, 1);
        assert_eq!(info.requested_capture, vec![CaptureCategory::Structural]);
        assert_eq!(info.archive_coverage.not_requested, 1);
        assert_eq!(info.page_coverage.not_requested, 1);
        assert_eq!(info.cookie_coverage.not_requested, 1);
        assert_eq!(info.max_manifest_bytes, MAX_MANIFEST_BYTES);
        assert_eq!(info.max_single_object_bytes, MAX_SINGLE_OBJECT_BYTES);
        assert!(!format!("{info:?}").contains("example.test"));
        assert_eq!(
            store
                .versioning_commit_info(Uuid::new_v4(), first[0].id)
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .versioning_commit_info(profile, CommitId::fresh())
                .unwrap(),
            None
        );
        assert!(
            store
                .list_versioning_commits(Uuid::new_v4(), 10, 0)
                .expect("wrong owner has no rows")
                .is_empty()
        );
        assert!(store.list_versioning_commits(profile, 0, 0).is_err());
        assert!(!format!("{first:?}").contains("before trial"));
        drop(store);
        let reopened = ProfileStore::open(&database, StoreMode::Normal).expect("reopen");
        assert_eq!(
            reopened
                .list_versioning_commits(profile, 2, 0)
                .expect("reopened listing"),
            first
        );
        reopened
            .connection
            .execute(
                "UPDATE vb_commits SET manifest_hash=?1 WHERE id=?2",
                params!["0".repeat(64), first[0].id.to_string()],
            )
            .expect("inject corrupt manifest hash");
        assert!(
            reopened
                .versioning_commit_info(profile, first[0].id)
                .is_err()
        );
        assert!(
            reopened
                .connection
                .execute(
                    "UPDATE vb_commits SET manifest_bytes=zeroblob(?1) WHERE id=?2",
                    params![
                        i64::try_from(MAX_MANIFEST_BYTES + 1).unwrap(),
                        first[0].id.to_string()
                    ],
                )
                .is_err(),
            "schema rejects oversized manifest before any read"
        );
        assert!(
            reopened
                .versioning_commit_info(profile, first[0].id)
                .is_err()
        );
        drop(reopened);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn deletion_preview_accounts_for_shared_objects_and_restart_finishes_quarantine() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let (object, object_path) = install_archive_object(&objects, b"shared archive");
        let database = roots.data.join("versioning-delete.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");

        let first = archive_manifest(profile, object.clone());
        let first_id = first.id.parse::<CommitId>().expect("commit id");
        let first_operation = OperationId::fresh();
        store
            .begin_versioning_capture(first_operation, profile)
            .expect("first reservation");
        store
            .publish_commit_with_objects(first_operation, profile, &first, &objects)
            .expect("first object-backed commit");

        let second = archive_manifest(profile, object);
        let second_id = second.id.parse::<CommitId>().expect("commit id");
        let second_operation = OperationId::fresh();
        store
            .begin_versioning_capture(second_operation, profile)
            .expect("second reservation");
        store
            .publish_commit_with_objects(second_operation, profile, &second, &objects)
            .expect("second object-backed commit");

        let preview = store
            .preview_versioning_delete(profile, first_id)
            .expect("first preview")
            .expect("first commit");
        assert_eq!(preview.object_links, 1);
        assert_eq!(preview.shared_objects, 1);
        assert_eq!(preview.reclaimable_objects, 0);
        let receipt = store
            .delete_versioning_commit(profile, first_id, preview.confirmation_snapshot())
            .expect("delete shared reference");
        assert!(!receipt.cleanup_pending);
        assert!(object_path.exists(), "second commit still owns the object");

        let preview = store
            .preview_versioning_delete(profile, second_id)
            .expect("second preview")
            .expect("second commit");
        assert_eq!(preview.shared_objects, 0);
        assert_eq!(preview.reclaimable_objects, 1);
        assert_eq!(preview.reclaimable_bytes, b"shared archive".len() as u64);
        let receipt = store
            .delete_versioning_commit(profile, second_id, preview.confirmation_snapshot())
            .expect("delete final reference");
        assert!(receipt.cleanup_pending, "file movement is deferred");

        // Simulate a process exit after the atomic metadata delete but before
        // the post-commit quarantine pass.
        drop(store);
        let mut reopened = ProfileStore::open(&database, StoreMode::Normal).expect("reopen");
        let report = reopened
            .collect_versioning_garbage(profile, &objects)
            .expect("restart collection");
        assert_eq!(report.quarantined_orphan_objects, 1);
        assert!(!object_path.exists());
        assert!(
            reopened
                .list_versioning_commits(profile, 10, 0)
                .expect("empty commit list")
                .is_empty()
        );
        drop(reopened);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn killed_deletion_is_atomic_and_restart_quarantines_only_committed_orphan() {
        use crate::versioning::retention::DeleteCheckpoint;
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
            sync::mpsc,
            thread,
            time::Duration,
        };

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let root = roots.temporary_root.as_ref().expect("temporary root");
        for (index, checkpoint) in [
            DeleteCheckpoint::AfterLinksDelete,
            DeleteCheckpoint::AfterCommitDelete,
            DeleteCheckpoint::AfterObjectIndexSweep,
            DeleteCheckpoint::BeforeSqliteCommit,
            DeleteCheckpoint::AfterSqliteCommit,
        ]
        .into_iter()
        .enumerate()
        {
            let profile = Uuid::new_v4();
            let objects = VersioningObjectStore::open(&roots.state, profile).expect("objects");
            let (object, object_path) = install_archive_object(&objects, b"delete crash fixture");
            let database = roots.data.join(format!("delete-crash-{index}.sqlite"));
            let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
            let capture = archive_manifest(profile, object);
            let commit = capture.id.parse::<CommitId>().expect("commit ID");
            let operation = OperationId::fresh();
            store
                .begin_versioning_capture(operation, profile)
                .expect("reservation");
            store
                .publish_commit_with_objects(operation, profile, &capture, &objects)
                .expect("durable fixture commit");
            drop(store);

            let mut child = Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "versioning::journal::tests::crash_child_during_commit_deletion",
                    "--nocapture",
                ])
                .env("FERRIC_DELETE_CRASH_ROOT", root)
                .env("FERRIC_DELETE_CRASH_DATABASE", &database)
                .env("FERRIC_DELETE_CRASH_PROFILE", profile.to_string())
                .env("FERRIC_DELETE_CRASH_COMMIT", commit.to_string())
                .env("FERRIC_DELETE_CRASH_POINT", format!("{checkpoint:?}"))
                .stdout(Stdio::piped())
                .spawn()
                .expect("spawn disposable deletion child");
            let stdout = child.stdout.take().expect("child stdout");
            let (sender, receiver) = mpsc::channel();
            let reader = thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    if line.is_ok_and(|line| line.contains("FERRIC_DELETE_CRASH_BOUNDARY")) {
                        let _ = sender.send(true);
                        return;
                    }
                }
                let _ = sender.send(false);
            });
            let reached = receiver
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or(false);
            let _ = child.kill();
            let status = child.wait().expect("reap disposable child");
            reader.join().expect("reader joined");
            assert!(reached, "child must reach {checkpoint:?}");
            assert!(!status.success(), "disposable child was killed");

            let mut reopened = ProfileStore::open(&database, StoreMode::Normal).expect("reopen");
            let committed = checkpoint == DeleteCheckpoint::AfterSqliteCommit;
            assert_eq!(
                reopened
                    .versioning_commit_info(profile, commit)
                    .expect("commit lookup")
                    .is_none(),
                committed,
                "{checkpoint:?}"
            );
            assert!(object_path.exists(), "object cleanup is deferred");
            let report = reopened
                .collect_versioning_garbage(profile, &objects)
                .expect("restart collection");
            assert_eq!(report.quarantined_orphan_objects, usize::from(committed));
            assert_eq!(object_path.exists(), !committed, "{checkpoint:?}");
        }
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn crash_child_during_commit_deletion() {
        use crate::versioning::retention::DeleteCheckpoint;
        use std::{io::Write, thread, time::Duration};

        let Ok(root) = std::env::var("FERRIC_DELETE_CRASH_ROOT") else {
            return;
        };
        let database = std::env::var("FERRIC_DELETE_CRASH_DATABASE").expect("database");
        let profile =
            Uuid::parse_str(&std::env::var("FERRIC_DELETE_CRASH_PROFILE").expect("profile UUID"))
                .expect("typed profile");
        let commit: CommitId = std::env::var("FERRIC_DELETE_CRASH_COMMIT")
            .expect("commit ID")
            .parse()
            .expect("typed commit");
        let checkpoint = std::env::var("FERRIC_DELETE_CRASH_POINT").expect("checkpoint");
        let roots =
            crate::StorageRoots::resolve(crate::RootSpec::Base(root.into())).expect("child roots");
        roots.ensure().expect("private child roots");
        let mut store = ProfileStore::open(database, StoreMode::Normal).expect("child metadata");
        let preview = store
            .preview_versioning_delete(profile, commit)
            .expect("deletion preview")
            .expect("fixture commit");
        store
            .delete_versioning_commit_with_hook(
                profile,
                commit,
                preview.confirmation_snapshot(),
                |point: DeleteCheckpoint| {
                    if format!("{point:?}") == checkpoint {
                        println!("FERRIC_DELETE_CRASH_BOUNDARY");
                        std::io::stdout().flush().expect("flush boundary marker");
                        loop {
                            thread::park_timeout(Duration::from_secs(60));
                        }
                    }
                },
            )
            .expect("deletion should reach checkpoint");
        panic!("child must be killed at the requested boundary");
    }

    #[test]
    fn history_clear_and_gc_preserve_named_archive_commit() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let (object, object_path) = install_archive_object(&objects, b"named archive bytes");
        let database = roots.data.join("versioning-named-history-clear.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        let capture = archive_manifest(profile, object);
        let commit_id = capture.id.parse::<CommitId>().expect("commit id");
        let operation = OperationId::fresh();
        store
            .begin_versioning_capture(operation, profile)
            .expect("capture reservation");
        store
            .publish_commit_with_objects(operation, profile, &capture, &objects)
            .expect("archive commit");
        store
            .record_visit("https://history.example/clear", "History", "navigate", 10)
            .expect("ordinary history row");
        store
            .connection
            .execute(
                "INSERT INTO vb_refs(id,profile_uuid,kind,name,commit_id,updated_at)
                 VALUES ('named-archive-protection',?1,'branch','saved-work',?2,0)",
                params![profile.hyphenated().to_string(), commit_id.to_string()],
            )
            .expect("named archive reference");

        assert_eq!(store.clear_history(None, None).expect("clear history"), 1);
        assert!(store.history(10).expect("history after clear").is_empty());
        assert_eq!(commit_count(&store), 1);
        assert!(
            store
                .versioning_commit_info(profile, commit_id)
                .expect("named commit info")
                .is_some()
        );
        let preview = store
            .preview_versioning_delete(profile, commit_id)
            .expect("named deletion preview")
            .expect("named commit");
        assert_eq!(preview.named_references, 1);
        assert!(!preview.can_delete);

        let report = store
            .collect_versioning_garbage(profile, &objects)
            .expect("collect preserves named object");
        assert_eq!(report.quarantined_orphan_objects, 0);
        assert_eq!(report.quarantined_corrupt_objects, 0);
        assert_eq!(report.missing_objects, 0);
        assert!(object_path.exists(), "named archive bytes remain available");

        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn garbage_collection_defers_while_capture_is_active_then_quarantines_staging() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let database = roots.data.join("versioning-active-gc.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        let operation = OperationId::fresh();
        store
            .begin_versioning_capture(operation, profile)
            .expect("durable reservation");
        let stage = objects.begin(operation).expect("active staging directory");

        let deferred = store
            .collect_versioning_garbage(profile, &objects)
            .expect("active capture defers GC");
        assert_eq!(deferred.deferred_active_operations, 1);
        assert!(stage.directory().exists());

        store
            .abort_versioning_capture(profile, operation)
            .expect("make failed operation terminal");
        let collected = store
            .collect_versioning_garbage(profile, &objects)
            .expect("quiescent GC");
        assert_eq!(collected.deferred_active_operations, 0);
        assert_eq!(collected.quarantined_staging_operations, 1);
        assert!(!stage.directory().exists());
        drop(stage);
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn garbage_collection_refuses_aggregate_object_scan_over_profile_budget() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let database = roots.data.join("versioning-gc-aggregate-bound.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        let capture = manifest(profile);
        let commit = capture.id.parse::<CommitId>().expect("commit id");
        let operation = OperationId::fresh();
        store
            .begin_versioning_capture(operation, profile)
            .expect("capture reservation");
        store
            .publish_structural_commit(operation, profile, &capture)
            .expect("structural root commit");

        let object_count = super::MAX_PROFILE_VERSIONING_BYTES / super::MAX_SINGLE_OBJECT_BYTES + 1;
        for index in 0..object_count {
            // These valid-looking index rows deliberately have no matching
            // files: the aggregate guard must refuse before classifying them
            // missing or moving anything into quarantine.
            let hash = format!("{index:064x}");
            store
                .connection
                .execute(
                    "INSERT INTO vb_objects(hash,kind,bytes,state,created_at)
                     VALUES (?1,'archive',?2,'ready',0)",
                    params![
                        hash,
                        i64::try_from(super::MAX_SINGLE_OBJECT_BYTES)
                            .expect("single object limit fits SQLite integer")
                    ],
                )
                .expect("bounded fixture object index row");
            store
                .connection
                .execute(
                    "INSERT INTO vb_commit_objects(commit_id,object_hash,role,logical_page_id)
                     VALUES (?1,?2,'archive',NULL)",
                    params![commit.to_string(), hash],
                )
                .expect("referenced fixture object row");
        }

        let error = store
            .collect_versioning_garbage(profile, &objects)
            .expect_err("aggregate claimed bytes exceed the GC profile bound");
        assert!(error.to_string().contains("GC object scan exceeds"));
        let remaining_ready: i64 = store
            .connection
            .query_row(
                "SELECT count(*) FROM vb_objects WHERE state='ready'",
                [],
                |row| row.get(0),
            )
            .expect("object state count");
        assert_eq!(
            remaining_ready,
            i64::try_from(object_count).expect("fixture count fits SQLite integer")
        );

        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_garbage_collection_preserves_a_materialized_archive_view() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let source = objects.begin(OperationId::fresh()).expect("source stage");
        let bytes = b"private live archive viewer fixture";
        std::fs::write(source.capture_path(), bytes).expect("source bytes");
        let object = source
            .install_object(1024, Some(bytes.len() as u64), None)
            .expect("immutable archive object");
        let operation = OperationId::fresh();
        let view = objects
            .materialize_archive_view_cancellable(
                operation,
                &object.digest_hex(),
                object.bytes(),
                || false,
            )
            .expect("materialized viewer file");
        let database = roots.data.join("versioning-live-view-gc.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        store
            .collect_versioning_garbage(profile, &objects)
            .expect("live profile GC");
        assert_eq!(
            std::fs::read(view.path()).expect("viewer file survives live GC"),
            bytes
        );
        objects
            .discard_archive_view(operation)
            .expect("exact viewer cleanup after blank");
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn corrupt_referenced_object_is_quarantined_and_commit_remains_listed() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let (object, object_path) = install_archive_object(&objects, b"safe bytes");
        let database = roots.data.join("versioning-corrupt-gc.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        let capture = archive_manifest(profile, object);
        let commit = capture.id.parse::<CommitId>().expect("commit id");
        let operation = OperationId::fresh();
        store
            .begin_versioning_capture(operation, profile)
            .expect("reservation");
        store
            .publish_commit_with_objects(operation, profile, &capture, &objects)
            .expect("object-backed commit");

        std::fs::write(&object_path, b"evil bytes").expect("corrupt only profile-local object");
        let report = store
            .collect_versioning_garbage(profile, &objects)
            .expect("quarantine corrupt object");
        assert_eq!(report.quarantined_corrupt_objects, 1);
        assert!(!object_path.exists());
        let info = store
            .versioning_commit_info(profile, commit)
            .expect("commit info")
            .expect("commit remains available as metadata");
        assert_eq!(info.quarantined_objects, 1);
        assert_eq!(info.missing_objects, 0);
        assert_eq!(commit_count(&store), 1);
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn killed_corrupt_gc_preserves_commit_and_quarantined_bytes() {
        use crate::versioning::retention::GcCheckpoint;
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
            sync::mpsc,
            thread,
            time::Duration,
        };

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let root = roots.temporary_root.as_ref().expect("temporary root");
        for (index, checkpoint) in [
            GcCheckpoint::AfterCorruptQuarantine,
            GcCheckpoint::AfterSqliteCommit,
        ]
        .into_iter()
        .enumerate()
        {
            let profile = Uuid::new_v4();
            let objects = VersioningObjectStore::open(&roots.state, profile).expect("objects");
            let (object, object_path) = install_archive_object(&objects, b"safe bytes");
            let database = roots.data.join(format!("corrupt-gc-crash-{index}.sqlite"));
            let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
            let capture = archive_manifest(profile, object);
            let commit = capture.id.parse::<CommitId>().expect("commit ID");
            let operation = OperationId::fresh();
            store
                .begin_versioning_capture(operation, profile)
                .expect("reservation");
            store
                .publish_commit_with_objects(operation, profile, &capture, &objects)
                .expect("durable fixture commit");
            drop(store);
            std::fs::write(&object_path, b"evil bytes").expect("corrupt local object");

            let mut child = Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "versioning::journal::tests::crash_child_during_corrupt_gc",
                    "--nocapture",
                ])
                .env("FERRIC_CORRUPT_GC_ROOT", root)
                .env("FERRIC_CORRUPT_GC_DATABASE", &database)
                .env("FERRIC_CORRUPT_GC_PROFILE", profile.to_string())
                .env("FERRIC_CORRUPT_GC_POINT", format!("{checkpoint:?}"))
                .stdout(Stdio::piped())
                .spawn()
                .expect("spawn disposable GC child");
            let stdout = child.stdout.take().expect("child stdout");
            let (sender, receiver) = mpsc::channel();
            let reader = thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    if line.is_ok_and(|line| line.contains("FERRIC_CORRUPT_GC_BOUNDARY")) {
                        let _ = sender.send(true);
                        return;
                    }
                }
                let _ = sender.send(false);
            });
            let reached = receiver
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or(false);
            let _ = child.kill();
            let status = child.wait().expect("reap disposable child");
            reader.join().expect("reader joined");
            assert!(reached, "child must reach {checkpoint:?}");
            assert!(!status.success(), "disposable child was killed");
            assert!(!object_path.exists(), "corrupt live object was moved");

            let quarantine = roots
                .state
                .join("versioning")
                .join(profile.hyphenated().to_string())
                .join("quarantine");
            let quarantined = std::fs::read_dir(quarantine)
                .expect("quarantine batches")
                .flat_map(|batch| {
                    let batch = batch.expect("private batch");
                    std::fs::read_dir(batch.path())
                        .expect("batch entries")
                        .filter_map(Result::ok)
                        .filter(|entry| entry.file_name().to_string_lossy().starts_with("corrupt-"))
                        .map(|entry| std::fs::read(entry.path()).expect("quarantined bytes"))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            assert_eq!(quarantined, [b"evil bytes".to_vec()]);

            let mut reopened = ProfileStore::open(&database, StoreMode::Normal).expect("reopen");
            assert!(
                reopened
                    .versioning_commit_info(profile, commit)
                    .expect("commit info")
                    .is_some(),
                "committed metadata survives {checkpoint:?}"
            );
            reopened
                .collect_versioning_garbage(profile, &objects)
                .expect("restart collection");
            let info = reopened
                .versioning_commit_info(profile, commit)
                .expect("commit info")
                .expect("commit remains listed");
            assert_eq!(info.quarantined_objects, 1);
            assert_eq!(info.missing_objects, 0);
            assert_eq!(commit_count(&reopened), 1);
            assert!(
                !object_path.exists(),
                "corrupt object is never restored live"
            );
            assert_eq!(
                reopened
                    .collect_versioning_garbage(profile, &objects)
                    .expect("repeat collection"),
                crate::versioning::VersioningGcReport::default(),
                "restart collection is idempotent after {checkpoint:?}"
            );
        }
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn crash_child_during_corrupt_gc() {
        use crate::versioning::retention::GcCheckpoint;
        use std::{io::Write, thread, time::Duration};

        let Ok(root) = std::env::var("FERRIC_CORRUPT_GC_ROOT") else {
            return;
        };
        let database = std::env::var("FERRIC_CORRUPT_GC_DATABASE").expect("database");
        let profile =
            Uuid::parse_str(&std::env::var("FERRIC_CORRUPT_GC_PROFILE").expect("profile UUID"))
                .expect("typed profile");
        let checkpoint = std::env::var("FERRIC_CORRUPT_GC_POINT").expect("checkpoint");
        let roots =
            crate::StorageRoots::resolve(crate::RootSpec::Base(root.into())).expect("child roots");
        roots.ensure().expect("private child roots");
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("child objects");
        let mut store = ProfileStore::open(database, StoreMode::Normal).expect("child metadata");
        store
            .collect_versioning_garbage_with_hook(profile, &objects, |point: GcCheckpoint| {
                if format!("{point:?}") == checkpoint {
                    println!("FERRIC_CORRUPT_GC_BOUNDARY");
                    std::io::stdout().flush().expect("flush boundary marker");
                    loop {
                        thread::park_timeout(Duration::from_secs(60));
                    }
                }
            })
            .expect("child GC");
        panic!("child should have been killed at its checkpoint");
    }

    fn fill_profile_budget_to_one_byte_below_cap(
        store: &mut ProfileStore,
        profile: Uuid,
        pinned_id: &CommitId,
    ) {
        let pinned_manifest_bytes: i64 = store
            .connection
            .query_row(
                "SELECT length(manifest_bytes) FROM vb_commits WHERE profile_uuid=?1 AND id=?2",
                params![profile.hyphenated().to_string(), pinned_id.to_string()],
                |row| row.get(0),
            )
            .expect("pinned manifest size");
        let mut remaining = MAX_PROFILE_VERSIONING_BYTES
            .checked_sub(u64::try_from(pinned_manifest_bytes).expect("nonnegative manifest size"))
            .and_then(|available| available.checked_sub(1))
            .expect("pinned metadata leaves room below quota");
        let mut index = 0_u64;
        while remaining > 0 {
            // Every stored object row is independently bounded, so distribute
            // the synthetic quota pressure across valid maximum-sized entries.
            let object_bytes = remaining.min(MAX_SINGLE_OBJECT_BYTES);
            let object_hash = if remaining <= MAX_SINGLE_OBJECT_BYTES {
                "f".repeat(64)
            } else {
                format!("{index:064x}")
            };
            store
                .connection
                .execute(
                    "INSERT INTO vb_objects(hash,kind,bytes,state,created_at)
                     VALUES (?1,'sha256',?2,'ready',0)",
                    params![
                        object_hash,
                        i64::try_from(object_bytes).expect("bounded object size")
                    ],
                )
                .expect("fill profile budget with retained object accounting");
            remaining -= object_bytes;
            index += 1;
        }
    }

    #[test]
    fn quarantined_object_bytes_still_consume_capture_budget() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database = roots.data.join("versioning-quarantined-budget.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        let retained = manifest(profile);
        let retained_id = retained.id.parse::<CommitId>().expect("commit id");
        let operation = OperationId::fresh();
        store
            .begin_versioning_capture(operation, profile)
            .expect("reservation");
        store
            .publish_structural_commit(operation, profile, &retained)
            .expect("retained commit");

        // Model a referenced corrupt object without allocating 2 GiB in this
        // budget-arithmetic test. Quarantine changes availability, not usage.
        fill_profile_budget_to_one_byte_below_cap(&mut store, profile, &retained_id);
        store
            .connection
            .execute(
                "UPDATE vb_objects SET state='quarantined' WHERE hash=?1",
                ["f".repeat(64)],
            )
            .expect("quarantined object index");
        store
            .connection
            .execute(
                "INSERT INTO vb_commit_objects(commit_id,object_hash,role)
                 VALUES (?1,?2,'archive')",
                params![retained_id.to_string(), "f".repeat(64)],
            )
            .expect("retained object link");

        assert!(
            store
                .begin_versioning_capture_reserved(OperationId::fresh(), profile, 2, 0)
                .is_err(),
            "a new reservation must not ignore retained quarantine bytes"
        );
        assert!(
            check_profile_versioning_budget(&store.connection, profile, &BTreeMap::new(), 0, 2)
                .is_err(),
            "publication must not ignore retained quarantine bytes"
        );
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn pinned_commit_survives_history_clear_gc_and_object_quota_exhaustion() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let database = roots.data.join("versioning-pinned-budget.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        let pinned = manifest(profile);
        let pinned_id = pinned.id.parse::<CommitId>().expect("commit id");
        let operation = OperationId::fresh();
        store
            .begin_versioning_capture(operation, profile)
            .expect("reservation");
        store
            .publish_structural_commit(operation, profile, &pinned)
            .expect("pinned structural commit");
        store
            .connection
            .execute(
                "INSERT INTO vb_refs(id,profile_uuid,kind,name,commit_id,updated_at)
                 VALUES ('named-protection',?1,'branch','protected',?2,0)",
                params![profile.hyphenated().to_string(), pinned_id.to_string()],
            )
            .expect("named reference");
        let named_preview = store
            .preview_versioning_delete(profile, pinned_id)
            .expect("named-reference preview")
            .expect("pinned commit");
        assert_eq!(named_preview.named_references, 1);
        assert!(!named_preview.can_delete);
        assert!(
            store
                .delete_versioning_commit(profile, pinned_id, named_preview.confirmation_snapshot())
                .is_err()
        );

        store
            .connection
            .execute("DELETE FROM vb_refs WHERE id='named-protection'", [])
            .expect("remove test ref");
        store
            .connection
            .execute(
                "UPDATE vb_commits SET pinned=1 WHERE profile_uuid=?1 AND id=?2",
                params![profile.hyphenated().to_string(), pinned_id.to_string()],
            )
            .expect("pin commit");
        let preview = store
            .preview_versioning_delete(profile, pinned_id)
            .expect("pinned preview")
            .expect("pinned commit");
        assert!(preview.pinned);
        assert!(!preview.can_delete);

        fill_profile_budget_to_one_byte_below_cap(&mut store, profile, &pinned_id);
        let (incoming, _) = install_archive_object(&objects, b"new");
        let rejected = archive_manifest(profile, incoming);
        let rejected_operation = OperationId::fresh();
        store
            .begin_versioning_capture(rejected_operation, profile)
            .expect("quota test reservation");
        assert!(
            store
                .publish_commit_with_objects(rejected_operation, profile, &rejected, &objects)
                .is_err()
        );
        store
            .abort_versioning_capture(profile, rejected_operation)
            .expect("finish rejected quota capture");
        assert_eq!(store.clear_history(None, None).expect("history clear"), 0);
        store
            .collect_versioning_garbage(profile, &objects)
            .expect("GC preserves explicitly pinned commits");
        assert_eq!(commit_count(&store), 1);
        assert!(
            store
                .versioning_commit_info(profile, pinned_id)
                .expect("pinned info")
                .is_some()
        );
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn capture_reservations_cover_archive_and_manifest_and_release_after_abort() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database = roots.data.join("versioning-capture-reservations.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        let reservation = crate::versioning::MAX_CAPTURE_RESERVATION_BYTES;
        let operations = (0..28).map(|_| OperationId::fresh()).collect::<Vec<_>>();
        for operation in &operations {
            store
                .begin_versioning_capture_reserved(*operation, profile, reservation, 0)
                .expect("capture plus manifest reservation fits profile quota");
        }

        let rejected = OperationId::fresh();
        assert!(
            store
                .begin_versioning_capture_reserved(rejected, profile, reservation, 0)
                .is_err()
        );

        store
            .abort_versioning_capture(profile, operations[0])
            .expect("release first reservation");
        store
            .begin_versioning_capture_reserved(rejected, profile, reservation, 0)
            .expect("released reservation admits the next capture");
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn page_capture_admission_counts_unpublished_quarantine_bytes() {
        use crate::{ProfileStoreWorker, StorageCommand, StorageCompletion};

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let stage = objects
            .begin(OperationId::fresh())
            .expect("abandoned stage");
        let reservation = crate::versioning::MAX_CAPTURE_RESERVATION_BYTES;
        let retained_bytes = MAX_PROFILE_VERSIONING_BYTES - reservation + 1;
        std::fs::OpenOptions::new()
            .write(true)
            .open(stage.capture_path())
            .expect("private stage file")
            .set_len(retained_bytes)
            .expect("sparse abandoned capture");
        drop(stage);
        let recovered = objects
            .quarantine_after_restart(&BTreeSet::new())
            .expect("private restart quarantine");
        assert_eq!(recovered.staging_operations, 1);
        assert_eq!(
            objects
                .unpublished_quarantine_bytes()
                .expect("bounded quarantine inventory"),
            retained_bytes
        );

        let database_directory = roots.data.join("profiles").join(profile.to_string());
        std::fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut worker = ProfileStoreWorker::spawn(&database).expect("metadata worker");
        let operation = OperationId::fresh();
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation,
                profile_uuid: profile,
                reserved_bytes: reservation,
                state_root: Some(roots.state.clone()),
            })
            .expect("typed capture request");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Err(_))
        ));
        drop(worker);
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("reopen metadata");
        assert_eq!(operation_count(&store), 0, "refusal made no reservation");
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn page_capture_admission_counts_unindexed_live_object_bytes() {
        use crate::{ProfileStoreWorker, StorageCommand, StorageCompletion};

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let abandoned = objects
            .begin(OperationId::fresh())
            .expect("abandoned stage");
        let reservation = crate::versioning::MAX_CAPTURE_RESERVATION_BYTES;
        let retained_bytes = MAX_PROFILE_VERSIONING_BYTES - reservation - 1;
        std::fs::OpenOptions::new()
            .write(true)
            .open(abandoned.capture_path())
            .expect("private stage file")
            .set_len(retained_bytes)
            .expect("sparse abandoned capture");
        drop(abandoned);
        objects
            .quarantine_after_restart(&BTreeSet::new())
            .expect("private restart quarantine");

        let live_stage = objects.begin(OperationId::fresh()).expect("new stage");
        std::fs::write(live_stage.capture_path(), b"xy").expect("new capture");
        let orphan = live_stage
            .install_object(64, Some(2), None)
            .expect("unindexed immutable object");
        std::fs::remove_file(live_stage.capture_path()).expect("retire disposable source stage");
        assert_eq!(
            objects
                .unindexed_live_object_bytes(&BTreeMap::new())
                .expect("orphan inventory"),
            2
        );

        let database_directory = roots.data.join("profiles").join(profile.to_string());
        std::fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut worker = ProfileStoreWorker::spawn(&database).expect("metadata worker");
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation: OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: reservation,
                state_root: Some(roots.state.clone()),
            })
            .expect("typed capture request");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Err(_))
        ));
        assert_eq!(
            std::fs::read(orphan.path()).expect("orphan retained"),
            b"xy"
        );
        drop(worker);
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("reopen metadata");
        assert_eq!(operation_count(&store), 0, "refusal made no reservation");
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn page_capture_admission_does_not_double_count_ready_object_bytes() {
        use crate::{ProfileStoreWorker, StorageCommand, StorageCompletion};

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let abandoned = objects
            .begin(OperationId::fresh())
            .expect("abandoned stage");
        let reservation = crate::versioning::MAX_CAPTURE_RESERVATION_BYTES;
        std::fs::OpenOptions::new()
            .write(true)
            .open(abandoned.capture_path())
            .expect("private stage file")
            .set_len(MAX_PROFILE_VERSIONING_BYTES - reservation - 2)
            .expect("sparse abandoned capture");
        drop(abandoned);
        objects
            .quarantine_after_restart(&BTreeSet::new())
            .expect("private restart quarantine");

        let stage = objects
            .begin(OperationId::fresh())
            .expect("ready-object stage");
        std::fs::write(stage.capture_path(), b"xy").expect("capture");
        let object = stage.install_object(64, Some(2), None).expect("object");
        std::fs::remove_file(stage.capture_path()).expect("retire disposable source stage");
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        std::fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        store
            .connection
            .execute(
                "INSERT INTO vb_objects(hash,kind,bytes,state,created_at)
                 VALUES (?1,'sha256',2,'ready',0)",
                [object.digest_hex()],
            )
            .expect("ready object index");
        drop(store);

        let mut worker = ProfileStoreWorker::spawn(&database).expect("metadata worker");
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation: OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: reservation,
                state_root: Some(roots.state.clone()),
            })
            .expect("typed capture request");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Ok(()))
        ));
        drop(worker);
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("reopen metadata");
        assert_eq!(operation_count(&store), 1, "reservation was admitted");
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn page_capture_admission_counts_live_archive_view_bytes() {
        use crate::{ProfileStoreWorker, StorageCommand, StorageCompletion};

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let abandoned = objects
            .begin(OperationId::fresh())
            .expect("abandoned stage");
        let reservation = crate::versioning::MAX_CAPTURE_RESERVATION_BYTES;
        std::fs::OpenOptions::new()
            .write(true)
            .open(abandoned.capture_path())
            .expect("private stage file")
            .set_len(MAX_PROFILE_VERSIONING_BYTES - reservation - 3)
            .expect("sparse abandoned capture");
        drop(abandoned);
        objects
            .quarantine_after_restart(&BTreeSet::new())
            .expect("private restart quarantine");

        let stage = objects.begin(OperationId::fresh()).expect("source stage");
        std::fs::write(stage.capture_path(), b"xy").expect("source bytes");
        let object = stage.install_object(64, Some(2), None).expect("object");
        std::fs::remove_file(stage.capture_path()).expect("retire disposable source stage");
        let view = objects
            .materialize_archive_view_cancellable(
                OperationId::fresh(),
                &object.digest_hex(),
                2,
                || false,
            )
            .expect("live viewer copy");
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        std::fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        store
            .connection
            .execute(
                "INSERT INTO vb_objects(hash,kind,bytes,state,created_at)
                 VALUES (?1,'sha256',2,'ready',0)",
                [object.digest_hex()],
            )
            .expect("ready object index");
        drop(store);

        let mut worker = ProfileStoreWorker::spawn(&database).expect("metadata worker");
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation: OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: reservation,
                state_root: Some(roots.state.clone()),
            })
            .expect("typed capture request");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Err(_))
        ));
        assert_eq!(std::fs::read(view.path()).expect("viewer retained"), b"xy");
        drop(worker);
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("reopen metadata");
        assert_eq!(operation_count(&store), 0, "refusal made no reservation");
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn page_capture_admission_counts_abandoned_live_stage_bytes() {
        use crate::{ProfileStoreWorker, StorageCommand, StorageCompletion};

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let abandoned = objects
            .begin(OperationId::fresh())
            .expect("abandoned stage");
        let reservation = crate::versioning::MAX_CAPTURE_RESERVATION_BYTES;
        std::fs::OpenOptions::new()
            .write(true)
            .open(abandoned.capture_path())
            .expect("private stage file")
            .set_len(MAX_PROFILE_VERSIONING_BYTES - reservation - 1)
            .expect("sparse abandoned capture");
        drop(abandoned);
        objects
            .quarantine_after_restart(&BTreeSet::new())
            .expect("private restart quarantine");
        let live_stage = objects.begin(OperationId::fresh()).expect("unowned stage");
        std::fs::write(live_stage.capture_path(), b"xy").expect("unowned bytes");

        let database_directory = roots.data.join("profiles").join(profile.to_string());
        std::fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut worker = ProfileStoreWorker::spawn(&database).expect("metadata worker");
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation: OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: reservation,
                state_root: Some(roots.state.clone()),
            })
            .expect("typed capture request");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Err(_))
        ));
        assert_eq!(
            std::fs::read(live_stage.capture_path()).expect("stage retained"),
            b"xy"
        );
        drop(worker);
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("reopen metadata");
        assert_eq!(operation_count(&store), 0, "refusal made no reservation");
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn page_capture_admission_counts_only_active_stage_excess() {
        use crate::{ProfileStoreWorker, StorageCommand, StorageCompletion};

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let abandoned = objects
            .begin(OperationId::fresh())
            .expect("abandoned stage");
        let incoming = crate::versioning::MAX_CAPTURE_RESERVATION_BYTES;
        std::fs::OpenOptions::new()
            .write(true)
            .open(abandoned.capture_path())
            .expect("private stage file")
            .set_len(MAX_PROFILE_VERSIONING_BYTES - incoming - 2)
            .expect("sparse abandoned capture");
        drop(abandoned);
        objects
            .quarantine_after_restart(&BTreeSet::new())
            .expect("private restart quarantine");

        let active = OperationId::fresh();
        let stage = objects.begin(active).expect("active stage");
        std::fs::write(stage.capture_path(), b"xyz").expect("stage exceeds reservation");
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        std::fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        store
            .begin_versioning_capture_reserved(active, profile, 2, 0)
            .expect("existing reservation");
        let active_reservations = store
            .active_versioning_capture_reservations(profile)
            .expect("active reservation map");
        assert_eq!(
            objects
                .unreserved_live_staging_bytes(&active_reservations)
                .expect("observed active excess"),
            1
        );
        drop(store);

        let mut worker = ProfileStoreWorker::spawn(&database).expect("metadata worker");
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation: OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: incoming,
                state_root: Some(roots.state.clone()),
            })
            .expect("typed capture request");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Err(_))
        ));

        std::fs::OpenOptions::new()
            .write(true)
            .open(stage.capture_path())
            .expect("private active file")
            .set_len(2)
            .expect("return to reserved length");
        assert_eq!(
            objects
                .unreserved_live_staging_bytes(&active_reservations)
                .expect("active stage fits reservation"),
            0
        );
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation: OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: incoming,
                state_root: Some(roots.state.clone()),
            })
            .expect("second typed capture request");
        let completion = next_worker_completion(&mut worker);
        assert!(
            matches!(completion, StorageCompletion::VersioningBegin(Ok(()))),
            "{completion:?}"
        );
        drop(worker);
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("reopen metadata");
        assert_eq!(operation_count(&store), 2, "only active and admitted rows");
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    fn next_worker_completion(worker: &mut crate::ProfileStoreWorker) -> crate::StorageCompletion {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(completion) = worker.poll_events().into_iter().next() {
                return completion;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "worker completion timed out"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn worker_refuses_a_manifest_for_another_profile() {
        use crate::{ProfileStoreWorker, StorageCommand, StorageCompletion, StorageWorkerError};

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        std::fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut worker = ProfileStoreWorker::spawn(&database).expect("worker");
        let other_profile = Uuid::new_v4();
        worker
            .try_submit(StorageCommand::VersioningStructuralPublish {
                operation: OperationId::fresh(),
                profile_uuid: other_profile,
                manifest: manifest(other_profile),
            })
            .expect("typed request accepted");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningStructuralPublish(Err(_))
        ));
        let valid_operation = OperationId::fresh();
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation: valid_operation,
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("valid reservation request");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Ok(()))
        ));
        worker
            .try_submit(StorageCommand::VersioningStructuralPublish {
                operation: valid_operation,
                profile_uuid: profile,
                manifest: manifest(profile),
            })
            .expect("second typed request accepted");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningStructuralPublish(Ok(()))
        ));
        let first_begin = OperationId::fresh();
        let second_begin = OperationId::fresh();
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation: first_begin,
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("first reservation");
        assert_eq!(
            worker.try_submit(StorageCommand::VersioningBegin {
                operation: second_begin,
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            }),
            Err(StorageWorkerError::Busy)
        );
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Ok(()))
        ));
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation: second_begin,
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("second reservation after completion");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Ok(()))
        ));
        drop(worker);
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("reopen");
        assert_eq!(commit_count(&store), 1);
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn object_references_are_verified_before_atomic_publication() {
        use crate::versioning::{
            CaptureCategory, ManifestCoverage, ManifestObjectRef, ManifestObjectRole,
            objects::VersioningObjectStore,
        };

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let stage = objects.begin(OperationId::fresh()).expect("stage");
        std::fs::write(stage.capture_path(), b"abc").expect("fixture capture");
        let installed = stage.install_object(64, Some(3), None).expect("object");
        let database = roots.data.join("object-publication.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata store");
        let mut capture = manifest(profile);
        capture
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        capture.windows[0].pages[0].archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: installed.digest_hex(),
            bytes: 3,
            archive_backend: Some(
                crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
            ),
        });
        capture.windows[0].pages[0].fidelity.archive = ManifestCoverage::CompleteForRequest;
        mark_archive_capture_interval(&mut capture);
        let operation = OperationId::fresh();
        store
            .begin_versioning_capture(operation, profile)
            .expect("durable reservation");
        store
            .publish_commit_with_objects(operation, profile, &capture, &objects)
            .expect("object-backed transaction");
        assert_eq!(commit_count(&store), 1);
        let links: i64 = store
            .connection
            .query_row("SELECT count(*) FROM vb_commit_objects", [], |row| {
                row.get(0)
            })
            .expect("object links");
        assert_eq!(links, 1);

        let mut later = manifest(profile);
        later
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        later.windows[0].pages[0].archive_object =
            capture.windows[0].pages[0].archive_object.clone();
        later.windows[0].pages[0].fidelity.archive = ManifestCoverage::CompleteForRequest;
        mark_archive_capture_interval(&mut later);
        std::fs::write(installed.path(), b"xyz").expect("simulate corrupt object");
        let later_operation = OperationId::fresh();
        store
            .begin_versioning_capture(later_operation, profile)
            .expect("later durable reservation");
        assert!(
            store
                .publish_commit_with_objects(later_operation, profile, &later, &objects)
                .is_err()
        );
        assert_eq!(commit_count(&store), 1);
        let links_after: i64 = store
            .connection
            .query_row("SELECT count(*) FROM vb_commit_objects", [], |row| {
                row.get(0)
            })
            .expect("object links after refusal");
        assert_eq!(links_after, 1);
        let mut missing = manifest(profile);
        missing
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        missing.windows[0].pages[0].archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: "0".repeat(64),
            bytes: 3,
            archive_backend: Some(
                crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
            ),
        });
        missing.windows[0].pages[0].fidelity.archive = ManifestCoverage::CompleteForRequest;
        mark_archive_capture_interval(&mut missing);
        let missing_operation = OperationId::fresh();
        store
            .begin_versioning_capture(missing_operation, profile)
            .expect("missing-object reservation");
        assert!(
            store
                .publish_commit_with_objects(missing_operation, profile, &missing, &objects)
                .is_err()
        );
        assert_eq!(commit_count(&store), 1);
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn worker_publishes_only_a_reserved_profile_local_object() {
        use crate::versioning::{
            CaptureCategory, ManifestCoverage, ManifestObjectRef, ManifestObjectRole,
            objects::VersioningObjectStore,
        };
        use crate::{ProfileStoreWorker, StorageCommand, StorageCompletion};

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        std::fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut worker = ProfileStoreWorker::spawn(&database).expect("worker");
        let operation = OperationId::fresh();
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation,
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("reserve");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Ok(()))
        ));
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("objects");
        let stage = objects.begin(operation).expect("native capture stage");
        std::fs::write(stage.capture_path(), b"abc").expect("capture");
        let installed = stage
            .install_object(64, Some(3), None)
            .expect("durable object");
        let mut capture = manifest(profile);
        capture
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        capture.windows[0].pages[0].archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: installed.digest_hex(),
            bytes: 3,
            archive_backend: Some(
                crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
            ),
        });
        capture.windows[0].pages[0].fidelity.archive = ManifestCoverage::CompleteForRequest;
        mark_archive_capture_interval(&mut capture);
        worker
            .try_submit(StorageCommand::VersioningObjectPublish {
                operation,
                profile_uuid: profile,
                state_root: roots.state.clone(),
                manifest: capture,
            })
            .expect("publish request");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningObjectPublish(Ok(crate::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            },))
        ));
        assert!(
            !stage.directory().exists(),
            "durable publication retires the exact source staging directory"
        );
        worker
            .try_submit(StorageCommand::VersioningList {
                profile_uuid: profile,
                limit: 10,
                offset: 0,
            })
            .expect("list request");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningList(Ok(summaries))
                if summaries.len() == 1 && summaries[0].object_links == 1
        ));
        drop(worker);
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("reopen metadata");
        assert_eq!(commit_count(&store), 1);
        let links: i64 = store
            .connection
            .query_row("SELECT count(*) FROM vb_commit_objects", [], |row| {
                row.get(0)
            })
            .expect("object links");
        assert_eq!(links, 1);
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn rejected_object_publication_aborts_and_restart_quarantines_unlinked_install() {
        use crate::{ProfileStoreWorker, StorageCommand, StorageCompletion};

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        std::fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut worker = ProfileStoreWorker::spawn(&database).expect("metadata worker");
        let operation = OperationId::fresh();
        worker
            .try_submit(StorageCommand::VersioningBegin {
                operation,
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("reserve capture");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningBegin(Ok(()))
        ));

        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let stage = objects.begin(operation).expect("private stage");
        std::fs::write(stage.capture_path(), b"abc").expect("capture bytes");
        let installed = stage
            .install_object(64, Some(3), None)
            .expect("durable unlinked object");
        let mut capture = manifest(profile);
        capture
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        capture.windows[0].pages[0].archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: installed.digest_hex(),
            bytes: 4,
            archive_backend: Some(
                crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
            ),
        });
        capture.windows[0].pages[0].fidelity.archive = ManifestCoverage::CompleteForRequest;
        mark_archive_capture_interval(&mut capture);
        worker
            .try_submit(StorageCommand::VersioningObjectPublish {
                operation,
                profile_uuid: profile,
                state_root: roots.state.clone(),
                manifest: capture,
            })
            .expect("submit mismatched publication");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningObjectPublish(Err(_))
        ));
        worker
            .try_submit(StorageCommand::VersioningAbort {
                operation,
                profile_uuid: profile,
            })
            .expect("abort failed capture");
        assert!(matches!(
            next_worker_completion(&mut worker),
            StorageCompletion::VersioningAbort(Ok(()))
        ));
        drop(worker);

        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("reopen metadata");
        assert_eq!(commit_count(&store), 0, "failed publish exposed no commit");
        let operation_state: String = store
            .connection
            .query_row(
                "SELECT state FROM vb_operations WHERE id=?1",
                [operation.to_string()],
                |row| row.get(0),
            )
            .expect("durable terminal operation");
        assert_eq!(operation_state, "failed");
        let recovered = store
            .recover_versioning_after_restart(profile, &objects)
            .expect("quarantine rejected install");
        assert_eq!(recovered.staging_operations, 1);
        assert_eq!(recovered.orphan_objects, 1);
        assert!(!installed.path().exists());
        assert_eq!(commit_count(&store), 0);
        let recovered_state: String = store
            .connection
            .query_row(
                "SELECT state FROM vb_operations WHERE id=?1",
                [operation.to_string()],
                |row| row.get(0),
            )
            .expect("recovered terminal operation");
        assert_eq!(recovered_state, "failed");
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn published_commit_survives_refused_staging_cleanup_without_deleting_unknown_data() {
        use std::os::unix::fs::PermissionsExt;

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("object store");
        let operation = OperationId::fresh();
        let stage = objects.begin(operation).expect("capture stage");
        let archive = b"durable object copy";
        std::fs::write(stage.capture_path(), archive).expect("capture bytes");
        let installed = stage
            .install_object(1024, Some(archive.len() as u64), None)
            .expect("immutable archive");
        let unknown = stage.directory().join("unowned-leaf");
        std::fs::write(&unknown, b"leave untouched").expect("unknown staged data");
        std::fs::set_permissions(&unknown, std::fs::Permissions::from_mode(0o600))
            .expect("private unknown staged data");
        let reference = ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: installed.digest_hex(),
            bytes: installed.bytes(),
            archive_backend: Some(
                crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
            ),
        };
        let capture = archive_manifest(profile, reference);
        let database = roots.data.join("post-publication-cleanup.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        store
            .begin_versioning_capture(operation, profile)
            .expect("durable reservation");

        let staging_cleanup_complete = store
            .publish_commit_with_objects(operation, profile, &capture, &objects)
            .expect("unsafe cleanup cannot undo a durable object-backed commit");

        assert!(!staging_cleanup_complete);
        assert_eq!(commit_count(&store), 1);
        assert!(stage.directory().exists());
        assert_eq!(
            std::fs::read(stage.capture_path()).expect("source capture remains"),
            archive.to_vec()
        );
        assert_eq!(
            std::fs::read(unknown).expect("unknown leaf remains"),
            b"leave untouched".to_vec()
        );
        assert_eq!(
            std::fs::read(installed.path()).expect("committed object remains"),
            archive.to_vec()
        );
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn post_commit_directory_sync_io_error_preserves_commit_and_recovery_state() {
        for (index, failed_sync, stage_directory_remains) in [
            (0, "operation-directory", true),
            (1, "staging-parent", false),
        ] {
            let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
            roots.ensure().expect("private roots");
            let profile = Uuid::new_v4();
            let objects = VersioningObjectStore::open(&roots.state, profile).expect("objects");
            let operation = OperationId::fresh();
            let stage = objects.begin(operation).expect("capture stage");
            let archive = format!("durable archive {index}").into_bytes();
            std::fs::write(stage.capture_path(), &archive).expect("capture bytes");
            let installed = stage
                .install_object(1024, Some(archive.len() as u64), None)
                .expect("immutable archive");
            let stage_directory = stage.directory().to_owned();
            let capture = archive_manifest(
                profile,
                ManifestObjectRef {
                    role: ManifestObjectRole::Archive,
                    sha256: installed.digest_hex(),
                    bytes: installed.bytes(),
                    archive_backend: Some(
                        crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
                    ),
                },
            );
            let database = roots
                .data
                .join(format!("stage-sync-refusal-{index}.sqlite"));
            let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
            store
                .begin_versioning_capture(operation, profile)
                .expect("durable reservation");

            let mut sync_calls = 0usize;
            let cleanup_complete = store
                .publish_commit_with_objects_with_checkpoint_and_sync(
                    operation,
                    profile,
                    &capture,
                    &objects,
                    |_| Ok(()),
                    |directory| {
                        sync_calls += 1;
                        if sync_calls == index + 1 {
                            Err(std::io::Error::from_raw_os_error(libc::EIO))
                        } else {
                            directory.sync_all()
                        }
                    },
                )
                .expect("sync error cannot undo durable publication");

            assert!(!cleanup_complete, "cleanup receipt is conservative");
            assert_eq!(sync_calls, index + 1, "failure at {failed_sync}");
            assert_eq!(commit_count(&store), 1, "publication remains committed");
            assert_eq!(
                std::fs::read(installed.path()).expect("committed archive object"),
                archive
            );
            assert_eq!(
                stage_directory.exists(),
                stage_directory_remains,
                "stage visibility after {failed_sync} sync failure"
            );
            if stage_directory_remains {
                assert_eq!(
                    std::fs::read_dir(&stage_directory)
                        .expect("empty cleanup stage")
                        .count(),
                    0,
                    "page sources were retired before the operation-directory sync failure"
                );
            }
            drop(store);

            let mut reopened = ProfileStore::open(&database, StoreMode::Normal).expect("reopen");
            let recovered = reopened
                .recover_versioning_after_restart(profile, &objects)
                .expect("reconcile cleanup boundary after restart");
            assert_eq!(
                recovered.staging_operations,
                usize::from(stage_directory_remains),
                "recovery accounts for exact stage state after {failed_sync} sync failure"
            );
            assert_eq!(commit_count(&reopened), 1, "recovery preserves the commit");
            assert_eq!(
                std::fs::read(installed.path()).expect("committed object after recovery"),
                archive
            );
            roots.cleanup().expect("cleanup");
        }
    }

    #[test]
    fn recovery_preserves_referenced_objects_and_marks_incomplete_operations() {
        use crate::versioning::{
            CaptureCategory, ManifestCoverage, ManifestObjectRef, ManifestObjectRole,
            objects::VersioningObjectStore,
        };

        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("objects");
        let published_stage = objects
            .begin(OperationId::fresh())
            .expect("published stage");
        std::fs::write(published_stage.capture_path(), b"abc").expect("capture");
        let published = published_stage
            .install_object(64, Some(3), None)
            .expect("published object");
        let orphan_stage = objects.begin(OperationId::fresh()).expect("orphan stage");
        std::fs::write(orphan_stage.capture_path(), b"xyz").expect("orphan capture");
        let orphan = orphan_stage
            .install_object(64, Some(3), None)
            .expect("orphan object");
        let database = roots.data.join("recovery.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        let mut capture = manifest(profile);
        capture
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        capture.windows[0].pages[0].archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: published.digest_hex(),
            bytes: 3,
            archive_backend: Some(
                crate::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned(),
            ),
        });
        capture.windows[0].pages[0].fidelity.archive = ManifestCoverage::CompleteForRequest;
        mark_archive_capture_interval(&mut capture);
        let published_operation = OperationId::fresh();
        store
            .begin_versioning_capture(published_operation, profile)
            .expect("durable published reservation");
        store
            .publish_commit_with_objects(published_operation, profile, &capture, &objects)
            .expect("committed object");
        let incomplete_operation = OperationId::fresh();
        let incomplete = incomplete_operation.to_string();
        store
            .begin_versioning_capture(incomplete_operation, profile)
            .expect("durable incomplete operation");
        let result = store
            .recover_versioning_after_restart(profile, &objects)
            .expect("recovery");
        assert_eq!(result.staging_operations, 2);
        assert_eq!(result.orphan_objects, 1);
        assert_eq!(
            std::fs::read(published.path()).expect("kept object"),
            b"abc"
        );
        assert!(!orphan.path().exists());
        let state: String = store
            .connection
            .query_row(
                "SELECT state FROM vb_operations WHERE id=?1",
                [&incomplete],
                |row| row.get(0),
            )
            .expect("operation state");
        assert_eq!(state, "interrupted");
        assert_eq!(
            store
                .recover_versioning_after_restart(profile, &objects)
                .expect("repeat recovery"),
            RecoverySummary::default()
        );
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn recovery_preserves_terminal_operation_outcomes() {
        let roots = crate::StorageRoots::resolve(crate::RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let objects = VersioningObjectStore::open(&roots.state, profile).expect("objects");
        let database = roots.data.join("terminal-recovery.sqlite");
        let mut store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata");
        let terminal_states = [
            "complete",
            "failed",
            "cancelled",
            "interrupted",
            "partial",
            "outcome-unknown",
        ];
        let mut operations = Vec::new();
        for (index, state) in terminal_states.into_iter().enumerate() {
            let id = OperationId::fresh().to_string();
            let metadata = format!("terminal-outcome-{index}");
            store
                .connection
                .execute(
                    "INSERT INTO vb_operations(id,profile_uuid,kind,state,safe_recovery_metadata)
                     VALUES (?1,?2,'capture',?3,?4)",
                    params![id, profile.hyphenated().to_string(), state, metadata],
                )
                .expect("terminal operation");
            operations.push((id, state, metadata));
        }

        assert_eq!(
            store
                .recover_versioning_after_restart(profile, &objects)
                .expect("terminal-only recovery"),
            RecoverySummary::default()
        );
        for (id, expected_state, expected_metadata) in operations {
            let actual: (String, String) = store
                .connection
                .query_row(
                    "SELECT state,safe_recovery_metadata FROM vb_operations WHERE id=?1",
                    [&id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("preserved outcome");
            assert_eq!(actual, (expected_state.to_owned(), expected_metadata));
        }
        let events: i64 = store
            .connection
            .query_row("SELECT count(*) FROM vb_journal", [], |row| row.get(0))
            .expect("journal count");
        assert_eq!(events, 0, "terminal-only restart must not invent recovery");
        drop(store);
        roots.cleanup().expect("cleanup");
    }

    fn commit_count(store: &ProfileStore) -> i64 {
        store
            .connection
            .query_row("SELECT count(*) FROM vb_commits", [], |row| row.get(0))
            .expect("commit count")
    }

    fn operation_count(store: &ProfileStore) -> i64 {
        store
            .connection
            .query_row("SELECT count(*) FROM vb_operations", [], |row| row.get(0))
            .expect("operation count")
    }
}
