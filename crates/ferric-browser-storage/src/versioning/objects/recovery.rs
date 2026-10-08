//! Recoverable quarantine for unpublished profile-local staging and objects.
//! This runs only at a quiescent normal-profile restart boundary.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[cfg(not(unix))]
use std::fs::DirBuilder;

#[cfg(unix)]
use ferric_browser_core::versioning::limits::MAX_PAGES_PER_WINDOW_CAPTURE;
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use super::{ObjectPathError, VersioningObjectStore, private_directory};

const MAX_RECOVERY_ENTRIES: usize = 100_000;

#[cfg(unix)]
fn account_scanned_entries(
    examined: &mut usize,
    additional: usize,
    maximum: usize,
) -> Result<(), ObjectPathError> {
    *examined = (*examined)
        .checked_add(additional)
        .filter(|total| *total <= maximum)
        .ok_or(ObjectPathError::CaptureTooLarge)?;
    Ok(())
}

#[cfg(unix)]
fn scan_stage_entries_accounted(
    path: &Path,
    owner_uid: u32,
    examined: &mut usize,
    maximum: usize,
    maximum_per_directory: usize,
) -> Result<Vec<StageEntry>, ObjectPathError> {
    let remaining = maximum.saturating_sub(*examined);
    let entries =
        scan_stage_entries_with_limit(path, owner_uid, maximum_per_directory.min(remaining))?;
    account_scanned_entries(examined, entries.len(), maximum)?;
    Ok(entries)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaptureStageCleanupCheckpoint {
    BeforeLeafRemoval(usize),
    AfterLeafRemoval(usize),
    BeforeOperationDirectorySync,
    AfterOperationDirectorySync,
    AfterOperationDirectoryRemoval,
    BeforeStagingDirectorySync,
    AfterStagingDirectorySync,
}

/// Files moved into private quarantine; no bytes are deleted by recovery.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RecoverySummary {
    pub staging_operations: usize,
    pub orphan_objects: usize,
}

struct Candidate {
    source: PathBuf,
    destination_name: String,
    kind: CandidateKind,
    #[cfg(unix)]
    stage_entries: Vec<StageEntry>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    links: u64,
}

#[derive(Clone, Copy)]
enum CandidateKind {
    StagingDirectory,
    ViewerDirectory,
    OrphanObject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoverySyncCheckpoint {
    BatchCreated,
    /// Test seam between the final source identity check and the rename.
    AfterSourceIdentityBeforeMove(usize),
    BeforeBatchSync(usize),
    BeforeSourceSync(usize),
    BeforeFinalQuarantineSync,
}

#[cfg(unix)]
#[derive(Eq, PartialEq)]
struct StageEntry {
    name: String,
    bytes: u64,
    device: u64,
    inode: u64,
    links: u64,
}

struct CorruptCandidate {
    source: PathBuf,
    destination_name: String,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl VersioningObjectStore {
    /// Counts retained capture staging bytes not covered by an active
    /// reservation. An active operation contributes only observed excess over
    /// its reservation; finished/abandoned operations contribute all leaves.
    pub(crate) fn unreserved_live_staging_bytes(
        &self,
        active_reservations: &BTreeMap<String, u64>,
    ) -> Result<u64, ObjectPathError> {
        #[cfg(not(unix))]
        {
            let _ = active_reservations;
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            private_directory(&self.profile_root, Some(self.owner_uid))?;
            let staging = self.profile_root.join("staging");
            private_directory(&staging, Some(self.owner_uid))?;
            let mut examined = 0usize;
            let mut unreserved = 0u64;
            for entry in fs::read_dir(&staging)? {
                examined += 1;
                if examined > MAX_RECOVERY_ENTRIES {
                    return Err(ObjectPathError::CaptureTooLarge);
                }
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                let operation = Uuid::parse_str(name).map_err(|_| ObjectPathError::UnsafeEntry)?;
                if operation.get_version() != Some(uuid::Version::Random) {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                private_directory(&entry.path(), Some(self.owner_uid))?;
                let mut operation_bytes = 0u64;
                for leaf in scan_stage_entries_accounted(
                    &entry.path(),
                    self.owner_uid,
                    &mut examined,
                    MAX_RECOVERY_ENTRIES,
                    MAX_PAGES_PER_WINDOW_CAPTURE + 3,
                )? {
                    operation_bytes = operation_bytes
                        .checked_add(leaf.bytes)
                        .ok_or(ObjectPathError::CaptureTooLarge)?;
                }
                let uncovered = operation_bytes
                    .saturating_sub(active_reservations.get(name).copied().unwrap_or(0));
                unreserved = unreserved
                    .checked_add(uncovered)
                    .ok_or(ObjectPathError::CaptureTooLarge)?;
            }
            Ok(unreserved)
        }
    }

    /// Retires the exact staging leaves for an operation after its manifest is
    /// durable. Missing directories are idempotent; unknown or unsafe leaves
    /// refuse cleanup before the first unlink.
    #[cfg(test)]
    pub(crate) fn discard_published_capture_staging(
        &self,
        operation: ferric_browser_core::OperationId,
    ) -> Result<(), ObjectPathError> {
        self.discard_published_capture_staging_with_checkpoint(operation, |_| Ok(()))
    }

    pub(crate) fn discard_published_capture_staging_with_checkpoint(
        &self,
        operation: ferric_browser_core::OperationId,
        checkpoint: impl FnMut(CaptureStageCleanupCheckpoint) -> Result<(), ObjectPathError>,
    ) -> Result<(), ObjectPathError> {
        self.discard_published_capture_staging_with_checkpoint_and_sync(
            operation,
            checkpoint,
            |directory| directory.sync_all(),
        )
    }

    /// Variant with a narrow directory-sync seam for deterministic I/O-error
    /// injection. Production callers use the wrapper above and real `sync_all`.
    pub(crate) fn discard_published_capture_staging_with_checkpoint_and_sync(
        &self,
        operation: ferric_browser_core::OperationId,
        mut checkpoint: impl FnMut(CaptureStageCleanupCheckpoint) -> Result<(), ObjectPathError>,
        mut sync_directory: impl FnMut(&std::fs::File) -> std::io::Result<()>,
    ) -> Result<(), ObjectPathError> {
        #[cfg(not(unix))]
        {
            let _ = (operation, &mut checkpoint, &mut sync_directory);
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            let staging = self.profile_root.join("staging");
            validate_recovery_ancestors(&self.profile_root, &staging, self.owner_uid)?;
            let staging_directory = super::dirfd::open_private_directory(&staging, self.owner_uid)?;
            let staging_identity = staging_directory.metadata()?;
            let operation_name = operation.as_uuid().hyphenated().to_string();
            let directory = staging.join(&operation_name);
            let operation_directory = match super::dirfd::open_private_directory_at(
                &staging_directory,
                &operation_name,
                self.owner_uid,
            ) {
                Ok(directory) => directory,
                Err(ObjectPathError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            let operation_identity = operation_directory.metadata()?;
            let mut remaining = scan_stage_entries(&directory, self.owner_uid)?;
            let mut removed = 0usize;

            while !remaining.is_empty() {
                validate_recovery_ancestors(&self.profile_root, &staging, self.owner_uid)?;
                let current_staging_identity = staging_directory.metadata()?;
                if current_staging_identity.dev() != staging_identity.dev()
                    || current_staging_identity.ino() != staging_identity.ino()
                {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                let current_operation_identity = operation_directory.metadata()?;
                if current_operation_identity.dev() != operation_identity.dev()
                    || current_operation_identity.ino() != operation_identity.ino()
                {
                    return Err(ObjectPathError::UnsafeEntry);
                }

                let entry = remaining.remove(0);
                checkpoint(CaptureStageCleanupCheckpoint::BeforeLeafRemoval(removed))?;
                let metadata = super::dirfd::regular_entry_identity_at(
                    &operation_directory,
                    &entry.name,
                    self.owner_uid,
                )?;
                if metadata.links != entry.links
                    || metadata.device != entry.device
                    || metadata.inode != entry.inode
                    || metadata.bytes != entry.bytes
                {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                super::dirfd::unlink_at(&operation_directory, &entry.name, false)?;
                checkpoint(CaptureStageCleanupCheckpoint::AfterLeafRemoval(removed))?;
                removed += 1;
            }

            validate_recovery_ancestors(&self.profile_root, &staging, self.owner_uid)?;
            let current_staging_identity = staging_directory.metadata()?;
            let current_operation_identity = operation_directory.metadata()?;
            if current_staging_identity.dev() != staging_identity.dev()
                || current_staging_identity.ino() != staging_identity.ino()
                || current_operation_identity.dev() != operation_identity.dev()
                || current_operation_identity.ino() != operation_identity.ino()
            {
                return Err(ObjectPathError::UnsafeEntry);
            }
            checkpoint(CaptureStageCleanupCheckpoint::BeforeOperationDirectorySync)?;
            sync_directory(&operation_directory)?;
            checkpoint(CaptureStageCleanupCheckpoint::AfterOperationDirectorySync)?;
            super::dirfd::unlink_at(&staging_directory, &operation_name, true)?;
            checkpoint(CaptureStageCleanupCheckpoint::AfterOperationDirectoryRemoval)?;
            checkpoint(CaptureStageCleanupCheckpoint::BeforeStagingDirectorySync)?;
            sync_directory(&staging_directory)?;
            checkpoint(CaptureStageCleanupCheckpoint::AfterStagingDirectorySync)?;
            Ok(())
        }
    }

    /// Counts live private archive-view copies. These are separate physical
    /// files from indexed immutable objects and may remain open in `WebEngine`;
    /// admission only observes them and never attempts cleanup.
    pub(crate) fn live_viewer_bytes(&self) -> Result<u64, ObjectPathError> {
        #[cfg(not(unix))]
        {
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            private_directory(&self.profile_root, Some(self.owner_uid))?;
            let views = self.profile_root.join("views");
            private_directory(&views, Some(self.owner_uid))?;
            let mut examined = 0usize;
            let mut bytes = 0u64;
            for entry in fs::read_dir(&views)? {
                examined += 1;
                if examined > MAX_RECOVERY_ENTRIES {
                    return Err(ObjectPathError::CaptureTooLarge);
                }
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                let operation = Uuid::parse_str(name).map_err(|_| ObjectPathError::UnsafeEntry)?;
                if operation.get_version() != Some(uuid::Version::Random) {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                private_directory(&entry.path(), Some(self.owner_uid))?;
                for leaf in scan_stage_entries_accounted(
                    &entry.path(),
                    self.owner_uid,
                    &mut examined,
                    MAX_RECOVERY_ENTRIES,
                    2,
                )? {
                    if !matches!(leaf.name.as_str(), "capture.bin" | "capture.mhtml") {
                        return Err(ObjectPathError::UnsafeEntry);
                    }
                    bytes = bytes
                        .checked_add(leaf.bytes)
                        .ok_or(ObjectPathError::CaptureTooLarge)?;
                }
            }
            Ok(bytes)
        }
    }

    /// Counts live immutable-object files not already represented by a ready
    /// SQLite object row. A failed publication can leave these bytes behind
    /// until restart recovery; admission must not treat that gap as free room.
    pub(crate) fn unindexed_live_object_bytes(
        &self,
        indexed_ready_objects: &BTreeMap<String, u64>,
    ) -> Result<u64, ObjectPathError> {
        #[cfg(not(unix))]
        {
            let _ = indexed_ready_objects;
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            private_directory(&self.profile_root, Some(self.owner_uid))?;
            let objects = self.objects_root();
            private_directory(&objects, Some(self.owner_uid))?;
            let mut examined = 0usize;
            let mut bytes = 0u64;
            for shard in fs::read_dir(&objects)? {
                examined += 1;
                if examined > MAX_RECOVERY_ENTRIES {
                    return Err(ObjectPathError::CaptureTooLarge);
                }
                let shard = shard?;
                let name = shard.file_name();
                let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                if !is_lower_hex(name, 2) {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                private_directory(&shard.path(), Some(self.owner_uid))?;
                for entry in fs::read_dir(shard.path())? {
                    examined += 1;
                    if examined > MAX_RECOVERY_ENTRIES {
                        return Err(ObjectPathError::CaptureTooLarge);
                    }
                    let entry = entry?;
                    let leaf = entry.file_name();
                    let leaf = leaf.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                    if !is_lower_hex(leaf, 62) {
                        return Err(ObjectPathError::UnsafeEntry);
                    }
                    let metadata = fs::symlink_metadata(entry.path())?;
                    if !metadata.file_type().is_file()
                        || metadata.uid() != self.owner_uid
                        || metadata.mode() & 0o077 != 0
                        || !matches!(metadata.nlink(), 1 | 2)
                    {
                        return Err(ObjectPathError::UnsafeEntry);
                    }
                    if let Some(indexed_bytes) = indexed_ready_objects.get(&format!("{name}{leaf}"))
                    {
                        if metadata.len() != *indexed_bytes {
                            return Err(ObjectPathError::UnsafeEntry);
                        }
                    } else {
                        bytes = bytes
                            .checked_add(metadata.len())
                            .ok_or(ObjectPathError::CaptureTooLarge)?;
                    }
                }
            }
            Ok(bytes)
        }
    }

    /// Counts unpublished stage, viewer, and orphan-object bytes retained in
    /// private quarantine. This is an admission input, not a physical quota:
    /// indexed corrupt objects are accounted for by SQLite separately.
    pub(crate) fn unpublished_quarantine_bytes(&self) -> Result<u64, ObjectPathError> {
        #[cfg(not(unix))]
        {
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            private_directory(&self.profile_root, Some(self.owner_uid))?;
            let quarantine = self.profile_root.join("quarantine");
            private_directory(&quarantine, Some(self.owner_uid))?;
            let mut examined = 0usize;
            let mut bytes = 0u64;
            for batch in fs::read_dir(&quarantine)? {
                examined += 1;
                if examined > MAX_RECOVERY_ENTRIES {
                    return Err(ObjectPathError::CaptureTooLarge);
                }
                let batch = batch?;
                let name = batch.file_name();
                let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                let id = Uuid::parse_str(name).map_err(|_| ObjectPathError::UnsafeEntry)?;
                if id.get_version() != Some(uuid::Version::Random) {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                private_directory(&batch.path(), Some(self.owner_uid))?;
                for entry in fs::read_dir(batch.path())? {
                    examined += 1;
                    if examined > MAX_RECOVERY_ENTRIES {
                        return Err(ObjectPathError::CaptureTooLarge);
                    }
                    let entry = entry?;
                    let name = entry.file_name();
                    let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                    if let Some(operation) = name
                        .strip_prefix("stage-")
                        .or_else(|| name.strip_prefix("view-"))
                    {
                        Uuid::parse_str(operation).map_err(|_| ObjectPathError::UnsafeEntry)?;
                        private_directory(&entry.path(), Some(self.owner_uid))?;
                        let per_directory_limit = if name.starts_with("view-") {
                            2
                        } else {
                            MAX_PAGES_PER_WINDOW_CAPTURE + 3
                        };
                        for leaf in scan_stage_entries_accounted(
                            &entry.path(),
                            self.owner_uid,
                            &mut examined,
                            MAX_RECOVERY_ENTRIES,
                            per_directory_limit,
                        )? {
                            if name.starts_with("view-")
                                && !matches!(leaf.name.as_str(), "capture.bin" | "capture.mhtml")
                            {
                                return Err(ObjectPathError::UnsafeEntry);
                            }
                            bytes = bytes
                                .checked_add(leaf.bytes)
                                .ok_or(ObjectPathError::CaptureTooLarge)?;
                        }
                    } else if let Some(hash) = name.strip_prefix("object-") {
                        if !is_lower_hex(hash, 64) {
                            return Err(ObjectPathError::UnsafeEntry);
                        }
                        let metadata = fs::symlink_metadata(entry.path())?;
                        if !metadata.file_type().is_file()
                            || metadata.uid() != self.owner_uid
                            || metadata.mode() & 0o077 != 0
                            || !matches!(metadata.nlink(), 1 | 2)
                        {
                            return Err(ObjectPathError::UnsafeEntry);
                        }
                        bytes = bytes
                            .checked_add(metadata.len())
                            .ok_or(ObjectPathError::CaptureTooLarge)?;
                    } else if let Some(hash) = name.strip_prefix("corrupt-") {
                        if !is_lower_hex(hash, 64) {
                            return Err(ObjectPathError::UnsafeEntry);
                        }
                        let metadata = fs::symlink_metadata(entry.path())?;
                        if !metadata.file_type().is_file()
                            || metadata.uid() != self.owner_uid
                            || metadata.mode() & 0o077 != 0
                            || metadata.nlink() != 1
                        {
                            return Err(ObjectPathError::UnsafeEntry);
                        }
                    } else {
                        return Err(ObjectPathError::UnsafeEntry);
                    }
                }
            }
            Ok(bytes)
        }
    }

    /// Finds exact corrupt-object names already held in private quarantine.
    /// A process can die after the durable file move but before its SQLite
    /// state update commits; recovery must not mislabel those bytes as absent.
    pub(crate) fn quarantined_corrupt_hashes(&self) -> Result<BTreeSet<String>, ObjectPathError> {
        #[cfg(not(unix))]
        {
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            let quarantine = self.profile_root.join("quarantine");
            private_directory(&self.profile_root, Some(self.owner_uid))?;
            private_directory(&quarantine, Some(self.owner_uid))?;
            let mut hashes = BTreeSet::new();
            let mut inspected = 0_usize;
            for batch in fs::read_dir(&quarantine)? {
                inspected += 1;
                if inspected > MAX_RECOVERY_ENTRIES {
                    return Err(ObjectPathError::CaptureTooLarge);
                }
                let batch = batch?;
                let name = batch.file_name();
                let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                let id = Uuid::parse_str(name).map_err(|_| ObjectPathError::UnsafeEntry)?;
                if id.get_version() != Some(uuid::Version::Random) {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                private_directory(&batch.path(), Some(self.owner_uid))?;
                for entry in fs::read_dir(batch.path())? {
                    inspected += 1;
                    if inspected > MAX_RECOVERY_ENTRIES {
                        return Err(ObjectPathError::CaptureTooLarge);
                    }
                    let entry = entry?;
                    let name = entry.file_name();
                    let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                    let Some(hash) = name.strip_prefix("corrupt-") else {
                        continue;
                    };
                    if !is_lower_hex(hash, 64) {
                        return Err(ObjectPathError::InvalidObjectDigest);
                    }
                    let metadata = fs::symlink_metadata(entry.path())?;
                    if !metadata.file_type().is_file()
                        || metadata.uid() != self.owner_uid
                        || metadata.mode() & 0o077 != 0
                        || metadata.nlink() != 1
                    {
                        return Err(ObjectPathError::UnsafeEntry);
                    }
                    hashes.insert(hash.to_owned());
                }
            }
            Ok(hashes)
        }
    }

    /// Moves exact referenced object files that failed digest/type validation
    /// into a private quarantine batch. Missing files are intentionally left
    /// absent; callers record them as missing in the metadata index.
    ///
    /// # Errors
    /// Rejects invalid digests, unsafe parents/entries, or failed durable moves.
    pub(crate) fn quarantine_corrupt_references(
        &self,
        hashes: &BTreeSet<String>,
    ) -> Result<usize, ObjectPathError> {
        self.quarantine_corrupt_references_with_hook(hashes, || Ok(()))
    }

    fn quarantine_corrupt_references_with_hook(
        &self,
        hashes: &BTreeSet<String>,
        hook: impl FnOnce() -> Result<(), ObjectPathError>,
    ) -> Result<usize, ObjectPathError> {
        self.quarantine_corrupt_references_with_hooks(hashes, hook, |_| Ok(()))
    }

    fn quarantine_corrupt_references_with_hooks(
        &self,
        hashes: &BTreeSet<String>,
        hook: impl FnOnce() -> Result<(), ObjectPathError>,
        move_hook: impl FnMut(usize) -> Result<(), ObjectPathError>,
    ) -> Result<usize, ObjectPathError> {
        #[cfg(not(unix))]
        {
            let _ = (hashes, hook, move_hook);
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            let mut move_hook = move_hook;
            if hashes.is_empty() {
                return Ok(0);
            }
            if hashes.len() > MAX_RECOVERY_ENTRIES {
                return Err(ObjectPathError::CaptureTooLarge);
            }
            if hashes.iter().any(|digest| !is_lower_hex(digest, 64)) {
                return Err(ObjectPathError::InvalidObjectDigest);
            }
            let objects = self.objects_root();
            private_directory(&self.profile_root, Some(self.owner_uid))?;
            private_directory(&objects, Some(self.owner_uid))?;
            // Validate every present source before moving the first one. A late
            // unsafe shard or hard link must not strand earlier references in a
            // partial quarantine batch.
            let mut candidates = Vec::new();
            for digest in hashes {
                let shard = objects.join(&digest[..2]);
                match fs::symlink_metadata(&shard) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                    Ok(_) => {}
                }
                private_directory(&shard, Some(self.owner_uid))?;
                let source = shard.join(&digest[2..]);
                let metadata = match fs::symlink_metadata(&source) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                };
                #[cfg(unix)]
                if !metadata.file_type().is_file()
                    || metadata.file_type().is_symlink()
                    || metadata.uid() != self.owner_uid
                    || metadata.mode() & 0o077 != 0
                    || metadata.nlink() != 1
                {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                #[cfg(not(unix))]
                if !metadata.file_type().is_file() {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                candidates.push(CorruptCandidate {
                    source,
                    destination_name: format!("corrupt-{digest}"),
                    #[cfg(unix)]
                    device: metadata.dev(),
                    #[cfg(unix)]
                    inode: metadata.ino(),
                });
            }
            if candidates.is_empty() {
                return Ok(0);
            }
            hook()?;
            for candidate in &candidates {
                validate_corrupt_candidate(candidate, &self.profile_root, self.owner_uid)?;
            }
            let quarantine = self.profile_root.join("quarantine");
            let quarantine_dir = super::dirfd::open_private_directory(&quarantine, self.owner_uid)?;
            let batch_name = Uuid::new_v4().hyphenated().to_string();
            super::dirfd::mkdir_at(&quarantine_dir, &batch_name)?;
            let batch_dir = super::dirfd::open_private_directory_at(
                &quarantine_dir,
                &batch_name,
                self.owner_uid,
            )?;
            quarantine_dir.sync_all()?;
            for (index, candidate) in candidates.iter().enumerate() {
                validate_corrupt_candidate(candidate, &self.profile_root, self.owner_uid)?;
                let source_parent = candidate
                    .source
                    .parent()
                    .ok_or(ObjectPathError::UnsafeEntry)?;
                let source_parent =
                    super::dirfd::open_private_directory(source_parent, self.owner_uid)?;
                let source_name = candidate
                    .source
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or(ObjectPathError::UnsafeEntry)?;
                let identity = super::dirfd::regular_entry_identity_at(
                    &source_parent,
                    source_name,
                    self.owner_uid,
                )?;
                if identity.device != candidate.device
                    || identity.inode != candidate.inode
                    || identity.links != 1
                {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                move_hook(index + 1)?;
                super::dirfd::rename_at(
                    &source_parent,
                    source_name,
                    &batch_dir,
                    &candidate.destination_name,
                )?;
                let moved_identity = super::dirfd::regular_entry_identity_at(
                    &batch_dir,
                    &candidate.destination_name,
                    self.owner_uid,
                );
                let moved_matches = matches!(
                    moved_identity,
                    Ok(identity)
                        if identity.device == candidate.device
                            && identity.inode == candidate.inode
                            && identity.links == 1
                );
                batch_dir.sync_all()?;
                source_parent.sync_all()?;
                if !moved_matches {
                    return Err(ObjectPathError::UnsafeEntry);
                }
            }
            quarantine_dir.sync_all()?;
            Ok(candidates.len())
        }
    }

    /// Moves unpublished staging directories and unreferenced objects into a
    /// fresh private quarantine batch. Call only after all earlier capture
    /// writers have stopped; active operations must never be swept.
    ///
    /// # Errors
    /// Refuses unsafe or oversized directory trees and propagates failed
    /// moves/fsyncs. A partial batch remains recoverable and is never deleted.
    pub(crate) fn quarantine_after_restart(
        &self,
        referenced_hashes: &BTreeSet<String>,
    ) -> Result<RecoverySummary, ObjectPathError> {
        self.quarantine_after_restart_with_hook(referenced_hashes, |_| Ok(()))
    }

    /// Live collection may reclaim capture staging and orphan objects, but
    /// cannot infer whether an isolated viewer still reads its private file.
    /// Only first-owner restart recovery scans the separate viewer root.
    pub(crate) fn quarantine_during_live_gc(
        &self,
        referenced_hashes: &BTreeSet<String>,
    ) -> Result<RecoverySummary, ObjectPathError> {
        self.quarantine_with_hooks(referenced_hashes, |_| Ok(()), |_| Ok(()), false)
    }

    fn quarantine_after_restart_with_hook(
        &self,
        referenced_hashes: &BTreeSet<String>,
        hook: impl FnMut(usize) -> Result<(), ObjectPathError>,
    ) -> Result<RecoverySummary, ObjectPathError> {
        self.quarantine_after_restart_with_hooks(referenced_hashes, hook, |_| Ok(()))
    }

    fn quarantine_after_restart_with_hooks(
        &self,
        referenced_hashes: &BTreeSet<String>,
        hook: impl FnMut(usize) -> Result<(), ObjectPathError>,
        sync_hook: impl FnMut(RecoverySyncCheckpoint) -> Result<(), ObjectPathError>,
    ) -> Result<RecoverySummary, ObjectPathError> {
        self.quarantine_with_hooks(referenced_hashes, hook, sync_hook, true)
    }

    #[allow(clippy::too_many_lines)]
    fn quarantine_with_hooks(
        &self,
        referenced_hashes: &BTreeSet<String>,
        mut hook: impl FnMut(usize) -> Result<(), ObjectPathError>,
        mut sync_hook: impl FnMut(RecoverySyncCheckpoint) -> Result<(), ObjectPathError>,
        include_viewers: bool,
    ) -> Result<RecoverySummary, ObjectPathError> {
        if referenced_hashes.len() > MAX_RECOVERY_ENTRIES {
            return Err(ObjectPathError::CaptureTooLarge);
        }
        if referenced_hashes
            .iter()
            .any(|digest| !is_lower_hex(digest, 64))
        {
            return Err(ObjectPathError::InvalidObjectDigest);
        }
        let staging = self.profile_root.join("staging");
        let views = self.profile_root.join("views");
        let objects = self.objects_root();
        let quarantine = self.profile_root.join("quarantine");
        private_directory(&staging, Some(self.owner_uid))?;
        private_directory(&views, Some(self.owner_uid))?;
        private_directory(&objects, Some(self.owner_uid))?;
        #[cfg(unix)]
        let quarantine_dir = super::dirfd::open_private_directory(&quarantine, self.owner_uid)?;
        #[cfg(not(unix))]
        let quarantine_dir = private_directory(&quarantine, Some(self.owner_uid))?;
        let mut candidates = Vec::new();
        let mut summary = RecoverySummary::default();
        let mut examined = 0usize;
        #[cfg(unix)]
        let mut linked_temporaries = BTreeMap::new();

        for entry in fs::read_dir(&staging)? {
            let entry = entry?;
            examined += 1;
            if examined > MAX_RECOVERY_ENTRIES {
                return Err(ObjectPathError::CaptureTooLarge);
            }
            let name = entry.file_name();
            let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
            if Uuid::parse_str(name).is_err() {
                return Err(ObjectPathError::UnsafeEntry);
            }
            private_directory(&entry.path(), Some(self.owner_uid))?;
            let metadata = fs::symlink_metadata(entry.path())?;
            #[cfg(unix)]
            let stage_entries = scan_stage_entries_accounted(
                &entry.path(),
                self.owner_uid,
                &mut examined,
                MAX_RECOVERY_ENTRIES,
                MAX_PAGES_PER_WINDOW_CAPTURE + 3,
            )?;
            #[cfg(unix)]
            for leaf in &stage_entries {
                if leaf.links == 2
                    && linked_temporaries
                        .insert((leaf.device, leaf.inode), entry.path().join(&leaf.name))
                        .is_some()
                {
                    return Err(ObjectPathError::UnsafeEntry);
                }
            }
            candidates.push(Candidate {
                source: entry.path(),
                destination_name: format!("stage-{name}"),
                kind: CandidateKind::StagingDirectory,
                #[cfg(unix)]
                stage_entries,
                #[cfg(unix)]
                device: metadata.dev(),
                #[cfg(unix)]
                inode: metadata.ino(),
                #[cfg(unix)]
                links: metadata.nlink(),
            });
            summary.staging_operations += 1;
        }

        if include_viewers {
            for entry in fs::read_dir(&views)? {
                let entry = entry?;
                examined += 1;
                if examined > MAX_RECOVERY_ENTRIES {
                    return Err(ObjectPathError::CaptureTooLarge);
                }
                let name = entry.file_name();
                let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                if Uuid::parse_str(name).is_err() {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                private_directory(&entry.path(), Some(self.owner_uid))?;
                let metadata = fs::symlink_metadata(entry.path())?;
                #[cfg(unix)]
                let stage_entries = scan_stage_entries_accounted(
                    &entry.path(),
                    self.owner_uid,
                    &mut examined,
                    MAX_RECOVERY_ENTRIES,
                    2,
                )?;
                #[cfg(unix)]
                if stage_entries
                    .iter()
                    .any(|leaf| !matches!(leaf.name.as_str(), "capture.bin" | "capture.mhtml"))
                {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                candidates.push(Candidate {
                    source: entry.path(),
                    destination_name: format!("view-{name}"),
                    kind: CandidateKind::ViewerDirectory,
                    #[cfg(unix)]
                    stage_entries,
                    #[cfg(unix)]
                    device: metadata.dev(),
                    #[cfg(unix)]
                    inode: metadata.ino(),
                    #[cfg(unix)]
                    links: metadata.nlink(),
                });
                summary.staging_operations += 1;
            }
        }

        for shard in fs::read_dir(&objects)? {
            let shard = shard?;
            examined += 1;
            if examined > MAX_RECOVERY_ENTRIES {
                return Err(ObjectPathError::CaptureTooLarge);
            }
            let shard_name = shard.file_name();
            let shard_name = shard_name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
            if !is_lower_hex(shard_name, 2) {
                return Err(ObjectPathError::UnsafeEntry);
            }
            private_directory(&shard.path(), Some(self.owner_uid))?;
            for entry in fs::read_dir(shard.path())? {
                let entry = entry?;
                examined += 1;
                if examined > MAX_RECOVERY_ENTRIES {
                    return Err(ObjectPathError::CaptureTooLarge);
                }
                let leaf = entry.file_name();
                let leaf = leaf.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
                if !is_lower_hex(leaf, 62) {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                let digest = format!("{shard_name}{leaf}");
                if referenced_hashes.contains(&digest) {
                    continue;
                }
                let metadata = fs::symlink_metadata(entry.path())?;
                #[cfg(unix)]
                if !metadata.is_file()
                    || metadata.file_type().is_symlink()
                    || metadata.uid() != self.owner_uid
                    || metadata.mode() & 0o077 != 0
                    || (metadata.nlink() != 1
                        && (metadata.nlink() != 2
                            || linked_temporaries
                                .remove(&(metadata.dev(), metadata.ino()))
                                .is_none()))
                {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                candidates.push(Candidate {
                    source: entry.path(),
                    destination_name: format!("object-{digest}"),
                    kind: CandidateKind::OrphanObject,
                    #[cfg(unix)]
                    stage_entries: Vec::new(),
                    #[cfg(unix)]
                    device: metadata.dev(),
                    #[cfg(unix)]
                    inode: metadata.ino(),
                    #[cfg(unix)]
                    links: metadata.nlink(),
                });
                summary.orphan_objects += 1;
            }
        }
        #[cfg(unix)]
        if !linked_temporaries.is_empty() {
            return Err(ObjectPathError::UnsafeEntry);
        }
        if candidates.is_empty() {
            return Ok(summary);
        }
        candidates.sort_by(|left, right| left.destination_name.cmp(&right.destination_name));
        for candidate in &candidates {
            validate_restart_candidate(candidate, &self.profile_root, self.owner_uid)?;
        }
        let batch_name = Uuid::new_v4().hyphenated().to_string();
        #[cfg(unix)]
        let batch_dir = {
            super::dirfd::mkdir_at(&quarantine_dir, &batch_name)?;
            super::dirfd::open_private_directory_at(&quarantine_dir, &batch_name, self.owner_uid)?
        };
        #[cfg(not(unix))]
        let batch_dir = {
            let batch_path = quarantine.join(&batch_name);
            let mut builder = DirBuilder::new();
            builder.create(&batch_path)?;
            private_directory(&batch_path, Some(self.owner_uid))?
        };
        sync_hook(RecoverySyncCheckpoint::BatchCreated)?;
        quarantine_dir.sync_all()?;
        let mut moved = 0usize;
        for candidate in candidates {
            validate_restart_candidate(&candidate, &self.profile_root, self.owner_uid)?;
            #[cfg(unix)]
            let _source_parent = {
                let source_parent_path = candidate
                    .source
                    .parent()
                    .ok_or(ObjectPathError::UnsafeEntry)?;
                let source_parent =
                    super::dirfd::open_private_directory(source_parent_path, self.owner_uid)?;
                let source_name = candidate
                    .source
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or(ObjectPathError::UnsafeEntry)?;
                match candidate.kind {
                    CandidateKind::StagingDirectory | CandidateKind::ViewerDirectory => {
                        let source_directory = super::dirfd::open_private_directory_at(
                            &source_parent,
                            source_name,
                            self.owner_uid,
                        )?;
                        let identity = source_directory.metadata()?;
                        if identity.dev() != candidate.device
                            || identity.ino() != candidate.inode
                            || identity.nlink() != candidate.links
                        {
                            return Err(ObjectPathError::UnsafeEntry);
                        }
                    }
                    CandidateKind::OrphanObject => {
                        let identity = super::dirfd::regular_entry_identity_at(
                            &source_parent,
                            source_name,
                            self.owner_uid,
                        )?;
                        if identity.device != candidate.device
                            || identity.inode != candidate.inode
                            || identity.links != candidate.links
                        {
                            return Err(ObjectPathError::UnsafeEntry);
                        }
                    }
                }
                sync_hook(RecoverySyncCheckpoint::AfterSourceIdentityBeforeMove(
                    moved + 1,
                ))?;
                super::dirfd::rename_at(
                    &source_parent,
                    source_name,
                    &batch_dir,
                    &candidate.destination_name,
                )?;
                let moved_matches = match candidate.kind {
                    CandidateKind::StagingDirectory | CandidateKind::ViewerDirectory => {
                        super::dirfd::open_private_directory_at(
                            &batch_dir,
                            &candidate.destination_name,
                            self.owner_uid,
                        )
                        .is_ok_and(|directory| {
                            directory.metadata().is_ok_and(|identity| {
                                identity.dev() == candidate.device
                                    && identity.ino() == candidate.inode
                                    && identity.nlink() == candidate.links
                            })
                        })
                    }
                    CandidateKind::OrphanObject => super::dirfd::regular_entry_identity_at(
                        &batch_dir,
                        &candidate.destination_name,
                        self.owner_uid,
                    )
                    .is_ok_and(|identity| {
                        identity.device == candidate.device
                            && identity.inode == candidate.inode
                            && identity.links == candidate.links
                    }),
                };
                sync_hook(RecoverySyncCheckpoint::BeforeBatchSync(moved + 1))?;
                batch_dir.sync_all()?;
                sync_hook(RecoverySyncCheckpoint::BeforeSourceSync(moved + 1))?;
                source_parent.sync_all()?;
                if !moved_matches {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                source_parent
            };
            #[cfg(not(unix))]
            let source_parent = {
                let destination = quarantine
                    .join(&batch_name)
                    .join(candidate.destination_name);
                fs::rename(&candidate.source, &destination)?;
                let source_parent = candidate
                    .source
                    .parent()
                    .ok_or(ObjectPathError::UnsafeEntry)?;
                private_directory(source_parent, Some(self.owner_uid))?
            };
            #[cfg(not(unix))]
            {
                sync_hook(RecoverySyncCheckpoint::BeforeBatchSync(moved + 1))?;
                batch_dir.sync_all()?;
                sync_hook(RecoverySyncCheckpoint::BeforeSourceSync(moved + 1))?;
                source_parent.sync_all()?;
            }
            moved += 1;
            hook(moved)?;
        }
        sync_hook(RecoverySyncCheckpoint::BeforeFinalQuarantineSync)?;
        quarantine_dir.sync_all()?;
        Ok(summary)
    }
}

fn validate_restart_candidate(
    candidate: &Candidate,
    profile_root: &Path,
    owner_uid: u32,
) -> Result<(), ObjectPathError> {
    if matches!(
        candidate.kind,
        CandidateKind::StagingDirectory | CandidateKind::ViewerDirectory
    ) {
        let parent = match candidate.kind {
            CandidateKind::StagingDirectory => "staging",
            CandidateKind::ViewerDirectory => "views",
            CandidateKind::OrphanObject => return Err(ObjectPathError::UnsafeEntry),
        };
        validate_recovery_ancestors(profile_root, &profile_root.join(parent), owner_uid)?;
        private_directory(&candidate.source, Some(owner_uid))?;
        #[cfg(unix)]
        if scan_stage_entries(&candidate.source, owner_uid)? != candidate.stage_entries {
            return Err(ObjectPathError::UnsafeEntry);
        }
    } else {
        validate_object_source_ancestors(profile_root, &candidate.source, owner_uid)?;
    }
    let metadata = fs::symlink_metadata(&candidate.source)?;
    #[cfg(unix)]
    if metadata.dev() != candidate.device
        || metadata.ino() != candidate.inode
        || metadata.nlink() != candidate.links
        || metadata.uid() != owner_uid
        || metadata.mode() & 0o077 != 0
        || match candidate.kind {
            CandidateKind::StagingDirectory | CandidateKind::ViewerDirectory => {
                !metadata.file_type().is_dir()
            }
            CandidateKind::OrphanObject => !metadata.file_type().is_file(),
        }
    {
        return Err(ObjectPathError::UnsafeEntry);
    }
    #[cfg(not(unix))]
    if match candidate.kind {
        CandidateKind::StagingDirectory | CandidateKind::ViewerDirectory => {
            !metadata.file_type().is_dir()
        }
        CandidateKind::OrphanObject => !metadata.file_type().is_file(),
    } {
        return Err(ObjectPathError::UnsafeEntry);
    }
    Ok(())
}

#[cfg(unix)]
fn scan_stage_entries(
    path: &std::path::Path,
    owner_uid: u32,
) -> Result<Vec<StageEntry>, ObjectPathError> {
    scan_stage_entries_with_limit(path, owner_uid, MAX_PAGES_PER_WINDOW_CAPTURE + 3)
}

#[cfg(unix)]
fn scan_stage_entries_with_limit(
    path: &std::path::Path,
    owner_uid: u32,
    maximum_entries: usize,
) -> Result<Vec<StageEntry>, ObjectPathError> {
    // Page zero uses capture.bin; subsequent window pages have bounded,
    // canonical ordinal leaves. Include the optional archive view and the two
    // install temporaries in the maximum so recovery remains bounded without
    // rejecting files created by the multi-page capture path. Retain identities
    // so a later rename cannot adopt a substituted link or newly added child.
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entries.len() >= maximum_entries {
            return Err(ObjectPathError::CaptureTooLarge);
        }
        let name = entry.file_name();
        let name = name.to_str().ok_or(ObjectPathError::UnsafeEntry)?;
        if !is_capture_stage_leaf(name) {
            return Err(ObjectPathError::UnsafeEntry);
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.file_type().is_file()
            || metadata.file_type().is_symlink()
            || metadata.uid() != owner_uid
            || metadata.mode() & 0o077 != 0
            || (metadata.nlink() != 1
                && (metadata.nlink() != 2 || !matches!(name, "object.tmp" | "sidecar.tmp")))
        {
            return Err(ObjectPathError::UnsafeEntry);
        }
        entries.push(StageEntry {
            name: name.to_owned(),
            bytes: metadata.len(),
            device: metadata.dev(),
            inode: metadata.ino(),
            links: metadata.nlink(),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

#[cfg(unix)]
fn is_capture_stage_leaf(name: &str) -> bool {
    matches!(
        name,
        "capture.bin" | "capture.mhtml" | "object.tmp" | "sidecar.tmp"
    ) || is_capture_page_stage_leaf(name)
}

#[cfg(unix)]
fn is_capture_page_stage_leaf(name: &str) -> bool {
    let Some(ordinal) = name
        .strip_prefix("capture-page-")
        .and_then(|name| name.strip_suffix(".bin"))
    else {
        return false;
    };
    let Ok(page_index) = ordinal.parse::<usize>() else {
        return false;
    };
    page_index > 0
        && page_index < MAX_PAGES_PER_WINDOW_CAPTURE
        && format!("capture-page-{page_index:03}.bin") == name
}

fn validate_corrupt_candidate(
    candidate: &CorruptCandidate,
    profile_root: &Path,
    owner_uid: u32,
) -> Result<(), ObjectPathError> {
    validate_object_source_ancestors(profile_root, &candidate.source, owner_uid)?;
    let metadata = fs::symlink_metadata(&candidate.source)?;
    #[cfg(unix)]
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner_uid
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
        || metadata.dev() != candidate.device
        || metadata.ino() != candidate.inode
    {
        return Err(ObjectPathError::UnsafeEntry);
    }
    #[cfg(not(unix))]
    {
        let _ = owner_uid;
        if !metadata.file_type().is_file() {
            return Err(ObjectPathError::UnsafeEntry);
        }
    }
    Ok(())
}

fn validate_recovery_ancestors(
    profile_root: &Path,
    child: &Path,
    owner_uid: u32,
) -> Result<(), ObjectPathError> {
    // Recheck each fixed profile-local ancestor during candidate preflight.
    // The later mutation is relative to held no-follow directory descriptors,
    // so replacing a checked parent pathname cannot redirect that syscall.
    // This still cannot exclude a same-UID leaf replacement or directory move.
    let versioning = profile_root.parent().ok_or(ObjectPathError::UnsafeEntry)?;
    let state_root = versioning.parent().ok_or(ObjectPathError::UnsafeEntry)?;
    for directory in [state_root, versioning, profile_root, child] {
        private_directory(directory, Some(owner_uid))?;
    }
    Ok(())
}

fn validate_object_source_ancestors(
    profile_root: &Path,
    source: &Path,
    owner_uid: u32,
) -> Result<(), ObjectPathError> {
    let objects = profile_root.join("objects");
    let sha256 = objects.join("sha256");
    let shard = source.parent().ok_or(ObjectPathError::UnsafeEntry)?;
    if shard.parent() != Some(sha256.as_path()) {
        return Err(ObjectPathError::UnsafeEntry);
    }
    validate_recovery_ancestors(profile_root, &objects, owner_uid)?;
    private_directory(&sha256, Some(owner_uid))?;
    private_directory(shard, Some(owner_uid))?;
    Ok(())
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(all(test, unix, not(any(target_os = "redox", target_os = "espidf"))))]
mod tests {
    use super::*;
    use crate::{RootSpec, StorageRoots};
    use ferric_browser_core::OperationId;
    use std::{
        fs::DirBuilder,
        os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink},
    };

    #[test]
    fn nested_stage_files_consume_the_global_recovery_scan_limit() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let first = roots.state.join("first-stage");
        let second = roots.state.join("second-stage");
        fs::create_dir(&first).expect("first stage");
        fs::create_dir(&second).expect("second stage");
        let first_capture = first.join("capture.bin");
        let second_capture = second.join("capture.bin");
        fs::write(&first_capture, b"first").expect("first capture");
        fs::write(&second_capture, b"second").expect("second capture");
        fs::set_permissions(&first_capture, fs::Permissions::from_mode(0o600))
            .expect("first capture mode");
        fs::set_permissions(&second_capture, fs::Permissions::from_mode(0o600))
            .expect("second capture mode");
        let owner_uid = fs::metadata(&roots.state).expect("state metadata").uid();

        // One slot is consumed by the containing stage directory itself.
        let mut examined = MAX_RECOVERY_ENTRIES - 2;
        account_scanned_entries(&mut examined, 1, MAX_RECOVERY_ENTRIES)
            .expect("containing stage directory");
        let entries = scan_stage_entries_accounted(
            &first,
            owner_uid,
            &mut examined,
            MAX_RECOVERY_ENTRIES,
            MAX_PAGES_PER_WINDOW_CAPTURE + 3,
        )
        .expect("nested file at global boundary");
        assert_eq!(entries.len(), 1);
        assert_eq!(examined, MAX_RECOVERY_ENTRIES);
        assert!(matches!(
            scan_stage_entries_accounted(
                &second,
                owner_uid,
                &mut examined,
                MAX_RECOVERY_ENTRIES,
                MAX_PAGES_PER_WINDOW_CAPTURE + 3,
            ),
            Err(ObjectPathError::CaptureTooLarge)
        ));

        let mut examined = usize::MAX;
        assert!(matches!(
            account_scanned_entries(&mut examined, 1, MAX_RECOVERY_ENTRIES),
            Err(ObjectPathError::CaptureTooLarge)
        ));
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn restart_refuses_invalid_reference_before_moving_orphan() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let stage = store.begin(operation).expect("stage");
        fs::write(stage.capture_path(), b"unreferenced bytes").expect("capture");
        let orphan = stage
            .install_object(64, Some(18), None)
            .expect("orphan object");
        let references = BTreeSet::from(["z".repeat(64)]);

        assert!(matches!(
            store.quarantine_after_restart(&references),
            Err(ObjectPathError::InvalidObjectDigest)
        ));
        assert_eq!(
            fs::read(orphan.path()).expect("orphan retained"),
            b"unreferenced bytes"
        );
        assert!(stage.directory().exists());
        assert_eq!(
            fs::read_dir(store.profile_root.join("quarantine"))
                .expect("quarantine")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn restart_refuses_oversized_reference_set_before_moving_orphan() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"unreferenced bytes").expect("capture");
        let orphan = stage
            .install_object(64, Some(18), None)
            .expect("orphan object");
        let references = (0..=MAX_RECOVERY_ENTRIES)
            .map(|index| format!("{index:064x}"))
            .collect::<BTreeSet<_>>();

        assert!(matches!(
            store.quarantine_after_restart(&references),
            Err(ObjectPathError::CaptureTooLarge)
        ));
        assert_eq!(
            fs::read(orphan.path()).expect("orphan retained"),
            b"unreferenced bytes"
        );
        assert!(stage.directory().exists());
        assert_eq!(
            fs::read_dir(store.profile_root.join("quarantine"))
                .expect("quarantine")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn restart_quarantines_only_unpublished_files_and_is_repeatable() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let published_stage = store.begin(OperationId::fresh()).expect("published stage");
        fs::write(published_stage.capture_path(), b"abc").expect("capture");
        let published = published_stage
            .install_object(64, Some(3), None)
            .expect("published candidate");
        let orphan_stage = store.begin(OperationId::fresh()).expect("orphan stage");
        fs::write(orphan_stage.capture_path(), b"xyz").expect("orphan capture");
        let orphan = orphan_stage
            .install_object(64, Some(3), None)
            .expect("orphan candidate");
        let referenced = BTreeSet::from([published.digest_hex()]);
        let summary = store
            .quarantine_after_restart(&referenced)
            .expect("recover after quiescence");
        assert_eq!(
            summary,
            RecoverySummary {
                staging_operations: 2,
                orphan_objects: 1,
            }
        );
        assert_eq!(
            fs::read(published.path()).expect("referenced bytes"),
            b"abc"
        );
        assert!(!orphan.path().exists());
        assert!(!published_stage.directory().exists());
        assert!(!orphan_stage.directory().exists());
        let quarantine = store.profile_root.join("quarantine");
        let batch = fs::read_dir(&quarantine)
            .expect("quarantine")
            .next()
            .expect("one batch")
            .expect("batch entry")
            .path();
        assert_eq!(fs::read_dir(&batch).expect("batch contents").count(), 3);
        assert_eq!(
            store
                .quarantine_after_restart(&referenced)
                .expect("idempotent recovery"),
            RecoverySummary::default()
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn quarantine_inventory_counts_retained_stage_and_orphan_bytes() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"retained stage").expect("capture");
        let orphan = stage
            .install_object(64, Some(14), None)
            .expect("orphan object");

        assert_eq!(
            store
                .quarantine_after_restart(&BTreeSet::new())
                .expect("quarantine abandoned capture"),
            RecoverySummary {
                staging_operations: 1,
                orphan_objects: 1,
            }
        );
        assert!(!orphan.path().exists());
        assert_eq!(
            store
                .unpublished_quarantine_bytes()
                .expect("retained quarantine bytes"),
            28
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn quarantine_inventory_rejects_symlink_without_following_it() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"retained bytes").expect("capture");
        store
            .quarantine_after_restart(&BTreeSet::new())
            .expect("quarantine abandoned capture");
        let batch = fs::read_dir(store.profile_root.join("quarantine"))
            .expect("quarantine")
            .next()
            .expect("batch")
            .expect("batch entry")
            .path();
        let sentinel = roots.state.join("sentinel");
        fs::write(&sentinel, b"outside quarantine").expect("sentinel");
        symlink(&sentinel, batch.join(format!("object-{}", "a".repeat(64))))
            .expect("unsafe quarantine entry");

        assert!(matches!(
            store.unpublished_quarantine_bytes(),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(sentinel).expect("sentinel retained"),
            b"outside quarantine"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_object_inventory_counts_only_objects_without_ready_index_rows() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"abc").expect("capture");
        let object = stage.install_object(64, Some(3), None).expect("object");

        assert_eq!(
            store
                .unindexed_live_object_bytes(&BTreeMap::new())
                .expect("unindexed object bytes"),
            3
        );
        assert_eq!(
            store
                .unindexed_live_object_bytes(&BTreeMap::from([(object.digest_hex(), 3)]))
                .expect("ready object excluded from extra bytes"),
            0
        );
        assert!(matches!(
            store.unindexed_live_object_bytes(&BTreeMap::from([(object.digest_hex(), 2)])),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(fs::read(object.path()).expect("object retained"), b"abc");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_object_inventory_rejects_symlink_without_following_it() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let shard = store.objects_root().join("aa");
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        builder.create(&shard).expect("private shard");
        let sentinel = roots.state.join("sentinel");
        fs::write(&sentinel, b"outside object root").expect("sentinel");
        symlink(&sentinel, shard.join("b".repeat(62))).expect("unsafe object entry");

        assert!(matches!(
            store.unindexed_live_object_bytes(&BTreeMap::new()),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(sentinel).expect("sentinel retained"),
            b"outside object root"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_viewer_inventory_counts_private_archive_copy() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("source stage");
        fs::write(stage.capture_path(), b"archive").expect("source bytes");
        let object = stage.install_object(64, Some(7), None).expect("object");
        let view = store
            .materialize_archive_view_cancellable(
                OperationId::fresh(),
                &object.digest_hex(),
                7,
                || false,
            )
            .expect("private viewer copy");

        assert_eq!(store.live_viewer_bytes().expect("viewer inventory"), 7);
        assert_eq!(fs::read(view.path()).expect("viewer retained"), b"archive");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_viewer_inventory_rejects_symlink_without_following_it() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("source stage");
        fs::write(stage.capture_path(), b"archive").expect("source bytes");
        let object = stage.install_object(64, Some(7), None).expect("object");
        let view = store
            .materialize_archive_view_cancellable(
                OperationId::fresh(),
                &object.digest_hex(),
                7,
                || false,
            )
            .expect("private viewer copy");
        let sentinel = roots.state.join("sentinel");
        fs::write(&sentinel, b"outside viewer root").expect("sentinel");
        fs::remove_file(view.path()).expect("remove disposable viewer leaf");
        symlink(&sentinel, view.path()).expect("unsafe viewer entry");

        assert!(matches!(
            store.live_viewer_bytes(),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(sentinel).expect("sentinel retained"),
            b"outside viewer root"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_staging_inventory_counts_only_bytes_above_active_reservation() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let stage = store.begin(operation).expect("stage");
        fs::write(stage.capture_path(), b"abc").expect("capture");
        let name = operation.as_uuid().hyphenated().to_string();

        assert_eq!(
            store
                .unreserved_live_staging_bytes(&BTreeMap::new())
                .expect("abandoned stage bytes"),
            3
        );
        assert_eq!(
            store
                .unreserved_live_staging_bytes(&BTreeMap::from([(name.clone(), 3)]))
                .expect("fully reserved stage"),
            0
        );
        assert_eq!(
            store
                .unreserved_live_staging_bytes(&BTreeMap::from([(name, 2)]))
                .expect("excess stage bytes"),
            1
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_staging_inventory_accepts_and_recovers_bounded_window_page_leaves() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let first = store.begin(operation).expect("first page");
        let page = store
            .prepare_window_page(operation, 1)
            .expect("additional page");
        let last_page = store
            .prepare_window_page(operation, MAX_PAGES_PER_WINDOW_CAPTURE - 1)
            .expect("last bounded page");
        fs::write(first.capture_path(), b"first").expect("first bytes");
        fs::write(page.capture_path(), b"second-page").expect("second-page bytes");
        fs::write(last_page.capture_path(), b"last").expect("last-page bytes");

        assert_eq!(
            store
                .unreserved_live_staging_bytes(&BTreeMap::new())
                .expect("page leaves are included in the bounded inventory"),
            20
        );
        assert_eq!(
            store
                .unreserved_live_staging_bytes(&BTreeMap::from([(
                    operation.as_uuid().hyphenated().to_string(),
                    15,
                )]))
                .expect("page leaves contribute to active-operation excess"),
            5
        );
        assert_eq!(
            store
                .quarantine_after_restart(&BTreeSet::new())
                .expect("multi-page staging can be recovered"),
            RecoverySummary {
                staging_operations: 1,
                orphan_objects: 0,
            }
        );
        assert_eq!(
            store
                .unpublished_quarantine_bytes()
                .expect("quarantine retains all capture leaves"),
            20
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_staging_inventory_rejects_noncanonical_or_out_of_range_page_leaves() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        let directory = stage.directory();

        for name in [
            "capture-page-000.bin",
            "capture-page-01.bin",
            "capture-page-0010.bin",
            "capture-page-128.bin",
            "capture-page-001.bin.bak",
        ] {
            let path = directory.join(name);
            fs::write(&path, b"not an owned capture leaf").expect("malformed leaf");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .expect("private malformed leaf");

            assert!(
                matches!(
                    store.unreserved_live_staging_bytes(&BTreeMap::new()),
                    Err(ObjectPathError::UnsafeEntry)
                ),
                "unexpectedly accepted {name}"
            );
            fs::remove_file(path).expect("remove malformed leaf");
        }
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn published_capture_cleanup_removes_exact_sources_and_preserves_objects() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let first = store.begin(operation).expect("first page");
        let second = store
            .prepare_window_page(operation, 1)
            .expect("second page");
        let first_bytes = b"verified archive zero";
        let second_bytes = b"verified archive one";
        fs::write(first.capture_path(), first_bytes).expect("first archive");
        fs::write(second.capture_path(), second_bytes).expect("second archive");
        let first_object = first
            .install_object(1024, Some(first_bytes.len() as u64), None)
            .expect("first immutable copy");
        let second_object = second
            .install_object(1024, Some(second_bytes.len() as u64), None)
            .expect("second immutable copy");
        let stage_directory = first.directory().to_owned();
        assert!(stage_directory.exists());

        store
            .discard_published_capture_staging(operation)
            .expect("retire exact operation staging");

        assert!(!stage_directory.exists());
        assert_eq!(
            fs::read(first_object.path()).expect("first object"),
            first_bytes.to_vec()
        );
        assert_eq!(
            fs::read(second_object.path()).expect("second object"),
            second_bytes.to_vec()
        );
        assert_eq!(
            store
                .unreserved_live_staging_bytes(&BTreeMap::new())
                .expect("retired source bytes no longer occupy live staging"),
            0
        );
        store
            .discard_published_capture_staging(operation)
            .expect("cleanup is idempotent after the exact directory is gone");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn published_capture_cleanup_retries_after_partial_leaf_retirement() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let first = store.begin(operation).expect("first page");
        let second = store
            .prepare_window_page(operation, 1)
            .expect("second page");
        let first_bytes = b"already retired archive";
        let second_bytes = b"remaining archive source";
        fs::write(first.capture_path(), first_bytes).expect("first archive");
        fs::write(second.capture_path(), second_bytes).expect("second archive");
        let first_object = first
            .install_object(1024, Some(first_bytes.len() as u64), None)
            .expect("first immutable copy");
        let second_object = second
            .install_object(1024, Some(second_bytes.len() as u64), None)
            .expect("second immutable copy");
        let stage_directory = first.directory().to_owned();

        // Model interruption after one of the exact leaves was removed.
        fs::remove_file(first.capture_path()).expect("retire first source leaf");
        store
            .discard_published_capture_staging(operation)
            .expect("retry removes only the remaining known leaf");

        assert!(!stage_directory.exists());
        assert_eq!(
            fs::read(first_object.path()).expect("first installed object"),
            first_bytes.to_vec()
        );
        assert_eq!(
            fs::read(second_object.path()).expect("second installed object"),
            second_bytes.to_vec()
        );
        assert_eq!(
            store
                .unreserved_live_staging_bytes(&BTreeMap::new())
                .expect("partial-retirement retry clears live stage bytes"),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn published_capture_cleanup_refuses_unknown_leaf_before_unlinking() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let stage = store.begin(operation).expect("stage");
        fs::write(stage.capture_path(), b"known source").expect("known source");
        let unknown = stage.directory().join("unexpected.tmp");
        fs::write(&unknown, b"unknown data").expect("unknown file");
        fs::set_permissions(&unknown, fs::Permissions::from_mode(0o600))
            .expect("private unknown file");

        assert!(matches!(
            store.discard_published_capture_staging(operation),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(stage.capture_path()).expect("known source retained"),
            b"known source".to_vec()
        );
        assert_eq!(
            fs::read(unknown).expect("unknown data retained"),
            b"unknown data".to_vec()
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_staging_inventory_rejects_symlink_without_following_it() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        let sentinel = roots.state.join("sentinel");
        fs::write(&sentinel, b"outside staging root").expect("sentinel");
        fs::remove_file(stage.capture_path()).expect("remove disposable stage leaf");
        symlink(&sentinel, stage.capture_path()).expect("unsafe stage entry");

        assert!(matches!(
            store.unreserved_live_staging_bytes(&BTreeMap::new()),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(sentinel).expect("sentinel retained"),
            b"outside staging root"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn restart_retries_after_a_durable_partial_quarantine_batch() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let first_stage = store.begin(OperationId::fresh()).expect("first stage");
        fs::write(first_stage.capture_path(), b"first").expect("first capture");
        let first = first_stage
            .install_object(64, Some(5), None)
            .expect("first object");
        let second_stage = store.begin(OperationId::fresh()).expect("second stage");
        fs::write(second_stage.capture_path(), b"second").expect("second capture");
        let second = second_stage
            .install_object(64, Some(6), None)
            .expect("second object");

        assert!(matches!(
            store.quarantine_after_restart_with_hook(&BTreeSet::new(), |moved| {
                if moved == 1 {
                    Err(ObjectPathError::Io(std::io::Error::other(
                        "injected post-move interruption",
                    )))
                } else {
                    Ok(())
                }
            }),
            Err(ObjectPathError::Io(_))
        ));
        let quarantine = store.profile_root.join("quarantine");
        let first_batch = fs::read_dir(&quarantine)
            .expect("quarantine")
            .next()
            .expect("retained partial batch")
            .expect("batch")
            .path();
        assert_eq!(
            fs::read_dir(first_batch).expect("batch contents").count(),
            1
        );

        assert_eq!(
            store
                .quarantine_after_restart(&BTreeSet::new())
                .expect("retry after interruption"),
            RecoverySummary {
                staging_operations: 2,
                orphan_objects: 1,
            }
        );
        assert!(!first.path().exists());
        assert!(!second.path().exists());
        assert!(!first_stage.directory().exists());
        assert!(!second_stage.directory().exists());
        let batches = fs::read_dir(&quarantine)
            .expect("quarantine batches")
            .map(|batch| {
                fs::read_dir(batch.expect("batch").path())
                    .expect("batch contents")
                    .count()
            })
            .collect::<Vec<_>>();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches.iter().sum::<usize>(), 4);
        assert_eq!(
            store
                .quarantine_after_restart(&BTreeSet::new())
                .expect("idempotent retry"),
            RecoverySummary::default()
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn restart_retries_after_injected_quarantine_directory_sync_errors() {
        use std::io;

        for fault in [
            RecoverySyncCheckpoint::BatchCreated,
            RecoverySyncCheckpoint::BeforeBatchSync(1),
            RecoverySyncCheckpoint::BeforeSourceSync(1),
            RecoverySyncCheckpoint::BeforeFinalQuarantineSync,
        ] {
            let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
            roots.ensure().expect("private roots");
            let profile = Uuid::new_v4();
            let store = VersioningObjectStore::open(&roots.state, profile).expect("store");
            let published_stage = store.begin(OperationId::fresh()).expect("published stage");
            fs::write(published_stage.capture_path(), b"published").expect("capture");
            let published = published_stage
                .install_object(64, Some(9), None)
                .expect("published object");
            let orphan_stage = store.begin(OperationId::fresh()).expect("orphan stage");
            fs::write(orphan_stage.capture_path(), b"orphan").expect("capture");
            let orphan = orphan_stage
                .install_object(64, Some(6), None)
                .expect("orphan object");
            let references = BTreeSet::from([published.digest_hex()]);

            assert!(matches!(
                store.quarantine_after_restart_with_hooks(
                    &references,
                    |_| Ok(()),
                    |point| {
                        if point == fault {
                            Err(ObjectPathError::Io(io::Error::from_raw_os_error(libc::EIO)))
                        } else {
                            Ok(())
                        }
                    },
                ),
                Err(ObjectPathError::Io(error)) if error.raw_os_error() == Some(libc::EIO)
            ));
            assert_eq!(
                fs::read(published.path()).expect("referenced bytes"),
                b"published"
            );

            let reopened = VersioningObjectStore::open(&roots.state, profile).expect("reopen");
            reopened
                .quarantine_after_restart(&references)
                .expect("retry after injected sync error");
            assert!(!orphan.path().exists());
            assert!(!orphan_stage.directory().exists());
            assert!(!published_stage.directory().exists());
            let quarantine = store.profile_root.join("quarantine");
            let entries = fs::read_dir(quarantine)
                .expect("batches")
                .map(|batch| {
                    fs::read_dir(batch.expect("batch").path())
                        .expect("batch entries")
                        .count()
                })
                .sum::<usize>();
            assert_eq!(entries, 3, "all unpublished entries retained at {fault:?}");
            assert_eq!(
                reopened
                    .quarantine_after_restart(&references)
                    .expect("idempotent retry"),
                RecoverySummary::default()
            );
            roots.cleanup().expect("cleanup");
        }
    }

    #[test]
    fn restart_leaf_swap_after_identity_check_is_quarantined_and_refused() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let store = VersioningObjectStore::open(&roots.state, profile).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"original").expect("capture");
        let object = stage
            .install_object(64, Some(8), None)
            .expect("installed object");
        let retained = roots.state.join("original-before-restart-leaf-swap");

        let result = store.quarantine_after_restart_with_hooks(
            &BTreeSet::new(),
            |_| Ok(()),
            |checkpoint| {
                if checkpoint == RecoverySyncCheckpoint::AfterSourceIdentityBeforeMove(1) {
                    fs::rename(object.path(), &retained)?;
                    fs::write(object.path(), b"replacement")?;
                    fs::set_permissions(object.path(), fs::Permissions::from_mode(0o600))?;
                }
                Ok(())
            },
        );
        assert!(matches!(result, Err(ObjectPathError::UnsafeEntry)));
        assert_eq!(fs::read(&retained).expect("original retained"), b"original");
        assert!(!object.path().exists());
        assert!(stage.directory().exists());

        let quarantine = store.profile_root.join("quarantine");
        let batch = fs::read_dir(quarantine)
            .expect("quarantine")
            .next()
            .expect("quarantine batch")
            .expect("batch entry")
            .path();
        assert_eq!(
            fs::read(batch.join(format!("object-{}", object.digest_hex())))
                .expect("replacement retained in quarantine"),
            b"replacement"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn killed_recovery_retries_after_first_durable_quarantine_move() {
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
            sync::mpsc,
            thread,
            time::Duration,
        };

        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let store = VersioningObjectStore::open(&roots.state, profile).expect("store");
        let first_stage = store.begin(OperationId::fresh()).expect("first stage");
        fs::write(first_stage.capture_path(), b"first").expect("first capture");
        let first = first_stage
            .install_object(64, Some(5), None)
            .expect("first object");
        let second_stage = store.begin(OperationId::fresh()).expect("second stage");
        fs::write(second_stage.capture_path(), b"second").expect("second capture");
        let second = second_stage
            .install_object(64, Some(6), None)
            .expect("second object");
        let root = roots.temporary_root.as_ref().expect("temporary root");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "versioning::objects::recovery::tests::crash_child_after_first_quarantine_move",
                "--nocapture",
            ])
            .env("FERRIC_RECOVERY_CRASH_ROOT", root)
            .env("FERRIC_RECOVERY_CRASH_PROFILE", profile.to_string())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn disposable recovery child");
        let stdout = child.stdout.take().expect("child stdout");
        let (sender, receiver) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if line.is_ok_and(|line| line.contains("FERRIC_RECOVERY_CRASH_BOUNDARY")) {
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
        assert!(reached, "child must reach first durable move");
        assert!(!status.success(), "disposable child was killed");

        let quarantine = store.profile_root.join("quarantine");
        let first_batch = fs::read_dir(&quarantine)
            .expect("quarantine")
            .next()
            .expect("partial batch")
            .expect("batch")
            .path();
        assert_eq!(fs::read_dir(first_batch).expect("first batch").count(), 1);

        let reopened = VersioningObjectStore::open(&roots.state, profile).expect("reopen store");
        assert_eq!(
            reopened
                .quarantine_after_restart(&BTreeSet::new())
                .expect("retry after killed child"),
            RecoverySummary {
                staging_operations: 2,
                orphan_objects: 1,
            }
        );
        assert!(!first.path().exists());
        assert!(!second.path().exists());
        assert!(!first_stage.directory().exists());
        assert!(!second_stage.directory().exists());
        let batches = fs::read_dir(&quarantine)
            .expect("quarantine batches")
            .map(|batch| {
                fs::read_dir(batch.expect("batch").path())
                    .expect("batch contents")
                    .count()
            })
            .collect::<Vec<_>>();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches.iter().sum::<usize>(), 4);
        assert_eq!(
            reopened
                .quarantine_after_restart(&BTreeSet::new())
                .expect("idempotent retry"),
            RecoverySummary::default()
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn killed_recovery_preserves_a_moved_viewer_file_for_restart() {
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
            sync::mpsc,
            thread,
            time::Duration,
        };

        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let store = VersioningObjectStore::open(&roots.state, profile).expect("store");
        let viewer = store
            .begin_in(&store.profile_root.join("views"), OperationId::fresh())
            .expect("viewer operation");
        let archived = viewer.directory().join("capture.mhtml");
        fs::write(viewer.capture_path(), b"retained archive bytes").expect("viewer bytes");
        fs::rename(viewer.capture_path(), &archived).expect("viewer MHTML leaf");
        let operation_name = viewer
            .directory()
            .file_name()
            .expect("operation name")
            .to_string_lossy()
            .into_owned();
        let viewer_directory = viewer.directory().to_path_buf();
        drop(viewer);

        let root = roots.temporary_root.as_ref().expect("temporary root");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "versioning::objects::recovery::tests::crash_child_after_first_quarantine_move",
                "--nocapture",
            ])
            .env("FERRIC_RECOVERY_CRASH_ROOT", root)
            .env("FERRIC_RECOVERY_CRASH_PROFILE", profile.to_string())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn disposable recovery child");
        let stdout = child.stdout.take().expect("child stdout");
        let (sender, receiver) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if line.is_ok_and(|line| line.contains("FERRIC_RECOVERY_CRASH_BOUNDARY")) {
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
        assert!(reached, "child must move the viewer before the kill");
        assert!(!status.success(), "disposable child was killed");

        assert!(!viewer_directory.exists());
        let quarantine = store.profile_root.join("quarantine");
        let batch = fs::read_dir(&quarantine)
            .expect("quarantine")
            .next()
            .expect("partial batch")
            .expect("batch")
            .path();
        let moved = batch
            .join(format!("view-{operation_name}"))
            .join("capture.mhtml");
        assert_eq!(
            fs::read(&moved).expect("retained quarantined bytes"),
            b"retained archive bytes"
        );

        let reopened = VersioningObjectStore::open(&roots.state, profile).expect("reopen store");
        assert_eq!(
            reopened
                .quarantine_after_restart(&BTreeSet::new())
                .expect("restart after killed viewer recovery"),
            RecoverySummary::default()
        );
        assert_eq!(
            fs::read(&moved).expect("viewer remains recoverable"),
            b"retained archive bytes"
        );
        assert_eq!(
            fs::read_dir(&quarantine)
                .expect("quarantine batches")
                .count(),
            1
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn crash_child_after_first_quarantine_move() {
        use std::{io::Write, thread, time::Duration};

        let Ok(root) = std::env::var("FERRIC_RECOVERY_CRASH_ROOT") else {
            return;
        };
        let profile =
            Uuid::parse_str(&std::env::var("FERRIC_RECOVERY_CRASH_PROFILE").expect("profile UUID"))
                .expect("typed profile");
        let roots = StorageRoots::resolve(RootSpec::Base(root.into())).expect("child roots");
        roots.ensure().expect("private child roots");
        let store = VersioningObjectStore::open(&roots.state, profile).expect("child store");
        store
            .quarantine_after_restart_with_hook(&BTreeSet::new(), |moved| {
                if moved == 1 {
                    println!("FERRIC_RECOVERY_CRASH_BOUNDARY");
                    std::io::stdout().flush().expect("flush boundary marker");
                    loop {
                        thread::park_timeout(Duration::from_secs(60));
                    }
                }
                Ok(())
            })
            .expect("recovery should reach kill boundary");
        panic!("child must be killed at the requested boundary");
    }

    #[test]
    fn restart_refuses_replaced_orphan_after_first_move() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let first_stage = store.begin(OperationId::fresh()).expect("first stage");
        fs::write(first_stage.capture_path(), b"first").expect("first capture");
        let first = first_stage
            .install_object(64, Some(5), None)
            .expect("first object");
        let second_stage = store.begin(OperationId::fresh()).expect("second stage");
        fs::write(second_stage.capture_path(), b"second").expect("second capture");
        let second = second_stage
            .install_object(64, Some(6), None)
            .expect("second object");
        let remaining = if first.digest_hex() < second.digest_hex() {
            second.path()
        } else {
            first.path()
        };
        let outside = roots.state.join("outside-orphan-sentinel");
        fs::write(&outside, b"outside stays untouched").expect("outside sentinel");
        assert!(matches!(
            store.quarantine_after_restart_with_hook(&BTreeSet::new(), |moved| {
                if moved == 1 {
                    fs::remove_file(remaining).expect("replace remaining orphan");
                    fs::hard_link(&outside, remaining).expect("outside hard link");
                }
                Ok(())
            }),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(&outside).expect("outside bytes"),
            b"outside stays untouched"
        );
        assert_eq!(
            fs::read(remaining).expect("remaining bytes"),
            b"outside stays untouched"
        );
        let quarantine = store.profile_root.join("quarantine");
        let batch = fs::read_dir(&quarantine)
            .expect("quarantine")
            .next()
            .expect("partial batch")
            .expect("batch")
            .path();
        assert_eq!(fs::read_dir(batch).expect("batch entries").count(), 1);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn restart_refuses_orphan_shard_symlink_swap_after_first_move() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let first_stage = store.begin(OperationId::fresh()).expect("first stage");
        fs::write(first_stage.capture_path(), b"first").expect("first capture");
        let first = first_stage
            .install_object(64, Some(5), None)
            .expect("first object");
        let second_stage = store.begin(OperationId::fresh()).expect("second stage");
        fs::write(second_stage.capture_path(), b"second").expect("second capture");
        let second = second_stage
            .install_object(64, Some(6), None)
            .expect("second object");
        let (remaining, expected) = if first.digest_hex() < second.digest_hex() {
            (second.path(), b"second".as_slice())
        } else {
            (first.path(), b"first".as_slice())
        };
        assert_ne!(first.path().parent(), second.path().parent());
        let shard = remaining.parent().expect("remaining shard");
        let outside_shard = roots.state.join("outside-restart-shard");
        let outside_object = outside_shard.join(remaining.file_name().expect("object name"));

        let result = store.quarantine_after_restart_with_hook(&BTreeSet::new(), |moved| {
            if moved == 1 {
                fs::rename(shard, &outside_shard)?;
                symlink(&outside_shard, shard)?;
            }
            Ok(())
        });
        assert!(matches!(result, Err(ObjectPathError::UnsafeEntry)));
        assert_eq!(
            fs::read(&outside_object).expect("outside object retained"),
            expected
        );
        let quarantine = store.profile_root.join("quarantine");
        let batch = fs::read_dir(&quarantine)
            .expect("quarantine")
            .next()
            .expect("partial batch")
            .expect("batch")
            .path();
        assert_eq!(fs::read_dir(batch).expect("batch entries").count(), 1);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn restart_refuses_replaced_staging_temporary_after_first_move() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let first_stage = store.begin(OperationId::fresh()).expect("first stage");
        fs::write(first_stage.capture_path(), b"first").expect("first capture");
        first_stage
            .install_object(64, Some(5), None)
            .expect("first object");
        let second_stage = store.begin(OperationId::fresh()).expect("second stage");
        fs::write(second_stage.capture_path(), b"second").expect("second capture");
        second_stage
            .install_object(64, Some(6), None)
            .expect("second object");
        let stage_to_replace = if first_stage.directory() < second_stage.directory() {
            first_stage.directory()
        } else {
            second_stage.directory()
        };
        let temporary = stage_to_replace.join("object.tmp");
        fs::write(&temporary, b"unpublished staged bytes").expect("staged temporary");
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
            .expect("private staged temporary");
        let outside = roots.state.join("outside-stage-sentinel");
        fs::write(&outside, b"outside stays untouched").expect("outside sentinel");
        assert!(matches!(
            store.quarantine_after_restart_with_hook(&BTreeSet::new(), |moved| {
                if moved == 1 {
                    fs::remove_file(&temporary).expect("replace staged temporary");
                    fs::hard_link(&outside, &temporary).expect("outside hard link");
                }
                Ok(())
            }),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(&outside).expect("outside bytes"),
            b"outside stays untouched"
        );
        assert_eq!(
            fs::read(&temporary).expect("temporary bytes"),
            b"outside stays untouched"
        );
        let quarantine = store.profile_root.join("quarantine");
        let batch = fs::read_dir(&quarantine)
            .expect("quarantine")
            .next()
            .expect("partial batch")
            .expect("batch")
            .path();
        assert_eq!(fs::read_dir(batch).expect("batch entries").count(), 2);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn orphan_with_external_hard_link_is_not_moved() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"synthetic orphan").expect("capture");
        let object = stage
            .install_object(64, Some(16), None)
            .expect("installed object");
        let outside = roots.state.join("outside-hard-link");
        fs::hard_link(object.path(), &outside).expect("outside hard link");

        assert!(matches!(
            store.quarantine_after_restart(&BTreeSet::new()),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(object.path()).expect("object retained"),
            b"synthetic orphan"
        );
        assert_eq!(
            fs::read(&outside).expect("outside retained"),
            b"synthetic orphan"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn staging_temporary_with_external_hard_link_is_not_a_crash_pair() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        let outside = roots.state.join("outside-staging-hard-link");
        fs::write(&outside, b"unrelated").expect("outside bytes");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600))
            .expect("private outside fixture");
        let temporary = stage.directory().join("object.tmp");
        fs::hard_link(&outside, &temporary).expect("outside hard link");

        assert!(matches!(
            store.quarantine_after_restart(&BTreeSet::new()),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(fs::read(&outside).expect("outside retained"), b"unrelated");
        assert_eq!(fs::read(&temporary).expect("stage retained"), b"unrelated");
        assert_eq!(
            fs::read_dir(store.profile_root.join("quarantine"))
                .expect("quarantine")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn invalid_corrupt_reference_digest_refuses_before_any_move() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"synthetic referenced object").expect("capture");
        let object = stage
            .install_object(64, Some(27), None)
            .expect("installed object");
        let references = BTreeSet::from([object.digest_hex(), "z".repeat(64)]);

        assert!(matches!(
            store.quarantine_corrupt_references(&references),
            Err(ObjectPathError::InvalidObjectDigest)
        ));
        assert_eq!(
            fs::read(object.path()).expect("object retained"),
            b"synthetic referenced object"
        );
        assert_eq!(
            fs::read_dir(store.profile_root.join("quarantine"))
                .expect("quarantine")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn quarantined_corrupt_index_accepts_exact_private_file_and_refuses_symlink() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"original").expect("capture");
        let object = stage
            .install_object(64, Some(8), None)
            .expect("installed object");
        let hash = object.digest_hex();
        fs::write(object.path(), b"corrupt!").expect("corrupt local object");
        assert_eq!(
            store
                .quarantine_corrupt_references(&BTreeSet::from([hash.clone()]))
                .expect("quarantine corrupt object"),
            1
        );
        assert_eq!(
            store
                .quarantined_corrupt_hashes()
                .expect("inspect private quarantine"),
            BTreeSet::from([hash])
        );

        let batch = fs::read_dir(store.profile_root.join("quarantine"))
            .expect("quarantine batches")
            .next()
            .expect("one batch")
            .expect("private batch")
            .path();
        let outside = roots.state.join("unrelated-object");
        fs::write(&outside, b"unrelated").expect("outside fixture");
        symlink(&outside, batch.join(format!("corrupt-{}", "a".repeat(64))))
            .expect("unsafe quarantine name");
        assert!(matches!(
            store.quarantined_corrupt_hashes(),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(fs::read(outside).expect("outside retained"), b"unrelated");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn late_unsafe_corrupt_reference_refuses_before_any_move() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let first_stage = store.begin(OperationId::fresh()).expect("first stage");
        fs::write(first_stage.capture_path(), b"first").expect("first capture");
        let first = first_stage
            .install_object(64, Some(5), None)
            .expect("first object");
        let second_stage = store.begin(OperationId::fresh()).expect("second stage");
        fs::write(second_stage.capture_path(), b"second").expect("second capture");
        let second = second_stage
            .install_object(64, Some(6), None)
            .expect("second object");
        let (earlier, later) = if first.digest_hex() < second.digest_hex() {
            (&first, &second)
        } else {
            (&second, &first)
        };
        let outside = roots.state.join("outside-late-hard-link");
        fs::hard_link(later.path(), &outside).expect("unsafe later reference");

        assert!(matches!(
            store.quarantine_corrupt_references(&BTreeSet::from([
                earlier.digest_hex(),
                later.digest_hex(),
            ])),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert!(earlier.path().exists(), "earlier source was not moved");
        assert!(later.path().exists(), "unsafe later source was not moved");
        assert!(outside.exists(), "outside hard link was not touched");
        assert_eq!(
            fs::read_dir(store.profile_root.join("quarantine"))
                .expect("quarantine")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn corrupt_reference_replacement_after_preflight_is_not_moved() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"original").expect("capture");
        let object = stage
            .install_object(64, Some(8), None)
            .expect("installed object");
        let retained = roots.state.join("original-after-preflight");

        assert!(matches!(
            store.quarantine_corrupt_references_with_hook(
                &BTreeSet::from([object.digest_hex()]),
                || {
                    fs::rename(object.path(), &retained)?;
                    fs::write(object.path(), b"replacement")?;
                    fs::set_permissions(object.path(), fs::Permissions::from_mode(0o600))?;
                    Ok(())
                },
            ),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(fs::read(&retained).expect("original retained"), b"original");
        assert_eq!(
            fs::read(object.path()).expect("replacement retained"),
            b"replacement"
        );
        assert_eq!(
            fs::read_dir(store.profile_root.join("quarantine"))
                .expect("quarantine")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn corrupt_reference_leaf_swap_after_identity_check_is_quarantined_and_refused() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"original").expect("capture");
        let object = stage
            .install_object(64, Some(8), None)
            .expect("installed object");
        let retained = roots.state.join("original-before-leaf-swap");

        let result = store.quarantine_corrupt_references_with_hooks(
            &BTreeSet::from([object.digest_hex()]),
            || Ok(()),
            |index| {
                assert_eq!(index, 1);
                fs::rename(object.path(), &retained)?;
                fs::write(object.path(), b"replacement")?;
                fs::set_permissions(object.path(), fs::Permissions::from_mode(0o600))?;
                Ok(())
            },
        );
        assert!(matches!(result, Err(ObjectPathError::UnsafeEntry)));
        assert_eq!(fs::read(&retained).expect("original retained"), b"original");
        assert!(!object.path().exists());

        let quarantine = store.profile_root.join("quarantine");
        let batch = fs::read_dir(quarantine)
            .expect("quarantine")
            .next()
            .expect("quarantine batch")
            .expect("batch entry")
            .path();
        assert_eq!(
            fs::read(batch.join(format!("corrupt-{}", object.digest_hex())))
                .expect("replacement retained in quarantine"),
            b"replacement"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn corrupt_reference_refuses_shard_symlink_swap_before_move() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"referenced bytes").expect("capture");
        let object = stage
            .install_object(64, Some(16), None)
            .expect("installed object");
        let shard = object.path().parent().expect("object shard");
        let outside_shard = roots.state.join("outside-object-shard");
        let outside_object = outside_shard.join(object.path().file_name().expect("object name"));

        let result = store.quarantine_corrupt_references_with_hook(
            &BTreeSet::from([object.digest_hex()]),
            || {
                fs::rename(shard, &outside_shard)?;
                symlink(&outside_shard, shard)?;
                Ok(())
            },
        );
        assert!(matches!(result, Err(ObjectPathError::UnsafeEntry)));
        assert_eq!(
            fs::read(&outside_object).expect("outside object retained"),
            b"referenced bytes"
        );
        assert_eq!(
            fs::read_dir(store.profile_root.join("quarantine"))
                .expect("quarantine")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn missing_corrupt_reference_shard_is_left_absent() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let digest = format!("ff{}", "0".repeat(62));
        assert!(!store.objects_root().join("ff").exists());

        assert_eq!(
            store
                .quarantine_corrupt_references(&BTreeSet::from([digest]))
                .expect("missing object is already absent"),
            0
        );
        assert_eq!(
            fs::read_dir(store.profile_root.join("quarantine"))
                .expect("quarantine")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn oversized_corrupt_reference_set_refuses_before_quarantine() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let references = (0..=MAX_RECOVERY_ENTRIES)
            .map(|index| format!("{index:064x}"))
            .collect();

        assert!(matches!(
            store.quarantine_corrupt_references(&references),
            Err(ObjectPathError::CaptureTooLarge)
        ));
        assert_eq!(
            fs::read_dir(store.profile_root.join("quarantine"))
                .expect("quarantine")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }
}
