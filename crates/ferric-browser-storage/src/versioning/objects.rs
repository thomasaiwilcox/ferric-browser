//! Restrictive paths for explicit normal-profile captures.
//!
//! This module does not publish an object or make a commit visible. The profile
//! worker must bind its caller to a validated normal-profile owner before use.

use std::{
    fmt,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};

use ferric_browser_core::{
    OperationId,
    versioning::{
        MAX_PAGE_SIDECAR_BYTES,
        limits::{MAX_ARCHIVE_BYTES, MAX_PAGES_PER_WINDOW_CAPTURE},
    },
};
use sha2::{Digest, Sha256};
use uuid::{Uuid, Variant, Version};

#[cfg(all(feature = "native-archive-fault-test", unix))]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[cfg(all(feature = "native-archive-fault-test", unix))]
static FAIL_NEXT_ARCHIVE_INSTALL_STORAGE_FULL: AtomicBool = AtomicBool::new(false);
#[cfg(all(feature = "native-archive-fault-test", unix))]
static ARCHIVE_INSTALL_STORAGE_FULL_SKIPS: AtomicUsize = AtomicUsize::new(0);
#[cfg(all(feature = "native-archive-fault-test", unix))]
static INJECTED_ARCHIVE_INSTALL_STORAGE_FULLS: AtomicUsize = AtomicUsize::new(0);

/// Arms one ENOSPC failure after the first archive-object temporary-file
/// chunk in a disposable native test. Absent from default builds.
#[cfg(all(feature = "native-archive-fault-test", unix))]
pub fn arm_one_archive_install_storage_full() {
    arm_archive_install_storage_full_after(0);
}

/// Arms one ENOSPC failure during the archive-object install after a chosen
/// number of preceding archive-object install invocations in a disposable
/// native test. Sidecar installations do not consume this count. Absent from
/// default builds.
#[cfg(all(feature = "native-archive-fault-test", unix))]
pub fn arm_archive_install_storage_full_after(successful_archive_installs: usize) {
    INJECTED_ARCHIVE_INSTALL_STORAGE_FULLS.store(0, Ordering::SeqCst);
    ARCHIVE_INSTALL_STORAGE_FULL_SKIPS.store(successful_archive_installs, Ordering::SeqCst);
    FAIL_NEXT_ARCHIVE_INSTALL_STORAGE_FULL.store(true, Ordering::SeqCst);
}

#[cfg(all(feature = "native-archive-fault-test", unix))]
fn inject_storage_full_in_current_archive_install() -> bool {
    if !FAIL_NEXT_ARCHIVE_INSTALL_STORAGE_FULL.load(Ordering::SeqCst) {
        return false;
    }
    loop {
        let remaining = ARCHIVE_INSTALL_STORAGE_FULL_SKIPS.load(Ordering::SeqCst);
        if remaining == 0 {
            return FAIL_NEXT_ARCHIVE_INSTALL_STORAGE_FULL
                .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok();
        }
        if ARCHIVE_INSTALL_STORAGE_FULL_SKIPS
            .compare_exchange(remaining, remaining - 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return false;
        }
    }
}

/// Counts only the archive-object copy failures injected by the test hook.
#[cfg(all(feature = "native-archive-fault-test", unix))]
#[must_use]
pub fn injected_archive_install_storage_fulls() -> usize {
    INJECTED_ARCHIVE_INSTALL_STORAGE_FULLS.load(Ordering::SeqCst)
}

#[cfg(unix)]
mod dirfd;
mod recovery;
mod worker;
pub(crate) use recovery::CaptureStageCleanupCheckpoint;
pub use recovery::RecoverySummary;
pub use worker::{
    VersioningObjectCommand, VersioningObjectCompletion, VersioningObjectFailure,
    VersioningObjectWorker, VersioningObjectWorkerError,
};
#[cfg(feature = "native-archive-fault-test")]
pub use worker::{
    archive_install_paused, arm_one_archive_cleanup_refusal, arm_one_archive_install_pause,
    arm_one_sidecar_install_pause, arm_one_sidecar_install_refusal,
    injected_archive_cleanup_refusals, injected_sidecar_install_refusals, resume_archive_install,
    resume_sidecar_install, sidecar_install_paused, sidecar_permission_denials,
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};

/// A path is rejected rather than repaired if another owner or a symlink has
/// installed it. Diagnostics omit filesystem details, page-derived names, and
/// file contents.
pub enum ObjectPathError {
    UnsupportedPlatform,
    InvalidProfile,
    UnsafeRoot,
    UnsafeEntry,
    ExistingOperation,
    EmptyCapture,
    CaptureTooLarge,
    CaptureSizeMismatch,
    CaptureDigestMismatch,
    CaptureChanged,
    ExistingObjectMismatch,
    InvalidObjectDigest,
    Cancelled,
    Io(io::Error),
}

impl std::fmt::Debug for ObjectPathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::UnsupportedPlatform => "UnsupportedPlatform",
            Self::InvalidProfile => "InvalidProfile",
            Self::UnsafeRoot => "UnsafeRoot",
            Self::UnsafeEntry => "UnsafeEntry",
            Self::ExistingOperation => "ExistingOperation",
            Self::EmptyCapture => "EmptyCapture",
            Self::CaptureTooLarge => "CaptureTooLarge",
            Self::CaptureSizeMismatch => "CaptureSizeMismatch",
            Self::CaptureDigestMismatch => "CaptureDigestMismatch",
            Self::CaptureChanged => "CaptureChanged",
            Self::ExistingObjectMismatch => "ExistingObjectMismatch",
            Self::InvalidObjectDigest => "InvalidObjectDigest",
            Self::Cancelled => "Cancelled",
            Self::Io(_) => {
                return formatter
                    .debug_tuple("ObjectPathError::Io")
                    .field(&"[redacted]")
                    .finish();
            }
        };
        formatter.write_str("ObjectPathError::")?;
        formatter.write_str(name)
    }
}

impl std::fmt::Display for ObjectPathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                formatter.write_str("secure object paths require supported Unix descriptor APIs")
            }
            Self::InvalidProfile => {
                formatter.write_str("profile identifier is not a random UUID v4")
            }
            Self::UnsafeRoot => {
                formatter.write_str("versioning root is not a private real directory")
            }
            Self::UnsafeEntry => formatter.write_str("versioning path contains an unsafe entry"),
            Self::ExistingOperation => {
                formatter.write_str("versioning operation directory already exists")
            }
            Self::EmptyCapture => formatter.write_str("versioning capture is empty"),
            Self::CaptureTooLarge => formatter.write_str("versioning capture exceeds byte limit"),
            Self::CaptureSizeMismatch => {
                formatter.write_str("versioning capture does not match native byte count")
            }
            Self::CaptureDigestMismatch => {
                formatter.write_str("versioning capture does not match expected SHA-256")
            }
            Self::CaptureChanged => {
                formatter.write_str("versioning capture changed during inspection")
            }
            Self::ExistingObjectMismatch => {
                formatter.write_str("existing immutable object does not match capture")
            }
            Self::InvalidObjectDigest => formatter.write_str("immutable object digest is invalid"),
            Self::Cancelled => formatter.write_str("versioning object job was cancelled"),
            Self::Io(_) => formatter.write_str("versioning path I/O failed"),
        }
    }
}

impl std::error::Error for ObjectPathError {}

impl From<io::Error> for ObjectPathError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Exact paths rooted under a verified normal-profile state directory.
pub struct VersioningObjectStore {
    profile_root: PathBuf,
    profile_uuid: Uuid,
    owner_uid: u32,
}

/// A single exclusive operation directory with a fixed native capture target.
pub struct StagedCapture {
    directory: PathBuf,
    capture_path: PathBuf,
    objects_root: PathBuf,
    owner_uid: u32,
}

fn ensure_supported_object_store_platform() -> Result<(), ObjectPathError> {
    #[cfg(any(not(unix), target_os = "redox", target_os = "espidf"))]
    {
        Err(ObjectPathError::UnsupportedPlatform)
    }
    #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
    {
        Ok(())
    }
}

/// Exact validated immutable bytes, not a published capture or commit.
pub struct StoredObject {
    path: PathBuf,
    sha256: [u8; 32],
    bytes: u64,
    reused: bool,
}

/// Exact path reserved for one operation's native page save. Debug output
/// deliberately omits the filesystem path.
#[derive(Clone, Eq, PartialEq)]
pub struct CaptureStageTarget {
    operation: OperationId,
    path: PathBuf,
}

/// Private, digest-verified MHTML materialized for one isolated archive view.
/// Its path is returned only through the application-owned callback route.
pub struct ArchiveViewFile {
    operation: OperationId,
    path: PathBuf,
    bytes: u64,
}

impl fmt::Debug for ArchiveViewFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArchiveViewFile")
            .field("operation", &"[redacted]")
            .field("path", &"[redacted]")
            .field("bytes", &self.bytes)
            .finish()
    }
}

impl ArchiveViewFile {
    #[must_use]
    pub const fn operation(&self) -> OperationId {
        self.operation
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl std::fmt::Debug for CaptureStageTarget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CaptureStageTarget")
            .field("operation", &"[redacted]")
            .field("path", &"[redacted]")
            .finish()
    }
}

impl CaptureStageTarget {
    #[must_use]
    pub const fn operation(&self) -> OperationId {
        self.operation
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Validated metadata for an immutable object installed by the blob worker.
/// It is not a manifest reference or a published commit.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct InstalledObject {
    pub sha256: [u8; 32],
    pub bytes: u64,
    pub reused: bool,
}

impl InstalledObject {
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    #[must_use]
    pub fn digest_hex(&self) -> String {
        sha256_hex(&self.sha256)
    }
}

impl fmt::Debug for InstalledObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstalledObject")
            .field("sha256", &"[content digest]")
            .field("bytes", &self.bytes)
            .field("reused", &self.reused)
            .finish()
    }
}

/// Verified sidecar bytes read from one committed, profile-local object.
/// Debug output deliberately exposes only the bounded length.
pub struct VersionedSidecarBytes(Vec<u8>);

impl std::fmt::Debug for VersionedSidecarBytes {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VersionedSidecarBytes")
            .field("length", &self.0.len())
            .finish_non_exhaustive()
    }
}

impl VersionedSidecarBytes {
    /// Consumes the wrapper and returns the validated bytes to the trusted
    /// application parser. Do not format or log the returned payload.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl StoredObject {
    /// Validated content-addressed path under this normal profile.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Number of validated bytes in the object.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Whether an identical, previously installed object was reused.
    #[must_use]
    pub fn reused(&self) -> bool {
        self.reused
    }

    /// SHA-256 identity; callers must not confuse this with publication.
    #[must_use]
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    /// Lowercase filesystem encoding of the SHA-256 identity.
    #[must_use]
    pub fn digest_hex(&self) -> String {
        sha256_hex(&self.sha256)
    }
}

impl VersioningObjectStore {
    /// Creates or verifies `versioning/<profile-uuid>` beneath a private state root.
    /// The caller must already have selected and locked a normal profile.
    ///
    /// # Errors
    /// Returns an error for an untrusted root, symlink, wrong owner, loose mode,
    /// or failed directory creation. Existing entries are never repaired.
    pub fn open(state_root: &Path, profile_uuid: Uuid) -> Result<Self, ObjectPathError> {
        // INVARIANT: only a validated random profile UUID becomes a path
        // component; the trusted caller must supply the normal-profile root.
        if profile_uuid.get_variant() != Variant::RFC4122
            || profile_uuid.get_version() != Some(Version::Random)
        {
            return Err(ObjectPathError::InvalidProfile);
        }
        #[cfg(any(not(unix), target_os = "redox", target_os = "espidf"))]
        {
            let _ = (state_root, profile_uuid);
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
        {
            let root =
                private_directory(state_root, None).map_err(|_| ObjectPathError::UnsafeRoot)?;
            let root_metadata = root.metadata()?;
            let owner_uid = root_metadata.uid();
            let canonical_root = fs::canonicalize(state_root)?;
            let versioning = canonical_root.join("versioning");
            // Pin every directory used to create this profile tree. Compare
            // the canonical-path walk with the descriptor opened from the
            // caller's state root so a concurrent root rename cannot redirect
            // initialization into a different private tree.
            let pinned_root = dirfd::open_private_directory(&canonical_root, owner_uid)
                .map_err(|_| ObjectPathError::UnsafeRoot)?;
            let pinned_root_metadata = pinned_root.metadata()?;
            if pinned_root_metadata.dev() != root_metadata.dev()
                || pinned_root_metadata.ino() != root_metadata.ino()
            {
                return Err(ObjectPathError::UnsafeRoot);
            }
            let versioning_directory =
                dirfd::ensure_private_directory_at(&pinned_root, "versioning", owner_uid)?;
            pinned_root.sync_all()?;
            let profile_root = versioning.join(profile_uuid.hyphenated().to_string());
            let profile_directory = dirfd::ensure_private_directory_at(
                &versioning_directory,
                &profile_uuid.hyphenated().to_string(),
                owner_uid,
            )?;
            versioning_directory.sync_all()?;
            for child in ["staging", "views", "objects", "quarantine"] {
                dirfd::ensure_private_directory_at(&profile_directory, child, owner_uid)?;
            }
            let objects_directory =
                dirfd::open_private_directory_at(&profile_directory, "objects", owner_uid)?;
            dirfd::ensure_private_directory_at(&objects_directory, "sha256", owner_uid)?;
            // INVARIANT: each newly created ancestor needed by a later object
            // link is durable before an object can be returned.
            profile_directory.sync_all()?;
            objects_directory.sync_all()?;
            Ok(Self {
                profile_root,
                profile_uuid,
                owner_uid,
            })
        }
    }

    /// Reserves an operation once and creates a mode-0600 capture file with a
    /// fixed name. No title, URL, cookie name or untrusted path is accepted.
    ///
    /// # Errors
    /// Returns an error if the operation already exists or a path is unsafe.
    pub fn begin(&self, operation: OperationId) -> Result<StagedCapture, ObjectPathError> {
        self.begin_in(&self.profile_root.join("staging"), operation)
    }

    fn begin_in(
        &self,
        parent: &Path,
        operation: OperationId,
    ) -> Result<StagedCapture, ObjectPathError> {
        ensure_supported_object_store_platform()?;
        // INVARIANT: untrusted page data never supplies a filename, and an
        // existing operation is not reopened or overwritten.
        #[cfg(unix)]
        let parent_directory = dirfd::open_private_directory(parent, self.owner_uid)?;
        #[cfg(not(unix))]
        private_directory(parent, Some(self.owner_uid))?;
        let operation_name = operation.as_uuid().hyphenated().to_string();
        let directory = parent.join(&operation_name);
        #[cfg(unix)]
        {
            match dirfd::mkdir_at(&parent_directory, &operation_name) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    return Err(ObjectPathError::ExistingOperation);
                }
                Err(error) => return Err(error.into()),
            }
        }
        #[cfg(not(unix))]
        {
            let mut builder = DirBuilder::new();
            builder.create(&directory).map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    ObjectPathError::ExistingOperation
                } else {
                    ObjectPathError::Io(error)
                }
            })?;
        }
        #[cfg(unix)]
        let operation_directory =
            dirfd::open_private_directory_at(&parent_directory, &operation_name, self.owner_uid)?;
        #[cfg(not(unix))]
        private_directory(&directory, Some(self.owner_uid))?;
        let capture_path = directory.join("capture.bin");
        #[cfg(unix)]
        let file = dirfd::create_private_regular_file_at(
            &operation_directory,
            "capture.bin",
            self.owner_uid,
        )?;
        #[cfg(not(unix))]
        let file = {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            options.open(&capture_path)?
        };
        file.sync_all()?;
        #[cfg(unix)]
        {
            operation_directory.sync_all()?;
            parent_directory.sync_all()?;
        }
        #[cfg(not(unix))]
        private_directory(parent, Some(self.owner_uid))?.sync_all()?;
        Ok(StagedCapture {
            directory,
            capture_path,
            objects_root: self.objects_root(),
            owner_uid: self.owner_uid,
        })
    }

    /// Reopens only the exact staging directory previously reserved for this
    /// operation. The capture file is revalidated without following links
    /// before any bytes are read.
    ///
    /// # Errors
    /// Rejects missing, substituted, or incorrectly owned staging entries.
    pub fn open_staged(&self, operation: OperationId) -> Result<StagedCapture, ObjectPathError> {
        self.open_staged_path(operation, "capture.bin")
    }

    /// Creates a separate, exclusive target for another page in the same
    /// captured window. Page zero is the initial `begin` target; additional
    /// pages have fixed, bounded names and cannot replace an earlier save.
    ///
    /// # Errors
    /// Refuses a missing operation, an out-of-range index, an existing leaf,
    /// or an unsafe staging directory.
    pub fn prepare_window_page(
        &self,
        operation: OperationId,
        page_index: usize,
    ) -> Result<StagedCapture, ObjectPathError> {
        ensure_supported_object_store_platform()?;
        let leaf = window_page_capture_leaf(page_index)?;
        let staging = self.profile_root.join("staging");
        #[cfg(unix)]
        let staging_directory = dirfd::open_private_directory(&staging, self.owner_uid)?;
        #[cfg(not(unix))]
        private_directory(&staging, Some(self.owner_uid))?;
        let operation_name = operation.as_uuid().hyphenated().to_string();
        let directory = staging.join(&operation_name);
        #[cfg(unix)]
        let operation_directory =
            dirfd::open_private_directory_at(&staging_directory, &operation_name, self.owner_uid)?;
        #[cfg(not(unix))]
        private_directory(&directory, Some(self.owner_uid))?;
        // Qt may unlink capture.bin when page zero is cancelled. A missing
        // first leaf is an explicit failed-page outcome, not loss of the
        // private operation directory. Still reject any surviving unsafe
        // first leaf before creating the next independently named target.
        match self.open_staged(operation) {
            Ok(_) => {}
            Err(ObjectPathError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let capture_path = directory.join(&leaf);
        #[cfg(unix)]
        let file =
            dirfd::create_private_regular_file_at(&operation_directory, &leaf, self.owner_uid)?;
        #[cfg(not(unix))]
        let file = {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            options.open(&capture_path)?
        };
        file.sync_all()?;
        #[cfg(unix)]
        operation_directory.sync_all()?;
        #[cfg(not(unix))]
        private_directory(&directory, Some(self.owner_uid))?.sync_all()?;
        Ok(StagedCapture {
            directory,
            capture_path,
            objects_root: self.objects_root(),
            owner_uid: self.owner_uid,
        })
    }

    /// Reopens one previously prepared window-page target by its bounded
    /// ordinal, checking the same owner/link/mode rules as the first page.
    ///
    /// # Errors
    /// Refuses a missing, substituted, or out-of-range page target.
    pub fn open_staged_window_page(
        &self,
        operation: OperationId,
        page_index: usize,
    ) -> Result<StagedCapture, ObjectPathError> {
        let leaf = if page_index == 0 {
            "capture.bin".to_owned()
        } else {
            window_page_capture_leaf(page_index)?
        };
        self.open_staged_path(operation, &leaf)
    }

    fn open_staged_path(
        &self,
        operation: OperationId,
        leaf: &str,
    ) -> Result<StagedCapture, ObjectPathError> {
        let staging = self.profile_root.join("staging");
        let operation_name = operation.as_uuid().hyphenated().to_string();
        let directory = staging.join(&operation_name);
        let capture_path = directory.join(leaf);
        #[cfg(unix)]
        {
            let staging_directory = dirfd::open_private_directory(&staging, self.owner_uid)?;
            let operation_directory = dirfd::open_private_directory_at(
                &staging_directory,
                &operation_name,
                self.owner_uid,
            )?;
            drop(dirfd::open_private_regular_file_at(
                &operation_directory,
                leaf,
                self.owner_uid,
            )?);
        }
        #[cfg(not(unix))]
        {
            private_directory(&staging, Some(self.owner_uid))?;
            private_directory(&directory, Some(self.owner_uid))?;
            let metadata = fs::symlink_metadata(&capture_path)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(ObjectPathError::UnsafeEntry);
            }
        }
        Ok(StagedCapture {
            directory,
            capture_path,
            objects_root: self.objects_root(),
            owner_uid: self.owner_uid,
        })
    }

    /// Profile-local immutable object root; publication is a later protocol.
    #[must_use]
    pub fn objects_root(&self) -> PathBuf {
        self.profile_root.join("objects/sha256")
    }

    /// Exact normal profile this object store was opened for.
    #[must_use]
    pub fn profile_uuid(&self) -> Uuid {
        self.profile_uuid
    }

    /// Revalidates a manifest reference against a private profile-local object
    /// before the metadata worker can publish that reference.
    ///
    /// # Errors
    /// Rejects an invalid digest/path, missing or corrupt object, unsafe mode
    /// or owner, and any object outside the caller's byte limit.
    pub fn verify_object_reference(
        &self,
        digest_hex: &str,
        expected_bytes: u64,
        max_bytes: u64,
    ) -> Result<(), ObjectPathError> {
        if digest_hex.len() != 64
            || !digest_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ObjectPathError::InvalidObjectDigest);
        }
        if expected_bytes == 0 || expected_bytes > max_bytes {
            return Err(ObjectPathError::CaptureTooLarge);
        }
        #[cfg(unix)]
        let shard_dir = {
            let objects_root = dirfd::open_private_directory(&self.objects_root(), self.owner_uid)?;
            dirfd::open_private_directory_at(&objects_root, &digest_hex[..2], self.owner_uid)?
        };
        #[cfg(not(unix))]
        let shard_dir = {
            let objects_root = self.objects_root();
            private_directory(&objects_root, Some(self.owner_uid))?;
            let shard = objects_root.join(&digest_hex[..2]);
            private_directory(&shard, Some(self.owner_uid))?
        };
        #[cfg(unix)]
        let mut object =
            dirfd::open_private_regular_file_at(&shard_dir, &digest_hex[2..], self.owner_uid)?;
        #[cfg(not(unix))]
        let mut object = {
            let objects_root = self.objects_root();
            private_directory(&objects_root, Some(self.owner_uid))?;
            let shard = objects_root.join(&digest_hex[..2]);
            let shard_dir = private_directory(&shard, Some(self.owner_uid))?;
            let object_path = shard.join(&digest_hex[2..]);
            let object = open_private_regular(&object_path, self.owner_uid)?;
            drop(shard_dir);
            object
        };
        if object.metadata()?.len() != expected_bytes {
            return Err(ObjectPathError::ExistingObjectMismatch);
        }
        let mut hasher = Sha256::new();
        let mut scratch = [0u8; 16 * 1024];
        let mut total = 0u64;
        loop {
            let length = object.read(&mut scratch)?;
            if length == 0 {
                break;
            }
            total = total
                .checked_add(length as u64)
                .ok_or(ObjectPathError::CaptureTooLarge)?;
            if total > expected_bytes || total > max_bytes {
                return Err(ObjectPathError::ExistingObjectMismatch);
            }
            hasher.update(&scratch[..length]);
        }
        let actual: [u8; 32] = hasher.finalize().into();
        if total != expected_bytes || sha256_hex(&actual) != digest_hex {
            return Err(ObjectPathError::ExistingObjectMismatch);
        }
        object.sync_all()?;
        Ok(())
    }

    /// Reads one bounded, content-addressed object and verifies its exact
    /// digest before returning it to the trusted application parser.
    ///
    /// # Errors
    /// Rejects an invalid digest, size, profile-local path, file owner, mode,
    /// object type, length or SHA-256 digest.
    pub fn read_object_reference(
        &self,
        digest_hex: &str,
        expected_bytes: u64,
        max_bytes: u64,
    ) -> Result<VersionedSidecarBytes, ObjectPathError> {
        if digest_hex.len() != 64
            || !digest_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ObjectPathError::InvalidObjectDigest);
        }
        if expected_bytes == 0 || expected_bytes > max_bytes {
            return Err(ObjectPathError::CaptureTooLarge);
        }
        let expected_capacity =
            usize::try_from(expected_bytes).map_err(|_| ObjectPathError::CaptureTooLarge)?;
        #[cfg(unix)]
        let mut object = {
            let objects_root = dirfd::open_private_directory(&self.objects_root(), self.owner_uid)?;
            let shard_dir =
                dirfd::open_private_directory_at(&objects_root, &digest_hex[..2], self.owner_uid)?;
            dirfd::open_private_regular_file_at(&shard_dir, &digest_hex[2..], self.owner_uid)?
        };
        #[cfg(not(unix))]
        let mut object = {
            let objects_root = self.objects_root();
            private_directory(&objects_root, Some(self.owner_uid))?;
            let shard = objects_root.join(&digest_hex[..2]);
            private_directory(&shard, Some(self.owner_uid))?;
            let object_path = shard.join(&digest_hex[2..]);
            open_private_regular(&object_path, self.owner_uid)?
        };
        if object.metadata()?.len() != expected_bytes {
            return Err(ObjectPathError::ExistingObjectMismatch);
        }

        let mut bytes = Vec::with_capacity(expected_capacity);
        let mut hasher = Sha256::new();
        let mut scratch = [0u8; 8 * 1024];
        let mut total = 0u64;
        loop {
            let length = object.read(&mut scratch)?;
            if length == 0 {
                break;
            }
            total = total
                .checked_add(length as u64)
                .ok_or(ObjectPathError::CaptureTooLarge)?;
            if total > expected_bytes || total > max_bytes {
                return Err(ObjectPathError::ExistingObjectMismatch);
            }
            hasher.update(&scratch[..length]);
            bytes.extend_from_slice(&scratch[..length]);
        }
        let actual: [u8; 32] = hasher.finalize().into();
        if total != expected_bytes || sha256_hex(&actual) != digest_hex {
            return Err(ObjectPathError::ExistingObjectMismatch);
        }
        Ok(VersionedSidecarBytes(bytes))
    }

    /// Copies one committed archive object into an exact private staging file
    /// for the isolated viewer, verifying its full content digest while
    /// streaming through a fixed buffer.
    ///
    /// # Errors
    /// Rejects invalid object references, unsafe staging entries, cancellation,
    /// and any size or digest mismatch. Failed materializations are removed.
    pub fn materialize_archive_view_cancellable(
        &self,
        operation: OperationId,
        digest_hex: &str,
        expected_bytes: u64,
        cancelled: impl FnMut() -> bool,
    ) -> Result<ArchiveViewFile, ObjectPathError> {
        self.materialize_archive_view_cancellable_with_sync(
            operation,
            digest_hex,
            expected_bytes,
            cancelled,
            |directory| directory.sync_all(),
        )
    }

    fn materialize_archive_view_cancellable_with_sync(
        &self,
        operation: OperationId,
        digest_hex: &str,
        expected_bytes: u64,
        cancelled: impl FnMut() -> bool,
        sync_directory: impl FnOnce(&File) -> io::Result<()>,
    ) -> Result<ArchiveViewFile, ObjectPathError> {
        // Viewer files have a distinct private root. Live GC may quarantine
        // capture staging but must never move a file held by WebEngine.
        let mut stage = self.begin_in(&self.profile_root.join("views"), operation)?;
        let result = stage
            .use_archive_view_path_with_sync(sync_directory)
            .and_then(|()| {
                stage.copy_archive_reference(operation, digest_hex, expected_bytes, cancelled)
            });
        match result {
            Ok(file) => Ok(file),
            Err(error) => {
                let _ = self.discard_archive_view(operation);
                Err(error)
            }
        }
    }

    /// Removes only the exact private staging file allocated to one archive
    /// view operation. Missing targets are idempotently accepted.
    ///
    /// # Errors
    /// Refuses a replaced directory, symlink, foreign file, or unexpected
    /// staging contents instead of recursively deleting user data.
    pub fn discard_archive_view(&self, operation: OperationId) -> Result<(), ObjectPathError> {
        self.discard_archive_view_with_sync(operation, |directory| directory.sync_all())
    }

    fn discard_archive_view_with_sync(
        &self,
        operation: OperationId,
        mut sync_directory: impl FnMut(&File) -> io::Result<()>,
    ) -> Result<(), ObjectPathError> {
        #[cfg(not(unix))]
        {
            let _ = (operation, &mut sync_directory);
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            let views = self.profile_root.join("views");
            let views_dir = dirfd::open_private_directory(&views, self.owner_uid)?;
            let operation_name = operation.as_uuid().hyphenated().to_string();
            let operation_dir =
                match dirfd::open_private_directory_at(&views_dir, &operation_name, self.owner_uid)
                {
                    Ok(directory) => directory,
                    Err(ObjectPathError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                        // A previous attempt may have removed this directory but
                        // failed to sync its parent. Make an idempotent retry
                        // complete that durability boundary before returning.
                        sync_directory(&views_dir)?;
                        return Ok(());
                    }
                    Err(error) => return Err(error),
                };
            // Both names are fixed by this module. `capture.bin` is the generic
            // staging leaf and `capture.mhtml` gives Qt/Chromium an unambiguous
            // archive MIME suffix. Never enumerate or recursively delete this
            // private operation directory.
            for capture_name in ["capture.bin", "capture.mhtml"] {
                let metadata = match dirfd::regular_entry_identity_at(
                    &operation_dir,
                    capture_name,
                    self.owner_uid,
                ) {
                    Err(ObjectPathError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                        continue;
                    }
                    Err(error) => return Err(error),
                    Ok(metadata) => metadata,
                };
                if metadata.links != 1 {
                    return Err(ObjectPathError::UnsafeEntry);
                }
                dirfd::unlink_at(&operation_dir, capture_name, false)?;
            }
            sync_directory(&operation_dir)?;
            dirfd::unlink_at(&views_dir, &operation_name, true)?;
            sync_directory(&views_dir)?;
            Ok(())
        }
    }

    /// Installs bounded renderer sidecar bytes as a separate immutable object
    /// inside the exact already-reserved capture operation. The raw payload
    /// is never used as a path component or included in diagnostics.
    ///
    /// # Errors
    /// Rejects an empty/oversized payload, unsafe staging paths, cancellation,
    /// corrupt pre-existing objects, or failed durability operations.
    pub fn install_sidecar_bytes_cancellable(
        &self,
        operation: OperationId,
        bytes: &[u8],
        cancelled: impl FnMut() -> bool,
    ) -> Result<StoredObject, ObjectPathError> {
        ensure_supported_object_store_platform()?;
        if bytes.is_empty() {
            return Err(ObjectPathError::EmptyCapture);
        }
        if bytes.len() > MAX_PAGE_SIDECAR_BYTES {
            return Err(ObjectPathError::CaptureTooLarge);
        }
        let stage = self.open_staged(operation)?;
        let directory = private_directory(&stage.directory, Some(self.owner_uid))?;
        #[cfg(unix)]
        let objects_root_directory =
            dirfd::open_private_directory(&self.objects_root(), self.owner_uid)?;
        #[cfg(not(unix))]
        private_directory(&self.objects_root(), Some(self.owner_uid))?;
        let temporary_path = stage.directory.join("sidecar.tmp");
        let mut cleanup = TemporaryObjectCleanup::new(
            temporary_path.clone(),
            &directory,
            "sidecar.tmp",
            self.owner_uid,
        )?;
        #[cfg(unix)]
        let mut temporary =
            dirfd::create_private_regular_file_at(&directory, "sidecar.tmp", self.owner_uid)?;
        #[cfg(not(unix))]
        let mut temporary = {
            let mut options = OpenOptions::new();
            options.write(true).read(true).create_new(true);
            options.open(&temporary_path)?
        };
        cleanup.arm(&temporary)?;
        let mut cancelled = cancelled;
        let mut hasher = Sha256::new();
        for chunk in bytes.chunks(16 * 1024) {
            if cancelled() {
                return Err(ObjectPathError::Cancelled);
            }
            temporary.write_all(chunk)?;
            hasher.update(chunk);
        }
        if cancelled() {
            return Err(ObjectPathError::Cancelled);
        }
        let sha256: [u8; 32] = hasher.finalize().into();
        temporary.sync_all()?;
        let digest_hex = sha256_hex(&sha256);
        let shard_name = &digest_hex[..2];
        let shard = self.objects_root().join(shard_name);
        #[cfg(unix)]
        let shard_dir = dirfd::ensure_private_directory_at(
            &objects_root_directory,
            shard_name,
            self.owner_uid,
        )?;
        #[cfg(unix)]
        objects_root_directory.sync_all()?;
        #[cfg(not(unix))]
        let shard_dir = {
            ensure_private_directory(&shard, self.owner_uid)?;
            private_directory(&self.objects_root(), Some(self.owner_uid))?.sync_all()?;
            private_directory(&shard, Some(self.owner_uid))?
        };
        let object_path = shard.join(&digest_hex[2..]);
        if cancelled() {
            return Err(ObjectPathError::Cancelled);
        }
        #[cfg(unix)]
        let link_result = dirfd::link_at(&directory, "sidecar.tmp", &shard_dir, &digest_hex[2..]);
        #[cfg(not(unix))]
        let link_result = fs::hard_link(&temporary_path, &object_path);
        let reused = match link_result {
            Ok(()) => {
                #[cfg(unix)]
                verify_temporary_object_link(
                    &directory,
                    "sidecar.tmp",
                    &shard_dir,
                    &digest_hex[2..],
                    &temporary,
                    self.owner_uid,
                    bytes.len() as u64,
                )?;
                false
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                #[cfg(unix)]
                let mut existing = dirfd::open_private_regular_file_at(
                    &shard_dir,
                    &digest_hex[2..],
                    self.owner_uid,
                )?;
                #[cfg(not(unix))]
                let mut existing = open_private_regular(&object_path, self.owner_uid)?;
                if existing.metadata()?.len() != bytes.len() as u64
                    || !files_equal(&mut temporary, &mut existing, bytes.len() as u64)?
                {
                    return Err(ObjectPathError::ExistingObjectMismatch);
                }
                true
            }
            Err(error) => return Err(error.into()),
        };
        if cancelled() {
            return Err(ObjectPathError::Cancelled);
        }
        #[cfg(unix)]
        unlink_temporary_leaf(&directory, "sidecar.tmp", &temporary, self.owner_uid)?;
        #[cfg(not(unix))]
        fs::remove_file(&temporary_path)?;
        cleanup.disarm();
        directory.sync_all()?;
        shard_dir.sync_all()?;
        Ok(StoredObject {
            path: object_path,
            sha256,
            bytes: bytes.len() as u64,
            reused,
        })
    }
}

fn window_page_capture_leaf(page_index: usize) -> Result<String, ObjectPathError> {
    if page_index == 0 || page_index >= MAX_PAGES_PER_WINDOW_CAPTURE {
        return Err(ObjectPathError::UnsafeEntry);
    }
    Ok(format!("capture-page-{page_index:03}.bin"))
}

#[cfg(unix)]
fn rename_capture_to_archive_view(
    directory: &File,
    capture: &File,
    owner_uid: u32,
) -> Result<(), ObjectPathError> {
    rename_capture_to_archive_view_with_hook(directory, capture, owner_uid, || {})
}

#[cfg(unix)]
fn rename_capture_to_archive_view_with_hook(
    directory: &File,
    capture: &File,
    owner_uid: u32,
    before_rename: impl FnOnce(),
) -> Result<(), ObjectPathError> {
    let opened = capture.metadata()?;
    if !opened.is_file()
        || opened.uid() != owner_uid
        || opened.mode() & 0o077 != 0
        || opened.nlink() != 1
    {
        return Err(ObjectPathError::UnsafeEntry);
    }
    let staged = dirfd::regular_entry_identity_at(directory, "capture.bin", owner_uid)?;
    if staged.device != opened.dev()
        || staged.inode != opened.ino()
        || staged.links != opened.nlink()
        || staged.bytes != opened.len()
    {
        return Err(ObjectPathError::UnsafeEntry);
    }

    before_rename();
    dirfd::rename_at(directory, "capture.bin", directory, "capture.mhtml").map_err(|error| {
        if error.kind() == io::ErrorKind::Unsupported {
            ObjectPathError::UnsupportedPlatform
        } else {
            ObjectPathError::Io(error)
        }
    })?;

    // A name substituted immediately before rename is moved, never followed.
    // Verify the moved entry against the pinned descriptor before treating it
    // as the viewer target; on mismatch leave it in the private operation dir
    // for recovery instead of unlinking data we did not create.
    let renamed = dirfd::regular_entry_identity_at(directory, "capture.mhtml", owner_uid)?;
    if renamed.device != opened.dev()
        || renamed.inode != opened.ino()
        || renamed.links != opened.nlink()
        || renamed.bytes != opened.len()
    {
        return Err(ObjectPathError::UnsafeEntry);
    }
    Ok(())
}

impl StagedCapture {
    /// Fixed capture path for the admitted native save operation.
    #[must_use]
    pub fn capture_path(&self) -> &Path {
        &self.capture_path
    }

    fn use_archive_view_path(&mut self) -> Result<(), ObjectPathError> {
        self.use_archive_view_path_with_sync(|directory| directory.sync_all())
    }

    fn use_archive_view_path_with_sync(
        &mut self,
        sync_directory: impl FnOnce(&File) -> io::Result<()>,
    ) -> Result<(), ObjectPathError> {
        if self.capture_path.file_name().and_then(|name| name.to_str()) != Some("capture.bin") {
            return Err(ObjectPathError::UnsafeEntry);
        }
        #[cfg(any(not(unix), target_os = "redox", target_os = "espidf"))]
        {
            let _ = (self, sync_directory);
            return Err(ObjectPathError::UnsupportedPlatform);
        }
        #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
        {
            let directory = dirfd::open_private_directory(&self.directory, self.owner_uid)?;
            let capture =
                dirfd::open_private_regular_file_at(&directory, "capture.bin", self.owner_uid)?;
            let archive_path = self.directory.join("capture.mhtml");
            // Rename the reserved empty leaf atomically instead of checking
            // its identity and then unlinking its name. If a replacement wins
            // before the rename, the post-rename identity check refuses it
            // while preserving the moved entry for recovery.
            rename_capture_to_archive_view(&directory, &capture, self.owner_uid)?;
            sync_directory(&directory)?;
            drop(capture);
            self.capture_path = archive_path;
            Ok(())
        }
    }

    /// Exact exclusive operation directory, not a general-purpose output root.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Opens the completed file without following a substituted symlink or
    /// blocking on a FIFO. The descriptor metadata is checked after open.
    ///
    /// # Errors
    /// Returns an error if the path was replaced, is not a private regular file,
    /// has multiple hard links, or was created under a different owner.
    pub fn open_capture(&self) -> Result<File, ObjectPathError> {
        #[cfg(unix)]
        {
            self.open_capture_with_directory().map(|(file, _)| file)
        }
        #[cfg(not(unix))]
        {
            private_directory(&self.directory, Some(self.owner_uid))?;
            let path_metadata = fs::symlink_metadata(&self.capture_path)?;
            if !path_metadata.is_file() || path_metadata.file_type().is_symlink() {
                return Err(ObjectPathError::UnsafeEntry);
            }
            let mut options = OpenOptions::new();
            options.read(true);
            let file = options.open(&self.capture_path)?;
            Ok(file)
        }
    }

    #[cfg(unix)]
    fn open_capture_with_directory(&self) -> Result<(File, File), ObjectPathError> {
        let directory = dirfd::open_private_directory(&self.directory, self.owner_uid)?;
        let name = self
            .capture_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ObjectPathError::UnsafeEntry)?;
        let file = dirfd::open_private_regular_file_at(&directory, name, self.owner_uid)?;
        Ok((file, directory))
    }

    fn copy_archive_reference(
        &self,
        operation: OperationId,
        digest_hex: &str,
        expected_bytes: u64,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<ArchiveViewFile, ObjectPathError> {
        if digest_hex.len() != 64
            || !digest_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ObjectPathError::InvalidObjectDigest);
        }
        if expected_bytes == 0 || expected_bytes > MAX_ARCHIVE_BYTES {
            return Err(ObjectPathError::CaptureTooLarge);
        }
        #[cfg(unix)]
        let mut source = {
            let objects_root = dirfd::open_private_directory(&self.objects_root, self.owner_uid)?;
            let shard_dir =
                dirfd::open_private_directory_at(&objects_root, &digest_hex[..2], self.owner_uid)?;
            dirfd::open_private_regular_file_at(&shard_dir, &digest_hex[2..], self.owner_uid)?
        };
        #[cfg(not(unix))]
        let mut source = {
            private_directory(&self.directory, Some(self.owner_uid))?;
            private_directory(&self.objects_root, Some(self.owner_uid))?;
            let shard = self.objects_root.join(&digest_hex[..2]);
            private_directory(&shard, Some(self.owner_uid))?;
            open_private_regular(&shard.join(&digest_hex[2..]), self.owner_uid)?
        };
        if source.metadata()?.len() != expected_bytes {
            return Err(ObjectPathError::ExistingObjectMismatch);
        }

        #[cfg(unix)]
        let target_directory = dirfd::open_private_directory(&self.directory, self.owner_uid)?;
        #[cfg(not(unix))]
        private_directory(&self.directory, Some(self.owner_uid))?;
        #[cfg(unix)]
        let mut target = {
            let name = self
                .capture_path
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| *name == "capture.mhtml")
                .ok_or(ObjectPathError::UnsafeEntry)?;
            dirfd::open_private_regular_file_update_at(&target_directory, name, self.owner_uid)?
        };
        #[cfg(not(unix))]
        let mut target = {
            let path_metadata = fs::symlink_metadata(&self.capture_path)?;
            if !path_metadata.is_file() || path_metadata.file_type().is_symlink() {
                return Err(ObjectPathError::UnsafeEntry);
            }
            let mut options = OpenOptions::new();
            // `use_archive_view_path` created this target empty. Opening with
            // truncation could mutate a substituted target before validation.
            options.write(true);
            options.open(&self.capture_path)?
        };
        if target.metadata()?.len() != 0 {
            return Err(ObjectPathError::UnsafeEntry);
        }

        let mut hasher = Sha256::new();
        let mut scratch = [0u8; 16 * 1024];
        let mut total = 0_u64;
        loop {
            if cancelled() {
                return Err(ObjectPathError::Cancelled);
            }
            let length = match source.read(&mut scratch) {
                Ok(length) => length,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            };
            if length == 0 {
                break;
            }
            total = total
                .checked_add(length as u64)
                .ok_or(ObjectPathError::CaptureTooLarge)?;
            if total > expected_bytes || total > MAX_ARCHIVE_BYTES {
                return Err(ObjectPathError::ExistingObjectMismatch);
            }
            target.write_all(&scratch[..length])?;
            hasher.update(&scratch[..length]);
        }
        if cancelled() {
            return Err(ObjectPathError::Cancelled);
        }
        let digest: [u8; 32] = hasher.finalize().into();
        if total != expected_bytes || sha256_hex(&digest) != digest_hex {
            return Err(ObjectPathError::ExistingObjectMismatch);
        }
        if target.metadata()?.len() != expected_bytes {
            return Err(ObjectPathError::ExistingObjectMismatch);
        }
        target.sync_all()?;
        #[cfg(unix)]
        target_directory.sync_all()?;
        #[cfg(not(unix))]
        private_directory(&self.directory, Some(self.owner_uid))?.sync_all()?;
        Ok(ArchiveViewFile {
            operation,
            path: self.capture_path.clone(),
            bytes: total,
        })
    }

    /// Streams a completed capture through a reusable 16-KiB buffer. The
    /// caller supplies the admitted size limit and optional native completion
    /// byte count. This does not install or publish an immutable object. On
    /// error, the caller must discard any partial sink state.
    ///
    /// # Errors
    /// Refuses an empty, oversized, unsafe or size-changing capture, and
    /// propagates read errors without exposing capture content in diagnostics.
    pub fn stream_capture(
        &self,
        max_bytes: u64,
        expected_size: Option<u64>,
        mut consume: impl FnMut(&[u8]),
    ) -> Result<u64, ObjectPathError> {
        self.scan_capture(
            max_bytes,
            expected_size,
            |chunk| {
                consume(chunk);
                Ok(())
            },
            || false,
        )
    }

    fn scan_capture(
        &self,
        max_bytes: u64,
        expected_size: Option<u64>,
        mut consume: impl FnMut(&[u8]) -> Result<(), ObjectPathError>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<u64, ObjectPathError> {
        // INVARIANT: the read and final leaf check use the same pinned
        // operation directory; pathname replacement must never redirect the
        // verification step to an outside file.
        #[cfg(unix)]
        let (mut file, directory) = self.open_capture_with_directory()?;
        #[cfg(not(unix))]
        let mut file = self.open_capture()?;
        let initial_metadata = file.metadata()?;
        let initial_size = initial_metadata.len();
        if initial_size == 0 {
            return Err(ObjectPathError::EmptyCapture);
        }
        if initial_size > max_bytes || expected_size.is_some_and(|size| size > max_bytes) {
            return Err(ObjectPathError::CaptureTooLarge);
        }
        if expected_size.is_some_and(|size| size != initial_size) {
            return Err(ObjectPathError::CaptureSizeMismatch);
        }
        let mut scratch = [0u8; 16 * 1024];
        let mut total = 0u64;
        loop {
            if cancelled() {
                return Err(ObjectPathError::Cancelled);
            }
            let length = match file.read(&mut scratch) {
                Ok(length) => length,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            };
            if length == 0 {
                break;
            }
            total = total
                .checked_add(length as u64)
                .ok_or(ObjectPathError::CaptureTooLarge)?;
            if total > max_bytes {
                return Err(ObjectPathError::CaptureTooLarge);
            }
            consume(&scratch[..length])?;
        }
        let file_metadata = file.metadata()?;
        #[cfg(unix)]
        {
            let pinned_directory_metadata = directory.metadata()?;
            let current_directory = dirfd::open_private_directory(&self.directory, self.owner_uid)
                .map_err(|_| ObjectPathError::CaptureChanged)?;
            let current_directory_metadata = current_directory.metadata()?;
            if current_directory_metadata.dev() != pinned_directory_metadata.dev()
                || current_directory_metadata.ino() != pinned_directory_metadata.ino()
            {
                return Err(ObjectPathError::CaptureChanged);
            }
            let name = self
                .capture_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(ObjectPathError::CaptureChanged)?;
            let current = dirfd::regular_entry_identity_at(&directory, name, self.owner_uid)
                .map_err(|_| ObjectPathError::CaptureChanged)?;
            if current.device != file_metadata.dev()
                || current.inode != file_metadata.ino()
                || current.links != 1
                || current.bytes != total
                || initial_metadata.mtime() != file_metadata.mtime()
                || initial_metadata.mtime_nsec() != file_metadata.mtime_nsec()
                || initial_metadata.ctime() != file_metadata.ctime()
                || initial_metadata.ctime_nsec() != file_metadata.ctime_nsec()
            {
                return Err(ObjectPathError::CaptureChanged);
            }
        }
        #[cfg(not(unix))]
        {
            let path_metadata = fs::symlink_metadata(&self.capture_path)?;
            if !path_metadata.is_file() || path_metadata.file_type().is_symlink() {
                return Err(ObjectPathError::CaptureChanged);
            }
        }
        if total != initial_size || file_metadata.len() != total {
            return Err(ObjectPathError::CaptureChanged);
        }
        if expected_size.is_some_and(|size| size != total) {
            return Err(ObjectPathError::CaptureSizeMismatch);
        }
        Ok(total)
    }

    /// Copies one bounded capture into a durable immutable SHA-256 object, or
    /// reuses only an existing object whose bytes match exactly. This does
    /// not create a manifest, reference, or visible commit.
    ///
    /// # Errors
    /// Refuses unsafe paths, mismatched native size/hash, a corrupt existing
    /// object, failed I/O or directory durability. A failure may leave an
    /// unreferenced staged/orphan object for later recovery, never a commit.
    pub fn install_object(
        &self,
        max_bytes: u64,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
    ) -> Result<StoredObject, ObjectPathError> {
        self.install_object_cancellable(max_bytes, expected_size, expected_sha256, || false)
    }

    /// Installs one bounded object while checking an operation-scoped
    /// cancellation predicate between chunks and before publication links.
    /// A cancellation after linking may leave an unreferenced orphan, which
    /// recovery quarantines; it never creates a visible commit.
    pub fn install_object_cancellable(
        &self,
        max_bytes: u64,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<StoredObject, ObjectPathError> {
        self.install_object_with_checkpoint_and_cancel(
            max_bytes,
            expected_size,
            expected_sha256,
            |_| Ok(()),
            cancelled,
        )
    }

    #[cfg(test)]
    fn install_object_with_checkpoint(
        &self,
        max_bytes: u64,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
        checkpoint: impl FnMut(InstallCheckpoint) -> Result<(), ObjectPathError>,
    ) -> Result<StoredObject, ObjectPathError> {
        self.install_object_with_checkpoint_and_cancel(
            max_bytes,
            expected_size,
            expected_sha256,
            checkpoint,
            || false,
        )
    }

    #[cfg(test)]
    fn install_object_with_checkpoint_and_sync(
        &self,
        max_bytes: u64,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
        checkpoint: impl FnMut(InstallCheckpoint) -> Result<(), ObjectPathError>,
        sync_directory: impl FnMut(&File) -> io::Result<()>,
    ) -> Result<StoredObject, ObjectPathError> {
        self.install_object_with_checkpoint_cancel_and_sync(
            max_bytes,
            expected_size,
            expected_sha256,
            checkpoint,
            || false,
            sync_directory,
        )
    }

    fn install_object_with_checkpoint_and_cancel(
        &self,
        max_bytes: u64,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
        checkpoint: impl FnMut(InstallCheckpoint) -> Result<(), ObjectPathError>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<StoredObject, ObjectPathError> {
        self.install_object_with_checkpoint_cancel_and_sync(
            max_bytes,
            expected_size,
            expected_sha256,
            checkpoint,
            cancelled,
            |directory| directory.sync_all(),
        )
    }

    fn install_object_with_checkpoint_cancel_and_sync(
        &self,
        max_bytes: u64,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
        mut checkpoint: impl FnMut(InstallCheckpoint) -> Result<(), ObjectPathError>,
        mut cancelled: impl FnMut() -> bool,
        mut sync_directory: impl FnMut(&File) -> io::Result<()>,
    ) -> Result<StoredObject, ObjectPathError> {
        ensure_supported_object_store_platform()?;
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        let inject_storage_full = inject_storage_full_in_current_archive_install();
        let staging_dir = private_directory(&self.directory, Some(self.owner_uid))?;
        #[cfg(unix)]
        let objects_root_directory =
            dirfd::open_private_directory(&self.objects_root, self.owner_uid)?;
        #[cfg(not(unix))]
        private_directory(&self.objects_root, Some(self.owner_uid))?;
        // INVARIANT: the sole temporary filename is fixed inside this exact
        // operation. A prior interrupted install cannot be overwritten.
        let temporary_path = self.directory.join("object.tmp");
        let mut cleanup = TemporaryObjectCleanup::new(
            temporary_path.clone(),
            &staging_dir,
            "object.tmp",
            self.owner_uid,
        )?;
        #[cfg(unix)]
        let mut temporary =
            dirfd::create_private_regular_file_at(&staging_dir, "object.tmp", self.owner_uid)?;
        #[cfg(not(unix))]
        let mut temporary = {
            let mut options = OpenOptions::new();
            options.write(true).read(true).create_new(true);
            options.open(&temporary_path)?
        };
        cleanup.arm(&temporary)?;
        let mut hasher = Sha256::new();
        let bytes = self.scan_capture(
            max_bytes,
            expected_size,
            |chunk| {
                temporary.write_all(chunk)?;
                #[cfg(all(feature = "native-archive-fault-test", unix))]
                if inject_storage_full {
                    INJECTED_ARCHIVE_INSTALL_STORAGE_FULLS.fetch_add(1, Ordering::SeqCst);
                    return Err(ObjectPathError::Io(io::Error::from_raw_os_error(
                        libc::ENOSPC,
                    )));
                }
                hasher.update(chunk);
                #[cfg(test)]
                checkpoint(InstallCheckpoint::AfterTemporaryChunk)?;
                Ok(())
            },
            &mut cancelled,
        )?;
        if cancelled() {
            return Err(ObjectPathError::Cancelled);
        }
        let sha256: [u8; 32] = hasher.finalize().into();
        if expected_sha256.is_some_and(|expected| expected != sha256) {
            return Err(ObjectPathError::CaptureDigestMismatch);
        }
        checkpoint(InstallCheckpoint::BeforeTemporarySync)?;
        temporary.sync_all()?;
        checkpoint(InstallCheckpoint::AfterTemporarySync)?;
        let digest_hex = sha256_hex(&sha256);
        let shard_name = &digest_hex[..2];
        let shard = self.objects_root.join(shard_name);
        #[cfg(unix)]
        let shard_dir = dirfd::ensure_private_directory_at(
            &objects_root_directory,
            shard_name,
            self.owner_uid,
        )?;
        #[cfg(unix)]
        sync_directory(&objects_root_directory)?;
        #[cfg(not(unix))]
        let shard_dir = {
            ensure_private_directory(&shard, self.owner_uid)?;
            // The shard itself must survive a crash before its child link can
            // be described as durable to the metadata worker.
            private_directory(&self.objects_root, Some(self.owner_uid))?.sync_all()?;
            private_directory(&shard, Some(self.owner_uid))?
        };
        let object_path = shard.join(&digest_hex[2..]);
        if cancelled() {
            return Err(ObjectPathError::Cancelled);
        }
        checkpoint(InstallCheckpoint::BeforeObjectLink)?;
        #[cfg(unix)]
        let link_result = dirfd::link_at(&staging_dir, "object.tmp", &shard_dir, &digest_hex[2..]);
        #[cfg(not(unix))]
        let link_result = fs::hard_link(&temporary_path, &object_path);
        let reused = match link_result {
            Ok(()) => {
                #[cfg(unix)]
                verify_temporary_object_link(
                    &staging_dir,
                    "object.tmp",
                    &shard_dir,
                    &digest_hex[2..],
                    &temporary,
                    self.owner_uid,
                    bytes,
                )?;
                false
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                #[cfg(unix)]
                let mut existing = dirfd::open_private_regular_file_at(
                    &shard_dir,
                    &digest_hex[2..],
                    self.owner_uid,
                )?;
                #[cfg(not(unix))]
                let mut existing = open_private_regular(&object_path, self.owner_uid)?;
                if existing.metadata()?.len() != bytes
                    || !files_equal(&mut temporary, &mut existing, bytes)?
                {
                    return Err(ObjectPathError::ExistingObjectMismatch);
                }
                true
            }
            Err(error) => return Err(error.into()),
        };
        checkpoint(InstallCheckpoint::AfterObjectLink)?;
        #[cfg(unix)]
        unlink_temporary_leaf(&staging_dir, "object.tmp", &temporary, self.owner_uid)?;
        #[cfg(not(unix))]
        fs::remove_file(&temporary_path)?;
        cleanup.disarm();
        checkpoint(InstallCheckpoint::BeforeDirectorySync)?;
        checkpoint(InstallCheckpoint::BeforeStagingDirectorySync)?;
        sync_directory(&staging_dir)?;
        checkpoint(InstallCheckpoint::BeforeShardDirectorySync)?;
        sync_directory(&shard_dir)?;
        Ok(StoredObject {
            path: object_path,
            sha256,
            bytes,
            reused,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstallCheckpoint {
    #[cfg(test)]
    AfterTemporaryChunk,
    BeforeTemporarySync,
    AfterTemporarySync,
    BeforeObjectLink,
    AfterObjectLink,
    BeforeDirectorySync,
    BeforeStagingDirectorySync,
    BeforeShardDirectorySync,
}

struct TemporaryObjectCleanup {
    #[cfg(not(unix))]
    path: PathBuf,
    #[cfg(unix)]
    directory: File,
    #[cfg(unix)]
    name: &'static str,
    #[cfg(unix)]
    owner_uid: u32,
    #[cfg(unix)]
    identity: Option<(u64, u64)>,
    armed: bool,
}

impl TemporaryObjectCleanup {
    fn new(
        path: PathBuf,
        directory: &File,
        name: &'static str,
        owner_uid: u32,
    ) -> Result<Self, ObjectPathError> {
        #[cfg(unix)]
        {
            let _ = path;
            Ok(Self {
                directory: directory.try_clone()?,
                name,
                owner_uid,
                identity: None,
                armed: false,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = (directory, name, owner_uid);
            Ok(Self { path, armed: false })
        }
    }

    fn arm(&mut self, file: &File) -> Result<(), ObjectPathError> {
        #[cfg(unix)]
        {
            let metadata = file.metadata()?;
            self.identity = Some((metadata.dev(), metadata.ino()));
        }
        #[cfg(not(unix))]
        let _ = file;
        self.armed = true;
        Ok(())
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TemporaryObjectCleanup {
    fn drop(&mut self) {
        if self.armed {
            // Cleanup is best-effort. An interrupted install can leave only
            // unreferenced staging state, which recovery must quarantine.
            #[cfg(unix)]
            if let Some((device, inode)) = self.identity {
                let still_owned =
                    dirfd::regular_entry_identity_at(&self.directory, self.name, self.owner_uid)
                        .is_ok_and(|entry| entry.device == device && entry.inode == inode);
                if still_owned {
                    let _ = dirfd::unlink_at(&self.directory, self.name, false);
                }
            }
            #[cfg(not(unix))]
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(unix)]
fn verify_temporary_object_link(
    staging_directory: &File,
    staging_name: &str,
    shard_directory: &File,
    object_name: &str,
    temporary: &File,
    owner_uid: u32,
    expected_bytes: u64,
) -> Result<(), ObjectPathError> {
    // INVARIANT: the digest path must name the same inode whose open
    // descriptor was hashed; during verification both fixed names account
    // for exactly two links to that file.
    let file = temporary.metadata()?;
    let staging = dirfd::regular_entry_identity_at(staging_directory, staging_name, owner_uid)?;
    let object = dirfd::regular_entry_identity_at(shard_directory, object_name, owner_uid)?;
    if file.nlink() != 2
        || file.len() != expected_bytes
        || staging.device != file.dev()
        || staging.inode != file.ino()
        || staging.links != 2
        || staging.bytes != expected_bytes
        || object.device != file.dev()
        || object.inode != file.ino()
        || object.links != 2
        || object.bytes != expected_bytes
    {
        return Err(ObjectPathError::UnsafeEntry);
    }
    Ok(())
}

#[cfg(unix)]
fn unlink_temporary_leaf(
    staging_directory: &File,
    staging_name: &str,
    temporary: &File,
    owner_uid: u32,
) -> Result<(), ObjectPathError> {
    // INVARIANT: never retire a fixed staging name that no longer resolves to
    // this worker's open temporary inode. POSIX still leaves a race between
    // this check and unlinkat against a same-UID writer.
    let file = temporary.metadata()?;
    let staging = dirfd::regular_entry_identity_at(staging_directory, staging_name, owner_uid)?;
    if staging.device != file.dev()
        || staging.inode != file.ino()
        || staging.links != file.nlink()
        || staging.bytes != file.len()
    {
        return Err(ObjectPathError::UnsafeEntry);
    }
    dirfd::unlink_at(staging_directory, staging_name, false)?;
    Ok(())
}

pub(crate) fn sha256_hex(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn files_equal(first: &mut File, second: &mut File, bytes: u64) -> Result<bool, ObjectPathError> {
    first.rewind()?;
    let mut remaining = bytes;
    let mut left = [0u8; 16 * 1024];
    let mut right = [0u8; 16 * 1024];
    while remaining != 0 {
        let length = usize::try_from(remaining.min(left.len() as u64))
            .map_err(|_| ObjectPathError::CaptureTooLarge)?;
        first.read_exact(&mut left[..length])?;
        second.read_exact(&mut right[..length])?;
        if left[..length] != right[..length] {
            return Ok(false);
        }
        remaining -= length as u64;
    }
    Ok(second.read(&mut right[..1])? == 0)
}

#[cfg(not(unix))]
fn open_private_regular(_path: &Path, _owner_uid: u32) -> Result<File, ObjectPathError> {
    Err(ObjectPathError::UnsupportedPlatform)
}

#[cfg(unix)]
fn private_directory(path: &Path, owner_uid: Option<u32>) -> Result<File, ObjectPathError> {
    if let Some(owner_uid) = owner_uid {
        return dirfd::open_private_directory(path, owner_uid);
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ObjectPathError::UnsafeEntry);
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
        return Err(ObjectPathError::UnsafeEntry);
    }
    Ok(file)
}

#[cfg(unix)]
fn ensure_private_directory(path: &Path, owner_uid: u32) -> Result<(), ObjectPathError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.mode(0o700);
            match builder.create(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(error) => return Err(error.into()),
    }
    private_directory(path, Some(owner_uid))?;
    Ok(())
}

#[cfg(not(unix))]
fn private_directory(_path: &Path, _owner_uid: Option<u32>) -> Result<File, ObjectPathError> {
    Err(ObjectPathError::UnsupportedPlatform)
}

#[cfg(all(test, unix, not(any(target_os = "redox", target_os = "espidf"))))]
mod tests {
    use super::*;
    use crate::{RootSpec, StorageRoots};
    use std::{
        collections::BTreeSet,
        io::{BufRead, BufReader, Write},
        os::unix::fs::{PermissionsExt, symlink},
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    fn roots() -> StorageRoots {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        roots
    }

    #[test]
    fn object_path_error_debug_and_display_redact_raw_io_details() {
        let path_sentinel = "/private/profile/versioning/object-path-sentinel";
        let error = ObjectPathError::Io(io::Error::other(path_sentinel.to_owned()));

        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(debug.contains("ObjectPathError::Io"));
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains(path_sentinel));
        assert_eq!(display, "versioning path I/O failed");
        assert!(!display.contains(path_sentinel));
    }

    #[test]
    fn killed_children_at_object_install_checkpoints_recover_unpublished_files() {
        let roots = roots();
        let root = roots.temporary_root.as_ref().expect("temporary root");
        for checkpoint in [
            InstallCheckpoint::BeforeTemporarySync,
            InstallCheckpoint::AfterTemporarySync,
            InstallCheckpoint::BeforeObjectLink,
            InstallCheckpoint::AfterObjectLink,
            InstallCheckpoint::BeforeDirectorySync,
            InstallCheckpoint::BeforeStagingDirectorySync,
            InstallCheckpoint::BeforeShardDirectorySync,
        ] {
            let profile = Uuid::new_v4();
            let operation = OperationId::fresh();
            let mut child = Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "versioning::objects::tests::crash_child_at_object_install_checkpoint",
                    "--nocapture",
                ])
                .env("FERRIC_OBJECT_CRASH_ROOT", root)
                .env("FERRIC_OBJECT_CRASH_PROFILE", profile.to_string())
                .env("FERRIC_OBJECT_CRASH_OPERATION", operation.to_string())
                .env("FERRIC_OBJECT_CRASH_POINT", format!("{checkpoint:?}"))
                .stdout(Stdio::piped())
                .spawn()
                .expect("spawn disposable install child");
            let stdout = child.stdout.take().expect("child stdout");
            let (sender, receiver) = mpsc::channel();
            let reader = thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    if line.is_ok_and(|line| line.contains("FERRIC_OBJECT_CRASH_BOUNDARY")) {
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

            let store = VersioningObjectStore::open(&roots.state, profile).expect("reopen store");
            let digest: [u8; 32] = Sha256::digest(b"abc").into();
            let hex = sha256_hex(&digest);
            let object = store.objects_root().join(&hex[..2]).join(&hex[2..]);
            let temporary = store
                .profile_root
                .join("staging")
                .join(operation.as_uuid().hyphenated().to_string())
                .join("object.tmp");
            let object_linked = matches!(
                checkpoint,
                InstallCheckpoint::AfterObjectLink
                    | InstallCheckpoint::BeforeDirectorySync
                    | InstallCheckpoint::BeforeStagingDirectorySync
                    | InstallCheckpoint::BeforeShardDirectorySync
            );
            assert_eq!(object.exists(), object_linked, "{checkpoint:?}");
            assert_eq!(
                temporary.exists(),
                !matches!(
                    checkpoint,
                    InstallCheckpoint::BeforeDirectorySync
                        | InstallCheckpoint::BeforeStagingDirectorySync
                        | InstallCheckpoint::BeforeShardDirectorySync
                ),
                "{checkpoint:?}"
            );
            if checkpoint == InstallCheckpoint::AfterObjectLink {
                assert_eq!(fs::metadata(&object).expect("orphan link").nlink(), 2);
                assert_eq!(fs::read(&temporary).expect("staging link"), b"abc");
                assert!(matches!(
                    store.quarantine_after_restart(&std::collections::BTreeSet::from([hex])),
                    Err(ObjectPathError::UnsafeEntry)
                ));
                assert!(object.exists());
                assert!(temporary.exists());
            }
            assert_eq!(
                store
                    .quarantine_after_restart(&std::collections::BTreeSet::new())
                    .expect("recover unpublished process-kill state"),
                RecoverySummary {
                    staging_operations: 1,
                    orphan_objects: usize::from(object_linked),
                },
                "{checkpoint:?}"
            );
            assert!(!object.exists());
            assert!(!temporary.exists());
        }
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn crash_child_at_object_install_checkpoint() {
        let Ok(root) = std::env::var("FERRIC_OBJECT_CRASH_ROOT") else {
            return;
        };
        let profile =
            Uuid::parse_str(&std::env::var("FERRIC_OBJECT_CRASH_PROFILE").expect("profile ID"))
                .expect("UUID");
        let operation: OperationId = std::env::var("FERRIC_OBJECT_CRASH_OPERATION")
            .expect("operation ID")
            .parse()
            .expect("typed operation");
        let checkpoint = std::env::var("FERRIC_OBJECT_CRASH_POINT").expect("checkpoint name");
        let roots = StorageRoots::resolve(RootSpec::Base(root.into())).expect("child roots");
        roots.ensure().expect("child roots ready");
        let store = VersioningObjectStore::open(&roots.state, profile).expect("child store");
        let stage = store.begin(operation).expect("child stage");
        fs::write(stage.capture_path(), b"abc").expect("child capture");
        let _ = stage.install_object_with_checkpoint(64, Some(3), None, |point| {
            if format!("{point:?}") == checkpoint {
                println!("FERRIC_OBJECT_CRASH_BOUNDARY");
                std::io::stdout().flush().expect("flush boundary marker");
                loop {
                    thread::park_timeout(Duration::from_secs(60));
                }
            }
            Ok(())
        });
        panic!("child must be killed at the requested boundary");
    }

    #[test]
    fn operation_path_uses_only_typed_ids_and_private_fixed_names() {
        let roots = roots();
        let profile_uuid = Uuid::new_v4();
        let store = VersioningObjectStore::open(&roots.state, profile_uuid).expect("profile paths");
        let operation = OperationId::fresh();
        let staged = store.begin(operation).expect("exclusive staging");
        assert_eq!(
            staged
                .capture_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("capture.bin")
        );
        assert!(staged.directory().starts_with(&roots.state));
        assert!(store.objects_root().starts_with(&roots.state));
        assert_eq!(
            fs::metadata(staged.directory())
                .expect("directory")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(staged.capture_path())
                .expect("capture")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!("operation:../../escape".parse::<OperationId>().is_err());
        assert!(matches!(
            VersioningObjectStore::open(&roots.state, Uuid::nil()),
            Err(ObjectPathError::InvalidProfile)
        ));
        let unrelated = staged.directory().join("unrelated");
        fs::write(&unrelated, b"preserve this file").expect("unrelated file");
        assert!(matches!(
            store.begin(operation),
            Err(ObjectPathError::ExistingOperation)
        ));
        assert_eq!(
            fs::read(&unrelated).expect("unrelated unchanged"),
            b"preserve this file"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn archive_view_materialization_is_bounded_private_verified_and_removable() {
        let roots = roots();
        let profile_uuid = Uuid::new_v4();
        let store = VersioningObjectStore::open(&roots.state, profile_uuid).expect("object store");
        let source = store.begin(OperationId::fresh()).expect("source staging");
        let bytes = b"synthetic archive object for isolated viewing";
        fs::write(source.capture_path(), bytes).expect("synthetic source archive");
        let object = source
            .install_object(MAX_ARCHIVE_BYTES, Some(bytes.len() as u64), None)
            .expect("install exact immutable archive");

        let operation = OperationId::fresh();
        let view = store
            .materialize_archive_view_cancellable(
                operation,
                &object.digest_hex(),
                object.bytes(),
                || false,
            )
            .expect("digest-verified isolated-view copy");
        assert_eq!(view.operation(), operation);
        assert_eq!(
            view.path().file_name().and_then(|name| name.to_str()),
            Some("capture.mhtml")
        );
        assert_eq!(view.bytes(), bytes.len() as u64);
        assert_eq!(fs::read(view.path()).expect("materialized archive"), bytes);
        assert_eq!(
            fs::metadata(view.path())
                .expect("private view file")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        store
            .discard_archive_view(operation)
            .expect("remove exact view staging");
        assert!(!view.path().exists());
        store
            .discard_archive_view(operation)
            .expect("cleanup is idempotent");

        let corrupt_operation = OperationId::fresh();
        let corrupt_stage = store
            .profile_root
            .join("views")
            .join(corrupt_operation.as_uuid().hyphenated().to_string());
        let mut corrupt_bytes = bytes.to_vec();
        corrupt_bytes[0] ^= 1;
        fs::write(object.path(), &corrupt_bytes).expect("corrupt stored object for fixture");
        assert!(matches!(
            store.materialize_archive_view_cancellable(
                corrupt_operation,
                &object.digest_hex(),
                object.bytes(),
                || false,
            ),
            Err(ObjectPathError::ExistingObjectMismatch)
        ));
        assert!(
            !corrupt_stage.exists(),
            "failed digest validation cleans the exact temporary view"
        );

        let missing_operation = OperationId::fresh();
        let missing_stage = store
            .profile_root
            .join("views")
            .join(missing_operation.as_uuid().hyphenated().to_string());
        fs::remove_file(object.path()).expect("remove object for missing-object fixture");
        assert!(matches!(
            store.materialize_archive_view_cancellable(
                missing_operation,
                &object.digest_hex(),
                object.bytes(),
                || false,
            ),
            Err(ObjectPathError::Io(_))
        ));
        assert!(
            !missing_stage.exists(),
            "missing object leaves no isolated-view staging directory"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn retained_archive_view_is_quarantined_after_restart() {
        let roots = roots();
        let profile_uuid = Uuid::new_v4();
        let store = VersioningObjectStore::open(&roots.state, profile_uuid).expect("object store");
        let source = store.begin(OperationId::fresh()).expect("source staging");
        let bytes = b"synthetic retained archive after viewer shutdown";
        fs::write(source.capture_path(), bytes).expect("synthetic source archive");
        let object = source
            .install_object(MAX_ARCHIVE_BYTES, Some(bytes.len() as u64), None)
            .expect("install exact immutable archive");
        let operation = OperationId::fresh();
        let view = store
            .materialize_archive_view_cancellable(
                operation,
                &object.digest_hex(),
                object.bytes(),
                || false,
            )
            .expect("private viewer file");
        let view_path = view.path().to_owned();
        assert_eq!(
            fs::read(&view_path).expect("view retained before restart"),
            bytes
        );
        drop(store);

        let reopened = VersioningObjectStore::open(&roots.state, profile_uuid)
            .expect("reopen after old viewer owner has stopped");
        let recovered = reopened
            .quarantine_after_restart(&BTreeSet::new())
            .expect("quarantine retained staging after restart");
        assert!(recovered.staging_operations >= 1);
        assert!(
            !view_path.exists(),
            "retained viewer path is no longer loadable"
        );
        let batches = fs::read_dir(reopened.profile_root.join("quarantine"))
            .expect("quarantine batches")
            .collect::<Result<Vec<_>, _>>()
            .expect("bounded batch entries");
        assert_eq!(batches.len(), 1, "one restart quarantine batch");
        let quarantined = batches[0]
            .path()
            .join(format!("view-{}", operation.as_uuid().hyphenated()));
        assert_eq!(
            fs::read(quarantined.join("capture.mhtml")).expect("exact retained bytes quarantined"),
            bytes
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn restart_recovery_refuses_substituted_viewer_leaf() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let mut view = store
            .begin_in(&store.profile_root.join("views"), operation)
            .expect("private viewer directory");
        view.use_archive_view_path().expect("exact viewer leaf");
        let path = view.capture_path().to_owned();
        fs::remove_file(&path).expect("remove fixture leaf");
        let outside = roots.data.join("outside-viewer-sentinel");
        fs::write(&outside, b"do not touch outside bytes").expect("sentinel");
        symlink(&outside, &path).expect("substituted viewer leaf");

        assert!(matches!(
            store.quarantine_after_restart(&BTreeSet::new()),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(&outside).expect("outside sentinel retained"),
            b"do not touch outside bytes"
        );
        assert!(
            path.is_symlink(),
            "unsafe viewer leaf is not moved into quarantine"
        );
        fs::remove_file(&path).expect("remove exact fixture symlink");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn live_gc_does_not_move_a_viewer_while_its_copy_is_in_flight() {
        let roots = roots();
        let profile_uuid = Uuid::new_v4();
        let store = VersioningObjectStore::open(&roots.state, profile_uuid).expect("store");
        let source = store.begin(OperationId::fresh()).expect("source stage");
        let bytes = b"synthetic archive copied while live GC runs";
        fs::write(source.capture_path(), bytes).expect("source bytes");
        let object = source
            .install_object(MAX_ARCHIVE_BYTES, Some(bytes.len() as u64), None)
            .expect("referenced source object");
        let digest = object.digest_hex();
        let expected_bytes = object.bytes();
        let operation = OperationId::fresh();
        let state_root = roots.state.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let digest_for_copy = digest.clone();
        let copy = thread::spawn(move || {
            let peer = VersioningObjectStore::open(&state_root, profile_uuid).expect("peer store");
            let mut first_chunk = true;
            peer.materialize_archive_view_cancellable(
                operation,
                &digest_for_copy,
                expected_bytes,
                || {
                    if first_chunk {
                        first_chunk = false;
                        entered_tx.send(()).expect("notify live GC");
                        resume_rx.recv().expect("resume copy");
                    }
                    false
                },
            )
            .expect("copy survives live GC")
        });
        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("copy paused before first read");
        let view_path = store
            .profile_root
            .join("views")
            .join(operation.as_uuid().hyphenated().to_string())
            .join("capture.mhtml");
        assert!(view_path.exists(), "viewer file is allocated before GC");
        let gc_result = store.quarantine_during_live_gc(&BTreeSet::from([digest]));
        let view_survived_gc = view_path.exists();
        resume_tx.send(()).expect("resume copy after GC");
        let view = copy.join().expect("copy worker");
        gc_result.expect("live GC keeps the viewer root outside its sweep");
        assert!(view_survived_gc, "live GC does not move an in-flight view");
        assert_eq!(fs::read(view.path()).expect("completed view"), bytes);
        store
            .discard_archive_view(operation)
            .expect("explicit exact viewer cleanup");
        roots.cleanup().expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn archive_view_rename_refuses_a_replacement_installed_after_identity_check() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let stage = store
            .begin_in(&store.profile_root.join("views"), operation)
            .expect("view staging");
        let directory =
            dirfd::open_private_directory(&stage.directory, store.owner_uid).expect("pinned view");
        let capture =
            dirfd::open_private_regular_file_at(&directory, "capture.bin", store.owner_uid)
                .expect("held original capture");
        let outside = roots.data.join("outside-archive-rename-sentinel");
        let sentinel = b"replacement must be retained";
        fs::write(&outside, sentinel).expect("outside sentinel");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600))
            .expect("private outside sentinel");

        let result =
            rename_capture_to_archive_view_with_hook(&directory, &capture, store.owner_uid, || {
                fs::remove_file(stage.capture_path()).expect("replace reserved leaf");
                fs::hard_link(&outside, stage.capture_path()).expect("install replacement link");
            });
        assert!(matches!(result, Err(ObjectPathError::UnsafeEntry)));
        let moved = stage.directory.join("capture.mhtml");
        assert_eq!(
            fs::read(&moved).expect("moved replacement retained"),
            sentinel
        );
        assert_eq!(
            fs::read(&outside).expect("outside sentinel retained"),
            sentinel
        );

        fs::remove_file(moved).expect("remove moved test link");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn archive_view_copy_refuses_outside_hard_link_without_truncation() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let source = store.begin(OperationId::fresh()).expect("source staging");
        let bytes = b"synthetic archive";
        fs::write(source.capture_path(), bytes).expect("source bytes");
        let object = source
            .install_object(MAX_ARCHIVE_BYTES, Some(bytes.len() as u64), None)
            .expect("installed object");

        let operation = OperationId::fresh();
        let mut stage = store
            .begin_in(&store.profile_root.join("views"), operation)
            .expect("view staging");
        stage.use_archive_view_path().expect("empty archive target");
        fs::remove_file(stage.capture_path()).expect("replace target");
        let outside = roots.data.join("outside-archive-sentinel");
        fs::write(&outside, b"outside bytes must survive").expect("outside sentinel");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600))
            .expect("private outside sentinel");
        fs::hard_link(&outside, stage.capture_path()).expect("substitute hard link");

        assert!(matches!(
            stage.copy_archive_reference(operation, &object.digest_hex(), object.bytes(), || false),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(&outside).expect("outside retained"),
            b"outside bytes must survive"
        );
        assert_eq!(
            fs::read(stage.capture_path()).expect("target retained"),
            b"outside bytes must survive"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn archive_view_transition_sync_failure_withholds_view_and_discards_stage() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let source = store.begin(OperationId::fresh()).expect("source staging");
        let bytes = b"durable archive source";
        fs::write(source.capture_path(), bytes).expect("source bytes");
        let object = source
            .install_object(MAX_ARCHIVE_BYTES, Some(bytes.len() as u64), None)
            .expect("installed object");

        let operation = OperationId::fresh();
        let view_directory = store
            .profile_root
            .join("views")
            .join(operation.as_uuid().hyphenated().to_string());
        let mut sync_calls = 0;
        let result = store.materialize_archive_view_cancellable_with_sync(
            operation,
            &object.digest_hex(),
            object.bytes(),
            || false,
            |_| {
                sync_calls += 1;
                Err(io::Error::other("injected directory sync failure"))
            },
        );

        assert!(matches!(result, Err(ObjectPathError::Io(_))));
        assert_eq!(sync_calls, 1);
        assert!(
            !view_directory.exists(),
            "failed sync must not leave a usable or live view stage"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn archive_view_cleanup_retry_resyncs_parent_after_final_sync_error() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let source = store.begin(OperationId::fresh()).expect("source staging");
        let bytes = b"archive cleanup retry";
        fs::write(source.capture_path(), bytes).expect("source bytes");
        let object = source
            .install_object(MAX_ARCHIVE_BYTES, Some(bytes.len() as u64), None)
            .expect("installed object");

        let operation = OperationId::fresh();
        let view = store
            .materialize_archive_view_cancellable(
                operation,
                &object.digest_hex(),
                object.bytes(),
                || false,
            )
            .expect("materialized archive view");
        let view_directory = view.path().parent().expect("view operation directory");
        assert!(view.path().exists());

        let mut failed_sync_calls = 0;
        let first_attempt = store.discard_archive_view_with_sync(operation, |_| {
            failed_sync_calls += 1;
            if failed_sync_calls == 2 {
                Err(io::Error::other("injected parent directory sync failure"))
            } else {
                Ok(())
            }
        });
        assert!(matches!(first_attempt, Err(ObjectPathError::Io(_))));
        assert_eq!(failed_sync_calls, 2);
        assert!(!view_directory.exists());

        let mut retry_sync_calls = 0;
        store
            .discard_archive_view_with_sync(operation, |_| {
                retry_sync_calls += 1;
                Ok(())
            })
            .expect("idempotent retry syncs the parent");
        assert_eq!(retry_sync_calls, 1);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn existing_symlink_directory_is_rejected_without_touching_target() {
        let roots = roots();
        let outside = roots.data.join("outside");
        fs::create_dir(&outside).expect("outside sentinel directory");
        let sentinel = outside.join("sentinel");
        fs::write(&sentinel, b"untouched").expect("sentinel");
        symlink(&outside, roots.state.join("versioning")).expect("substitute directory");
        assert!(matches!(
            VersioningObjectStore::open(&roots.state, Uuid::new_v4()),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(&sentinel).expect("sentinel still present"),
            b"untouched"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn staging_parent_symlink_cannot_redirect_operation_creation() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staging = store.profile_root.join("staging");
        let moved_staging = store.profile_root.join("staging-moved");
        fs::rename(&staging, &moved_staging).expect("move staging parent");

        let outside = roots.data.join("outside-staging");
        fs::create_dir(&outside).expect("outside directory");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o700))
            .expect("private outside directory");
        let sentinel = outside.join("sentinel");
        fs::write(&sentinel, b"must remain untouched").expect("outside sentinel");
        symlink(&outside, &staging).expect("replace staging path with symlink");

        let operation = OperationId::fresh();
        assert!(matches!(
            store.begin(operation),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(&sentinel).expect("outside sentinel remains"),
            b"must remain untouched"
        );
        assert_eq!(
            fs::read_dir(&outside)
                .expect("outside remains readable")
                .count(),
            1
        );
        assert!(
            !moved_staging
                .join(operation.as_uuid().hyphenated().to_string())
                .exists()
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn window_pages_have_separate_exclusive_bounded_capture_targets() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let first = store.begin(operation).expect("first window page");
        let second = store
            .prepare_window_page(operation, 1)
            .expect("second window page");
        assert_ne!(first.capture_path(), second.capture_path());
        assert!(matches!(
            store.prepare_window_page(operation, 1),
            Err(ObjectPathError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists
        ));
        assert!(matches!(
            store.prepare_window_page(operation, MAX_PAGES_PER_WINDOW_CAPTURE),
            Err(ObjectPathError::UnsafeEntry)
        ));
        fs::write(first.capture_path(), b"first archive").expect("first native save");
        fs::write(second.capture_path(), b"second archive").expect("second native save");
        let first_reopened = store
            .open_staged_window_page(operation, 0)
            .expect("reopen first");
        let second_reopened = store
            .open_staged_window_page(operation, 1)
            .expect("reopen second");
        let first_object = first_reopened
            .install_object(MAX_ARCHIVE_BYTES, Some(13), None)
            .expect("install first independently");
        let second_object = second_reopened
            .install_object(MAX_ARCHIVE_BYTES, Some(14), None)
            .expect("install second independently");
        assert_ne!(first_object.sha256(), second_object.sha256());
        let outside = roots.data.join("outside-window-capture");
        fs::write(&outside, b"outside sentinel").expect("outside sentinel");
        fs::remove_file(second.capture_path()).expect("replace second target");
        symlink(&outside, second.capture_path()).expect("substitute second target");
        assert!(matches!(
            store.open_staged_window_page(operation, 1),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(
            fs::read(&outside).expect("outside retained"),
            b"outside sentinel"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn cancelled_first_window_page_can_stage_next_without_trusting_a_replacement() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let operation = OperationId::fresh();
        let first = store.begin(operation).expect("first window page");
        fs::remove_file(first.capture_path()).expect("Qt cancellation removes first leaf");
        let second = store
            .prepare_window_page(operation, 1)
            .expect("private operation directory still admits second page");
        assert!(!first.capture_path().exists());
        assert!(second.capture_path().is_file());

        let outside = roots.data.join("outside-window-page");
        fs::write(&outside, b"untouched").expect("outside sentinel");
        symlink(&outside, first.capture_path()).expect("substitute cancelled first leaf");
        assert!(matches!(
            store.prepare_window_page(operation, 2),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(fs::read(&outside).expect("outside retained"), b"untouched");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn substituted_capture_symlink_and_loose_mode_are_refused() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        let outside = roots.data.join("outside");
        fs::write(&outside, b"unrelated").expect("outside file");
        fs::remove_file(staged.capture_path()).expect("replace synthetic capture");
        symlink(&outside, staged.capture_path()).expect("substitute symlink");
        assert!(matches!(
            staged.open_capture(),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(fs::read(&outside).expect("outside unchanged"), b"unrelated");
        fs::remove_file(staged.capture_path()).expect("remove symlink");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(staged.capture_path())
            .expect("recreate capture");
        file.write_all(b"fixture").expect("fixture bytes");
        fs::set_permissions(staged.capture_path(), fs::Permissions::from_mode(0o644))
            .expect("loosen mode");
        assert!(matches!(
            staged.open_capture(),
            Err(ObjectPathError::UnsafeEntry)
        ));
        fs::set_permissions(staged.capture_path(), fs::Permissions::from_mode(0o600))
            .expect("restore private mode");
        let hardlink = roots.data.join("capture-hardlink");
        fs::hard_link(staged.capture_path(), &hardlink).expect("hardlink fixture");
        assert!(matches!(
            staged.open_capture(),
            Err(ObjectPathError::UnsafeEntry)
        ));
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn bounded_capture_stream_rejects_empty_and_oversized_files() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        let mut observed = Vec::new();
        assert!(matches!(
            staged.stream_capture(8, None, |chunk| observed.extend_from_slice(chunk)),
            Err(ObjectPathError::EmptyCapture)
        ));
        fs::write(staged.capture_path(), b"123456789").expect("oversized capture");
        assert!(matches!(
            staged.stream_capture(8, None, |chunk| observed.extend_from_slice(chunk)),
            Err(ObjectPathError::CaptureTooLarge)
        ));
        assert!(
            observed.is_empty(),
            "known oversize must not reach the sink"
        );
        fs::write(staged.capture_path(), b"12345678").expect("bounded capture");
        assert_eq!(
            staged
                .stream_capture(8, None, |chunk| observed.extend_from_slice(chunk))
                .expect("exact cap accepted"),
            8
        );
        assert_eq!(observed, b"12345678");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn bounded_capture_stream_refuses_native_size_mismatch_before_consuming_bytes() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        fs::write(staged.capture_path(), b"capture").expect("capture bytes");
        let mut observed = Vec::new();
        assert!(matches!(
            staged.stream_capture(64, Some(8), |chunk| observed.extend_from_slice(chunk)),
            Err(ObjectPathError::CaptureSizeMismatch)
        ));
        assert!(observed.is_empty());
        assert_eq!(
            staged
                .stream_capture(64, Some(7), |chunk| observed.extend_from_slice(chunk))
                .expect("exact native size accepted"),
            7
        );
        assert_eq!(observed, b"capture");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn bounded_capture_stream_detects_a_file_change_during_inspection() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        fs::write(staged.capture_path(), b"before").expect("capture bytes");
        let mut injected = false;
        assert!(matches!(
            staged.stream_capture(64, None, |_| {
                if !injected {
                    fs::write(staged.capture_path(), b"x").expect("simulate interrupted write");
                    injected = true;
                }
            }),
            Err(ObjectPathError::CaptureChanged)
        ));
        assert!(injected);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn bounded_capture_stream_detects_same_inode_same_size_rewrite() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        let original = vec![b'a'; 64 * 1024];
        fs::write(staged.capture_path(), &original).expect("capture bytes");
        let original_metadata = fs::metadata(staged.capture_path()).expect("initial metadata");
        let mut injected = false;

        assert!(matches!(
            staged.stream_capture(128 * 1024, None, |_| {
                if !injected {
                    fs::write(staged.capture_path(), vec![b'b'; original.len()])
                        .expect("rewrite capture in place");
                    // Do not assume a timestamp tick is 1 ns (or even 1 ms).
                    // Wait until this filesystem actually reports the rewrite,
                    // bounded so unsupported/coarse metadata cannot hang CI.
                    let deadline = Instant::now() + Duration::from_secs(2);
                    let mut timestamp_changed = false;
                    while Instant::now() < deadline {
                        let current = fs::metadata(staged.capture_path())
                            .expect("rewritten capture metadata");
                        if current.mtime() != original_metadata.mtime()
                            || current.mtime_nsec() != original_metadata.mtime_nsec()
                            || current.ctime() != original_metadata.ctime()
                            || current.ctime_nsec() != original_metadata.ctime_nsec()
                        {
                            timestamp_changed = true;
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    assert!(
                        timestamp_changed,
                        "filesystem did not expose changed mtime/ctime within the bound"
                    );
                    injected = true;
                }
            }),
            Err(ObjectPathError::CaptureChanged)
        ));
        assert!(injected);
        let rewritten_metadata = fs::metadata(staged.capture_path()).expect("rewritten metadata");
        assert_eq!(rewritten_metadata.ino(), original_metadata.ino());
        assert_eq!(rewritten_metadata.len(), original_metadata.len());
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn bounded_capture_stream_detects_path_substitution_without_following_it() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        fs::write(staged.capture_path(), b"capture").expect("capture bytes");
        let outside = roots.data.join("outside-capture");
        fs::write(&outside, b"untouched").expect("outside sentinel");
        let moved = staged.directory().join("old-capture");
        let mut injected = false;
        assert!(matches!(
            staged.stream_capture(64, None, |_| {
                if !injected {
                    fs::rename(staged.capture_path(), &moved).expect("move capture");
                    symlink(&outside, staged.capture_path()).expect("replace path");
                    injected = true;
                }
            }),
            Err(ObjectPathError::CaptureChanged)
        ));
        assert!(injected);
        assert_eq!(fs::read(&outside).expect("outside unchanged"), b"untouched");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn bounded_capture_stream_detects_operation_parent_replacement() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        let expected = b"pinned capture bytes";
        fs::write(staged.capture_path(), expected).expect("capture bytes");
        let outside = roots.data.join("outside-operation");
        fs::create_dir(&outside).expect("outside directory");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o700))
            .expect("private outside directory");
        let sentinel = outside.join("capture.bin");
        fs::write(&sentinel, b"outside sentinel bytes").expect("outside sentinel");
        let moved = staged.directory().with_extension("moved");
        let mut observed = Vec::new();
        let mut replaced = false;

        assert!(matches!(
            staged.stream_capture(64, None, |chunk| {
                observed.extend_from_slice(chunk);
                if !replaced {
                    fs::rename(staged.directory(), &moved).expect("move pinned operation");
                    symlink(&outside, staged.directory()).expect("replace operation path");
                    replaced = true;
                }
            }),
            Err(ObjectPathError::CaptureChanged)
        ));
        assert!(replaced);
        assert_eq!(observed.as_slice(), &expected[..]);
        assert_eq!(
            fs::read(&sentinel).expect("outside sentinel remains"),
            b"outside sentinel bytes"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn bounded_capture_stream_uses_fixed_chunks_across_a_larger_capture() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        fs::write(staged.capture_path(), vec![b'x'; 40_000]).expect("capture bytes");
        let mut chunks = 0;
        let mut observed = 0;
        assert_eq!(
            staged
                .stream_capture(40_000, None, |chunk| {
                    assert!(!chunk.is_empty() && chunk.len() <= 16 * 1024);
                    assert!(chunk.iter().all(|byte| *byte == b'x'));
                    chunks += 1;
                    observed += chunk.len();
                })
                .expect("exact cap accepted"),
            40_000
        );
        assert_eq!(observed, 40_000);
        assert!(chunks > 1);
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn identical_captures_install_one_verified_immutable_object() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let first = store.begin(OperationId::fresh()).expect("first stage");
        fs::write(first.capture_path(), b"abc").expect("first capture");
        let installed = first
            .install_object(
                64,
                Some(3),
                Some([
                    0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d,
                    0xae, 0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10,
                    0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
                ]),
            )
            .expect("first object");
        assert_eq!(
            installed.digest_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(installed.bytes(), 3);
        assert!(!installed.reused());
        assert_eq!(fs::read(installed.path()).expect("immutable bytes"), b"abc");
        store
            .verify_object_reference(&installed.digest_hex(), 3, 64)
            .expect("durable reference verified");
        assert!(matches!(
            store.verify_object_reference("../../escape", 3, 64),
            Err(ObjectPathError::InvalidObjectDigest)
        ));
        assert_eq!(
            fs::metadata(installed.path())
                .expect("object metadata")
                .nlink(),
            1
        );

        let second = store.begin(OperationId::fresh()).expect("second stage");
        fs::write(second.capture_path(), b"abc").expect("second capture");
        let reused = second
            .install_object(64, Some(3), None)
            .expect("reuse exact object");
        assert!(reused.reused());
        assert_eq!(reused.path(), installed.path());
        assert_eq!(
            fs::read(installed.path()).expect("object unchanged"),
            b"abc"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn object_install_refuses_a_replaced_temporary_leaf() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        let source_bytes = b"verified source object";
        let replacement_bytes = b"different leaf contents";
        fs::write(staged.capture_path(), source_bytes).expect("capture bytes");
        let temporary = staged.directory().join("object.tmp");
        let mut replaced = false;

        let result = staged.install_object_with_checkpoint(
            128,
            Some(source_bytes.len() as u64),
            None,
            |checkpoint| {
                if checkpoint == InstallCheckpoint::BeforeObjectLink && !replaced {
                    fs::remove_file(&temporary).expect("replace temporary leaf");
                    fs::write(&temporary, replacement_bytes).expect("write replacement leaf");
                    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
                        .expect("make replacement private");
                    replaced = true;
                }
                Ok(())
            },
        );
        assert!(replaced);
        assert!(matches!(result, Err(ObjectPathError::UnsafeEntry)));

        let digest: [u8; 32] = Sha256::digest(source_bytes).into();
        let digest_hex = sha256_hex(&digest);
        let object = store
            .objects_root()
            .join(&digest_hex[..2])
            .join(&digest_hex[2..]);
        assert_eq!(
            fs::read(&temporary).expect("replacement is retained"),
            replacement_bytes
        );
        assert_eq!(
            fs::read(&object).expect("unpublished object leaf"),
            replacement_bytes
        );
        fs::remove_file(&temporary).expect("remove synthetic replacement");
        fs::remove_file(&object).expect("remove unpublished object link");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn object_install_does_not_unlink_a_leaf_replaced_after_link() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let staged = store.begin(OperationId::fresh()).expect("stage");
        let source_bytes = b"verified source object";
        let replacement_bytes = b"different leaf contents";
        fs::write(staged.capture_path(), source_bytes).expect("capture bytes");
        let temporary = staged.directory().join("object.tmp");
        let mut replaced = false;

        let result = staged.install_object_with_checkpoint(
            128,
            Some(source_bytes.len() as u64),
            None,
            |checkpoint| {
                if checkpoint == InstallCheckpoint::AfterObjectLink && !replaced {
                    fs::remove_file(&temporary).expect("replace linked temporary leaf");
                    fs::write(&temporary, replacement_bytes).expect("write replacement leaf");
                    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
                        .expect("make replacement private");
                    replaced = true;
                }
                Ok(())
            },
        );
        assert!(replaced);
        assert!(matches!(result, Err(ObjectPathError::UnsafeEntry)));

        let digest: [u8; 32] = Sha256::digest(source_bytes).into();
        let digest_hex = sha256_hex(&digest);
        let object = store
            .objects_root()
            .join(&digest_hex[..2])
            .join(&digest_hex[2..]);
        assert_eq!(
            fs::read(&temporary).expect("replacement is retained"),
            replacement_bytes
        );
        assert_eq!(
            fs::read(&object).expect("verified object remains unpublished"),
            source_bytes
        );
        fs::remove_file(&temporary).expect("remove synthetic replacement");
        fs::remove_file(&object).expect("remove unpublished object link");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn sidecar_bytes_install_as_a_distinct_bounded_immutable_object() {
        let roots = roots();
        let profile = Uuid::new_v4();
        let store = VersioningObjectStore::open(&roots.state, profile).expect("store");
        let operation = OperationId::fresh();
        let stage = store.begin(operation).expect("stage");
        fs::write(stage.capture_path(), b"archive object").expect("archive bytes");
        let sidecar = br#"{"schema_version":1}"#;
        let object = store
            .install_sidecar_bytes_cancellable(operation, sidecar, || false)
            .expect("sidecar object");
        assert_eq!(object.bytes(), sidecar.len() as u64);
        assert_eq!(fs::read(object.path()).expect("sidecar bytes"), sidecar);
        assert!(
            store
                .verify_object_reference(
                    &object.digest_hex(),
                    object.bytes(),
                    MAX_PAGE_SIDECAR_BYTES as u64,
                )
                .is_ok()
        );
        assert!(matches!(
            store.install_sidecar_bytes_cancellable(operation, &[], || false),
            Err(ObjectPathError::EmptyCapture)
        ));
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn mismatching_existing_object_and_symlink_never_get_overwritten() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let shard = store.objects_root().join("ba");
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        builder.create(&shard).expect("object shard");
        let object_path =
            shard.join("7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"abc").expect("capture");
        let mut wrong = OpenOptions::new();
        wrong.write(true).create_new(true).mode(0o600);
        wrong
            .open(&object_path)
            .expect("existing object")
            .write_all(b"xyz")
            .expect("wrong bytes");
        assert!(matches!(
            stage.install_object(64, Some(3), None),
            Err(ObjectPathError::ExistingObjectMismatch)
        ));
        assert_eq!(fs::read(&object_path).expect("unchanged bytes"), b"xyz");
        assert!(matches!(
            store.verify_object_reference(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                3,
                64
            ),
            Err(ObjectPathError::ExistingObjectMismatch)
        ));
        fs::remove_file(&object_path).expect("remove synthetic object");
        let outside = roots.data.join("outside-object");
        fs::write(&outside, b"unrelated").expect("outside sentinel");
        symlink(&outside, &object_path).expect("substitute object symlink");
        assert!(matches!(
            stage.install_object(64, Some(3), None),
            Err(ObjectPathError::UnsafeEntry)
        ));
        assert_eq!(fs::read(&outside).expect("outside unchanged"), b"unrelated");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn expected_digest_mismatch_refuses_object_installation() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"abc").expect("capture");
        assert!(matches!(
            stage.install_object(64, Some(3), Some([0; 32])),
            Err(ObjectPathError::CaptureDigestMismatch)
        ));
        assert_eq!(
            fs::read_dir(store.objects_root())
                .expect("object root")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn incomplete_native_capture_never_installs_an_object() {
        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        fs::write(stage.capture_path(), b"partial").expect("incomplete native capture");
        assert!(matches!(
            stage.install_object(64, Some(8), None),
            Err(ObjectPathError::CaptureSizeMismatch)
        ));
        assert!(!stage.directory().join("object.tmp").exists());
        assert_eq!(
            fs::read_dir(store.objects_root())
                .expect("object root")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn cancellation_during_chunked_hashing_leaves_no_installed_object() {
        use std::cell::Cell;

        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        let bytes = vec![b'x'; 64 * 1024];
        fs::write(stage.capture_path(), &bytes).expect("synthetic capture");
        let checks = Cell::new(0usize);
        assert!(matches!(
            stage.install_object_cancellable(128 * 1024, Some(bytes.len() as u64), None, || {
                let check = checks.get();
                checks.set(check + 1);
                check >= 1
            }),
            Err(ObjectPathError::Cancelled)
        ));
        assert!(!stage.directory().join("object.tmp").exists());
        assert_eq!(
            fs::read_dir(store.objects_root())
                .expect("object root")
                .count(),
            0
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn interrupted_install_never_returns_success_and_orphans_are_recoverable() {
        use std::collections::BTreeSet;

        for fault in [
            InstallCheckpoint::BeforeTemporarySync,
            InstallCheckpoint::AfterTemporarySync,
            InstallCheckpoint::BeforeObjectLink,
            InstallCheckpoint::AfterObjectLink,
            InstallCheckpoint::BeforeDirectorySync,
        ] {
            let roots = roots();
            let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
            let stage = store.begin(OperationId::fresh()).expect("stage");
            fs::write(stage.capture_path(), b"abc").expect("capture");
            assert!(matches!(
                stage.install_object_with_checkpoint(64, Some(3), None, |point| {
                    if point == fault {
                        Err(ObjectPathError::Io(io::Error::other(
                            "injected install interruption",
                        )))
                    } else {
                        Ok(())
                    }
                }),
                Err(ObjectPathError::Io(_))
            ));
            assert!(!stage.directory().join("object.tmp").exists());
            let recovered = store
                .quarantine_after_restart(&BTreeSet::new())
                .expect("quarantine interrupted operation");
            assert_eq!(recovered.staging_operations, 1);
            assert_eq!(
                recovered.orphan_objects,
                usize::from(matches!(
                    fault,
                    InstallCheckpoint::AfterObjectLink | InstallCheckpoint::BeforeDirectorySync
                ))
            );
            roots.cleanup().expect("cleanup");
        }
    }

    #[test]
    fn exhausted_space_during_chunked_object_copy_leaves_no_installed_object() {
        use std::collections::BTreeSet;

        let roots = roots();
        let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
        let stage = store.begin(OperationId::fresh()).expect("stage");
        let capture = vec![0x5a; 32 * 1024];
        fs::write(stage.capture_path(), &capture).expect("bounded staged capture");
        let mut written_chunks = 0;
        let result = stage.install_object_with_checkpoint(
            64 * 1024,
            Some(capture.len() as u64),
            None,
            |point| {
                if point == InstallCheckpoint::AfterTemporaryChunk {
                    written_chunks += 1;
                    return Err(ObjectPathError::Io(io::Error::from_raw_os_error(
                        libc::ENOSPC,
                    )));
                }
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(ObjectPathError::Io(error)) if error.raw_os_error() == Some(libc::ENOSPC)
        ));
        assert_eq!(written_chunks, 1, "failure follows one bounded write");
        assert!(!stage.directory().join("object.tmp").exists());
        assert_eq!(
            fs::read_dir(store.objects_root())
                .expect("object root")
                .count(),
            0
        );
        let recovered = store
            .quarantine_after_restart(&BTreeSet::new())
            .expect("quarantine failed staging operation");
        assert_eq!(recovered.staging_operations, 1);
        assert_eq!(recovered.orphan_objects, 0);
        roots.cleanup().expect("cleanup");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn kernel_file_size_limit_during_object_copy_recovers_exact_stage() {
        let roots = roots();
        let root = roots.temporary_root.as_ref().expect("temporary root");
        let profile = Uuid::new_v4();
        let operation = OperationId::fresh();
        let capture = vec![0x5a; 32 * 1024];
        let store = VersioningObjectStore::open(&roots.state, profile).expect("store");
        let stage = store.begin(operation).expect("stage");
        fs::write(stage.capture_path(), &capture).expect("staged capture");
        File::open(stage.capture_path())
            .expect("open staged capture")
            .sync_all()
            .expect("sync staged capture");
        drop(stage);
        drop(store);

        let child = Command::new("bash")
            .args([
                "-c",
                "ulimit -f 8 || exit 90; trap '' XFSZ || exit 91; exec \"$@\"",
                "ferric-object-limit",
            ])
            .arg(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "versioning::objects::tests::child_object_copy_under_kernel_file_limit",
                "--nocapture",
            ])
            .env("FERRIC_OBJECT_LIMIT_ROOT", root)
            .env("FERRIC_OBJECT_LIMIT_PROFILE", profile.to_string())
            .env("FERRIC_OBJECT_LIMIT_OPERATION", operation.to_string())
            .output()
            .expect("run disposable size-limited child");
        assert!(
            child.status.success(),
            "size-limited child failed: stdout={:?}; stderr={:?}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        assert!(
            String::from_utf8_lossy(&child.stdout).contains("FERRIC_OBJECT_KERNEL_LIMIT_VERIFIED"),
            "the exact child test must reach its kernel error assertion"
        );

        let store = VersioningObjectStore::open(&roots.state, profile).expect("reopen store");
        assert_eq!(
            fs::read_dir(store.objects_root())
                .expect("object root")
                .count(),
            0,
            "a short write cannot install an immutable object"
        );
        assert!(
            !store
                .profile_root
                .join("staging")
                .join(operation.as_uuid().hyphenated().to_string())
                .join("object.tmp")
                .exists(),
            "failed copy must remove its temporary object"
        );
        assert_eq!(
            store
                .quarantine_after_restart(&BTreeSet::new())
                .expect("recover failed copy"),
            RecoverySummary {
                staging_operations: 1,
                orphan_objects: 0,
            }
        );
        let batch = fs::read_dir(store.profile_root.join("quarantine"))
            .expect("quarantine")
            .next()
            .expect("one batch")
            .expect("private batch");
        let retained = batch.path().join(format!(
            "stage-{}/capture.bin",
            operation.as_uuid().hyphenated()
        ));
        assert_eq!(fs::read(retained).expect("retained staged bytes"), capture);
        assert_eq!(
            store
                .quarantine_after_restart(&BTreeSet::new())
                .expect("idempotent recovery"),
            RecoverySummary::default()
        );
        roots.cleanup().expect("cleanup");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn child_object_copy_under_kernel_file_limit() {
        let Ok(root) = std::env::var("FERRIC_OBJECT_LIMIT_ROOT") else {
            return;
        };
        let profile =
            Uuid::parse_str(&std::env::var("FERRIC_OBJECT_LIMIT_PROFILE").expect("profile"))
                .expect("profile UUID");
        let operation: OperationId = std::env::var("FERRIC_OBJECT_LIMIT_OPERATION")
            .expect("operation")
            .parse()
            .expect("operation ID");
        let roots = StorageRoots::resolve(RootSpec::Base(root.into())).expect("child roots");
        roots.ensure().expect("private child roots");
        let store = VersioningObjectStore::open(&roots.state, profile).expect("child store");
        let stage = store.open_staged(operation).expect("exact existing stage");

        // The parent launched only this disposable child with a low RLIMIT_FSIZE
        // and SIGXFSZ ignored, so the kernel reports EFBIG on the short copy.
        assert!(matches!(
            stage.install_object(64 * 1024, Some(32 * 1024), None),
            Err(ObjectPathError::Io(error)) if error.raw_os_error() == Some(libc::EFBIG)
        ));
        assert!(!stage.directory().join("object.tmp").exists());
        println!("FERRIC_OBJECT_KERNEL_LIMIT_VERIFIED");
    }

    #[test]
    fn failed_directory_sync_after_object_link_withholds_receipt_and_recovers_orphan() {
        use std::collections::BTreeSet;

        for (failed_sync, sync_call) in [("staging-directory", 2usize), ("object-shard", 3)] {
            let roots = roots();
            let store = VersioningObjectStore::open(&roots.state, Uuid::new_v4()).expect("store");
            let stage = store.begin(OperationId::fresh()).expect("stage");
            fs::write(stage.capture_path(), b"abc").expect("capture");
            let mut sync_calls = 0usize;
            let result = stage.install_object_with_checkpoint_and_sync(
                64,
                Some(3),
                None,
                |_| Ok(()),
                |directory| {
                    sync_calls += 1;
                    if sync_calls == sync_call {
                        Err(io::Error::from_raw_os_error(libc::EIO))
                    } else {
                        directory.sync_all()
                    }
                },
            );
            assert_eq!(sync_calls, sync_call, "failure at {failed_sync}");
            assert!(matches!(
                result,
                Err(ObjectPathError::Io(error)) if error.raw_os_error() == Some(libc::EIO)
            ));
            assert!(!stage.directory().join("object.tmp").exists());
            let recovered = store
                .quarantine_after_restart(&BTreeSet::new())
                .expect("quarantine unacknowledged object");
            assert_eq!(recovered.staging_operations, 1, "{failed_sync}");
            assert_eq!(recovered.orphan_objects, 1, "{failed_sync}");
            roots.cleanup().expect("cleanup");
        }
    }
}

#[cfg(all(test, any(not(unix), target_os = "redox", target_os = "espidf")))]
mod unsupported_platform_mutator_tests {
    use super::*;

    #[test]
    fn unsupported_store_mutators_refuse_without_filesystem_mutation() {
        let root = std::env::temp_dir().join(format!(
            "ferric-unsupported-store-mutation-{}",
            Uuid::new_v4()
        ));
        let store = VersioningObjectStore {
            profile_root: root.join("profile"),
            profile_uuid: Uuid::new_v4(),
            owner_uid: 0,
        };
        let operation = OperationId::fresh();

        assert!(matches!(
            store.begin(operation),
            Err(ObjectPathError::UnsupportedPlatform)
        ));
        assert!(matches!(
            store.prepare_window_page(operation, 1),
            Err(ObjectPathError::UnsupportedPlatform)
        ));
        assert!(matches!(
            store.install_sidecar_bytes_cancellable(operation, b"sidecar", || false),
            Err(ObjectPathError::UnsupportedPlatform)
        ));

        let stage = StagedCapture {
            directory: root.join("stage"),
            capture_path: root.join("stage/capture.bin"),
            objects_root: root.join("objects"),
            owner_uid: 0,
        };
        assert!(matches!(
            stage.install_object_cancellable(1024, None, None, || false),
            Err(ObjectPathError::UnsupportedPlatform)
        ));
        assert!(!root.exists());
    }

    #[test]
    fn archive_view_transition_refuses_unsupported_platform_without_mutation() {
        let directory = std::env::temp_dir().join(format!(
            "ferric-unsupported-archive-view-{}",
            Uuid::new_v4()
        ));
        let capture_path = directory.join("capture.bin");
        let mut stage = StagedCapture {
            directory: directory.clone(),
            capture_path: capture_path.clone(),
            objects_root: directory.join("objects"),
            owner_uid: 0,
        };

        assert!(matches!(
            stage.use_archive_view_path(),
            Err(ObjectPathError::UnsupportedPlatform)
        ));
        assert_eq!(stage.capture_path(), capture_path);
        assert!(!directory.exists());
    }
}
