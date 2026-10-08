//! Explicit capture metadata, separate from schema-2 session recovery.

use ferric_browser_core::versioning::{
    MAX_PAGE_SIDECAR_BYTES,
    limits::{MAX_ARCHIVE_BYTES, MAX_PAGES_PER_WINDOW_CAPTURE},
};

pub(crate) mod journal;
mod manifest;
#[cfg(target_os = "linux")]
mod media_fingerprint;
pub mod objects;
mod retention;
pub(crate) mod schema;

#[cfg(target_os = "linux")]
pub(crate) use media_fingerprint::delete_all_marked as delete_marked_media_fingerprint_keys;
#[cfg(target_os = "linux")]
pub use media_fingerprint::{MediaFingerprintKey, MediaFingerprintKeyError};

/// Maximum file plus manifest bytes reserved before an explicit page save.
pub const MAX_CAPTURE_RESERVATION_BYTES: u64 =
    MAX_ARCHIVE_BYTES + MAX_PAGE_SIDECAR_BYTES as u64 + manifest::MAX_MANIFEST_BYTES as u64;

/// Upper bound on an explicit multi-page window reservation. The profile's
/// lower disk-budget limit still rejects windows too large to reserve.
pub const MAX_WINDOW_CAPTURE_RESERVATION_BYTES: u64 =
    MAX_CAPTURE_RESERVATION_BYTES * MAX_PAGES_PER_WINDOW_CAPTURE as u64;

pub use objects::{
    ArchiveViewFile, CaptureStageTarget, InstalledObject, VersionedSidecarBytes,
    VersioningObjectCommand, VersioningObjectCompletion, VersioningObjectFailure,
    VersioningObjectWorker, VersioningObjectWorkerError,
};
#[cfg(feature = "native-archive-fault-test")]
pub use objects::{
    archive_install_paused, arm_one_archive_cleanup_refusal, arm_one_archive_install_pause,
    arm_one_sidecar_install_pause, arm_one_sidecar_install_refusal,
    injected_archive_cleanup_refusals, injected_sidecar_install_refusals, resume_archive_install,
    resume_sidecar_install, sidecar_install_paused, sidecar_permission_denials,
};
#[cfg(all(feature = "native-archive-fault-test", unix))]
pub use objects::{
    arm_archive_install_storage_full_after, arm_one_archive_install_storage_full,
    injected_archive_install_storage_fulls,
};

pub use journal::{CommitArchiveFailure, CommitInfo, CommitSummary, CoverageCounts};
pub use manifest::{
    ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1, CaptureCategory, CaptureInterval,
    CaptureMonotonicInterval, CaptureRequest, InvalidContextMember, InvalidContextMemberReason,
    ManifestCategoryCoverage, ManifestCoverage, ManifestError, ManifestFidelity, ManifestHeader,
    ManifestMutationEvidence, ManifestObjectRef, ManifestObjectRole, ManifestPage,
    ManifestPageArchiveFailure, ManifestPageCapture, ManifestPageCategory, ManifestPolicySnapshot,
    ManifestSafeUrl, ManifestScope, ManifestScroll, ManifestUnavailable, ManifestWindow,
    VersionedCaptureManifest,
};
pub use retention::{CommitDeletePreview, CommitDeleteReceipt, VersioningGcReport};
