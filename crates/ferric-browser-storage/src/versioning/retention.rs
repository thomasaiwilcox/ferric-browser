//! Bounded versioning history, commit deletion previews, and quiescent object
//! collection. Metadata ownership stays on the profile worker; file cleanup is
//! recoverable and never follows paths outside the profile object store.

use std::collections::BTreeSet;

use ferric_browser_core::{
    CommitId, OperationId,
    versioning::limits::{MAX_ARCHIVE_BYTES, MAX_JOURNAL_EVENTS},
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::manifest::MAX_MANIFEST_BYTES;
use super::objects::{ObjectPathError, VersioningObjectStore};
use crate::{ProfileStore, StoreError};

pub(super) const MAX_PROFILE_VERSIONING_BYTES: u64 = 2 * 1024 * 1024 * 1024;
// Publication uses the same row/root ceiling as bounded GC scans so normal
// writes cannot make history uncollectable by exceeding the scan contract.
pub(super) const MAX_RETENTION_ROWS: usize = 100_000;
const MAX_TERMINAL_OPERATIONS: i64 = 256;

/// Fixed journal categories. Page-derived strings, URLs, titles and paths can
/// never enter a persisted summary through this API.
///
/// INVARIANT: journal summaries are selected from this closed event set; never
/// accept browser- or page-provided text as an event value.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum VersioningJournalEvent {
    CaptureFailed,
    CommitDeleted,
    GarbageCollected,
    RecoveryQuarantined,
    CapturePublished {
        created_at_ms: i64,
        operation: OperationId,
    },
}

impl std::fmt::Debug for VersioningJournalEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::CaptureFailed => "VersioningJournalEvent::CaptureFailed",
            Self::CommitDeleted => "VersioningJournalEvent::CommitDeleted",
            Self::GarbageCollected => "VersioningJournalEvent::GarbageCollected",
            Self::RecoveryQuarantined => "VersioningJournalEvent::RecoveryQuarantined",
            Self::CapturePublished { .. } => "VersioningJournalEvent::CapturePublished",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeleteCheckpoint {
    AfterLinksDelete,
    AfterCommitDelete,
    AfterObjectIndexSweep,
    BeforeSqliteCommit,
    AfterSqliteCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GcCheckpoint {
    AfterCorruptQuarantine,
    AfterSqliteCommit,
}

/// Counts-only deletion projection. The private snapshot binds confirmation to
/// the exact metadata state reviewed by the user and is never serialized.
#[derive(Clone, Eq, PartialEq)]
pub struct CommitDeletePreview {
    pub id: CommitId,
    pub pinned: bool,
    pub child_commits: u32,
    pub named_references: u32,
    pub active_operations: u32,
    pub object_links: u32,
    pub shared_objects: u32,
    pub reclaimable_objects: u32,
    pub reclaimable_bytes: u64,
    pub can_delete: bool,
    pub(crate) snapshot: [u8; 32],
}

impl std::fmt::Debug for CommitDeletePreview {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommitDeletePreview")
            .field("id", &"[redacted]")
            .field("pinned", &self.pinned)
            .field("child_commits", &self.child_commits)
            .field("named_references", &self.named_references)
            .field("active_operations", &self.active_operations)
            .field("object_links", &self.object_links)
            .field("shared_objects", &self.shared_objects)
            .field("reclaimable_objects", &self.reclaimable_objects)
            .field("reclaimable_bytes", &self.reclaimable_bytes)
            .field("can_delete", &self.can_delete)
            .field("snapshot", &"[opaque]")
            .finish()
    }
}

impl CommitDeletePreview {
    /// Opaque metadata snapshot for the application-owned confirmation path.
    /// It is not a durable ID and must not be projected to UI or IPC.
    #[must_use]
    pub const fn confirmation_snapshot(&self) -> [u8; 32] {
        self.snapshot
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CommitDeleteReceipt {
    pub id: CommitId,
    pub reclaimable_objects: u32,
    pub reclaimable_bytes: u64,
    /// Object bytes are unlinked from durable history before this background
    /// cleanup. A later collect/restart quarantines the profile-local files.
    pub cleanup_pending: bool,
    pub quarantined_staging_operations: usize,
    pub quarantined_orphan_objects: usize,
    pub quarantined_corrupt_objects: usize,
    pub missing_objects: usize,
}

impl std::fmt::Debug for CommitDeleteReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommitDeleteReceipt")
            .field("id", &"[redacted]")
            .field("reclaimable_objects", &self.reclaimable_objects)
            .field("reclaimable_bytes", &self.reclaimable_bytes)
            .field("cleanup_pending", &self.cleanup_pending)
            .field(
                "quarantined_staging_operations",
                &self.quarantined_staging_operations,
            )
            .field(
                "quarantined_orphan_objects",
                &self.quarantined_orphan_objects,
            )
            .field(
                "quarantined_corrupt_objects",
                &self.quarantined_corrupt_objects,
            )
            .field("missing_objects", &self.missing_objects)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VersioningGcReport {
    pub deferred_active_operations: u32,
    pub quarantined_staging_operations: usize,
    pub quarantined_orphan_objects: usize,
    pub quarantined_corrupt_objects: usize,
    pub missing_objects: usize,
    pub removed_object_rows: usize,
}

impl ProfileStore {
    /// Marks a failed capture reservation terminal so later collection can
    /// distinguish it from a live writer without storing its failure details.
    pub(crate) fn abort_versioning_capture(
        &mut self,
        profile_uuid: Uuid,
        operation: OperationId,
    ) -> Result<(), StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE vb_operations
             SET state='failed',safe_recovery_metadata='capture-failed'
             WHERE profile_uuid=?1 AND id=?2 AND kind='capture' AND state='staging'",
            params![profile_uuid.hyphenated().to_string(), operation.to_string()],
        )?;
        if changed != 0 {
            append_bounded_journal(
                &transaction,
                profile_uuid,
                VersioningJournalEvent::CaptureFailed,
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Computes a bounded, redacted preview for deleting one exact commit.
    ///
    /// # Errors
    /// Rejects a mismatched profile, malformed index rows, and excessive roots.
    pub(crate) fn preview_versioning_delete(
        &self,
        profile_uuid: Uuid,
        id: CommitId,
    ) -> Result<Option<CommitDeletePreview>, StoreError> {
        preview_delete_from(&self.connection, profile_uuid, id)
    }

    /// Deletes only the reviewed, still-unpinned and unreferenced commit. The
    /// SQLite transaction removes manifest/link metadata and records a fixed
    /// event; physical object cleanup is deliberately deferred to GC/recovery.
    ///
    /// # Errors
    /// Rejects stale confirmation, active captures, pins and external refs.
    pub(crate) fn delete_versioning_commit(
        &mut self,
        profile_uuid: Uuid,
        id: CommitId,
        expected_snapshot: [u8; 32],
    ) -> Result<CommitDeleteReceipt, StoreError> {
        self.delete_versioning_commit_with_hook(profile_uuid, id, expected_snapshot, |_| {})
    }

    pub(crate) fn delete_versioning_commit_with_hook(
        &mut self,
        profile_uuid: Uuid,
        id: CommitId,
        expected_snapshot: [u8; 32],
        mut hook: impl FnMut(DeleteCheckpoint),
    ) -> Result<CommitDeleteReceipt, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // The object-index sweep below is global to this profile database.
        // Validate its row ceiling before the first metadata mutation so a
        // damaged oversized index cannot turn commit deletion into an
        // unbounded DELETE.
        ensure_bounded_object_index_rows(&transaction, MAX_RETENTION_ROWS)?;
        let current = preview_delete_from(&transaction, profile_uuid, id)?.ok_or(
            StoreError::InvalidInput("versioned commit no longer exists"),
        )?;
        if current.snapshot != expected_snapshot || !current.can_delete {
            return Err(StoreError::InvalidInput(
                "versioned commit deletion preview is stale or blocked",
            ));
        }

        let profile = profile_uuid.hyphenated().to_string();
        let commit = id.to_string();
        transaction.execute(
            "DELETE FROM vb_commit_objects WHERE commit_id=?1",
            [&commit],
        )?;
        hook(DeleteCheckpoint::AfterLinksDelete);
        let deleted = transaction.execute(
            "DELETE FROM vb_commits WHERE profile_uuid=?1 AND id=?2 AND pinned=0",
            params![profile, commit],
        )?;
        if deleted != 1 {
            return Err(StoreError::InvalidInput(
                "versioned commit deletion lost its target",
            ));
        }
        hook(DeleteCheckpoint::AfterCommitDelete);
        // The transaction is also a mark/sweep boundary: only rows with no
        // remaining manifest link are candidates for post-commit quarantine.
        transaction.execute(
            "DELETE FROM vb_objects
             WHERE NOT EXISTS (
                 SELECT 1 FROM vb_commit_objects co WHERE co.object_hash=vb_objects.hash
             )",
            [],
        )?;
        hook(DeleteCheckpoint::AfterObjectIndexSweep);
        append_bounded_journal(
            &transaction,
            profile_uuid,
            VersioningJournalEvent::CommitDeleted,
        )?;
        hook(DeleteCheckpoint::BeforeSqliteCommit);
        transaction.commit()?;
        hook(DeleteCheckpoint::AfterSqliteCommit);

        Ok(CommitDeleteReceipt {
            id,
            reclaimable_objects: current.reclaimable_objects,
            reclaimable_bytes: current.reclaimable_bytes,
            cleanup_pending: current.reclaimable_objects > 0,
            quarantined_staging_operations: 0,
            quarantined_orphan_objects: 0,
            quarantined_corrupt_objects: 0,
            missing_objects: 0,
        })
    }

    /// Runs bounded mark-and-sweep only when no durable capture reservation is
    /// active. During capture it is a read-only deferral; a retry or restart
    /// performs the sweep from the same exact profile root.
    ///
    /// # Errors
    /// Rejects a different profile's object store or unsafe/oversized roots.
    pub(crate) fn collect_versioning_garbage(
        &mut self,
        profile_uuid: Uuid,
        objects: &VersioningObjectStore,
    ) -> Result<VersioningGcReport, StoreError> {
        self.collect_versioning_garbage_with_hook(profile_uuid, objects, |_| {})
    }

    pub(crate) fn collect_versioning_garbage_with_hook(
        &mut self,
        profile_uuid: Uuid,
        objects: &VersioningObjectStore,
        hook: impl FnMut(GcCheckpoint),
    ) -> Result<VersioningGcReport, StoreError> {
        if objects.profile_uuid() != profile_uuid {
            return Err(StoreError::InvalidInput("versioning GC owner is invalid"));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active = active_operation_count(&transaction, profile_uuid)?;
        if active != 0 {
            return Ok(VersioningGcReport {
                deferred_active_operations: active,
                ..VersioningGcReport::default()
            });
        }
        sweep_versioning_data(transaction, profile_uuid, objects, hook)
    }
}

fn sweep_versioning_data(
    transaction: Transaction<'_>,
    profile_uuid: Uuid,
    objects: &VersioningObjectStore,
    mut hook: impl FnMut(GcCheckpoint),
) -> Result<VersioningGcReport, StoreError> {
    // Do this before reference verification, file access, row updates or
    // quarantine moves. The final anti-join DELETE must stay within the same
    // per-profile metadata ceiling as the read-side GC projections.
    ensure_bounded_object_index_rows(&transaction, MAX_RETENTION_ROWS)?;
    let referenced = load_gc_references(&transaction, profile_uuid, MAX_RETENTION_ROWS)?;
    let (corrupt_hashes, missing_hashes) =
        inspect_gc_objects(&transaction, profile_uuid, objects, MAX_RETENTION_ROWS)?;
    for hash in &missing_hashes {
        transaction.execute(
            "UPDATE vb_objects SET state='missing' WHERE hash=?1 AND state='ready'",
            [hash],
        )?;
    }
    for hash in &corrupt_hashes {
        transaction.execute(
            "UPDATE vb_objects SET state='quarantined' WHERE hash=?1 AND state='ready'",
            [hash],
        )?;
    }
    let quarantined_corrupt_objects = objects
        .quarantine_corrupt_references(&corrupt_hashes)
        .map_err(|_| StoreError::InvalidInput("corrupt versioning object quarantine failed"))?;
    hook(GcCheckpoint::AfterCorruptQuarantine);

    // File moves occur only after roots are read under the immediate
    // transaction. A crash can leave extra quarantine data, never remove
    // an object still referenced by a durable commit.
    let recovered = objects
        .quarantine_during_live_gc(&referenced)
        .map_err(|_| StoreError::InvalidInput("versioning GC quarantine failed"))?;
    let removed_object_rows = transaction.execute(
        "DELETE FROM vb_objects
         WHERE NOT EXISTS (
             SELECT 1 FROM vb_commit_objects co WHERE co.object_hash=vb_objects.hash
         )",
        [],
    )?;
    if recovered != super::objects::RecoverySummary::default()
        || removed_object_rows != 0
        || !corrupt_hashes.is_empty()
        || !missing_hashes.is_empty()
    {
        append_bounded_journal(
            &transaction,
            profile_uuid,
            VersioningJournalEvent::GarbageCollected,
        )?;
    }
    transaction.commit()?;
    hook(GcCheckpoint::AfterSqliteCommit);

    Ok(VersioningGcReport {
        deferred_active_operations: 0,
        quarantined_staging_operations: recovered.staging_operations,
        quarantined_orphan_objects: recovered.orphan_objects,
        quarantined_corrupt_objects,
        missing_objects: missing_hashes.len(),
        removed_object_rows,
    })
}

fn load_gc_references(
    connection: &Connection,
    profile_uuid: Uuid,
    maximum: usize,
) -> Result<BTreeSet<String>, StoreError> {
    let profile = profile_uuid.hyphenated().to_string();
    load_gc_references_with_link_limit(connection, &profile, maximum, MAX_RETENTION_ROWS)
}

fn load_gc_references_with_link_limit(
    connection: &Connection,
    profile: &str,
    maximum: usize,
    maximum_link_rows: usize,
) -> Result<BTreeSet<String>, StoreError> {
    // Bound raw profile link rows before DISTINCT. A LIMIT after DISTINCT
    // bounds returned roots but can still traverse an arbitrarily duplicate-
    // heavy corrupt link index.
    ensure_bounded_profile_object_link_rows(connection, profile, maximum_link_rows)?;
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("versioning GC scan limit is invalid"))?;
    let mut referenced = BTreeSet::new();
    let mut statement = connection.prepare(
        "SELECT DISTINCT co.object_hash
         FROM vb_commit_objects co
         JOIN vb_commits c ON c.id=co.commit_id
         WHERE c.profile_uuid=?1 ORDER BY co.object_hash LIMIT ?2",
    )?;
    let rows = statement.query_map(params![profile, scan_limit], |row| row.get::<_, String>(0))?;
    for row in rows {
        if referenced.len() >= maximum {
            return Err(StoreError::InvalidInput(
                "versioning GC reference index exceeds limit",
            ));
        }
        referenced.insert(row?);
    }
    Ok(referenced)
}

fn ensure_bounded_object_index_rows(
    connection: &Connection,
    maximum: usize,
) -> Result<(), StoreError> {
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("versioning object index limit is invalid"))?;
    let count: i64 = connection.query_row(
        "SELECT count(*) FROM (SELECT 1 FROM vb_objects LIMIT ?1) AS bounded_rows",
        [scan_limit],
        |row| row.get(0),
    )?;
    let count = usize::try_from(count)
        .map_err(|_| StoreError::InvalidInput("versioning object index count is invalid"))?;
    if count > maximum {
        return Err(StoreError::InvalidInput(
            "versioning object index exceeds limit",
        ));
    }
    Ok(())
}

fn inspect_gc_objects(
    connection: &Connection,
    profile_uuid: Uuid,
    objects: &VersioningObjectStore,
    maximum: usize,
) -> Result<(BTreeSet<String>, BTreeSet<String>), StoreError> {
    let mut corrupt_hashes = BTreeSet::new();
    let mut missing_hashes = BTreeSet::new();
    let mut already_quarantined = None;
    let mut inspected_bytes = 0_u64;
    let mut ready_objects = Vec::new();
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("versioning GC scan limit is invalid"))?;
    let mut statement = connection.prepare(
        "SELECT DISTINCT o.hash,o.bytes,o.state
         FROM vb_objects o
         JOIN vb_commit_objects co ON co.object_hash=o.hash
         JOIN vb_commits c ON c.id=co.commit_id
         WHERE c.profile_uuid=?1 AND o.state='ready'
         ORDER BY o.hash LIMIT ?2",
    )?;
    let rows = statement.query_map(
        params![profile_uuid.hyphenated().to_string(), scan_limit],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    )?;
    for (inspected, row) in rows.enumerate() {
        if inspected >= maximum {
            return Err(StoreError::InvalidInput(
                "versioning GC object index exceeds limit",
            ));
        }
        let (hash, bytes, state) = row?;
        if state != "ready" {
            continue;
        }
        let bytes = u64::try_from(bytes)
            .map_err(|_| StoreError::InvalidInput("stored object size is invalid"))?;
        inspected_bytes = account_gc_inspection_bytes(inspected_bytes, bytes)?;
        ready_objects.push((hash, bytes));
    }

    // Validate every claimed size before opening any object. A corrupt index
    // that exceeds the profile scan budget must not force hashing the prefix
    // of a huge object set before the collector eventually refuses.
    for (hash, bytes) in ready_objects {
        match objects.verify_object_reference(&hash, bytes, MAX_ARCHIVE_BYTES) {
            Ok(()) => {}
            Err(ObjectPathError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                if already_quarantined.is_none() {
                    already_quarantined =
                        Some(objects.quarantined_corrupt_hashes().map_err(|_| {
                            StoreError::InvalidInput("versioning quarantine inspection failed")
                        })?);
                }
                if already_quarantined
                    .as_ref()
                    .is_some_and(|known: &BTreeSet<String>| known.contains(&hash))
                {
                    corrupt_hashes.insert(hash);
                } else {
                    missing_hashes.insert(hash);
                }
            }
            Err(_) => {
                corrupt_hashes.insert(hash);
            }
        }
    }
    Ok((corrupt_hashes, missing_hashes))
}

fn account_gc_inspection_bytes(inspected: u64, next: u64) -> Result<u64, StoreError> {
    if next > MAX_ARCHIVE_BYTES {
        return Err(StoreError::InvalidInput(
            "versioning GC object exceeds the per-object limit",
        ));
    }
    let total = inspected
        .checked_add(next)
        .filter(|total| *total <= MAX_PROFILE_VERSIONING_BYTES)
        .ok_or(StoreError::InvalidInput(
            "versioning GC object scan exceeds the profile byte limit",
        ))?;
    Ok(total)
}

fn preview_delete_from(
    connection: &Connection,
    profile_uuid: Uuid,
    id: CommitId,
) -> Result<Option<CommitDeletePreview>, StoreError> {
    preview_delete_from_with_limit(connection, profile_uuid, id, MAX_RETENTION_ROWS)
}

fn preview_delete_from_with_limit(
    connection: &Connection,
    profile_uuid: Uuid,
    id: CommitId,
    maximum_rows: usize,
) -> Result<Option<CommitDeletePreview>, StoreError> {
    let profile = profile_uuid.hyphenated().to_string();
    let commit = id.to_string();
    ensure_bounded_profile_object_link_rows(connection, &profile, MAX_RETENTION_ROWS)?;
    let metadata: Option<(i64, String)> = connection
        .query_row(
            "SELECT pinned,manifest_hash FROM vb_commits WHERE profile_uuid=?1 AND id=?2",
            params![profile, commit],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((pinned, manifest_hash)) = metadata else {
        return Ok(None);
    };
    let pinned = match pinned {
        0 => false,
        1 => true,
        _ => return Err(StoreError::InvalidInput("stored pin state is invalid")),
    };
    if manifest_hash.len() != 64
        || !manifest_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(StoreError::InvalidInput("stored manifest hash is invalid"));
    }
    // The later link DELETE is keyed only by commit_id. Count raw links too,
    // including damaged dangling rows that the object-join summary omits.
    ensure_bounded_commit_object_link_rows(connection, &commit, maximum_rows)?;
    let child_commits = count_bounded_u32(
        connection,
        "SELECT 1 FROM vb_commits WHERE profile_uuid=?1 AND parent_id=?2",
        &profile,
        &commit,
        maximum_rows,
    )?;
    let named_references = count_bounded_u32(
        connection,
        "SELECT 1 FROM vb_refs WHERE profile_uuid=?1 AND commit_id=?2",
        &profile,
        &commit,
        maximum_rows,
    )?;
    let active_operations = active_operation_count(connection, profile_uuid)?;
    let object_summary = summarize_commit_objects(connection, &profile, &commit, maximum_rows)?;
    let can_delete =
        !pinned && child_commits == 0 && named_references == 0 && active_operations == 0;
    let mut snapshot_hasher = Sha256::new();
    snapshot_hasher.update(id.to_string().as_bytes());
    snapshot_hasher.update(profile.as_bytes());
    snapshot_hasher.update(manifest_hash.as_bytes());
    snapshot_hasher.update([u8::from(pinned)]);
    snapshot_hasher.update(child_commits.to_le_bytes());
    snapshot_hasher.update(named_references.to_le_bytes());
    snapshot_hasher.update(active_operations.to_le_bytes());
    snapshot_hasher.update(object_summary.object_links.to_le_bytes());
    snapshot_hasher.update(object_summary.shared_objects.to_le_bytes());
    snapshot_hasher.update(object_summary.reclaimable_objects.to_le_bytes());
    snapshot_hasher.update(object_summary.reclaimable_bytes.to_le_bytes());
    snapshot_hasher.update(object_summary.graph_fingerprint);
    let snapshot = snapshot_hasher.finalize().into();
    Ok(Some(CommitDeletePreview {
        id,
        pinned,
        child_commits,
        named_references,
        active_operations,
        object_links: object_summary.object_links,
        shared_objects: object_summary.shared_objects,
        reclaimable_objects: object_summary.reclaimable_objects,
        reclaimable_bytes: object_summary.reclaimable_bytes,
        can_delete,
        snapshot,
    }))
}

struct DeleteObjectSummary {
    object_links: u32,
    shared_objects: u32,
    reclaimable_objects: u32,
    reclaimable_bytes: u64,
    graph_fingerprint: [u8; 32],
}

impl std::fmt::Debug for DeleteObjectSummary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeleteObjectSummary")
            .field("object_links", &self.object_links)
            .field("shared_objects", &self.shared_objects)
            .field("reclaimable_objects", &self.reclaimable_objects)
            .field("reclaimable_bytes", &self.reclaimable_bytes)
            .field("graph_fingerprint", &"[redacted]")
            .finish()
    }
}

fn summarize_commit_objects(
    connection: &Connection,
    profile: &str,
    commit: &str,
    maximum_rows: usize,
) -> Result<DeleteObjectSummary, StoreError> {
    let mut object_links = 0_u32;
    let mut shared_objects = 0_u32;
    let mut reclaimable_objects = 0_u32;
    let mut reclaimable_bytes = 0_u64;
    let mut object_graph_hasher = Sha256::new();
    let mut seen = BTreeSet::new();
    let mut statement = connection.prepare(
        "SELECT co.object_hash,o.bytes,
                (SELECT count(*) FROM (
                    SELECT DISTINCT other.commit_id
                    FROM vb_commit_objects other
                    JOIN vb_commits owner ON owner.id=other.commit_id
                    WHERE owner.profile_uuid=?1
                      AND other.object_hash=co.object_hash
                      AND other.commit_id<>co.commit_id
                    LIMIT ?4
                 ) AS bounded_sharers)
         FROM vb_commit_objects co
         JOIN vb_commits target ON target.id=co.commit_id
         JOIN vb_objects o ON o.hash=co.object_hash
         WHERE target.profile_uuid=?1 AND co.commit_id=?2
         ORDER BY co.object_hash
         LIMIT ?3",
    )?;
    let scan_limit = i64::try_from(maximum_rows.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("commit object index limit is invalid"))?;
    let rows = statement.query_map(params![profile, commit, scan_limit, scan_limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        if object_links as usize >= maximum_rows {
            return Err(StoreError::InvalidInput(
                "commit object index exceeds limit",
            ));
        }
        let (hash, bytes, other_commits) = row?;
        let other_commit_count = usize::try_from(other_commits)
            .map_err(|_| StoreError::InvalidInput("shared object count is invalid"))?;
        if other_commit_count > maximum_rows {
            return Err(StoreError::InvalidInput(
                "shared object reference index exceeds limit",
            ));
        }
        object_links = object_links
            .checked_add(1)
            .ok_or(StoreError::InvalidInput("object link count is invalid"))?;
        if !seen.insert(hash.clone()) {
            continue;
        }
        let bytes = u64::try_from(bytes)
            .map_err(|_| StoreError::InvalidInput("stored object size is invalid"))?;
        object_graph_hasher.update((hash.len() as u64).to_le_bytes());
        object_graph_hasher.update(hash.as_bytes());
        object_graph_hasher.update(bytes.to_le_bytes());
        object_graph_hasher.update(other_commits.to_le_bytes());
        if other_commit_count == 0 {
            reclaimable_objects = reclaimable_objects
                .checked_add(1)
                .ok_or(StoreError::InvalidInput("object count is invalid"))?;
            reclaimable_bytes = reclaimable_bytes
                .checked_add(bytes)
                .ok_or(StoreError::InvalidInput("object byte count is invalid"))?;
        } else {
            shared_objects = shared_objects
                .checked_add(1)
                .ok_or(StoreError::InvalidInput("shared object count is invalid"))?;
        }
        if seen.len() > maximum_rows {
            return Err(StoreError::InvalidInput(
                "commit object index exceeds limit",
            ));
        }
    }
    Ok(DeleteObjectSummary {
        object_links,
        shared_objects,
        reclaimable_objects,
        reclaimable_bytes,
        graph_fingerprint: object_graph_hasher.finalize().into(),
    })
}

fn active_operation_count(connection: &Connection, profile_uuid: Uuid) -> Result<u32, StoreError> {
    let count = count_bounded_profile_rows(
        connection,
        "SELECT 1 FROM vb_operations
         WHERE profile_uuid=?1
           AND state NOT IN (
               'complete','failed','cancelled','interrupted','partial','outcome-unknown'
           )",
        profile_uuid,
        MAX_RETENTION_ROWS,
        "active versioning operation index exceeds limit",
    )?;
    u32::try_from(count).map_err(|_| StoreError::InvalidInput("operation count is invalid"))
}

fn count_bounded_profile_rows(
    connection: &Connection,
    sql: &str,
    profile_uuid: Uuid,
    maximum: usize,
    limit_error: &'static str,
) -> Result<usize, StoreError> {
    let bounded_sql = format!("SELECT count(*) FROM ({sql} LIMIT ?2) AS bounded_rows");
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("versioning count limit is invalid"))?;
    let count: i64 = connection.query_row(
        &bounded_sql,
        params![profile_uuid.hyphenated().to_string(), scan_limit],
        |row| row.get(0),
    )?;
    let count = usize::try_from(count)
        .map_err(|_| StoreError::InvalidInput("versioning count is invalid"))?;
    if count > maximum {
        return Err(StoreError::InvalidInput(limit_error));
    }
    Ok(count)
}

/// Returns the bounded raw link count for one profile. Repeated roots count
/// separately because delete, recovery and GC joins can do work per link even
/// when the links collapse to one distinct object hash.
pub(super) fn ensure_bounded_profile_object_link_rows(
    connection: &Connection,
    profile: &str,
    maximum: usize,
) -> Result<usize, StoreError> {
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("profile object link limit is invalid"))?;
    let count: i64 = connection.query_row(
        "SELECT count(*) FROM (
            SELECT 1
            FROM vb_commit_objects co
            JOIN vb_commits c ON c.id=co.commit_id
            WHERE c.profile_uuid=?1
            LIMIT ?2
         ) AS bounded_links",
        params![profile, scan_limit],
        |row| row.get(0),
    )?;
    let count = usize::try_from(count)
        .map_err(|_| StoreError::InvalidInput("profile object link count is invalid"))?;
    if count > maximum {
        return Err(StoreError::InvalidInput(
            "versioning profile commit-object-link index exceeds limit",
        ));
    }
    Ok(count)
}

fn count_bounded_u32(
    connection: &Connection,
    sql: &str,
    profile: &str,
    commit: &str,
    maximum: usize,
) -> Result<u32, StoreError> {
    // Bound the rows consumed by COUNT. This does not claim a hard SQLite page
    // visit or latency bound for a sparse/corrupted index without a matching
    // index path.
    let bounded_sql = format!("SELECT count(*) FROM ({sql} LIMIT ?3) AS bounded_rows");
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("versioning count limit is invalid"))?;
    let count: i64 =
        connection.query_row(&bounded_sql, params![profile, commit, scan_limit], |row| {
            row.get(0)
        })?;
    let count = usize::try_from(count)
        .map_err(|_| StoreError::InvalidInput("versioning count is invalid"))?;
    if count > maximum {
        return Err(StoreError::InvalidInput("versioning count exceeds limit"));
    }
    u32::try_from(count).map_err(|_| StoreError::InvalidInput("versioning count is invalid"))
}

fn ensure_bounded_commit_object_link_rows(
    connection: &Connection,
    commit: &str,
    maximum: usize,
) -> Result<(), StoreError> {
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("commit object link limit is invalid"))?;
    let count: i64 = connection.query_row(
        "SELECT count(*) FROM (
             SELECT 1 FROM vb_commit_objects WHERE commit_id=?1 LIMIT ?2
         ) AS bounded_links",
        params![commit, scan_limit],
        |row| row.get(0),
    )?;
    let count = usize::try_from(count)
        .map_err(|_| StoreError::InvalidInput("commit object link count is invalid"))?;
    if count > maximum {
        return Err(StoreError::InvalidInput(
            "commit object link index exceeds limit",
        ));
    }
    Ok(())
}

pub(crate) fn append_bounded_journal(
    connection: &Connection,
    profile_uuid: Uuid,
    event: VersioningJournalEvent,
) -> Result<(), StoreError> {
    ensure_journal_prune_input_bounds(connection, profile_uuid, MAX_RETENTION_ROWS)?;
    let (kind, summary, created_at_ms, operation_id) = match event {
        VersioningJournalEvent::CaptureFailed => {
            ("capture", "capture failed before publication", None, None)
        }
        VersioningJournalEvent::CommitDeleted => ("delete", "commit deleted", None, None),
        VersioningJournalEvent::GarbageCollected => (
            "gc",
            "versioning data quarantined or marked unavailable",
            None,
            None,
        ),
        VersioningJournalEvent::RecoveryQuarantined => {
            ("recovery", "unpublished capture quarantined", None, None)
        }
        VersioningJournalEvent::CapturePublished {
            created_at_ms,
            operation,
        } => (
            "capture",
            "commit published",
            Some(created_at_ms),
            Some(operation.to_string()),
        ),
    };
    let profile = profile_uuid.hyphenated().to_string();
    connection.execute(
        "INSERT INTO vb_journal(profile_uuid,time,operation_id,kind,safe_summary)
         VALUES (?1,COALESCE(?2,CAST(strftime('%s','now') AS INTEGER)*1000),?3,?4,?5)",
        params![profile, created_at_ms, operation_id, kind, summary],
    )?;
    prune_versioning_journal(connection, profile_uuid)
}

pub(crate) fn prune_versioning_journal(
    connection: &Connection,
    profile_uuid: Uuid,
) -> Result<(), StoreError> {
    // One event may have just been appended, so permit that single bounded
    // overage while pruning back to the normal journal/operation retention.
    ensure_journal_prune_input_bounds(connection, profile_uuid, MAX_RETENTION_ROWS + 1)?;
    let profile = profile_uuid.hyphenated().to_string();
    connection.execute(
        "DELETE FROM vb_journal
         WHERE profile_uuid=?1 AND sequence NOT IN (
             SELECT sequence FROM vb_journal WHERE profile_uuid=?1
             ORDER BY sequence DESC LIMIT ?2
         )",
        params![
            profile,
            i64::try_from(MAX_JOURNAL_EVENTS).unwrap_or(i64::MAX)
        ],
    )?;
    connection.execute(
        "UPDATE vb_journal SET operation_id=NULL
         WHERE profile_uuid=?1 AND operation_id IN (
             SELECT id FROM vb_operations
             WHERE profile_uuid=?1
               AND state IN ('complete','failed','cancelled','interrupted','partial','outcome-unknown')
             ORDER BY rowid DESC LIMIT -1 OFFSET ?2
         )",
        params![profile, MAX_TERMINAL_OPERATIONS],
    )?;
    connection.execute(
        "DELETE FROM vb_operations
         WHERE profile_uuid=?1
           AND state IN ('complete','failed','cancelled','interrupted','partial','outcome-unknown')
           AND rowid NOT IN (
               SELECT rowid FROM vb_operations
               WHERE profile_uuid=?1
                 AND state IN ('complete','failed','cancelled','interrupted','partial','outcome-unknown')
               ORDER BY rowid DESC LIMIT ?2
           )",
        params![profile, MAX_TERMINAL_OPERATIONS],
    )?;
    Ok(())
}

fn ensure_journal_prune_input_bounds(
    connection: &Connection,
    profile_uuid: Uuid,
    maximum: usize,
) -> Result<(), StoreError> {
    count_bounded_profile_rows(
        connection,
        "SELECT 1 FROM vb_journal WHERE profile_uuid=?1",
        profile_uuid,
        maximum,
        "versioning journal index exceeds limit",
    )?;
    count_bounded_profile_rows(
        connection,
        "SELECT 1 FROM vb_operations
         WHERE profile_uuid=?1
           AND state IN ('complete','failed','cancelled','interrupted','partial','outcome-unknown')",
        profile_uuid,
        maximum,
        "terminal operation index exceeds limit",
    )?;
    Ok(())
}

pub(crate) fn check_profile_versioning_budget(
    connection: &Connection,
    profile_uuid: Uuid,
    incoming: &std::collections::BTreeMap<String, u64>,
    incoming_link_rows: usize,
    incoming_manifest_bytes: u64,
) -> Result<(), StoreError> {
    check_profile_commit_capacity(connection, profile_uuid)?;
    check_profile_object_link_capacity(
        connection,
        profile_uuid,
        incoming_link_rows,
        MAX_RETENTION_ROWS,
    )?;
    check_profile_object_reference_capacity(connection, profile_uuid, incoming)?;
    let used = bounded_profile_versioning_bytes(connection, profile_uuid, MAX_RETENTION_ROWS)?;
    let mut new_bytes = incoming_manifest_bytes;
    for (hash, bytes) in incoming {
        let existing: Option<(i64, String)> = connection
            .query_row(
                "SELECT bytes,state FROM vb_objects WHERE hash=?1",
                [hash],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match existing {
            Some((known_bytes, state))
                if state == "ready" && u64::try_from(known_bytes).ok() == Some(*bytes) => {}
            Some(_) => {
                return Err(StoreError::InvalidInput(
                    "versioning object index is not reusable",
                ));
            }
            None => {
                new_bytes = new_bytes
                    .checked_add(*bytes)
                    .ok_or(StoreError::InvalidInput("versioning size is invalid"))?;
            }
        }
    }
    if used
        .checked_add(new_bytes)
        .is_none_or(|total| total > MAX_PROFILE_VERSIONING_BYTES)
    {
        return Err(StoreError::InvalidInput(
            "versioning profile disk budget exceeded; pinned commits were retained",
        ));
    }
    Ok(())
}

pub(super) fn bounded_profile_versioning_bytes(
    connection: &Connection,
    profile_uuid: Uuid,
    maximum: usize,
) -> Result<u64, StoreError> {
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("versioning size-row limit is invalid"))?;
    let mut total = 0_u64;
    {
        let mut statement = connection.prepare("SELECT bytes,state FROM vb_objects LIMIT ?1")?;
        let rows = statement.query_map([scan_limit], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for (scanned, row) in rows.enumerate() {
            if scanned >= maximum {
                return Err(StoreError::InvalidInput(
                    "versioning object-size index exceeds limit",
                ));
            }
            let (bytes, state) = row?;
            match state.as_str() {
                "ready" | "quarantined" => {
                    account_versioning_bytes(&mut total, bytes, MAX_ARCHIVE_BYTES)?;
                }
                "missing" => {}
                _ => return Err(StoreError::InvalidInput("stored object state is invalid")),
            }
        }
    }
    {
        let mut statement = connection.prepare(
            "SELECT length(manifest_bytes) FROM vb_commits
             WHERE profile_uuid=?1
             LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![profile_uuid.hyphenated().to_string(), scan_limit],
            |row| row.get::<_, i64>(0),
        )?;
        for (scanned, row) in rows.enumerate() {
            if scanned >= maximum {
                return Err(StoreError::InvalidInput(
                    "versioning manifest-size index exceeds limit",
                ));
            }
            account_versioning_bytes(&mut total, row?, MAX_MANIFEST_BYTES as u64)?;
        }
    }
    Ok(total)
}

fn account_versioning_bytes(
    total: &mut u64,
    stored: i64,
    per_object_limit: u64,
) -> Result<(), StoreError> {
    let bytes = u64::try_from(stored)
        .map_err(|_| StoreError::InvalidInput("stored versioning size is invalid"))?;
    if bytes > per_object_limit {
        return Err(StoreError::InvalidInput(
            "stored versioning entry exceeds its size limit",
        ));
    }
    *total = total
        .checked_add(bytes)
        .filter(|total| *total <= MAX_PROFILE_VERSIONING_BYTES)
        .ok_or(StoreError::InvalidInput(
            "stored versioning size exceeds the profile limit",
        ))?;
    Ok(())
}

pub(super) fn check_profile_commit_capacity(
    connection: &Connection,
    profile_uuid: Uuid,
) -> Result<(), StoreError> {
    check_profile_commit_rows(connection, profile_uuid, MAX_RETENTION_ROWS)
}

/// Leaves one durable commit slot for every active capture reservation so
/// native capture work is not admitted when bounded GC could not retain its
/// eventual publication. Publication still checks durable commits separately
/// because it consumes the current operation's reserved slot.
pub(super) fn check_profile_capture_slot_capacity(
    connection: &Connection,
    profile_uuid: Uuid,
) -> Result<(), StoreError> {
    let commits = count_bounded_profile_rows(
        connection,
        "SELECT 1 FROM vb_commits WHERE profile_uuid=?1",
        profile_uuid,
        MAX_RETENTION_ROWS,
        "versioning profile commit-row limit reached",
    )?;
    let active_captures = count_bounded_profile_rows(
        connection,
        "SELECT 1 FROM vb_operations
         WHERE profile_uuid=?1 AND kind='capture' AND state='staging'",
        profile_uuid,
        MAX_RETENTION_ROWS,
        "versioning profile capture reservation limit reached",
    )?;
    if commits
        .checked_add(active_captures)
        .is_none_or(|reserved| reserved >= MAX_RETENTION_ROWS)
    {
        return Err(StoreError::InvalidInput(
            "versioning profile commit-row limit reached",
        ));
    }
    Ok(())
}

fn check_profile_object_reference_capacity(
    connection: &Connection,
    profile_uuid: Uuid,
    incoming: &std::collections::BTreeMap<String, u64>,
) -> Result<(), StoreError> {
    check_profile_object_reference_rows(connection, profile_uuid, incoming, MAX_RETENTION_ROWS)
}

fn check_profile_object_link_capacity(
    connection: &Connection,
    profile_uuid: Uuid,
    incoming_link_rows: usize,
    maximum: usize,
) -> Result<(), StoreError> {
    let profile = profile_uuid.hyphenated().to_string();
    let existing = ensure_bounded_profile_object_link_rows(connection, &profile, maximum)?;
    if existing
        .checked_add(incoming_link_rows)
        .is_none_or(|rows| rows > maximum)
    {
        return Err(StoreError::InvalidInput(
            "versioning profile commit-object-link limit reached",
        ));
    }
    Ok(())
}

fn check_profile_object_reference_rows(
    connection: &Connection,
    profile_uuid: Uuid,
    incoming: &std::collections::BTreeMap<String, u64>,
    maximum: usize,
) -> Result<(), StoreError> {
    // Count only the bounded distinct-root projection. This refuses an
    // already-oversized index without materializing its full root set; it is
    // not a hard bound on SQLite page visits for a sparse/corrupted index.
    let scan_limit = i64::try_from(maximum.saturating_add(1))
        .map_err(|_| StoreError::InvalidInput("versioning count limit is invalid"))?;
    let referenced: i64 = connection.query_row(
        "SELECT count(*) FROM (
            SELECT DISTINCT co.object_hash
            FROM vb_commit_objects co
            JOIN vb_commits c ON c.id=co.commit_id
            WHERE c.profile_uuid=?1
            LIMIT ?2
         ) AS bounded_roots",
        params![profile_uuid.hyphenated().to_string(), scan_limit],
        |row| row.get(0),
    )?;
    let mut projected = usize::try_from(referenced)
        .map_err(|_| StoreError::InvalidInput("stored object reference count is invalid"))?;
    if projected > maximum {
        return Err(StoreError::InvalidInput(
            "versioning profile object-reference limit reached",
        ));
    }

    for hash in incoming.keys() {
        let already_referenced: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM vb_commit_objects co
                JOIN vb_commits c ON c.id=co.commit_id
                WHERE c.profile_uuid=?1 AND co.object_hash=?2
             )",
            rusqlite::params![profile_uuid.hyphenated().to_string(), hash],
            |row| row.get(0),
        )?;
        if !already_referenced {
            projected = projected.checked_add(1).ok_or(StoreError::InvalidInput(
                "object reference count is invalid",
            ))?;
            if projected > maximum {
                return Err(StoreError::InvalidInput(
                    "versioning profile object-reference limit reached",
                ));
            }
        }
    }
    Ok(())
}

fn check_profile_commit_rows(
    connection: &Connection,
    profile_uuid: Uuid,
    maximum: usize,
) -> Result<(), StoreError> {
    let count = count_bounded_profile_rows(
        connection,
        "SELECT 1 FROM vb_commits WHERE profile_uuid=?1",
        profile_uuid,
        maximum,
        "versioning profile commit-row limit reached",
    )?;
    if count >= maximum {
        return Err(StoreError::InvalidInput(
            "versioning profile commit-row limit reached",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use ferric_browser_core::CommitId;
    use rusqlite::Connection;
    use uuid::Uuid;

    #[cfg(unix)]
    use super::super::objects::VersioningObjectStore;
    use super::{
        CommitDeletePreview, CommitDeleteReceipt, MAX_ARCHIVE_BYTES, MAX_PROFILE_VERSIONING_BYTES,
        MAX_RETENTION_ROWS, account_gc_inspection_bytes, bounded_profile_versioning_bytes,
        check_profile_commit_capacity, check_profile_commit_rows,
        check_profile_object_link_capacity, check_profile_object_reference_rows,
        count_bounded_profile_rows, count_bounded_u32, ensure_bounded_commit_object_link_rows,
        ensure_bounded_object_index_rows, ensure_bounded_profile_object_link_rows,
        ensure_journal_prune_input_bounds, preview_delete_from_with_limit,
        summarize_commit_objects,
    };
    #[cfg(unix)]
    use super::{inspect_gc_objects, load_gc_references, load_gc_references_with_link_limit};

    #[test]
    fn journal_event_debug_redacts_publication_correlation_identity() {
        let operation = ferric_browser_core::OperationId::fresh();
        let event = super::VersioningJournalEvent::CapturePublished {
            created_at_ms: 123,
            operation,
        };

        let debug = format!("{event:?}");
        assert!(debug.contains("CapturePublished"));
        assert!(!debug.contains(&operation.to_string()));
        assert!(!debug.contains("123"));
    }

    #[test]
    fn deletion_summary_debug_redacts_graph_fingerprint() {
        let fingerprint = [0x5a; 32];
        let summary = super::DeleteObjectSummary {
            object_links: 1,
            shared_objects: 0,
            reclaimable_objects: 1,
            reclaimable_bytes: 12,
            graph_fingerprint: fingerprint,
        };

        let debug = format!("{summary:?}");
        assert!(debug.contains("object_links: 1"));
        assert!(!debug.contains(&format!("{fingerprint:?}")));
    }

    #[test]
    fn delete_diagnostics_redact_commit_identity() {
        let id = CommitId::fresh();
        let id_text = id.to_string();
        let preview = CommitDeletePreview {
            id,
            pinned: false,
            child_commits: 0,
            named_references: 0,
            active_operations: 0,
            object_links: 1,
            shared_objects: 0,
            reclaimable_objects: 1,
            reclaimable_bytes: 64,
            can_delete: true,
            snapshot: [0; 32],
        };
        let receipt = CommitDeleteReceipt {
            id,
            reclaimable_objects: 1,
            reclaimable_bytes: 64,
            cleanup_pending: true,
            quarantined_staging_operations: 0,
            quarantined_orphan_objects: 0,
            quarantined_corrupt_objects: 0,
            missing_objects: 0,
        };
        assert!(!format!("{preview:?}").contains(id_text.as_str()));
        assert!(!format!("{receipt:?}").contains(id_text.as_str()));
    }

    #[cfg(unix)]
    fn gc_scan_fixture(profile: Uuid, rows: usize) -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_commits(id TEXT NOT NULL,profile_uuid TEXT NOT NULL);
                 CREATE TABLE vb_commit_objects(commit_id TEXT NOT NULL,object_hash TEXT NOT NULL);
                 CREATE TABLE vb_objects(hash TEXT NOT NULL,bytes INTEGER NOT NULL,state TEXT NOT NULL);",
            )
            .expect("GC fixture schema");
        let profile_text = profile.hyphenated().to_string();
        for index in 0..rows {
            let commit = format!("gc-commit-{index}");
            let hash = format!("{index:064x}");
            connection
                .execute(
                    "INSERT INTO vb_commits(id,profile_uuid) VALUES (?1,?2)",
                    (&commit, &profile_text),
                )
                .expect("GC fixture commit");
            connection
                .execute(
                    "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES (?1,?2)",
                    (&commit, &hash),
                )
                .expect("GC fixture root");
            connection
                .execute(
                    "INSERT INTO vb_objects(hash,bytes,state) VALUES (?1,1,'ready')",
                    [&hash],
                )
                .expect("GC fixture object");
        }
        connection
    }

    #[cfg(unix)]
    #[test]
    fn gc_scans_bound_distinct_roots_despite_repeated_commit_links() {
        let profile = Uuid::new_v4();
        let connection = gc_scan_fixture(profile, 3);
        let profile_text = profile.hyphenated().to_string();
        let shared_hash = format!("{:064x}", 0);
        for index in 0..10 {
            let commit = format!("shared-gc-commit-{index}");
            connection
                .execute(
                    "INSERT INTO vb_commits(id,profile_uuid) VALUES (?1,?2)",
                    (&commit, &profile_text),
                )
                .expect("shared-root fixture commit");
            connection
                .execute(
                    "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES (?1,?2)",
                    (&commit, &shared_hash),
                )
                .expect("shared-root fixture link");
        }
        assert_eq!(
            load_gc_references(&connection, profile, 3)
                .expect("exact distinct GC root boundary")
                .len(),
            3
        );
        assert!(load_gc_references(&connection, profile, 2).is_err());

        let root = std::env::temp_dir().join(format!("ferric-retention-gc-{profile}"));
        std::fs::create_dir(&root).expect("private object-store root");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("restrict object-store root permissions");
        let objects = VersioningObjectStore::open(&root, profile).expect("profile object store");
        let (_, missing) = inspect_gc_objects(&connection, profile, &objects, 3)
            .expect("exact distinct GC object boundary");
        assert_eq!(missing.len(), 3);
        assert!(inspect_gc_objects(&connection, profile, &objects, 2).is_err());
        drop(objects);
        std::fs::remove_dir_all(root).expect("remove private object-store root");
    }

    #[cfg(unix)]
    #[test]
    fn gc_refuses_duplicate_heavy_link_index_before_distinct_root_scan() {
        let profile = Uuid::new_v4();
        let connection = gc_scan_fixture(profile, 1);
        let profile_text = profile.hyphenated().to_string();
        let hash = format!("{:064x}", 0);
        for _ in 0..2 {
            connection
                .execute(
                    "INSERT INTO vb_commit_objects(commit_id,object_hash)
                     VALUES ('gc-commit-0',?1)",
                    [&hash],
                )
                .expect("duplicate root link");
        }

        let error = load_gc_references_with_link_limit(&connection, &profile_text, 1, 2)
            .expect_err("raw duplicate links exceed the bounded GC input");
        assert!(
            error
                .to_string()
                .contains("commit-object-link index exceeds limit")
        );
    }

    #[test]
    fn object_index_preflight_accepts_exact_limit_and_refuses_over_limit_read_only() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_objects(hash TEXT NOT NULL);
                 INSERT INTO vb_objects(hash) VALUES ('one'),('two');",
            )
            .expect("object-index fixture");

        ensure_bounded_object_index_rows(&connection, 2)
            .expect("exact object-index boundary is accepted");
        connection
            .execute("INSERT INTO vb_objects(hash) VALUES ('three')", [])
            .expect("over-limit object row");

        let error = ensure_bounded_object_index_rows(&connection, 2)
            .expect_err("first object-index row above the bound is refused");
        assert!(error.to_string().contains("object index exceeds limit"));
        let rows: i64 = connection
            .query_row("SELECT count(*) FROM vb_objects", [], |row| row.get(0))
            .expect("object-index row count");
        assert_eq!(rows, 3, "preflight refusal must leave rows unchanged");
    }

    #[test]
    fn commit_link_preflight_bounds_even_dangling_rows_before_delete() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_commit_objects(commit_id TEXT NOT NULL,object_hash TEXT NOT NULL);
                 INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES
                    ('target','one'),('target','two');",
            )
            .expect("commit-link fixture");

        ensure_bounded_commit_object_link_rows(&connection, "target", 2)
            .expect("exact raw link boundary is accepted");
        connection
            .execute(
                "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES ('target','dangling')",
                [],
            )
            .expect("damaged dangling link");

        let error = ensure_bounded_commit_object_link_rows(&connection, "target", 2)
            .expect_err("first raw link above the bound is refused");
        assert!(
            error
                .to_string()
                .contains("commit object link index exceeds limit")
        );
        let rows: i64 = connection
            .query_row(
                "SELECT count(*) FROM vb_commit_objects WHERE commit_id='target'",
                [],
                |row| row.get(0),
            )
            .expect("target link count");
        assert_eq!(rows, 3, "preflight refusal must leave links unchanged");
    }

    #[test]
    fn profile_link_capacity_counts_duplicate_roots_and_is_profile_scoped() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_commits(id TEXT NOT NULL,profile_uuid TEXT NOT NULL);
                 CREATE TABLE vb_commit_objects(commit_id TEXT NOT NULL,object_hash TEXT NOT NULL);
                 INSERT INTO vb_commits(id,profile_uuid) VALUES
                    ('current','profile-a'),('other','profile-b');
                 INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES
                    ('current','same-root'),('current','same-root'),('current','other-root'),
                    ('other','external-root');",
            )
            .expect("profile link fixture");
        let profile =
            Uuid::parse_str("00000000-0000-0000-0000-00000000000a").expect("fixed profile UUID");
        connection
            .execute(
                "UPDATE vb_commits SET profile_uuid=?1 WHERE id='current'",
                [profile.hyphenated().to_string()],
            )
            .expect("bind current profile");

        assert_eq!(
            ensure_bounded_profile_object_link_rows(&connection, &profile.to_string(), 3)
                .expect("duplicate roots count as bounded raw rows"),
            3
        );
        check_profile_object_link_capacity(&connection, profile, 0, 3)
            .expect("exact raw-link boundary remains admissible");
        let error = check_profile_object_link_capacity(&connection, profile, 1, 3)
            .expect_err("new references cannot exceed the raw profile-link bound");
        assert!(
            error
                .to_string()
                .contains("commit-object-link limit reached")
        );

        connection
            .execute(
                "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES
                    ('current','same-root')",
                [],
            )
            .expect("damaged duplicate link");
        let error = ensure_bounded_profile_object_link_rows(&connection, &profile.to_string(), 3)
            .expect_err("duplicate-heavy profile index is refused");
        assert!(
            error
                .to_string()
                .contains("commit-object-link index exceeds limit")
        );
    }

    #[test]
    fn delete_preview_refuses_over_limit_dangling_links_before_delete() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_commits(
                    id TEXT NOT NULL,profile_uuid TEXT NOT NULL,parent_id TEXT,
                    pinned INTEGER NOT NULL,manifest_hash TEXT NOT NULL
                 );
                 CREATE TABLE vb_commit_objects(commit_id TEXT NOT NULL,object_hash TEXT NOT NULL);
                 CREATE TABLE vb_objects(hash TEXT NOT NULL,bytes INTEGER NOT NULL);
                 CREATE TABLE vb_refs(profile_uuid TEXT NOT NULL,commit_id TEXT);
                 CREATE TABLE vb_operations(profile_uuid TEXT NOT NULL,state TEXT NOT NULL);",
            )
            .expect("delete-preview fixture schema");
        let profile = Uuid::new_v4();
        let profile_text = profile.hyphenated().to_string();
        let commit = CommitId::fresh();
        let commit_text = commit.to_string();
        connection
            .execute(
                "INSERT INTO vb_commits(id,profile_uuid,pinned,manifest_hash)
                 VALUES (?1,?2,0,?3)",
                rusqlite::params![commit_text, profile_text, "a".repeat(64)],
            )
            .expect("delete-preview commit");
        for hash in ["dangling-one", "dangling-two"] {
            connection
                .execute(
                    "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES (?1,?2)",
                    (&commit_text, hash),
                )
                .expect("dangling commit link");
        }

        let preview = preview_delete_from_with_limit(&connection, profile, commit, 2)
            .expect("exact raw link limit")
            .expect("target commit preview");
        assert_eq!(
            preview.object_links, 0,
            "dangling links are absent from join summary"
        );

        connection
            .execute(
                "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES (?1,'dangling-three')",
                [&commit_text],
            )
            .expect("first over-limit dangling link");
        let error = preview_delete_from_with_limit(&connection, profile, commit, 2)
            .expect_err("over-limit raw links must block the deletion preview");
        assert!(
            error
                .to_string()
                .contains("commit object link index exceeds limit")
        );
        let rows: i64 = connection
            .query_row(
                "SELECT count(*) FROM vb_commit_objects WHERE commit_id=?1",
                [&commit_text],
                |row| row.get(0),
            )
            .expect("raw commit-link count");
        assert_eq!(rows, 3, "failed preview leaves dangling links untouched");
    }

    #[test]
    fn commit_row_limit_is_profile_scoped_and_refuses_at_capacity() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute("CREATE TABLE vb_commits(profile_uuid TEXT NOT NULL)", [])
            .expect("commit fixture schema");
        let profile = Uuid::new_v4();
        let other_profile = Uuid::new_v4();
        for _ in 0..2 {
            connection
                .execute(
                    "INSERT INTO vb_commits(profile_uuid) VALUES (?1)",
                    [profile.hyphenated().to_string()],
                )
                .expect("target-profile commit");
        }
        for _ in 0..3 {
            connection
                .execute(
                    "INSERT INTO vb_commits(profile_uuid) VALUES (?1)",
                    [other_profile.hyphenated().to_string()],
                )
                .expect("other-profile commit");
        }

        assert!(check_profile_commit_rows(&connection, profile, 3).is_ok());
        let at_capacity = check_profile_commit_rows(&connection, profile, 2)
            .expect_err("exact capacity must refuse another commit");
        assert!(at_capacity.to_string().contains("commit-row limit"));
        assert!(check_profile_commit_rows(&connection, other_profile, 4).is_ok());
        assert!(check_profile_commit_rows(&connection, other_profile, 3).is_err());
    }

    #[test]
    fn production_commit_capacity_matches_the_gc_row_bound() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute("CREATE TABLE vb_commits(profile_uuid TEXT NOT NULL)", [])
            .expect("commit fixture schema");
        let profile = Uuid::new_v4();
        let profile_text = profile.hyphenated().to_string();
        let transaction = connection.transaction().expect("fixture transaction");
        {
            let mut insert = transaction
                .prepare_cached("INSERT INTO vb_commits(profile_uuid) VALUES (?1)")
                .expect("commit insert statement");
            for _ in 0..MAX_RETENTION_ROWS {
                insert
                    .execute([profile_text.as_str()])
                    .expect("bounded fixture row");
            }
        }
        transaction.commit().expect("commit fixture rows");

        let full = check_profile_commit_capacity(&connection, profile)
            .expect_err("production capacity must match the collector bound");
        assert!(full.to_string().contains("commit-row limit"));
    }

    #[test]
    fn deletion_preview_counts_refuse_after_the_bounded_row_limit() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_refs(profile_uuid TEXT NOT NULL, commit_id TEXT NOT NULL);
                 INSERT INTO vb_refs(profile_uuid,commit_id) VALUES
                    ('profile','commit'),('profile','commit'),('profile','commit');",
            )
            .expect("reference fixture");

        let exact = count_bounded_u32(
            &connection,
            "SELECT 1 FROM vb_refs WHERE profile_uuid=?1 AND commit_id=?2",
            "profile",
            "commit",
            3,
        )
        .expect("exact row boundary is accepted");
        assert_eq!(exact, 3);
        let over_limit = count_bounded_u32(
            &connection,
            "SELECT 1 FROM vb_refs WHERE profile_uuid=?1 AND commit_id=?2",
            "profile",
            "commit",
            2,
        )
        .expect_err("one row beyond the scan bound must refuse");
        assert!(
            over_limit
                .to_string()
                .contains("versioning count exceeds limit")
        );
    }

    #[test]
    fn deletion_preview_object_link_scan_refuses_over_limit_rows() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_commits(id TEXT PRIMARY KEY, profile_uuid TEXT NOT NULL);
                 CREATE TABLE vb_objects(hash TEXT PRIMARY KEY, bytes INTEGER NOT NULL);
                 CREATE TABLE vb_commit_objects(commit_id TEXT NOT NULL, object_hash TEXT NOT NULL);
                 INSERT INTO vb_commits(id,profile_uuid) VALUES ('commit','profile');
                 INSERT INTO vb_objects(hash,bytes) VALUES ('object',7);
                 INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES
                    ('commit','object'),('commit','object'),('commit','object');",
            )
            .expect("object-link fixture");

        let exact = summarize_commit_objects(&connection, "profile", "commit", 3)
            .expect("exact row boundary is accepted");
        assert_eq!(exact.object_links, 3);
        assert_eq!(exact.reclaimable_objects, 1);
        let over_limit = summarize_commit_objects(&connection, "profile", "commit", 2)
            .expect_err("one object link beyond the scan bound must refuse");
        assert!(
            over_limit
                .to_string()
                .contains("commit object index exceeds limit")
        );
    }

    #[test]
    fn deletion_preview_shared_commit_count_stops_at_the_configured_limit() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_commits(id TEXT PRIMARY KEY,profile_uuid TEXT NOT NULL);
                 CREATE TABLE vb_objects(hash TEXT PRIMARY KEY,bytes INTEGER NOT NULL);
                 CREATE TABLE vb_commit_objects(commit_id TEXT NOT NULL,object_hash TEXT NOT NULL);
                 INSERT INTO vb_commits(id,profile_uuid) VALUES
                    ('target','profile'),('shared-a','profile'),('shared-b','profile');
                 INSERT INTO vb_objects(hash,bytes) VALUES ('object',7);
                 INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES
                    ('target','object'),('shared-a','object'),('shared-b','object');",
            )
            .expect("shared-object fixture");

        let exact = summarize_commit_objects(&connection, "profile", "target", 2)
            .expect("exact distinct-sharer boundary is accepted");
        assert_eq!(exact.shared_objects, 1);

        connection
            .execute(
                "INSERT INTO vb_commits(id,profile_uuid) VALUES ('shared-c','profile')",
                [],
            )
            .expect("extra sharing commit");
        connection
            .execute(
                "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES ('shared-c','object')",
                [],
            )
            .expect("extra sharing root");
        let over_limit = summarize_commit_objects(&connection, "profile", "target", 2)
            .expect_err("first distinct-sharer row beyond the cap must refuse");
        assert!(
            over_limit
                .to_string()
                .contains("shared object reference index exceeds limit")
        );
    }

    #[test]
    fn profile_row_count_refuses_after_the_configured_scan_limit() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_operations(profile_uuid TEXT NOT NULL, state TEXT NOT NULL);
                 INSERT INTO vb_operations(profile_uuid,state) VALUES
                    ('00000000-0000-0000-0000-000000000001','staging'),
                    ('00000000-0000-0000-0000-000000000001','staging'),
                    ('00000000-0000-0000-0000-000000000001','complete');",
            )
            .expect("operation fixture");
        let profile =
            Uuid::parse_str("00000000-0000-0000-0000-000000000001").expect("profile UUID");
        let exact = count_bounded_profile_rows(
            &connection,
            "SELECT 1 FROM vb_operations WHERE profile_uuid=?1 AND state <> 'complete'",
            profile,
            2,
            "active operation index exceeds limit",
        )
        .expect("exact row boundary is accepted");
        assert_eq!(exact, 2);
        let over_limit = count_bounded_profile_rows(
            &connection,
            "SELECT 1 FROM vb_operations WHERE profile_uuid=?1 AND state <> 'complete'",
            profile,
            1,
            "active operation index exceeds limit",
        )
        .expect_err("one active operation beyond the bound must refuse");
        assert!(
            over_limit
                .to_string()
                .contains("active operation index exceeds limit")
        );
    }

    #[test]
    fn gc_object_scan_bytes_are_bounded_by_profile_and_object_limits() {
        assert_eq!(
            account_gc_inspection_bytes(
                MAX_PROFILE_VERSIONING_BYTES - MAX_ARCHIVE_BYTES,
                MAX_ARCHIVE_BYTES,
            )
            .expect("exact profile scan boundary"),
            MAX_PROFILE_VERSIONING_BYTES,
        );
        assert!(account_gc_inspection_bytes(MAX_PROFILE_VERSIONING_BYTES, 1).is_err());
        assert!(account_gc_inspection_bytes(0, MAX_ARCHIVE_BYTES + 1).is_err());
    }

    #[test]
    fn profile_budget_size_walk_is_row_and_byte_bounded() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_objects(bytes INTEGER NOT NULL,state TEXT NOT NULL);
                 CREATE TABLE vb_commits(profile_uuid TEXT NOT NULL,manifest_bytes BLOB NOT NULL);
                 INSERT INTO vb_objects(bytes,state) VALUES (3,'ready'),(5,'quarantined');
                 INSERT INTO vb_commits(profile_uuid,manifest_bytes)
                    VALUES ('profile',x'01020304');",
            )
            .expect("size fixture");
        let profile =
            Uuid::parse_str("00000000-0000-0000-0000-00000000000a").expect("fixed profile UUID");
        connection
            .execute(
                "UPDATE vb_commits SET profile_uuid=?1",
                [profile.hyphenated().to_string()],
            )
            .expect("bind profile");

        assert_eq!(
            bounded_profile_versioning_bytes(&connection, profile, 2)
                .expect("exact indexed-row boundary"),
            12
        );
        connection
            .execute("INSERT INTO vb_objects(bytes,state) VALUES (1,'ready')", [])
            .expect("extra object row");
        assert!(bounded_profile_versioning_bytes(&connection, profile, 2).is_err());

        connection
            .execute("DELETE FROM vb_objects", [])
            .expect("clear size rows");
        connection
            .execute(
                "INSERT INTO vb_objects(bytes,state) VALUES (1,'unknown')",
                [],
            )
            .expect("corrupt state row");
        assert!(bounded_profile_versioning_bytes(&connection, profile, 2).is_err());
        connection
            .execute("DELETE FROM vb_objects", [])
            .expect("clear corrupt state");
        for _ in 0..=MAX_PROFILE_VERSIONING_BYTES / MAX_ARCHIVE_BYTES {
            connection
                .execute(
                    "INSERT INTO vb_objects(bytes,state) VALUES (?1,'ready')",
                    [i64::try_from(MAX_ARCHIVE_BYTES).expect("archive size fits SQLite")],
                )
                .expect("bounded-size object row");
        }
        assert!(bounded_profile_versioning_bytes(&connection, profile, 100).is_err());
    }

    #[test]
    fn journal_prune_refuses_oversized_inputs_before_mutating_rows() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_journal(profile_uuid TEXT NOT NULL, sequence INTEGER NOT NULL);
                 CREATE TABLE vb_operations(profile_uuid TEXT NOT NULL, state TEXT NOT NULL);
                 INSERT INTO vb_journal(profile_uuid,sequence) VALUES
                    ('profile',1),('profile',2),('profile',3);",
            )
            .expect("journal fixture");
        let profile =
            Uuid::parse_str("00000000-0000-0000-0000-00000000000a").expect("fixed profile UUID");
        connection
            .execute(
                "UPDATE vb_journal SET profile_uuid=?1",
                [profile.hyphenated().to_string()],
            )
            .expect("bind journal profile");

        assert!(ensure_journal_prune_input_bounds(&connection, profile, 2).is_err());
        let journal_rows: i64 = connection
            .query_row("SELECT count(*) FROM vb_journal", [], |row| row.get(0))
            .expect("journal rows remain");
        assert_eq!(journal_rows, 3);

        connection
            .execute("DELETE FROM vb_journal", [])
            .expect("clear journal fixture");
        connection
            .execute_batch(
                "INSERT INTO vb_operations(profile_uuid,state) VALUES
                    ('profile','complete'),('profile','failed'),('profile','cancelled');",
            )
            .expect("terminal operation fixture");
        connection
            .execute(
                "UPDATE vb_operations SET profile_uuid=?1",
                [profile.hyphenated().to_string()],
            )
            .expect("bind operation profile");
        assert!(ensure_journal_prune_input_bounds(&connection, profile, 2).is_err());
        let operation_rows: i64 = connection
            .query_row("SELECT count(*) FROM vb_operations", [], |row| row.get(0))
            .expect("operation rows remain");
        assert_eq!(operation_rows, 3);
    }

    #[test]
    fn object_reference_capacity_counts_only_new_profile_roots() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_commits(id TEXT PRIMARY KEY,profile_uuid TEXT NOT NULL);
                 CREATE TABLE vb_commit_objects(commit_id TEXT NOT NULL,object_hash TEXT NOT NULL);
                 INSERT INTO vb_commits(id,profile_uuid) VALUES ('one','profile-a'),('two','profile-b');
                 INSERT INTO vb_commit_objects(commit_id,object_hash)
                    VALUES ('one','existing-a'),('one','existing-b'),('two','other-profile');",
            )
            .expect("reference fixture schema and rows");
        let profile =
            Uuid::parse_str("00000000-0000-0000-0000-00000000000a").expect("fixed profile UUID");
        connection
            .execute(
                "UPDATE vb_commits SET profile_uuid=?1 WHERE id='one'",
                [profile.hyphenated().to_string()],
            )
            .expect("bind target profile");

        let incoming = BTreeMap::from([("existing-a".to_owned(), 1), ("new-c".to_owned(), 1)]);
        assert!(check_profile_object_reference_rows(&connection, profile, &incoming, 3).is_ok());

        let too_many = BTreeMap::from([
            ("existing-a".to_owned(), 1),
            ("new-c".to_owned(), 1),
            ("new-d".to_owned(), 1),
        ]);
        let error = check_profile_object_reference_rows(&connection, profile, &too_many, 3)
            .expect_err("new distinct references must respect the GC root cap");
        assert!(error.to_string().contains("object-reference limit"));
    }

    #[test]
    fn object_reference_capacity_stops_at_first_root_over_the_limit() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE vb_commits(id TEXT PRIMARY KEY,profile_uuid TEXT NOT NULL);
                 CREATE TABLE vb_commit_objects(commit_id TEXT NOT NULL,object_hash TEXT NOT NULL);
                 INSERT INTO vb_commits(id,profile_uuid) VALUES ('one','profile');
                 INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES
                    ('one','root-a'),('one','root-b'),('one','root-c');",
            )
            .expect("root fixture");
        let profile =
            Uuid::parse_str("00000000-0000-0000-0000-00000000000a").expect("fixed profile UUID");
        connection
            .execute(
                "UPDATE vb_commits SET profile_uuid=?1 WHERE id='one'",
                [profile.hyphenated().to_string()],
            )
            .expect("bind profile");

        let no_incoming = BTreeMap::new();
        assert!(check_profile_object_reference_rows(&connection, profile, &no_incoming, 3).is_ok());
        connection
            .execute(
                "INSERT INTO vb_commit_objects(commit_id,object_hash) VALUES ('one','root-d')",
                [],
            )
            .expect("over-limit root");
        let error = check_profile_object_reference_rows(&connection, profile, &no_incoming, 3)
            .expect_err("one root beyond the limit must refuse");
        assert!(error.to_string().contains("object-reference limit"));
    }
}
