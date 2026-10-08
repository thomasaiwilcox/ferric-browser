mod capability;
mod capture;
mod commands;
mod diff;
mod error;
mod identity;
pub mod limits;
mod merge;
mod model;
mod page_sidecar;
mod routing;

pub use capability::{
    CapabilityFact, CapabilityFactError, CapabilityFingerprint, CapabilityKey,
    CapabilityRequirement, CapabilityScope, EvidenceId, PrimitiveGate, ProductionGate,
    RequirementAvailability, SafeReasonCode,
};
pub use capture::{
    CaptureField, CaptureUnavailable, Consistency, Coverage, CoverageBreakdown, CoverageError,
    CoverageKind, Fidelity, FidelityLabel, Identity, PageCaptureInput, Persistence, Representation,
    Retention, SafePageUrl, ScrollPoint, StructuralCaptureError, StructuralPage,
    StructuralSnapshot, StructuralWindow, project_structural_scope,
};
pub use commands::{
    DiffKind, ResolutionChoice, RestoreInto, RestoreMode, VersioningCommand,
    VersioningCommandError, VersioningRef, VersioningScope, command_capabilities,
};
pub use diff::{
    FieldAvailability, StructuralChange, StructuralDiffError, StructuralDiffEvent,
    StructuralDiffPage, diff_structural, diff_structural_kind,
};
pub use error::{SensitiveCategory, VersioningFailure, VersioningReason};
pub use identity::{
    ApplicationOwnerId, BranchId, CommitId, LogicalPageId, LogicalWindowId, OperationId,
    ProfileInstanceId, RetainedViewId, StashId, VersioningIdError,
};
pub use limits::{VersioningLimitError, VersioningLimits};
pub use merge::{
    AbsenceReason, AttributePresence, BackendAttributeName, CookieExpiry, CookieKey,
    CookieKeyInput, CookieMergeDecision, CookieMergeError, CookieMergeItem, CookieMergePlan,
    CookieMergeUnavailable, CookieModelError, CookieObservation, CookieRecord, CookieRecordInput,
    CookieScope, CookieSnapshot, CookieSnapshotMeta, IdentityProvenance, PartitionIdentity,
    RecordFidelity, SameSite, SnapshotCompleteness, SnapshotConsistency, SnapshotIdentityFidelity,
    plan_cookie_merge,
};
pub use model::{
    BranchError, BranchLifecycle, BranchName, BranchRecord, CapturedOwner, CapturedScope,
    InheritanceReport, InheritanceRequest, NativeProfileOwner, RetainedLifecycle, RetentionRecord,
    ScopeError, ScopeSelector, VersioningEvent, VersioningState, ViewMembership, resolve_scope,
};
pub use page_sidecar::{
    ExclusionReason, GeometryPlanError, GeometryRestorePlan, MAX_PAGE_SIDECAR_BYTES,
    PAGE_STATE_HELPER_VERSION, PageStateSidecar, SidecarCategory, SidecarError,
    SidecarQualification, SidecarSummary, capture_fingerprint_for_request, page_route_for_url,
    route_fingerprint_for_url,
};
pub use routing::{VersioningEngineEffect, VersioningRequest};
