//! Versioned explicit-capture metadata. This module does not write a file or
//! alter recovery checkpoints; publication is a separate worker protocol.

use std::{collections::BTreeSet, fmt};

use ferric_browser_core::{
    CaptureField, CaptureUnavailable, CommitId, LogicalPageId, LogicalWindowId, SafePageUrl,
    StructuralSnapshot, VersioningReason, versioning::MAX_PAGE_SIDECAR_BYTES,
};
use serde::{Deserialize, Serialize};
use uuid::{Uuid, Variant, Version};

pub const MANIFEST_SCHEMA_VERSION: u32 = 3;
const OLDEST_READABLE_MANIFEST_SCHEMA_VERSION: u32 = 1;
/// Fixed archive representation emitted by the current public Qt `WebEngine`
/// save path. Unknown and missing backend IDs remain readable as metadata but
/// are never eligible for archive viewing.
pub const ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1: &str = "qt-webengine-mhtml-v1";
pub const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_WINDOWS: usize = 256;
const MAX_CONTEXT_MEMBERS: usize = 256;
const MAX_CONTEXT_MEMBERS_U16: u16 = 256;
const MAX_PAGES: usize = 5_000;
const MAX_PAGES_PER_WINDOW: usize = 128;
const MAX_MESSAGE_BYTES: usize = 1_024;
const MAX_URL_BYTES: usize = 16 * 1024;
const MAX_TITLE_BYTES: usize = 4 * 1024;
const MAX_CONTEXT_BYTES: usize = 256;
const MAX_WARNINGS: usize = 64;
const MAX_OBJECT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SCROLL_HUNDREDTHS: u32 = 100_000_000;
const MAX_CAPTURE_INTERVAL_MS: u64 = 5 * 60 * 1_000;

#[derive(Deserialize)]
struct SchemaProbe {
    schema_version: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestError {
    Malformed,
    TooLarge,
    LegacySchema(u32),
    NewerSchema(u32),
    Invalid(&'static str),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => formatter.write_str("capture manifest is malformed"),
            Self::TooLarge => formatter.write_str("capture manifest exceeds its byte limit"),
            Self::LegacySchema(version) => write!(
                formatter,
                "capture manifest schema {version} is older than supported"
            ),
            Self::NewerSchema(version) => write!(
                formatter,
                "capture manifest schema {version} is newer than supported"
            ),
            Self::Invalid(field) => write!(formatter, "capture manifest has an invalid {field}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Admission data supplied by the application after it has selected a normal
/// profile and captured an exact reducer revision. It is not a storage write.
#[derive(Clone, Eq, PartialEq)]
pub struct ManifestHeader {
    pub id: CommitId,
    pub profile_uuid: Uuid,
    pub created_at_ms: u64,
    pub message: String,
    pub parent_commit: Option<CommitId>,
    pub configuration_revision: u64,
    pub scope: ManifestScope,
    pub capture_interval: CaptureInterval,
    pub engine_fingerprint: String,
    pub helper_bundle_version: String,
}

impl fmt::Debug for ManifestHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManifestHeader")
            .field("id", &"[redacted]")
            .field("profile_uuid", &"[redacted]")
            .field("message", &"[redacted]")
            .field("parent_commit", &self.parent_commit.map(|_| "[redacted]"))
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestScope {
    Tab,
    Window,
    Context,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureCategory {
    Structural,
    Archive,
    Page,
    Cookies,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureRequest {
    pub categories: BTreeSet<CaptureCategory>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_partial_pages: bool,
}

impl CaptureRequest {
    #[must_use]
    pub fn structural_only() -> Self {
        Self {
            categories: BTreeSet::from([CaptureCategory::Structural]),
            allow_partial_pages: false,
        }
    }

    #[must_use]
    pub fn contains(&self, category: CaptureCategory) -> bool {
        self.categories.contains(&category)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureInterval {
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
}

/// A capture-local monotonic interval. Offsets are measured from the page
/// capture operation's `Instant` origin; they are not wall-clock timestamps.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureMonotonicInterval {
    pub started_offset_ms: u64,
    pub finished_offset_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestCoverage {
    Unknown,
    NotRequested,
    Unavailable,
    Partial,
    CompleteForRequest,
}

/// Payload-free reason why one requested page has no archive in an expressly
/// partial window capture. A failed page is never represented as a saved
/// archive, even when another page in the same commit succeeded.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestPageArchiveFailure {
    Cancelled,
    Interrupted,
    EmptyArchive,
    NativeViewLost,
    DocumentChanged,
    Timeout,
    StorageFull,
    InvalidArchive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestFidelity {
    pub structural: bool,
    pub archive: ManifestCoverage,
    pub page: ManifestCoverage,
    pub cookies: ManifestCoverage,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestPageCategory {
    Geometry,
    Controls,
    Editable,
    Selection,
    Media,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestCategoryCoverage {
    pub category: ManifestPageCategory,
    pub requested: u16,
    pub captured: u16,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestMutationEvidence {
    NotObserved,
    ObservedStable,
    ObservedChanged,
}

/// Bounded evidence for the independent archive and semantic-page intervals.
/// It is not a certificate that a dynamic DOM was captured atomically.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestPageCapture {
    pub archive_interval: Option<CaptureInterval>,
    pub sidecar_interval: Option<CaptureInterval>,
    #[serde(default)]
    pub archive_monotonic_interval: Option<CaptureMonotonicInterval>,
    #[serde(default)]
    pub sidecar_monotonic_interval: Option<CaptureMonotonicInterval>,
    pub mutation_evidence: ManifestMutationEvidence,
    pub categories: Vec<ManifestCategoryCoverage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestUnavailable {
    NotObserved,
    Unsupported,
    UnsafeNavigation,
}

impl From<CaptureUnavailable> for ManifestUnavailable {
    fn from(value: CaptureUnavailable) -> Self {
        match value {
            CaptureUnavailable::NotObserved => Self::NotObserved,
            CaptureUnavailable::Unsupported => Self::Unsupported,
            CaptureUnavailable::UnsafeNavigation => Self::UnsafeNavigation,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "kebab-case")]
pub enum ManifestSafeUrl {
    Replayable(String),
    Placeholder(ManifestUnavailable),
}

impl fmt::Debug for ManifestSafeUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Replayable(_) => formatter.write_str("Replayable([redacted])"),
            Self::Placeholder(reason) => {
                formatter.debug_tuple("Placeholder").field(reason).finish()
            }
        }
    }
}

impl From<&SafePageUrl> for ManifestSafeUrl {
    fn from(value: &SafePageUrl) -> Self {
        value.replay_url().map_or_else(
            || {
                Self::Placeholder(
                    value
                        .unavailable_reason()
                        .unwrap_or(CaptureUnavailable::NotObserved)
                        .into(),
                )
            },
            |url| Self::Replayable(url.to_owned()),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "kebab-case")]
pub enum ManifestScroll {
    Known {
        x_hundredths: u32,
        y_hundredths: u32,
    },
    Absent,
    Unavailable(ManifestUnavailable),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestObjectRole {
    Archive,
    Sidecar,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestObjectRef {
    pub role: ManifestObjectRole,
    pub sha256: String,
    pub bytes: u64,
    /// Optional only for compatibility with older schema-1 archive objects.
    /// Unknown bounded IDs are preserved but refused by the archive viewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_backend: Option<String>,
}

impl fmt::Debug for ManifestObjectRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManifestObjectRef")
            .field("role", &self.role)
            .field("sha256", &"[content digest]")
            .field("bytes", &self.bytes)
            .field(
                "archive_backend",
                &self.archive_backend.as_ref().map(|backend| {
                    if backend == ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1 {
                        ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1
                    } else {
                        "[redacted]"
                    }
                }),
            )
            .finish()
    }
}

impl ManifestObjectRef {
    #[must_use]
    pub fn supports_archive_view(&self) -> bool {
        self.role == ManifestObjectRole::Archive
            && self.archive_backend.as_deref() == Some(ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1)
    }
}

/// Only allowlisted, declarative values may be added here. Structural-only
/// conversion leaves this absent; the policy slice owns its collection.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestPolicySnapshot {
    pub schema_sha256: String,
    pub javascript_enabled: bool,
}

impl fmt::Debug for ManifestPolicySnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManifestPolicySnapshot")
            .field("schema_sha256", &"[redacted]")
            .field("javascript_enabled", &self.javascript_enabled)
            .finish()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestPage {
    pub logical_page_id: String,
    pub lineage_parent: Option<String>,
    pub safe_url_or_placeholder: ManifestSafeUrl,
    pub title_redacted: Option<String>,
    pub pinned: bool,
    pub muted: bool,
    pub zoom_hundredths: u16,
    pub document_scroll: ManifestScroll,
    pub archive_object: Option<ManifestObjectRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_failure: Option<ManifestPageArchiveFailure>,
    pub sidecar_object: Option<ManifestObjectRef>,
    pub fidelity: ManifestFidelity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_capture: Option<ManifestPageCapture>,
}

impl fmt::Debug for ManifestPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManifestPage")
            .field("logical_page_id", &"[redacted]")
            .field("safe_url_or_placeholder", &"[redacted]")
            .field(
                "title_redacted",
                &self.title_redacted.as_ref().map(|_| "[redacted]"),
            )
            .field("fidelity", &self.fidelity)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestWindow {
    pub logical_window_id: String,
    pub context_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_interval: Option<CaptureInterval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_member_ordinal: Option<u16>,
    pub selected_logical_page: Option<String>,
    pub pages: Vec<ManifestPage>,
}

impl fmt::Debug for ManifestWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManifestWindow")
            .field("logical_window_id", &"[redacted]")
            .field(
                "context_ref",
                &self.context_ref.as_ref().map(|_| "[redacted]"),
            )
            .field("has_capture_interval", &self.capture_interval.is_some())
            .field("context_member_ordinal", &self.context_member_ordinal)
            .field(
                "selected_logical_page",
                &self.selected_logical_page.as_ref().map(|_| "[redacted]"),
            )
            .field("page_count", &self.pages.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InvalidContextMemberReason {
    InvalidWindowIdentity,
    DuplicateWindow,
    ClosedWindow,
    WrongProfile,
    ChangedMembership,
    NoLivePages,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvalidContextMember {
    pub ordinal: u16,
    pub reason: InvalidContextMemberReason,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedCaptureManifest {
    pub schema_version: u32,
    pub id: String,
    pub profile_uuid: Uuid,
    pub created_at_ms: u64,
    pub message: String,
    pub parent_commit: Option<String>,
    pub application_revision: u64,
    pub configuration_revision: u64,
    pub scope: ManifestScope,
    pub requested_capture: CaptureRequest,
    pub capture_interval: CaptureInterval,
    pub windows: Vec<ManifestWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_member_count: Option<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invalid_context_members: Vec<InvalidContextMember>,
    pub policy_snapshot: Option<ManifestPolicySnapshot>,
    pub engine_fingerprint: String,
    pub helper_bundle_version: String,
    pub warnings: Vec<String>,
}

impl fmt::Debug for VersionedCaptureManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VersionedCaptureManifest")
            .field("schema_version", &self.schema_version)
            .field("id", &"[redacted]")
            .field("profile_uuid", &"[redacted]")
            .field("window_count", &self.windows.len())
            .field(
                "page_count",
                &self
                    .windows
                    .iter()
                    .map(|window| window.pages.len())
                    .sum::<usize>(),
            )
            .field("context_member_count", &self.context_member_count)
            .field(
                "invalid_context_member_count",
                &self.invalid_context_members.len(),
            )
            .field("warning_count", &self.warnings.len())
            .finish_non_exhaustive()
    }
}

impl VersionedCaptureManifest {
    /// Converts only a safe structural projection. No page body, cookies,
    /// title, policy value or object reference is inferred from it.
    ///
    /// # Errors
    /// Rejects an invalid header or structural projection.
    pub fn from_structural(
        header: ManifestHeader,
        structural: &StructuralSnapshot,
    ) -> Result<Self, ManifestError> {
        if structural.windows.len() > MAX_WINDOWS
            || structural
                .windows
                .iter()
                .any(|window| window.pages.len() > MAX_PAGES_PER_WINDOW)
            || structural
                .windows
                .iter()
                .map(|window| window.pages.len())
                .sum::<usize>()
                > MAX_PAGES
        {
            return Err(ManifestError::Invalid("structural size"));
        }
        let windows = structural
            .windows
            .iter()
            .map(|window| ManifestWindow {
                logical_window_id: window.logical_id.to_string(),
                context_ref: window.context_ref.clone(),
                capture_interval: None,
                context_member_ordinal: None,
                selected_logical_page: window.selected_page.map(|id| id.to_string()),
                pages: window
                    .pages
                    .iter()
                    .map(|page| ManifestPage {
                        logical_page_id: page.logical_id.to_string(),
                        lineage_parent: page.lineage_parent.map(|id| id.to_string()),
                        safe_url_or_placeholder: ManifestSafeUrl::from(&page.url),
                        title_redacted: None,
                        pinned: page.pinned,
                        muted: page.muted,
                        zoom_hundredths: page.zoom_hundredths,
                        document_scroll: match page.scroll {
                            CaptureField::Known(point) => ManifestScroll::Known {
                                x_hundredths: point.x_hundredths,
                                y_hundredths: point.y_hundredths,
                            },
                            CaptureField::Absent => ManifestScroll::Absent,
                            CaptureField::Unavailable(reason) => {
                                ManifestScroll::Unavailable(reason.into())
                            }
                        },
                        archive_object: None,
                        archive_failure: None,
                        sidecar_object: None,
                        fidelity: ManifestFidelity {
                            structural: true,
                            archive: ManifestCoverage::NotRequested,
                            page: ManifestCoverage::NotRequested,
                            cookies: ManifestCoverage::NotRequested,
                        },
                        page_capture: None,
                    })
                    .collect(),
            })
            .collect();
        let manifest = Self {
            schema_version: MANIFEST_SCHEMA_VERSION,
            id: header.id.to_string(),
            profile_uuid: header.profile_uuid,
            created_at_ms: header.created_at_ms,
            message: header.message,
            parent_commit: header.parent_commit.map(|id| id.to_string()),
            application_revision: structural.application_revision,
            configuration_revision: header.configuration_revision,
            scope: header.scope,
            requested_capture: CaptureRequest::structural_only(),
            capture_interval: header.capture_interval,
            windows,
            context_member_count: None,
            invalid_context_members: Vec::new(),
            policy_snapshot: None,
            engine_fingerprint: header.engine_fingerprint,
            helper_bundle_version: header.helper_bundle_version,
            warnings: Vec::new(),
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Parses bounded current-schema bytes, with explicit non-writing
    /// migrations from schemas 1 and 2; this never runs recovery or writes a file.
    /// Unknown fields and newer schemas fail closed.
    ///
    /// # Errors
    /// Rejects malformed, oversized, unsupported or invalid manifests.
    pub fn decode(bytes: &[u8]) -> Result<Self, ManifestError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ManifestError::TooLarge);
        }
        let schema: SchemaProbe =
            serde_json::from_slice(bytes).map_err(|_| ManifestError::Malformed)?;
        if schema.schema_version < OLDEST_READABLE_MANIFEST_SCHEMA_VERSION {
            return Err(ManifestError::LegacySchema(schema.schema_version));
        }
        if schema.schema_version > MANIFEST_SCHEMA_VERSION {
            return Err(ManifestError::NewerSchema(schema.schema_version));
        }
        let mut manifest: Self =
            serde_json::from_slice(bytes).map_err(|_| ManifestError::Malformed)?;
        if schema.schema_version == 1 {
            manifest.migrate_schema_1_to_2();
        }
        if manifest.schema_version == 2 {
            manifest.migrate_schema_2_to_3();
        }
        // Older readable captures can predate interval provenance. Retain
        // their metadata without allowing the same shape to be published anew.
        manifest.validate_readable()?;
        if schema.schema_version == MANIFEST_SCHEMA_VERSION {
            manifest.validate_capture_outcomes()?;
        }
        Ok(manifest)
    }

    /// Schema 1 did not define archive-backend identity. Preserve its object
    /// references but never trust a schema-2 backend claim found in old bytes.
    fn migrate_schema_1_to_2(&mut self) {
        for page in self
            .windows
            .iter_mut()
            .flat_map(|window| window.pages.iter_mut())
        {
            if let Some(archive) = &mut page.archive_object {
                archive.archive_backend = None;
            }
        }
        self.schema_version = 2;
    }

    /// Schema 2 had no page-local archive failures or partial-page policy.
    /// Ignore those fields even if an old-schema document tried to supply them.
    fn migrate_schema_2_to_3(&mut self) {
        self.requested_capture.allow_partial_pages = false;
        for page in self
            .windows
            .iter_mut()
            .flat_map(|window| window.pages.iter_mut())
        {
            page.archive_failure = None;
        }
        self.schema_version = MANIFEST_SCHEMA_VERSION;
    }

    /// Encodes a validated manifest to bounded canonical JSON bytes.
    ///
    /// # Errors
    /// Rejects invalid or oversized metadata; no partial bytes escape.
    pub fn encode(&self) -> Result<Vec<u8>, ManifestError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| ManifestError::Malformed)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ManifestError::TooLarge);
        }
        Ok(bytes)
    }

    /// Validates every untrusted field and cross-reference before publication.
    ///
    /// # Errors
    /// Returns a payload-free reason for a malformed or unsupported manifest.
    pub fn validate(&self) -> Result<(), ManifestError> {
        self.validate_readable()?;
        self.validate_capture_outcomes()?;
        for page in self.windows.iter().flat_map(|window| &window.pages) {
            let evidence = page.page_capture.as_ref();
            if page.fidelity.page == ManifestCoverage::CompleteForRequest
                && evidence.is_some_and(|facts| facts.categories.is_empty())
            {
                return Err(ManifestError::Invalid("page capture completeness"));
            }
            if page
                .archive_object
                .as_ref()
                .is_some_and(ManifestObjectRef::supports_archive_view)
                && evidence.and_then(|facts| facts.archive_interval).is_none()
            {
                return Err(ManifestError::Invalid("archive capture interval"));
            }
            if page.sidecar_object.is_some()
                && evidence.and_then(|facts| facts.sidecar_interval).is_none()
            {
                return Err(ManifestError::Invalid("page capture interval"));
            }
        }
        Ok(())
    }

    fn validate_readable(&self) -> Result<(), ManifestError> {
        self.validate_metadata()?;
        self.validate_scope()?;
        self.validate_warnings()?;
        self.validate_windows()
    }

    fn validate_metadata(&self) -> Result<(), ManifestError> {
        if self.schema_version < MANIFEST_SCHEMA_VERSION {
            return Err(ManifestError::LegacySchema(self.schema_version));
        }
        if self.schema_version > MANIFEST_SCHEMA_VERSION {
            return Err(ManifestError::NewerSchema(self.schema_version));
        }
        self.id
            .parse::<CommitId>()
            .map_err(|_| ManifestError::Invalid("commit id"))?;
        if self
            .parent_commit
            .as_deref()
            .is_some_and(|parent| parent.parse::<CommitId>().is_err() || parent == self.id)
        {
            return Err(ManifestError::Invalid("parent commit"));
        }
        if self.profile_uuid.get_variant() != Variant::RFC4122
            || self.profile_uuid.get_version() != Some(Version::Random)
        {
            return Err(ManifestError::Invalid("owner profile"));
        }
        if self.created_at_ms == 0
            || self.capture_interval.started_at_ms == 0
            || self.capture_interval.finished_at_ms < self.capture_interval.started_at_ms
            || self.capture_interval.finished_at_ms - self.capture_interval.started_at_ms
                > MAX_CAPTURE_INTERVAL_MS
            || self.created_at_ms < self.capture_interval.finished_at_ms
        {
            return Err(ManifestError::Invalid("capture interval"));
        }
        if !bounded_text(&self.message, MAX_MESSAGE_BYTES, false) {
            return Err(ManifestError::Invalid("message"));
        }
        if !bounded_ascii(&self.engine_fingerprint, 256)
            || !bounded_ascii(&self.helper_bundle_version, 64)
        {
            return Err(ManifestError::Invalid("backend version"));
        }
        Ok(())
    }

    fn validate_scope(&self) -> Result<(), ManifestError> {
        if !self.requested_capture.contains(CaptureCategory::Structural)
            || self.windows.is_empty()
            || self.windows.len() > MAX_WINDOWS
        {
            return Err(ManifestError::Invalid("scope"));
        }
        if matches!(self.scope, ManifestScope::Tab | ManifestScope::Window)
            && self.windows.len() != 1
        {
            return Err(ManifestError::Invalid("scope"));
        }
        if self.invalid_context_members.len() > MAX_CONTEXT_MEMBERS {
            return Err(ManifestError::Invalid("context membership"));
        }
        match (self.scope, self.context_member_count) {
            (ManifestScope::Context, Some(member_count)) => {
                if !(1..=MAX_CONTEXT_MEMBERS_U16).contains(&member_count) {
                    return Err(ManifestError::Invalid("context membership"));
                }
                let mut ordinals = BTreeSet::new();
                let mut context_refs = BTreeSet::new();
                for window in &self.windows {
                    let ordinal = window
                        .context_member_ordinal
                        .filter(|ordinal| *ordinal < member_count)
                        .ok_or(ManifestError::Invalid("context membership"))?;
                    if !ordinals.insert(ordinal)
                        || window.capture_interval.is_none()
                        || window.context_ref.is_none()
                    {
                        return Err(ManifestError::Invalid("context membership"));
                    }
                    if let Some(context_ref) = &window.context_ref {
                        context_refs.insert(context_ref.as_str());
                    }
                }
                for invalid in &self.invalid_context_members {
                    if invalid.ordinal >= member_count || !ordinals.insert(invalid.ordinal) {
                        return Err(ManifestError::Invalid("context membership"));
                    }
                }
                if ordinals.len() != usize::from(member_count) || context_refs.len() != 1 {
                    return Err(ManifestError::Invalid("context membership"));
                }
            }
            (ManifestScope::Context, None)
                if !self.invalid_context_members.is_empty()
                    || self.windows.iter().any(|window| {
                        window.context_member_ordinal.is_some() || window.capture_interval.is_some()
                    }) =>
            {
                return Err(ManifestError::Invalid("context membership"));
            }
            (ManifestScope::Tab | ManifestScope::Window, Some(_) | None)
                if !self.invalid_context_members.is_empty()
                    || self
                        .windows
                        .iter()
                        .any(|window| window.context_member_ordinal.is_some()) =>
            {
                return Err(ManifestError::Invalid("context membership"));
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_warnings(&self) -> Result<(), ManifestError> {
        if let Some(policy) = &self.policy_snapshot
            && !valid_sha256(&policy.schema_sha256)
        {
            return Err(ManifestError::Invalid("policy schema hash"));
        }
        if self.warnings.len() > MAX_WARNINGS {
            return Err(ManifestError::Invalid("warnings"));
        }
        let mut warning_set = BTreeSet::new();
        for warning in &self.warnings {
            if !VersioningReason::ALL
                .iter()
                .any(|reason| reason.as_str() == warning)
                || !warning_set.insert(warning)
            {
                return Err(ManifestError::Invalid("warning code"));
            }
        }
        Ok(())
    }

    fn validate_capture_outcomes(&self) -> Result<(), ManifestError> {
        if self.requested_capture.allow_partial_pages
            && (self.scope != ManifestScope::Window
                || !self.requested_capture.contains(CaptureCategory::Archive))
        {
            return Err(ManifestError::Invalid("partial page policy"));
        }
        if !self.requested_capture.contains(CaptureCategory::Archive) {
            return Ok(());
        }
        let pages = self.windows.iter().flat_map(|window| &window.pages);
        let mut successful = 0usize;
        let mut failed = 0usize;
        for page in pages {
            if page.archive_object.is_some() {
                if page.fidelity.archive != ManifestCoverage::CompleteForRequest {
                    return Err(ManifestError::Invalid("archive page result"));
                }
                successful += 1;
            } else if page.archive_failure.is_some() {
                failed += 1;
            } else {
                return Err(ManifestError::Invalid("archive page result"));
            }
        }
        if successful == 0
            || (failed > 0
                && (self.scope != ManifestScope::Window
                    || !self.requested_capture.allow_partial_pages))
        {
            return Err(ManifestError::Invalid("partial page policy"));
        }
        Ok(())
    }

    fn validate_windows(&self) -> Result<(), ManifestError> {
        let mut windows = BTreeSet::new();
        let mut pages = BTreeSet::new();
        for window in &self.windows {
            let window_id = window
                .logical_window_id
                .parse::<LogicalWindowId>()
                .map_err(|_| ManifestError::Invalid("logical window id"))?;
            if !windows.insert(window_id) || window.pages.len() > MAX_PAGES_PER_WINDOW {
                return Err(ManifestError::Invalid("window structure"));
            }
            if window
                .context_ref
                .as_deref()
                .is_some_and(|value| !bounded_text(value, MAX_CONTEXT_BYTES, false))
            {
                return Err(ManifestError::Invalid("context reference"));
            }
            if let Some(interval) = window.capture_interval {
                if interval.started_at_ms < self.capture_interval.started_at_ms
                    || interval.finished_at_ms > self.capture_interval.finished_at_ms
                    || interval.finished_at_ms < interval.started_at_ms
                    || interval.finished_at_ms - interval.started_at_ms > MAX_CAPTURE_INTERVAL_MS
                {
                    return Err(ManifestError::Invalid("window capture interval"));
                }
            } else if matches!(self.scope, ManifestScope::Context)
                && self.context_member_count.is_some()
            {
                return Err(ManifestError::Invalid("window capture interval"));
            }
            let selected = window
                .selected_logical_page
                .as_deref()
                .map(str::parse::<LogicalPageId>)
                .transpose()
                .map_err(|_| ManifestError::Invalid("selected page"))?;
            let mut local_pages = BTreeSet::new();
            for page in &window.pages {
                let page_id = page
                    .logical_page_id
                    .parse::<LogicalPageId>()
                    .map_err(|_| ManifestError::Invalid("logical page id"))?;
                if !pages.insert(page_id) || !local_pages.insert(page_id) || pages.len() > MAX_PAGES
                {
                    return Err(ManifestError::Invalid("page structure"));
                }
                validate_page(page, &self.requested_capture, self.capture_interval)?;
            }
            if selected.is_some_and(|id| !local_pages.contains(&id)) {
                return Err(ManifestError::Invalid("selected page"));
            }
        }
        if matches!(self.scope, ManifestScope::Tab) && pages.len() != 1 {
            return Err(ManifestError::Invalid("tab scope"));
        }
        Ok(())
    }
}

fn validate_page(
    page: &ManifestPage,
    request: &CaptureRequest,
    capture_interval: CaptureInterval,
) -> Result<(), ManifestError> {
    validate_page_structure(page)?;
    validate_page_objects(page, request)?;
    validate_page_capture_evidence(page, request, capture_interval)?;
    validate_page_coverage(page, request)
}

fn validate_page_structure(page: &ManifestPage) -> Result<(), ManifestError> {
    if page.lineage_parent.as_deref().is_some_and(|value| {
        value.parse::<LogicalPageId>().is_err() || value == page.logical_page_id
    }) {
        return Err(ManifestError::Invalid("page lineage"));
    }
    if !(25..=500).contains(&page.zoom_hundredths) || !page.fidelity.structural {
        return Err(ManifestError::Invalid("page fidelity"));
    }
    if page
        .title_redacted
        .as_deref()
        .is_some_and(|value| !bounded_text(value, MAX_TITLE_BYTES, false))
    {
        return Err(ManifestError::Invalid("redacted title"));
    }
    match &page.safe_url_or_placeholder {
        ManifestSafeUrl::Replayable(url)
            if url.len() > MAX_URL_BYTES
                || SafePageUrl::from_observed(Some(url), true).replay_url()
                    != Some(url.as_str()) =>
        {
            return Err(ManifestError::Invalid("safe URL"));
        }
        _ => {}
    }
    if let ManifestScroll::Known {
        x_hundredths,
        y_hundredths,
    } = page.document_scroll
        && (x_hundredths > MAX_SCROLL_HUNDREDTHS || y_hundredths > MAX_SCROLL_HUNDREDTHS)
    {
        return Err(ManifestError::Invalid("scroll position"));
    }
    Ok(())
}

fn validate_page_objects(
    page: &ManifestPage,
    request: &CaptureRequest,
) -> Result<(), ManifestError> {
    if page.archive_failure.is_some()
        && (!request.contains(CaptureCategory::Archive)
            || page.archive_object.is_some()
            || page.fidelity.archive != ManifestCoverage::Unavailable
            || page
                .page_capture
                .as_ref()
                .is_some_and(|evidence| evidence.archive_interval.is_some()))
    {
        return Err(ManifestError::Invalid("archive page failure"));
    }
    if let Some(object) = &page.archive_object {
        validate_object(object, ManifestObjectRole::Archive)?;
        if !request.contains(CaptureCategory::Archive) {
            return Err(ManifestError::Invalid("archive request"));
        }
    }
    if let Some(object) = &page.sidecar_object {
        validate_object(object, ManifestObjectRole::Sidecar)?;
        if !request.contains(CaptureCategory::Page) {
            return Err(ManifestError::Invalid("page request"));
        }
    }
    Ok(())
}

fn validate_page_capture_evidence(
    page: &ManifestPage,
    request: &CaptureRequest,
    capture_interval: CaptureInterval,
) -> Result<(), ManifestError> {
    if let Some(evidence) = &page.page_capture {
        if evidence.categories.len() > 5
            || (evidence.archive_interval.is_some() && !request.contains(CaptureCategory::Archive))
            || (evidence.sidecar_interval.is_some() && !request.contains(CaptureCategory::Page))
            || (!evidence.categories.is_empty() && !request.contains(CaptureCategory::Page))
        {
            return Err(ManifestError::Invalid("page capture evidence"));
        }
        let mut seen = BTreeSet::new();
        for interval in [evidence.archive_interval, evidence.sidecar_interval]
            .into_iter()
            .flatten()
        {
            if interval.started_at_ms < capture_interval.started_at_ms
                || interval.finished_at_ms > capture_interval.finished_at_ms
                || interval.finished_at_ms < interval.started_at_ms
            {
                return Err(ManifestError::Invalid("page component interval"));
            }
        }
        for interval in [
            evidence.archive_monotonic_interval,
            evidence.sidecar_monotonic_interval,
        ]
        .into_iter()
        .flatten()
        {
            if interval.finished_offset_ms < interval.started_offset_ms
                || interval.finished_offset_ms > MAX_CAPTURE_INTERVAL_MS
            {
                return Err(ManifestError::Invalid("page monotonic interval"));
            }
        }
        if evidence.archive_interval.is_some() != evidence.archive_monotonic_interval.is_some()
            || evidence.sidecar_interval.is_some() != evidence.sidecar_monotonic_interval.is_some()
            || evidence.archive_monotonic_interval.is_some_and(|archive| {
                evidence
                    .sidecar_monotonic_interval
                    .is_some_and(|sidecar| archive.started_offset_ms < sidecar.finished_offset_ms)
            })
        {
            return Err(ManifestError::Invalid("page monotonic interval"));
        }
        for category in &evidence.categories {
            if category.requested == 0
                || category.captured > category.requested
                || !seen.insert(category.category)
            {
                return Err(ManifestError::Invalid("page category coverage"));
            }
        }
        if page.fidelity.page == ManifestCoverage::CompleteForRequest
            && (evidence.sidecar_interval.is_none()
                || evidence.sidecar_monotonic_interval.is_none()
                || evidence
                    .categories
                    .iter()
                    .any(|category| category.truncated)
                || evidence.mutation_evidence != ManifestMutationEvidence::ObservedStable)
        {
            return Err(ManifestError::Invalid("page capture completeness"));
        }
    }
    Ok(())
}

fn validate_page_coverage(
    page: &ManifestPage,
    request: &CaptureRequest,
) -> Result<(), ManifestError> {
    if page.fidelity.archive == ManifestCoverage::CompleteForRequest
        && page.archive_object.is_none()
    {
        return Err(ManifestError::Invalid("archive coverage"));
    }
    if page.fidelity.page == ManifestCoverage::CompleteForRequest && page.sidecar_object.is_none() {
        return Err(ManifestError::Invalid("page coverage"));
    }
    if !coverage_matches_request(
        request.contains(CaptureCategory::Archive),
        page.fidelity.archive,
    ) {
        return Err(ManifestError::Invalid("archive coverage"));
    }
    if !coverage_matches_request(request.contains(CaptureCategory::Page), page.fidelity.page) {
        return Err(ManifestError::Invalid("page coverage"));
    }
    if !coverage_matches_request(
        request.contains(CaptureCategory::Cookies),
        page.fidelity.cookies,
    ) {
        return Err(ManifestError::Invalid("cookie coverage"));
    }
    Ok(())
}

fn coverage_matches_request(requested: bool, coverage: ManifestCoverage) -> bool {
    if requested {
        coverage != ManifestCoverage::NotRequested
    } else {
        coverage == ManifestCoverage::NotRequested
    }
}

fn bounded_text(value: &str, limit: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.trim().is_empty())
        && value.len() <= limit
        && !value.chars().any(char::is_control)
}

#[allow(clippy::trivially_copy_pass_by_ref)] // Serde skip_serializing_if receives a field reference.
fn is_false(value: &bool) -> bool {
    !value
}

fn bounded_ascii(value: &str, limit: usize) -> bool {
    !value.is_empty()
        && value.len() <= limit
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'\\')
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_object(
    object: &ManifestObjectRef,
    role: ManifestObjectRole,
) -> Result<(), ManifestError> {
    let max = if role == ManifestObjectRole::Sidecar {
        MAX_PAGE_SIDECAR_BYTES as u64
    } else {
        MAX_OBJECT_BYTES
    };
    let backend_is_valid = object
        .archive_backend
        .as_deref()
        .is_none_or(valid_archive_backend_id);
    let backend_matches_role =
        role == ManifestObjectRole::Archive || object.archive_backend.is_none();
    if object.role != role
        || !valid_sha256(&object.sha256)
        || object.bytes == 0
        || object.bytes > max
        || !backend_is_valid
        || !backend_matches_role
    {
        return Err(ManifestError::Invalid("object reference"));
    }
    Ok(())
}

fn valid_archive_backend_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionSnapshot;
    use ferric_browser_core::{
        CaptureField, LogicalPageId, LogicalWindowId, ScrollPoint, StructuralPage,
        StructuralWindow, versioning::Coverage,
    };
    use serde_json::{Value, json};

    const SCHEMA1_STRUCTURAL_FIXTURE: &str = r#"{
      "schema_version": 1,
      "id": "commit:00000000-0000-4000-8000-000000000001",
      "profile_uuid": "00000000-0000-4000-8000-000000000002",
      "created_at_ms": 102,
      "message": "before trial",
      "parent_commit": null,
      "application_revision": 17,
      "configuration_revision": 9,
      "scope": "tab",
      "requested_capture": { "categories": ["structural"] },
      "capture_interval": { "started_at_ms": 100, "finished_at_ms": 101 },
      "windows": [{
        "logical_window_id": "logical-window:00000000-0000-4000-8000-000000000003",
        "context_ref": "work",
        "selected_logical_page": "logical-page:00000000-0000-4000-8000-000000000004",
        "pages": [{
          "logical_page_id": "logical-page:00000000-0000-4000-8000-000000000004",
          "lineage_parent": null,
          "safe_url_or_placeholder": { "state": "replayable", "value": "https://example.test/path" },
          "title_redacted": null,
          "pinned": true,
          "muted": false,
          "zoom_hundredths": 125,
          "document_scroll": { "state": "known", "value": { "x_hundredths": 1250, "y_hundredths": 0 } },
          "archive_object": null,
          "sidecar_object": null,
          "fidelity": { "structural": true, "archive": "not-requested", "page": "not-requested", "cookies": "not-requested" }
        }]
      }],
      "policy_snapshot": null,
      "engine_fingerprint": "qt-6.11-linux-test",
      "helper_bundle_version": "structural-1",
      "warnings": []
    }"#;

    #[test]
    fn manifest_policy_debug_redacts_stable_configuration_fingerprint() {
        let sentinel = "private-policy-fingerprint-sentinel";
        let policy = ManifestPolicySnapshot {
            schema_sha256: sentinel.into(),
            javascript_enabled: true,
        };

        let debug = format!("{policy:?}");
        assert!(debug.contains("javascript_enabled: true"));
        assert!(!debug.contains(sentinel));
    }

    #[test]
    fn manifest_object_reference_debug_redacts_content_digest() {
        let digest_sentinel = "private-page-content-digest-sentinel";
        let reference = ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: digest_sentinel.to_owned(),
            bytes: 123,
            archive_backend: Some(ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned()),
        };

        let debug = format!("{reference:?}");
        assert!(debug.contains("[content digest]"));
        assert!(!debug.contains(digest_sentinel));
    }

    #[test]
    fn schema_one_structural_fixture_migrates_and_round_trips_as_schema_three() {
        let manifest = VersionedCaptureManifest::decode(SCHEMA1_STRUCTURAL_FIXTURE.as_bytes())
            .expect("checked-in schema-1 fixture");
        assert_eq!(manifest.schema_version, MANIFEST_SCHEMA_VERSION);
        assert_eq!(manifest.windows[0].pages.len(), 1);
        assert_eq!(
            manifest.requested_capture,
            CaptureRequest::structural_only()
        );
        let encoded = manifest.encode().expect("canonical encoding");
        assert_eq!(VersionedCaptureManifest::decode(&encoded), Ok(manifest));
    }

    fn sample_manifest() -> VersionedCaptureManifest {
        let page = LogicalPageId::fresh();
        let window = LogicalWindowId::fresh();
        let structural = StructuralSnapshot {
            application_revision: 17,
            windows: vec![StructuralWindow {
                logical_id: window,
                context_ref: Some("work".into()),
                selected_page: Some(page),
                pages: vec![StructuralPage {
                    logical_id: page,
                    lineage_parent: None,
                    url: SafePageUrl::from_observed(Some("https://example.test/path"), true),
                    pinned: true,
                    muted: false,
                    zoom_hundredths: 125,
                    scroll: CaptureField::Known(ScrollPoint::new(12.5, 0.0).unwrap()),
                    page_coverage: Coverage::from_counts(Some(1), Some(1)).unwrap(),
                    cookie_coverage: Coverage::from_counts(Some(1), Some(1)).unwrap(),
                }],
            }],
        };
        VersionedCaptureManifest::from_structural(
            ManifestHeader {
                id: CommitId::fresh(),
                profile_uuid: Uuid::new_v4(),
                created_at_ms: 102,
                message: "before trial".into(),
                parent_commit: None,
                configuration_revision: 9,
                scope: ManifestScope::Tab,
                capture_interval: CaptureInterval {
                    started_at_ms: 100,
                    finished_at_ms: 101,
                },
                engine_fingerprint: "qt-6.11-linux-test".into(),
                helper_bundle_version: "structural-1".into(),
            },
            &structural,
        )
        .unwrap()
    }

    #[test]
    fn context_scope_requires_bounded_complete_member_ordinals_and_intervals() {
        let mut manifest = sample_manifest();
        manifest.scope = ManifestScope::Context;
        manifest.context_member_count = Some(2);
        manifest.windows[0].context_ref = Some("context-uuid".into());
        manifest.windows[0].capture_interval = Some(manifest.capture_interval);
        manifest.windows[0].context_member_ordinal = Some(0);
        manifest.invalid_context_members = vec![InvalidContextMember {
            ordinal: 1,
            reason: InvalidContextMemberReason::ClosedWindow,
        }];
        manifest
            .validate()
            .expect("complete context member accounting");
        let decoded =
            VersionedCaptureManifest::decode(&manifest.encode().expect("bounded context manifest"))
                .expect("context manifest round-trips");
        assert_eq!(decoded, manifest);

        let mut duplicate = manifest.clone();
        duplicate.invalid_context_members[0].ordinal = 0;
        assert!(duplicate.validate().is_err());

        let mut unaccounted = manifest;
        unaccounted.context_member_count = Some(3);
        assert!(unaccounted.validate().is_err());
    }

    fn mutated(mut value: Value, path: &[&str], replacement: Value) -> Vec<u8> {
        let mut field = &mut value;
        for segment in path {
            field = &mut field[*segment];
        }
        *field = replacement;
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn structural_capture_round_trips_without_pages_cookies_or_runtime_ids() {
        let manifest = sample_manifest();
        let bytes = manifest.encode().expect("bounded manifest");
        assert_eq!(
            VersionedCaptureManifest::decode(&bytes),
            Ok(manifest.clone())
        );
        assert_eq!(manifest.schema_version, MANIFEST_SCHEMA_VERSION);
        assert_eq!(manifest.application_revision, 17);
        assert_eq!(manifest.configuration_revision, 9);
        assert_eq!(manifest.windows[0].pages[0].title_redacted, None);
        assert_eq!(manifest.windows[0].pages[0].archive_object, None);
        assert_eq!(manifest.windows[0].pages[0].sidecar_object, None);
        assert_eq!(
            manifest.windows[0].pages[0].fidelity.page,
            ManifestCoverage::NotRequested
        );
        assert_eq!(
            manifest.windows[0].pages[0].fidelity.cookies,
            ManifestCoverage::NotRequested
        );
        assert!(manifest.policy_snapshot.is_none());
        assert!(
            !bytes
                .windows(b"cookie_value".len())
                .any(|part| part == b"cookie_value")
        );
        assert!(!bytes.windows(b"tabid-".len()).any(|part| part == b"tabid-"));
        assert!(
            !bytes
                .windows(b"document-".len())
                .any(|part| part == b"document-")
        );
    }

    #[test]
    fn malformed_oversized_and_unsupported_schema_bytes_fail_closed() {
        let value = serde_json::to_value(sample_manifest()).unwrap();
        assert_eq!(
            VersionedCaptureManifest::decode(b"{"),
            Err(ManifestError::Malformed)
        );
        assert_eq!(
            VersionedCaptureManifest::decode(&vec![b' '; MAX_MANIFEST_BYTES + 1]),
            Err(ManifestError::TooLarge)
        );
        assert_eq!(
            VersionedCaptureManifest::decode(&mutated(
                value.clone(),
                &["schema_version"],
                json!(0)
            )),
            Err(ManifestError::LegacySchema(0))
        );
        assert_eq!(
            VersionedCaptureManifest::decode(&mutated(
                value.clone(),
                &["schema_version"],
                json!(4)
            )),
            Err(ManifestError::NewerSchema(4))
        );
        let mut unknown = value.clone();
        unknown["newer_feature"] = json!(true);
        assert_eq!(
            VersionedCaptureManifest::decode(&serde_json::to_vec(&unknown).unwrap()),
            Err(ManifestError::Malformed)
        );
        unknown["schema_version"] = json!(4);
        assert_eq!(
            VersionedCaptureManifest::decode(&serde_json::to_vec(&unknown).unwrap()),
            Err(ManifestError::NewerSchema(4))
        );
        assert_eq!(
            VersionedCaptureManifest::decode(&mutated(
                value.clone(),
                &["message"],
                json!("x".repeat(MAX_MESSAGE_BYTES + 1))
            )),
            Err(ManifestError::Invalid("message"))
        );
        assert_eq!(
            VersionedCaptureManifest::decode(&mutated(
                value,
                &["profile_uuid"],
                json!(Uuid::nil())
            )),
            Err(ManifestError::Invalid("owner profile"))
        );
        let mut interval = sample_manifest();
        interval.capture_interval.finished_at_ms =
            interval.capture_interval.started_at_ms + MAX_CAPTURE_INTERVAL_MS + 1;
        interval.created_at_ms = interval.capture_interval.finished_at_ms;
        assert_eq!(
            interval.encode(),
            Err(ManifestError::Invalid("capture interval"))
        );
    }

    #[test]
    fn logical_ids_selection_urls_and_positions_are_checked() {
        let base = serde_json::to_value(sample_manifest()).unwrap();
        for (path, replacement, expected) in [
            (vec!["id"], json!("commit:not-a-uuid"), "commit id"),
            (
                vec!["windows", "0", "logical_window_id"],
                json!("logical-page:00000000-0000-4000-8000-000000000001"),
                "logical window id",
            ),
            (
                vec!["windows", "0", "selected_logical_page"],
                json!(LogicalPageId::fresh().to_string()),
                "selected page",
            ),
        ] {
            let mut value = base.clone();
            let target = if path.len() == 1 {
                &mut value[path[0]]
            } else {
                &mut value["windows"][0][path[2]]
            };
            *target = replacement;
            assert_eq!(
                VersionedCaptureManifest::decode(&serde_json::to_vec(&value).unwrap()),
                Err(ManifestError::Invalid(expected))
            );
        }
        let mut duplicate = sample_manifest();
        duplicate.scope = ManifestScope::Window;
        let duplicate_page = duplicate.windows[0].pages[0].clone();
        duplicate.windows[0].pages.push(duplicate_page);
        assert_eq!(
            duplicate.encode(),
            Err(ManifestError::Invalid("page structure"))
        );
        let mut unsafe_url = sample_manifest();
        unsafe_url.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://user:password@example.test/?token=secret".into());
        assert_eq!(unsafe_url.encode(), Err(ManifestError::Invalid("safe URL")));
        let mut scroll = sample_manifest();
        scroll.windows[0].pages[0].document_scroll = ManifestScroll::Known {
            x_hundredths: MAX_SCROLL_HUNDREDTHS + 1,
            y_hundredths: 0,
        };
        assert_eq!(
            scroll.encode(),
            Err(ManifestError::Invalid("scroll position"))
        );
        let mut negative = serde_json::to_value(sample_manifest()).unwrap();
        negative["windows"][0]["pages"][0]["document_scroll"]["value"]["x_hundredths"] = json!(-1);
        assert_eq!(
            VersionedCaptureManifest::decode(&serde_json::to_vec(&negative).unwrap()),
            Err(ManifestError::Malformed)
        );
    }

    #[test]
    fn object_references_require_exact_role_hash_size_and_requested_category() {
        let mut manifest = sample_manifest();
        let object = ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: "a".repeat(64),
            bytes: 1_024,
            archive_backend: Some(ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned()),
        };
        assert!(object.supports_archive_view());
        manifest
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        manifest.windows[0].pages[0].archive_object = Some(object.clone());
        manifest.windows[0].pages[0].fidelity.archive = ManifestCoverage::CompleteForRequest;
        manifest.windows[0].pages[0].page_capture = Some(ManifestPageCapture {
            archive_interval: Some(manifest.capture_interval),
            sidecar_interval: None,
            archive_monotonic_interval: Some(CaptureMonotonicInterval {
                started_offset_ms: 0,
                finished_offset_ms: 1,
            }),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        });
        assert!(manifest.encode().is_ok());
        manifest.windows[0].pages[0]
            .archive_object
            .as_mut()
            .unwrap()
            .role = ManifestObjectRole::Sidecar;
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("object reference"))
        );
        manifest.windows[0].pages[0]
            .archive_object
            .as_mut()
            .unwrap()
            .role = ManifestObjectRole::Archive;
        manifest.windows[0].pages[0]
            .archive_object
            .as_mut()
            .unwrap()
            .sha256 = "A".repeat(64);
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("object reference"))
        );
        manifest.windows[0].pages[0].archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: "a".repeat(64),
            bytes: 0,
            archive_backend: Some(ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned()),
        });
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("object reference"))
        );
        manifest.windows[0].pages[0].archive_object = Some(object);
        manifest
            .requested_capture
            .categories
            .remove(&CaptureCategory::Archive);
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("archive request"))
        );
    }

    #[test]
    fn window_archive_requires_explicit_per_page_results_and_partial_policy() {
        let mut manifest = sample_manifest();
        manifest.scope = ManifestScope::Window;
        manifest
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        manifest.requested_capture.allow_partial_pages = true;
        let mut failed = manifest.windows[0].pages[0].clone();
        failed.logical_page_id = LogicalPageId::fresh().to_string();
        failed.fidelity.archive = ManifestCoverage::Unavailable;
        failed.archive_failure = Some(ManifestPageArchiveFailure::Timeout);
        manifest.windows[0].pages.push(failed);
        let saved = &mut manifest.windows[0].pages[0];
        saved.archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: "a".repeat(64),
            bytes: 1_024,
            archive_backend: Some(ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned()),
        });
        saved.fidelity.archive = ManifestCoverage::CompleteForRequest;
        saved.page_capture = Some(ManifestPageCapture {
            archive_interval: Some(manifest.capture_interval),
            sidecar_interval: None,
            archive_monotonic_interval: Some(CaptureMonotonicInterval {
                started_offset_ms: 0,
                finished_offset_ms: 1,
            }),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        });
        let encoded = manifest.encode().expect("explicit partial window capture");
        assert_eq!(
            VersionedCaptureManifest::decode(&encoded),
            Ok(manifest.clone())
        );

        let mut strict = manifest.clone();
        strict.requested_capture.allow_partial_pages = false;
        assert_eq!(
            strict.encode(),
            Err(ManifestError::Invalid("partial page policy"))
        );
        let mut strict_bytes = serde_json::to_value(&strict).unwrap();
        strict_bytes["schema_version"] = json!(MANIFEST_SCHEMA_VERSION);
        assert_eq!(
            VersionedCaptureManifest::decode(&serde_json::to_vec(&strict_bytes).unwrap()),
            Err(ManifestError::Invalid("partial page policy"))
        );
        let mut missing = manifest.clone();
        missing.windows[0].pages[1].archive_failure = None;
        assert_eq!(
            missing.encode(),
            Err(ManifestError::Invalid("archive page result"))
        );
        let mut contradictory = manifest.clone();
        contradictory.windows[0].pages[0].archive_failure =
            Some(ManifestPageArchiveFailure::Interrupted);
        assert_eq!(
            contradictory.encode(),
            Err(ManifestError::Invalid("archive page failure"))
        );
        let mut false_success = manifest.clone();
        false_success.windows[0].pages[0].fidelity.archive = ManifestCoverage::Unknown;
        assert_eq!(
            false_success.encode(),
            Err(ManifestError::Invalid("archive page result"))
        );
        let mut irrelevant_policy = sample_manifest();
        irrelevant_policy.scope = ManifestScope::Window;
        irrelevant_policy.requested_capture.allow_partial_pages = true;
        assert_eq!(
            irrelevant_policy.encode(),
            Err(ManifestError::Invalid("partial page policy"))
        );
        let mut no_success = manifest;
        no_success.windows[0].pages[0].archive_object = None;
        no_success.windows[0].pages[0].page_capture = None;
        no_success.windows[0].pages[0].fidelity.archive = ManifestCoverage::Unavailable;
        no_success.windows[0].pages[0].archive_failure =
            Some(ManifestPageArchiveFailure::StorageFull);
        assert_eq!(
            no_success.encode(),
            Err(ManifestError::Invalid("partial page policy"))
        );
    }

    #[test]
    fn schema_two_strict_capture_migrates_without_inventing_page_failures() {
        let mut value = serde_json::to_value(sample_manifest()).unwrap();
        value["schema_version"] = json!(2);
        value["requested_capture"]["allow_partial_pages"] = json!(true);
        value["windows"][0]["pages"][0]["archive_failure"] = json!("timeout");
        let decoded = VersionedCaptureManifest::decode(&serde_json::to_vec(&value).unwrap())
            .expect("schema-two claims about newer fields cannot gain authority");
        assert_eq!(decoded.schema_version, MANIFEST_SCHEMA_VERSION);
        assert!(!decoded.requested_capture.allow_partial_pages);
        assert!(decoded.windows[0].pages[0].archive_failure.is_none());
        assert!(decoded.encode().is_ok());
    }

    #[test]
    fn unknown_archive_backend_is_preserved_but_not_eligible_for_viewing() {
        let mut manifest = sample_manifest();
        manifest
            .requested_capture
            .categories
            .insert(CaptureCategory::Archive);
        manifest.windows[0].pages[0].archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: "a".repeat(64),
            bytes: 1_024,
            archive_backend: Some("future-format-v2".to_owned()),
        });
        manifest.windows[0].pages[0].fidelity.archive = ManifestCoverage::CompleteForRequest;

        let bytes = manifest
            .encode()
            .expect("unknown backend remains valid metadata");
        let decoded = VersionedCaptureManifest::decode(&bytes)
            .expect("future archive backend metadata is preserved");
        let object = decoded.windows[0].pages[0]
            .archive_object
            .as_ref()
            .expect("archive reference remains listed");
        assert_eq!(object.archive_backend.as_deref(), Some("future-format-v2"));
        assert!(!object.supports_archive_view());

        let mut schema_one: Value =
            serde_json::from_str(SCHEMA1_STRUCTURAL_FIXTURE).expect("old structural fixture");
        schema_one["requested_capture"]["categories"] = json!(["structural", "archive"]);
        schema_one["windows"][0]["pages"][0]["archive_object"] = json!({
            "role": "archive",
            "sha256": "a".repeat(64),
            "bytes": 1_024
        });
        schema_one["windows"][0]["pages"][0]["fidelity"]["archive"] = json!("complete-for-request");
        let legacy = VersionedCaptureManifest::decode(
            &serde_json::to_vec(&schema_one).expect("schema-one fixture bytes"),
        )
        .expect("schema-1 archive without backend migrates in memory");
        assert_eq!(legacy.schema_version, MANIFEST_SCHEMA_VERSION);
        assert_eq!(
            legacy.windows[0].pages[0]
                .archive_object
                .as_ref()
                .expect("legacy archive reference")
                .bytes,
            1_024
        );
        assert!(
            !legacy.windows[0].pages[0]
                .archive_object
                .as_ref()
                .expect("legacy archive reference")
                .supports_archive_view()
        );
    }

    #[test]
    fn schema_one_cannot_smuggle_a_schema_two_archive_backend_claim() {
        let mut legacy: Value =
            serde_json::from_str(SCHEMA1_STRUCTURAL_FIXTURE).expect("schema-one fixture");
        legacy["requested_capture"]["categories"] = json!(["structural", "archive"]);
        legacy["windows"][0]["pages"][0]["archive_object"] = json!({
            "role": "archive",
            "sha256": "a".repeat(64),
            "bytes": 1_024,
            "archive_backend": ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1
        });
        legacy["windows"][0]["pages"][0]["fidelity"]["archive"] = json!("complete-for-request");

        let migrated = VersionedCaptureManifest::decode(
            &serde_json::to_vec(&legacy).expect("legacy manifest bytes"),
        )
        .expect("schema-one data is structurally migrated");
        let archive = migrated.windows[0].pages[0]
            .archive_object
            .as_ref()
            .expect("legacy reference is preserved");
        assert_eq!(archive.archive_backend, None);
        assert!(!archive.supports_archive_view());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn page_capture_manifest_keeps_archive_sidecar_fidelity_and_intervals_independent() {
        let mut manifest = sample_manifest();
        manifest
            .requested_capture
            .categories
            .extend([CaptureCategory::Archive, CaptureCategory::Page]);
        let page = &mut manifest.windows[0].pages[0];
        page.archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: "a".repeat(64),
            bytes: 1024,
            archive_backend: Some(ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned()),
        });
        page.sidecar_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Sidecar,
            sha256: "b".repeat(64),
            bytes: 2048,
            archive_backend: None,
        });
        page.fidelity.archive = ManifestCoverage::CompleteForRequest;
        page.fidelity.page = ManifestCoverage::CompleteForRequest;
        page.page_capture = Some(ManifestPageCapture {
            archive_interval: Some(CaptureInterval {
                started_at_ms: 100,
                finished_at_ms: 101,
            }),
            sidecar_interval: Some(CaptureInterval {
                started_at_ms: 100,
                finished_at_ms: 100,
            }),
            archive_monotonic_interval: Some(CaptureMonotonicInterval {
                started_offset_ms: 1,
                finished_offset_ms: 2,
            }),
            sidecar_monotonic_interval: Some(CaptureMonotonicInterval {
                started_offset_ms: 0,
                finished_offset_ms: 1,
            }),
            mutation_evidence: ManifestMutationEvidence::ObservedStable,
            categories: vec![ManifestCategoryCoverage {
                category: ManifestPageCategory::Geometry,
                requested: 3,
                captured: 3,
                truncated: false,
            }],
        });
        assert!(manifest.encode().is_ok());

        let mut missing_category_evidence = manifest.clone();
        missing_category_evidence.windows[0].pages[0]
            .page_capture
            .as_mut()
            .expect("page evidence")
            .categories
            .clear();
        assert_eq!(
            missing_category_evidence.encode(),
            Err(ManifestError::Invalid("page capture completeness"))
        );
        let older_empty_category_bytes =
            serde_json::to_vec(&missing_category_evidence).expect("older manifest bytes");
        assert!(VersionedCaptureManifest::decode(&older_empty_category_bytes).is_ok());

        let mut missing_archive_interval = manifest.clone();
        let archive_evidence = missing_archive_interval.windows[0].pages[0]
            .page_capture
            .as_mut()
            .expect("page evidence");
        archive_evidence.archive_interval = None;
        archive_evidence.archive_monotonic_interval = None;
        assert_eq!(
            missing_archive_interval.encode(),
            Err(ManifestError::Invalid("archive capture interval"))
        );

        let mut missing_sidecar_interval = manifest.clone();
        let sidecar_evidence = missing_sidecar_interval.windows[0].pages[0]
            .page_capture
            .as_mut()
            .expect("page evidence");
        sidecar_evidence.sidecar_interval = None;
        sidecar_evidence.sidecar_monotonic_interval = None;
        missing_sidecar_interval.windows[0].pages[0].fidelity.page = ManifestCoverage::Partial;
        assert_eq!(
            missing_sidecar_interval.encode(),
            Err(ManifestError::Invalid("page capture interval"))
        );

        let mut missing_all_evidence = manifest.clone();
        missing_all_evidence.windows[0].pages[0].page_capture = None;
        missing_all_evidence.windows[0].pages[0].fidelity.page = ManifestCoverage::Partial;
        assert_eq!(
            missing_all_evidence.encode(),
            Err(ManifestError::Invalid("archive capture interval"))
        );
        let older_bytes = serde_json::to_vec(&missing_all_evidence).expect("older manifest bytes");
        assert!(VersionedCaptureManifest::decode(&older_bytes).is_ok());

        manifest.windows[0].pages[0]
            .page_capture
            .as_mut()
            .unwrap()
            .sidecar_monotonic_interval = None;
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("page monotonic interval"))
        );
        {
            let evidence = manifest.windows[0].pages[0].page_capture.as_mut().unwrap();
            evidence.sidecar_monotonic_interval = Some(CaptureMonotonicInterval {
                started_offset_ms: 0,
                finished_offset_ms: 1,
            });
            evidence.archive_monotonic_interval = Some(CaptureMonotonicInterval {
                started_offset_ms: 0,
                finished_offset_ms: 2,
            });
        }
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("page monotonic interval"))
        );
        manifest.windows[0].pages[0]
            .page_capture
            .as_mut()
            .unwrap()
            .archive_monotonic_interval = Some(CaptureMonotonicInterval {
            started_offset_ms: 1,
            finished_offset_ms: MAX_CAPTURE_INTERVAL_MS + 1,
        });
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("page monotonic interval"))
        );
        manifest.windows[0].pages[0]
            .page_capture
            .as_mut()
            .unwrap()
            .archive_monotonic_interval = Some(CaptureMonotonicInterval {
            started_offset_ms: 1,
            finished_offset_ms: 2,
        });

        manifest.windows[0].pages[0]
            .page_capture
            .as_mut()
            .unwrap()
            .mutation_evidence = ManifestMutationEvidence::ObservedChanged;
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("page capture completeness"))
        );
        manifest.windows[0].pages[0]
            .page_capture
            .as_mut()
            .unwrap()
            .mutation_evidence = ManifestMutationEvidence::NotObserved;
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("page capture completeness"))
        );
        manifest.windows[0].pages[0].fidelity.page = ManifestCoverage::Partial;
        assert!(manifest.encode().is_ok());

        manifest.windows[0].pages[0]
            .page_capture
            .as_mut()
            .unwrap()
            .archive_interval = Some(CaptureInterval {
            started_at_ms: 99,
            finished_at_ms: 101,
        });
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("page component interval"))
        );
    }

    #[test]
    fn archive_commit_remains_representable_when_sidecar_capture_is_unavailable() {
        let mut manifest = sample_manifest();
        manifest
            .requested_capture
            .categories
            .extend([CaptureCategory::Archive, CaptureCategory::Page]);
        let page = &mut manifest.windows[0].pages[0];
        page.archive_object = Some(ManifestObjectRef {
            role: ManifestObjectRole::Archive,
            sha256: "a".repeat(64),
            bytes: 1024,
            archive_backend: Some(ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1.to_owned()),
        });
        page.fidelity.archive = ManifestCoverage::CompleteForRequest;
        page.fidelity.page = ManifestCoverage::Unavailable;
        page.page_capture = Some(ManifestPageCapture {
            archive_interval: Some(CaptureInterval {
                started_at_ms: 100,
                finished_at_ms: 101,
            }),
            sidecar_interval: None,
            archive_monotonic_interval: Some(CaptureMonotonicInterval {
                started_offset_ms: 0,
                finished_offset_ms: 1,
            }),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        });
        assert!(manifest.encode().is_ok());

        manifest.windows[0].pages[0].fidelity.page = ManifestCoverage::CompleteForRequest;
        assert_eq!(
            manifest.encode(),
            Err(ManifestError::Invalid("page capture completeness"))
        );
    }

    #[test]
    fn structural_conversion_rejects_oversize_before_materialization() {
        let mut structural = StructuralSnapshot {
            application_revision: 1,
            windows: vec![],
        };
        let page = sample_manifest().windows[0].pages[0]
            .logical_page_id
            .parse::<LogicalPageId>()
            .unwrap();
        structural.windows.push(StructuralWindow {
            logical_id: LogicalWindowId::fresh(),
            context_ref: None,
            selected_page: None,
            pages: (0..=MAX_PAGES_PER_WINDOW)
                .map(|_| StructuralPage {
                    logical_id: page,
                    lineage_parent: None,
                    url: SafePageUrl::placeholder(CaptureUnavailable::NotObserved),
                    pinned: false,
                    muted: false,
                    zoom_hundredths: 100,
                    scroll: CaptureField::Absent,
                    page_coverage: Coverage::unknown(),
                    cookie_coverage: Coverage::unknown(),
                })
                .collect(),
        });
        let header = ManifestHeader {
            id: CommitId::fresh(),
            profile_uuid: Uuid::new_v4(),
            created_at_ms: 2,
            message: "x".into(),
            parent_commit: None,
            configuration_revision: 1,
            scope: ManifestScope::Window,
            capture_interval: CaptureInterval {
                started_at_ms: 1,
                finished_at_ms: 2,
            },
            engine_fingerprint: "test".into(),
            helper_bundle_version: "1".into(),
        };
        assert_eq!(
            VersionedCaptureManifest::from_structural(header, &structural),
            Err(ManifestError::Invalid("structural size"))
        );
    }

    #[test]
    fn debug_is_redacted_and_schema_two_recovery_bytes_are_unchanged() {
        let mut manifest = sample_manifest();
        let commit_id = manifest.id.to_string();
        let window_id = manifest.windows[0].logical_window_id.to_string();
        let page_id = manifest.windows[0].pages[0].logical_page_id.to_string();
        manifest.message = "message-secret-sentinel".into();
        manifest.profile_uuid = Uuid::parse_str("f3c10252-5267-4ce1-9a4f-5a3fbd8a5602")
            .expect("valid profile UUID sentinel");
        manifest.windows[0].context_ref = Some("context-secret-sentinel".into());
        manifest.windows[0].pages[0].title_redacted = Some("title-secret-sentinel".into());
        let debug = format!("{manifest:?}");
        let window_debug = format!("{:?}", manifest.windows[0]);
        let page_debug = format!("{:?}", manifest.windows[0].pages[0]);
        let direct_debug = format!("{window_debug} {page_debug}");
        assert!(!debug.contains(commit_id.as_str()));
        assert!(!debug.contains(window_id.as_str()));
        assert!(!debug.contains(page_id.as_str()));
        for secret in [
            "message-secret-sentinel",
            "f3c10252-5267-4ce1-9a4f-5a3fbd8a5602",
            "context-secret-sentinel",
            "title-secret-sentinel",
            "https://example.test/path",
        ] {
            assert!(!debug.contains(secret));
            assert!(!direct_debug.contains(secret));
        }
        let id = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
        let profile = Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap();
        let recovery =
            SessionSnapshot::new(id, "work", profile, 7, "2026-09-27T00:00:00Z", vec![]).unwrap();
        let bytes = serde_json::to_string(&recovery).unwrap();
        assert_eq!(
            bytes,
            "{\"schema_version\":2,\"session_id\":\"00000000-0000-4000-8000-000000000001\",\"name\":\"work\",\"profile_id\":\"00000000-0000-4000-8000-000000000002\",\"revision\":7,\"saved_at\":\"2026-09-27T00:00:00Z\",\"windows\":[],\"closed_tabs\":[]}"
        );
        assert!(!bytes.contains("pages"));
        assert!(!bytes.contains("cookies"));
        assert_eq!(recovery.schema_version, 2);
    }

    #[test]
    fn manifest_header_debug_redacts_profile_identity() {
        let profile_sentinel = Uuid::parse_str("73518acf-c3f4-470a-8e1e-77821ad8fb42")
            .expect("valid profile UUID sentinel");
        let commit_sentinel = CommitId::fresh();
        let commit_text = commit_sentinel.to_string();
        let header = ManifestHeader {
            id: commit_sentinel,
            profile_uuid: profile_sentinel,
            created_at_ms: 1,
            message: "header-message-sentinel".into(),
            parent_commit: None,
            configuration_revision: 0,
            scope: ManifestScope::Tab,
            capture_interval: CaptureInterval {
                started_at_ms: 1,
                finished_at_ms: 1,
            },
            engine_fingerprint: "test".into(),
            helper_bundle_version: "test".into(),
        };

        let debug = format!("{header:?}");
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains(commit_text.as_str()));
        assert!(!debug.contains("73518acf-c3f4-470a-8e1e-77821ad8fb42"));
        assert!(!debug.contains("header-message-sentinel"));
    }
}
