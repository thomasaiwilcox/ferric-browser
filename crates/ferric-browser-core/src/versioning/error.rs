use std::fmt;

use serde::Serialize;

/// Stable, payload-free reasons for a versioning operation to fail.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum VersioningReason {
    VersioningDisabled,
    UnsupportedCapability,
    UnqualifiedRuntime,
    StaleOwner,
    DocumentChanged,
    WrongProfile,
    PrivatePersistenceDenied,
    ConfirmationRequired,
    ConfirmationExpired,
    AmbiguousReference,
    Busy,
    QuotaExceeded,
    CaptureTimeout,
    CaptureInterrupted,
    UnsafeRestoreDescriptor,
    ArchiveIsolationUnavailable,
    RetentionUnavailable,
    RetainedRendererLost,
    CookieIdentityLossy,
    CookieSnapshotIncomplete,
    CookieSeedUnverified,
    MergeStale,
    MergeConflict,
    MergeApplyUnavailable,
    MergePartial,
    MergeOutcomeUnknown,
    ObjectCorrupt,
    SensitiveCaptureDenied,
}

impl VersioningReason {
    pub const ALL: [Self; 28] = [
        Self::VersioningDisabled,
        Self::UnsupportedCapability,
        Self::UnqualifiedRuntime,
        Self::StaleOwner,
        Self::DocumentChanged,
        Self::WrongProfile,
        Self::PrivatePersistenceDenied,
        Self::ConfirmationRequired,
        Self::ConfirmationExpired,
        Self::AmbiguousReference,
        Self::Busy,
        Self::QuotaExceeded,
        Self::CaptureTimeout,
        Self::CaptureInterrupted,
        Self::UnsafeRestoreDescriptor,
        Self::ArchiveIsolationUnavailable,
        Self::RetentionUnavailable,
        Self::RetainedRendererLost,
        Self::CookieIdentityLossy,
        Self::CookieSnapshotIncomplete,
        Self::CookieSeedUnverified,
        Self::MergeStale,
        Self::MergeConflict,
        Self::MergeApplyUnavailable,
        Self::MergePartial,
        Self::MergeOutcomeUnknown,
        Self::ObjectCorrupt,
        Self::SensitiveCaptureDenied,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VersioningDisabled => "versioning-disabled",
            Self::UnsupportedCapability => "unsupported-capability",
            Self::UnqualifiedRuntime => "unqualified-runtime",
            Self::StaleOwner => "stale-owner",
            Self::DocumentChanged => "document-changed",
            Self::WrongProfile => "wrong-profile",
            Self::PrivatePersistenceDenied => "private-persistence-denied",
            Self::ConfirmationRequired => "confirmation-required",
            Self::ConfirmationExpired => "confirmation-expired",
            Self::AmbiguousReference => "ambiguous-reference",
            Self::Busy => "busy",
            Self::QuotaExceeded => "quota-exceeded",
            Self::CaptureTimeout => "capture-timeout",
            Self::CaptureInterrupted => "capture-interrupted",
            Self::UnsafeRestoreDescriptor => "unsafe-restore-descriptor",
            Self::ArchiveIsolationUnavailable => "archive-isolation-unavailable",
            Self::RetentionUnavailable => "retention-unavailable",
            Self::RetainedRendererLost => "retained-renderer-lost",
            Self::CookieIdentityLossy => "cookie-identity-lossy",
            Self::CookieSnapshotIncomplete => "cookie-snapshot-incomplete",
            Self::CookieSeedUnverified => "cookie-seed-unverified",
            Self::MergeStale => "merge-stale",
            Self::MergeConflict => "merge-conflict",
            Self::MergeApplyUnavailable => "merge-apply-unavailable",
            Self::MergePartial => "merge-partial",
            Self::MergeOutcomeUnknown => "merge-outcome-unknown",
            Self::ObjectCorrupt => "object-corrupt",
            Self::SensitiveCaptureDenied => "sensitive-capture-denied",
        }
    }
}

/// A fixed category; no page, form, profile, or cookie content is accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SensitiveCategory {
    None,
    Cookie,
    Editable,
    Page,
    Profile,
    Credential,
    ExternalAction,
}

impl SensitiveCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Cookie => "cookie",
            Self::Editable => "editable",
            Self::Page => "page",
            Self::Profile => "profile",
            Self::Credential => "credential",
            Self::ExternalAction => "external-action",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct FailureProjection {
    pub reason_code: &'static str,
    pub category: &'static str,
}

/// The error itself deliberately has no field capable of carrying raw data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersioningFailure {
    reason: VersioningReason,
    category: SensitiveCategory,
}

impl VersioningFailure {
    #[must_use]
    pub const fn new(reason: VersioningReason, category: SensitiveCategory) -> Self {
        Self { reason, category }
    }

    #[must_use]
    pub const fn reason(self) -> VersioningReason {
        self.reason
    }

    #[must_use]
    pub const fn projection(self) -> FailureProjection {
        FailureProjection {
            reason_code: self.reason.as_str(),
            category: self.category.as_str(),
        }
    }
}

impl fmt::Display for VersioningFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Versioning operation could not be completed")
    }
}

impl std::error::Error for VersioningFailure {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_and_category_projection_carries_no_payload() {
        let failure = VersioningFailure::new(
            VersioningReason::CookieIdentityLossy,
            SensitiveCategory::Cookie,
        );
        let projection = failure.projection();
        assert_eq!(projection.reason_code, "cookie-identity-lossy");
        assert_eq!(projection.category, "cookie");
        assert!(!format!("{failure:?}").contains("value"));
        assert!(!failure.to_string().contains("cookie="));
    }

    #[test]
    fn all_reasons_have_stable_unique_codes() {
        let mut codes = std::collections::HashSet::new();
        for reason in VersioningReason::ALL {
            assert!(codes.insert(reason.as_str()));
        }
        assert_eq!(codes.len(), 28);
    }
}
