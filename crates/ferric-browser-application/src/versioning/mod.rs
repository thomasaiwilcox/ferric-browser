//! Application-owned, bounded versioning operation protocol. No engine or
//! persistence write is performed by this module.

mod archive_view;
mod branch;
mod capture;
mod capture_consent;
mod cookie_seed;
mod cookies;
mod coordinator;
mod diff;
mod policy;
mod projection;
mod restore;
mod routing;
mod stash;

pub use archive_view::VersionedArchiveViewMetadata;
pub(crate) use archive_view::{
    ActiveArchiveView, PendingArchiveViewManifest, PendingArchiveViewObject,
};
pub use branch::{
    BranchStructuralExportPreview, CleanBranchForkEvent, CleanBranchLeaseError,
    CleanBranchProfileLeases, CleanBranchSwitchTarget, CleanBranchWindowPage,
    CleanBranchWindowPlan,
};
pub(crate) use branch::{
    PendingBranchStructuralExport, PendingCleanBranchFork, PublishedCleanBranch,
};
pub use capture::{
    CommitEvent, CommitReadEvent, ContextWindowCaptureCandidate, FederatedOwnerEffects,
    PreparedContextCandidateCommit, PreparedStructuralCommit,
};
pub(crate) use capture::{CommitReadKind, PendingCommit, PendingCommitDelete};
pub use capture_consent::{PageCaptureAuthorization, PageCaptureConsent, PageCaptureProvenance};
pub use cookie_seed::{
    CookieDestinationRead, CookieSeedDestination, CookieSeedDispatchError, CookieSeedError,
    CookieSeedOutcome, CookieSeedPlan, CookieSeedReadError, seed_prepared_destination,
};
pub use cookies::{
    CookieBackendCapabilities, CookieBackendError, CookieMirrorChange, CookieMirrorEvent,
    CookieMirrorInvalidation, CookieObservationMirror, CookieObservationSource,
    CookieReadProjection, CookieReadResult, ExclusiveApplyCapability, IdentityCapability,
    ReadBackCapability, ReadInterval, SnapshotCapability, WriteCapability,
};
pub use coordinator::{
    CallbackDisposition, OperationEffect, OperationEnvelope, OperationError, OperationKind,
    OperationState, PrivacyPolicy, SensitiveOperation, StageResult, StageTicket,
    VersioningCoordinator,
};
pub use diff::CommitDiffEvent;
pub(crate) use diff::PendingStructuralDiff;
pub use policy::{
    ALLOWLISTED_POLICY_SCHEMA_SHA256, PolicyRestoreEvent, PolicyRestorePreview, PolicySettingChange,
};
pub use projection::{
    AuthorizedPreview, ConfirmationPermit, NativeDecisionProof, NativeDecisionSink,
    PageCaptureFailurePolicy, PlanId, Preview, PreviewChoice, PreviewCounts, PreviewDraft,
    PreviewError, PreviewKind, RedactedPreview, VersioningConsentError,
};
pub(crate) use projection::{PreviewBook, PreviewContext};
pub use restore::{
    AppliedNewWindowRestore, AppliedPristineWindowRestore, NewWindowRestorePlan,
    PlannedRestorePage, RestoreNavigation, RestorePlanEvent, RestoreSidecarEvent,
};
pub(crate) use restore::{
    PendingLazyRestorePage, PendingRestorePlan, PendingRestoreSidecar, PendingRestoreSidecarIntent,
    StoredRestoreSidecar,
};
pub(crate) use stash::PendingStash;
pub use stash::{
    Activity, ActivityKind, EditableState, PreservationIntent, StashAbort, StashAssessment,
    StashBlocker, StashError, StashObservation, StashPreparation, StashStage, VersioningEffects,
    assess_preservation,
};
