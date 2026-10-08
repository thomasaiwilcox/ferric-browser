//! Redacted previews and one-use native confirmation permits. Nothing here
//! serializes a cookie record or performs an engine operation.

use std::{collections::BTreeSet, fmt, str::FromStr};

use ferric_browser_core::{
    BranchId, CapturedOwner, CommitId, NativeProfileOwner, OperationId, ResolutionChoice,
    versioning::{CookieMergeDecision, CookieMergePlan, limits::CONFIRMATION_PLAN_TTL_MS},
};
use serde::Serialize;
use uuid::Uuid;

use super::{OperationEnvelope, OperationError, OperationKind, OperationState, SensitiveOperation};

const MAX_PENDING_PREVIEWS: usize = 17;
const MAX_PREVIEW_ITEMS: usize = 4_096;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlanId(Uuid);

impl fmt::Debug for PlanId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PlanId([opaque])")
    }
}

impl fmt::Display for PlanId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "plan:{}", self.0.hyphenated())
    }
}

impl FromStr for PlanId {
    type Err = PreviewError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let encoded = value
            .strip_prefix("plan:")
            .ok_or(PreviewError::InvalidPlanId)?;
        if encoded.len() != 36 {
            return Err(PreviewError::InvalidPlanId);
        }
        let uuid = Uuid::parse_str(encoded).map_err(|_| PreviewError::InvalidPlanId)?;
        if uuid.hyphenated().to_string() != encoded
            || uuid.get_variant() != uuid::Variant::RFC4122
            || uuid.get_version() != Some(uuid::Version::Random)
        {
            return Err(PreviewError::InvalidPlanId);
        }
        Ok(Self(uuid))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewKind {
    Merge,
    Restore,
    Drop,
    StashClean,
    StashDirty,
    PageArchive,
    PageArchiveEditable,
    PageArchiveSelection,
    PageArchiveEditableSelection,
    PageArchivePartial,
    PageArchiveEditablePartial,
    PageArchiveSelectionPartial,
    PageArchiveEditableSelectionPartial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageCaptureFailurePolicy {
    Strict,
    AllowPartialPages,
}

impl PreviewKind {
    /// Fixed native disclosure copy. Page text is never interpolated here.
    #[must_use]
    pub const fn volatile_disclosure(self) -> Option<&'static str> {
        match self {
            Self::StashClean => Some(
                "This live stash is volatile. Closing the browser or a renderer crash may lose it. Nothing is saved to disk.",
            ),
            Self::StashDirty => Some(
                "This page has unsaved edits. A live stash keeps them only in memory; it does not save them. Closing the browser or a renderer crash may lose them.",
            ),
            Self::Merge
            | Self::Restore
            | Self::Drop
            | Self::PageArchive
            | Self::PageArchiveEditable
            | Self::PageArchiveSelection
            | Self::PageArchiveEditableSelection
            | Self::PageArchivePartial
            | Self::PageArchiveEditablePartial
            | Self::PageArchiveSelectionPartial
            | Self::PageArchiveEditableSelectionPartial => None,
        }
    }

    /// Fixed-copy native disclosure for durable page capture. It names both
    /// the archive and bounded same-origin sidecar, plus the local-file risk.
    #[must_use]
    pub const fn page_capture_disclosure(self) -> Option<&'static str> {
        match self {
            Self::PageArchive => Some(
                "Save a page archive and bounded same-origin page-state metadata for scroll positions, supported control state, and available media position/paused state for this exact current scope? Media never resumes automatically. For supported media, sidecar metadata stores a deterministic, unkeyed digest of the source URL for matching; this is not encryption, common URLs may be guessed, and the same source may be correlated across sidecars. The archive may include personal data, embedded secrets, and resource URLs. Ferric does not sanitize it. Saved archives may omit resources and are not guaranteed to be atomic snapshots. The archive and metadata use restrictive local permissions but are not encrypted at rest and may be read by the same OS account or retained in backups.",
            ),
            Self::PageArchiveEditable => Some(
                "Save a page archive, bounded same-origin page-state metadata for scroll positions, supported control state, and available media position/paused state, plus requested editable values for this exact current scope? Media never resumes automatically. For supported media, sidecar metadata stores a deterministic, unkeyed digest of the source URL for matching; this is not encryption, common URLs may be guessed, and the same source may be correlated across sidecars. The archive may include personal data, embedded secrets, and resource URLs. Ferric does not sanitize it. Saved archives may omit resources and are not guaranteed to be atomic snapshots. Editable text may also contain secrets. The archive and metadata use restrictive local permissions but are not encrypted at rest and may be read by the same OS account or retained in backups.",
            ),
            Self::PageArchiveSelection => Some(
                "Save a page archive, bounded same-origin page-state metadata for scroll positions, supported control state, and available media position/paused state, plus the current selection position for this exact current scope? Media never resumes automatically. For supported media, sidecar metadata stores a deterministic, unkeyed digest of the source URL for matching; this is not encryption, common URLs may be guessed, and the same source may be correlated across sidecars. The archive may include personal data, embedded secrets, and resource URLs. Ferric does not sanitize it. Saved archives may omit resources and are not guaranteed to be atomic snapshots. Selection text itself is not stored. The archive and metadata use restrictive local permissions but are not encrypted at rest and may be read by the same OS account or retained in backups.",
            ),
            Self::PageArchiveEditableSelection => Some(
                "Save a page archive, bounded same-origin page-state metadata for scroll positions, supported control state, and available media position/paused state, requested editable values, and current selection position for this exact current scope? Media never resumes automatically. For supported media, sidecar metadata stores a deterministic, unkeyed digest of the source URL for matching; this is not encryption, common URLs may be guessed, and the same source may be correlated across sidecars. The archive may include personal data, embedded secrets, and resource URLs. Ferric does not sanitize it. Saved archives may omit resources and are not guaranteed to be atomic snapshots. Editable text may also contain secrets; selection text itself is not stored. The archive and metadata use restrictive local permissions but are not encrypted at rest and may be read by the same OS account or retained in backups.",
            ),
            Self::PageArchivePartial => Some(
                "Save page archives and bounded same-origin page-state metadata for this window? If a page fails, Ferric may save the successful pages and record the failure. Media never resumes automatically. For supported media, sidecar metadata stores a deterministic, unkeyed digest of the source URL for matching; this is not encryption, common URLs may be guessed, and the same source may be correlated across sidecars. Archives may include personal data, embedded secrets, and resource URLs; Ferric does not sanitize them. Saved archives may omit resources and are not guaranteed to be atomic snapshots. Local files are not encrypted at rest.",
            ),
            Self::PageArchiveEditablePartial => Some(
                "Save page archives, bounded same-origin page-state metadata, and requested editable values for this window? If a page fails, Ferric may save the successful pages and record the failure. Media never resumes automatically. For supported media, sidecar metadata stores a deterministic, unkeyed digest of the source URL for matching; this is not encryption, common URLs may be guessed, and the same source may be correlated across sidecars. Archives and editable text may contain secrets and resource URLs; Ferric does not sanitize them. Saved archives may omit resources and are not guaranteed to be atomic snapshots. Local files are not encrypted at rest.",
            ),
            Self::PageArchiveSelectionPartial => Some(
                "Save page archives, bounded same-origin page-state metadata, and selection positions for this window? If a page fails, Ferric may save the successful pages and record the failure. Selection text is not stored and media never resumes automatically. For supported media, sidecar metadata stores a deterministic, unkeyed digest of the source URL for matching; this is not encryption, common URLs may be guessed, and the same source may be correlated across sidecars. Archives may contain secrets and resource URLs; Ferric does not sanitize them. Saved archives may omit resources and are not guaranteed to be atomic snapshots. Local files are not encrypted at rest.",
            ),
            Self::PageArchiveEditableSelectionPartial => Some(
                "Save page archives, bounded same-origin page-state metadata, requested editable values, and selection positions for this window? If a page fails, Ferric may save the successful pages and record the failure. Selection text is not stored and media never resumes automatically. For supported media, sidecar metadata stores a deterministic, unkeyed digest of the source URL for matching; this is not encryption, common URLs may be guessed, and the same source may be correlated across sidecars. Archives and editable text may contain secrets and resource URLs; Ferric does not sanitize them. Saved archives may omit resources and are not guaranteed to be atomic snapshots. Local files are not encrypted at rest.",
            ),
            Self::Merge | Self::Restore | Self::Drop | Self::StashClean | Self::StashDirty => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreviewChoice {
    pub item: u16,
    pub resolution: ResolutionChoice,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PreviewCounts {
    pub additions: u16,
    pub updates: u16,
    pub deletions: u16,
    pub conflicts: u16,
    pub unavailable: u16,
    pub unchanged: u16,
}

#[derive(Eq, PartialEq)]
pub struct PreviewDraft {
    kind: PreviewKind,
    counts: PreviewCounts,
    choices: Vec<PreviewChoice>,
    risky: bool,
    approvable: bool,
    payload: AuthorizedPreview,
}

#[derive(Eq, PartialEq)]
pub enum AuthorizedPreview {
    Merge(CookieMergePlan),
    Restore(CommitId),
    Drop(BranchId),
    Stash {
        owner: CapturedOwner,
        dirty: bool,
    },
    PageCapture {
        page_count: u16,
        editable: bool,
        selection: bool,
        failure_policy: PageCaptureFailurePolicy,
    },
}

impl fmt::Debug for AuthorizedPreview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Merge(_) => "AuthorizedPreview::Merge([redacted])",
            Self::Restore(_) => "AuthorizedPreview::Restore([redacted])",
            Self::Drop(_) => "AuthorizedPreview::Drop([redacted])",
            Self::Stash { .. } => "AuthorizedPreview::Stash([redacted])",
            Self::PageCapture { .. } => "AuthorizedPreview::PageCapture([redacted])",
        })
    }
}

impl fmt::Debug for PreviewDraft {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreviewDraft")
            .field("kind", &self.kind)
            .field("counts", &self.counts)
            .field("risky", &self.risky)
            .field("approvable", &self.approvable)
            .finish_non_exhaustive()
    }
}

impl PreviewDraft {
    /// Projects only decision counts. Cookie names, values, hosts and paths
    /// remain in the application-only payload, never in the presentation
    /// preview or IPC DTO.
    ///
    /// # Errors
    /// Rejects oversized, duplicate, out-of-range or non-conflict choices.
    pub fn merge(
        plan: &CookieMergePlan,
        choices: impl IntoIterator<Item = PreviewChoice>,
    ) -> Result<Self, PreviewError> {
        if plan.items().len() > MAX_PREVIEW_ITEMS {
            return Err(PreviewError::TooManyItems);
        }
        let mut counts = PreviewCounts::default();
        let mut conflict_items = BTreeSet::new();
        for (index, item) in plan.items().iter().enumerate() {
            let count = match item.decision() {
                CookieMergeDecision::ProposeSet if item.expected_parent().is_some() => {
                    &mut counts.updates
                }
                CookieMergeDecision::ProposeSet => &mut counts.additions,
                CookieMergeDecision::ProposeDelete => &mut counts.deletions,
                CookieMergeDecision::Conflict => {
                    conflict_items
                        .insert(u16::try_from(index).map_err(|_| PreviewError::TooManyItems)?);
                    &mut counts.conflicts
                }
                CookieMergeDecision::Unavailable => &mut counts.unavailable,
                CookieMergeDecision::KeepParent | CookieMergeDecision::Converged => {
                    &mut counts.unchanged
                }
            };
            *count += 1;
        }
        let mut choices: Vec<_> = choices.into_iter().collect();
        if choices.len() > MAX_PREVIEW_ITEMS {
            return Err(PreviewError::TooManyItems);
        }
        choices.sort_unstable_by_key(|choice| choice.item);
        let chosen: BTreeSet<_> = choices.iter().map(|choice| choice.item).collect();
        if chosen.len() != choices.len() || !chosen.is_subset(&conflict_items) {
            return Err(PreviewError::InvalidChoices);
        }
        Ok(Self {
            kind: PreviewKind::Merge,
            counts,
            choices,
            risky: !plan.items().is_empty(),
            approvable: counts.unavailable == 0 && chosen == conflict_items,
            payload: AuthorizedPreview::Merge(plan.clone()),
        })
    }

    /// A restore preview describes replacement and policy risk without page
    /// URLs, editable content or a native profile pointer.
    ///
    /// # Errors
    /// Rejects an empty or excessive target set.
    pub fn restore(
        target: CommitId,
        page_count: u16,
        replaces_current_window: bool,
        applies_old_policies: bool,
    ) -> Result<Self, PreviewError> {
        if page_count == 0 || usize::from(page_count) > MAX_PREVIEW_ITEMS {
            return Err(PreviewError::TooManyItems);
        }
        Ok(Self {
            kind: PreviewKind::Restore,
            counts: PreviewCounts {
                additions: page_count,
                deletions: u16::from(replaces_current_window),
                ..PreviewCounts::default()
            },
            choices: Vec::new(),
            risky: replaces_current_window || applies_old_policies,
            approvable: true,
            payload: AuthorizedPreview::Restore(target),
        })
    }

    /// A drop preview exposes only a bounded count of at-risk live items.
    ///
    /// # Errors
    /// Rejects an empty or excessive target set.
    pub fn drop(target: BranchId, only_live_count: u16) -> Result<Self, PreviewError> {
        if only_live_count == 0 || usize::from(only_live_count) > MAX_PREVIEW_ITEMS {
            return Err(PreviewError::TooManyItems);
        }
        Ok(Self {
            kind: PreviewKind::Drop,
            counts: PreviewCounts {
                deletions: only_live_count,
                ..PreviewCounts::default()
            },
            choices: Vec::new(),
            risky: true,
            approvable: true,
            payload: AuthorizedPreview::Drop(target),
        })
    }

    /// Disclosure for one already-admitted live view. It never contains page
    /// text; the kind selects fixed native copy for clean or dirty work.
    #[must_use]
    pub fn stash(owner: CapturedOwner, dirty: bool) -> Self {
        Self {
            kind: if dirty {
                PreviewKind::StashDirty
            } else {
                PreviewKind::StashClean
            },
            counts: PreviewCounts::default(),
            choices: Vec::new(),
            risky: true,
            approvable: true,
            payload: AuthorizedPreview::Stash { owner, dirty },
        }
    }

    /// Fixed-count consent for a bounded durable page capture. The page
    /// contents, URL and title are deliberately not part of this preview.
    ///
    /// # Errors
    /// Rejects empty or excessive page sets.
    pub fn page_capture(page_count: u16, editable: bool) -> Result<Self, PreviewError> {
        Self::page_capture_with_options(page_count, editable, false)
    }

    /// Fixed-count consent for a durable page capture with separately opted-in
    /// editable values and/or selection position metadata.
    ///
    /// # Errors
    /// Rejects empty or excessive page sets.
    pub fn page_capture_with_options(
        page_count: u16,
        editable: bool,
        selection: bool,
    ) -> Result<Self, PreviewError> {
        Self::page_capture_with_failure_policy(
            page_count,
            editable,
            selection,
            PageCaptureFailurePolicy::Strict,
        )
    }

    /// Fixed-copy consent for a multi-page window that may publish an
    /// explicitly failed page only when this policy was shown and accepted.
    ///
    /// # Errors
    /// Rejects empty/excessive page sets or a partial policy for one page.
    pub fn page_capture_with_failure_policy(
        page_count: u16,
        editable: bool,
        selection: bool,
        failure_policy: PageCaptureFailurePolicy,
    ) -> Result<Self, PreviewError> {
        if page_count == 0 || usize::from(page_count) > MAX_PREVIEW_ITEMS {
            return Err(PreviewError::TooManyItems);
        }
        if failure_policy == PageCaptureFailurePolicy::AllowPartialPages && page_count < 2 {
            return Err(PreviewError::InvalidChoices);
        }
        Ok(Self {
            kind: match (failure_policy, editable, selection) {
                (PageCaptureFailurePolicy::Strict, false, false) => PreviewKind::PageArchive,
                (PageCaptureFailurePolicy::Strict, true, false) => PreviewKind::PageArchiveEditable,
                (PageCaptureFailurePolicy::Strict, false, true) => {
                    PreviewKind::PageArchiveSelection
                }
                (PageCaptureFailurePolicy::Strict, true, true) => {
                    PreviewKind::PageArchiveEditableSelection
                }
                (PageCaptureFailurePolicy::AllowPartialPages, false, false) => {
                    PreviewKind::PageArchivePartial
                }
                (PageCaptureFailurePolicy::AllowPartialPages, true, false) => {
                    PreviewKind::PageArchiveEditablePartial
                }
                (PageCaptureFailurePolicy::AllowPartialPages, false, true) => {
                    PreviewKind::PageArchiveSelectionPartial
                }
                (PageCaptureFailurePolicy::AllowPartialPages, true, true) => {
                    PreviewKind::PageArchiveEditableSelectionPartial
                }
            },
            counts: PreviewCounts {
                additions: page_count,
                ..PreviewCounts::default()
            },
            choices: Vec::new(),
            risky: true,
            approvable: true,
            payload: AuthorizedPreview::PageCapture {
                page_count,
                editable,
                selection,
                failure_policy,
            },
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
struct PreviewPresentation {
    kind: PreviewKind,
    counts: PreviewCounts,
    choices: Vec<PreviewChoice>,
    risky: bool,
    approvable: bool,
}

/// This is the only projection intended for routine IPC. It has no raw
/// identity or secret fields and is not a confirmation token.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RedactedPreview {
    pub kind: PreviewKind,
    pub counts: PreviewCounts,
    pub authentication_risk: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub volatile_work_risk: bool,
    pub approvable: bool,
}

// serde's skip_serializing_if callback takes a reference by contract.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Eq, PartialEq)]
pub struct Preview {
    id: PlanId,
    operation: OperationId,
    expires_at_ms: u64,
    draft: PreviewPresentation,
}

impl fmt::Debug for Preview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Preview")
            .field("id", &self.id)
            .field("kind", &self.draft.kind)
            .field("counts", &self.draft.counts)
            .finish_non_exhaustive()
    }
}

impl Preview {
    #[must_use]
    pub const fn id(&self) -> PlanId {
        self.id
    }

    #[must_use]
    pub const fn operation(&self) -> OperationId {
        self.operation
    }

    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    #[must_use]
    pub const fn redacted(&self) -> RedactedPreview {
        let volatile_work_risk = matches!(
            self.draft.kind,
            PreviewKind::StashClean | PreviewKind::StashDirty
        );
        RedactedPreview {
            kind: self.draft.kind,
            counts: self.draft.counts,
            authentication_risk: self.draft.risky && !volatile_work_risk,
            volatile_work_risk,
            approvable: self.draft.approvable,
        }
    }

    #[must_use]
    pub fn choices(&self) -> &[PreviewChoice] {
        &self.draft.choices
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreviewError {
    InvalidPlanId,
    TooManyItems,
    InvalidChoices,
    TooManyPlans,
    DuplicateOperation,
    WrongState,
    WrongOwner,
    StalePlan,
    Expired,
    NotApprovable,
    InvalidDecision,
    AlreadyDecided,
}

impl fmt::Display for PreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "versioning preview rejected: {self:?}")
    }
}

impl std::error::Error for PreviewError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersioningConsentError {
    Operation(OperationError),
    Preview(PreviewError),
}

impl From<OperationError> for VersioningConsentError {
    fn from(error: OperationError) -> Self {
        Self::Operation(error)
    }
}

impl From<PreviewError> for VersioningConsentError {
    fn from(error: PreviewError) -> Self {
        Self::Preview(error)
    }
}

impl fmt::Display for VersioningConsentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "versioning consent rejected: {self:?}")
    }
}

impl std::error::Error for VersioningConsentError {}

/// A non-serializable, non-clonable capability handed once to the trusted
/// native UI adapter. Page, command and IPC handlers must never receive it.
pub struct NativeDecisionSink {
    authority: Uuid,
}

impl fmt::Debug for NativeDecisionSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NativeDecisionSink([redacted])")
    }
}

impl NativeDecisionSink {
    /// Call only after the user accepts the exact visible native preview.
    /// This is deliberately not a boolean or a serializable IPC argument.
    #[must_use]
    pub fn accept(&self, preview: &Preview) -> NativeDecisionProof {
        NativeDecisionProof {
            authority: self.authority,
            preview: preview.clone(),
        }
    }
}

pub struct NativeDecisionProof {
    authority: Uuid,
    preview: Preview,
}

impl fmt::Debug for NativeDecisionProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NativeDecisionProof([redacted])")
    }
}

pub struct ConfirmationPermit {
    plan: PlanId,
    nonce: Box<Uuid>,
}

impl fmt::Debug for ConfirmationPermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ConfirmationPermit([redacted])")
    }
}

struct PendingPreview {
    preview: Preview,
    payload: AuthorizedPreview,
    envelope: OperationEnvelope,
    security_revision: u64,
    permit_nonce: Option<Uuid>,
}

#[derive(Clone, Copy)]
pub(crate) struct PreviewContext<'a> {
    pub envelope: &'a OperationEnvelope,
    pub state: OperationState,
    pub current_owner: Option<NativeProfileOwner>,
    pub configuration_revision: u64,
    pub security_revision: u64,
    pub now_ms: u64,
}

/// Exact in-memory equality is the confirmation seal: no lossy hash of a
/// secret-bearing plan is serialized or used as authority.
pub(crate) struct PreviewBook {
    authority: Uuid,
    native_sink_taken: bool,
    pending: Vec<PendingPreview>,
}

impl Default for PreviewBook {
    fn default() -> Self {
        Self {
            authority: Uuid::new_v4(),
            native_sink_taken: false,
            pending: Vec::new(),
        }
    }
}

impl PreviewBook {
    /// Transfer once to the trusted native adapter during startup, never to
    /// a page, command parser or IPC request handler.
    pub fn take_native_sink(&mut self) -> Option<NativeDecisionSink> {
        if self.native_sink_taken {
            None
        } else {
            self.native_sink_taken = true;
            Some(NativeDecisionSink {
                authority: self.authority,
            })
        }
    }

    /// # Errors
    /// Rejects an unbound, stale or excessive plan.
    pub fn prepare(
        &mut self,
        context: PreviewContext<'_>,
        draft: PreviewDraft,
    ) -> Result<Preview, PreviewError> {
        self.reap(context.now_ms);
        let expected = match draft.kind {
            PreviewKind::Merge => SensitiveOperation::MergeApply,
            PreviewKind::Restore => SensitiveOperation::Restore,
            PreviewKind::Drop => SensitiveOperation::Drop,
            PreviewKind::StashClean | PreviewKind::StashDirty => SensitiveOperation::Stash,
            PreviewKind::PageArchive
            | PreviewKind::PageArchiveEditable
            | PreviewKind::PageArchiveSelection
            | PreviewKind::PageArchiveEditableSelection
            | PreviewKind::PageArchivePartial
            | PreviewKind::PageArchiveEditablePartial
            | PreviewKind::PageArchiveSelectionPartial
            | PreviewKind::PageArchiveEditableSelectionPartial => SensitiveOperation::PageCapture,
        };
        if context.state != OperationState::AwaitingConsent
            || context.envelope.kind() != OperationKind::Sensitive(expected)
        {
            return Err(PreviewError::WrongState);
        }
        if context.current_owner != Some(context.envelope.owner()) {
            return Err(PreviewError::WrongOwner);
        }
        if context.envelope.configuration_revision() != context.configuration_revision {
            return Err(PreviewError::StalePlan);
        }
        let expires_at_ms = context
            .now_ms
            .saturating_add(CONFIRMATION_PLAN_TTL_MS)
            .min(context.envelope.deadline_ms());
        if context.now_ms >= expires_at_ms {
            return Err(PreviewError::Expired);
        }
        if self.pending.len() >= MAX_PENDING_PREVIEWS {
            return Err(PreviewError::TooManyPlans);
        }
        if self
            .pending
            .iter()
            .any(|pending| pending.preview.operation == context.envelope.id())
        {
            return Err(PreviewError::DuplicateOperation);
        }
        let PreviewDraft {
            kind,
            counts,
            choices,
            risky,
            approvable,
            payload,
        } = draft;
        let preview = Preview {
            id: PlanId(Uuid::new_v4()),
            operation: context.envelope.id(),
            expires_at_ms,
            draft: PreviewPresentation {
                kind,
                counts,
                choices,
                risky,
                approvable,
            },
        };
        self.pending.push(PendingPreview {
            preview: preview.clone(),
            payload,
            envelope: context.envelope.clone(),
            security_revision: context.security_revision,
            permit_nonce: None,
        });
        Ok(preview)
    }

    /// # Errors
    /// Rejects a fabricated, altered, stale, expired or repeated native proof.
    pub fn capture_native_decision(
        &mut self,
        proof: NativeDecisionProof,
        context: PreviewContext<'_>,
    ) -> Result<ConfirmationPermit, PreviewError> {
        let NativeDecisionProof { authority, preview } = proof;
        if authority != self.authority {
            return Err(PreviewError::InvalidDecision);
        }
        let pending = self
            .pending
            .iter_mut()
            .find(|pending| pending.preview.id == preview.id)
            .ok_or(PreviewError::StalePlan)?;
        Self::validate(pending, context)?;
        if preview != pending.preview {
            return Err(PreviewError::InvalidDecision);
        }
        if !pending.preview.draft.approvable {
            return Err(PreviewError::NotApprovable);
        }
        if pending.permit_nonce.is_some() {
            return Err(PreviewError::AlreadyDecided);
        }
        let nonce = Uuid::new_v4();
        pending.permit_nonce = Some(nonce);
        Ok(ConfirmationPermit {
            plan: pending.preview.id,
            nonce: Box::new(nonce),
        })
    }

    /// Consumes a matching permit exactly once. A caller may then advance the
    /// already-validated operation; any failure after consumption fails closed.
    ///
    /// # Errors
    /// Rejects replay, changed scope/owner/choice/revision or expiry.
    pub fn consume(
        &mut self,
        permit: ConfirmationPermit,
        context: PreviewContext<'_>,
    ) -> Result<AuthorizedPreview, PreviewError> {
        let ConfirmationPermit { plan, nonce } = permit;
        let index = self
            .pending
            .iter()
            .position(|pending| pending.preview.id == plan)
            .ok_or(PreviewError::StalePlan)?;
        let pending = &self.pending[index];
        Self::validate(pending, context)?;
        if pending.permit_nonce != Some(*nonce) {
            return Err(PreviewError::InvalidDecision);
        }
        Ok(self.pending.swap_remove(index).payload)
    }

    fn validate(pending: &PendingPreview, context: PreviewContext<'_>) -> Result<(), PreviewError> {
        if context.state != OperationState::AwaitingConsent {
            return Err(PreviewError::WrongState);
        }
        if context.current_owner != Some(context.envelope.owner()) {
            return Err(PreviewError::WrongOwner);
        }
        if context.now_ms >= pending.preview.expires_at_ms
            || context.now_ms >= context.envelope.deadline_ms()
        {
            return Err(PreviewError::Expired);
        }
        if &pending.envelope != context.envelope
            || pending.preview.operation != context.envelope.id()
            || pending.security_revision != context.security_revision
            || context.envelope.configuration_revision() != context.configuration_revision
        {
            return Err(PreviewError::StalePlan);
        }
        Ok(())
    }

    pub fn invalidate_operation(&mut self, operation: OperationId) {
        self.pending
            .retain(|pending| pending.preview.operation != operation);
    }

    pub fn invalidate_owner(&mut self, owner: NativeProfileOwner) {
        self.pending
            .retain(|pending| pending.envelope.owner() != owner);
    }

    pub fn invalidate_all(&mut self) {
        self.pending.clear();
    }

    pub fn reap(&mut self, now_ms: u64) {
        self.pending
            .retain(|pending| now_ms < pending.preview.expires_at_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::versioning::{OperationKind, VersioningCoordinator};
    use ferric_browser_core::{
        ApplicationOwnerId, PrivacyKind, ProfileInstanceId, ScopeSelector,
        versioning::{
            CookieKey, CookieKeyInput, CookieObservation, CookieRecord, CookieRecordInput,
            CookieScope, CookieSnapshot, CookieSnapshotMeta, IdentityProvenance, PartitionIdentity,
            RecordFidelity, SameSite, SnapshotCompleteness, SnapshotConsistency,
            SnapshotIdentityFidelity, plan_cookie_merge,
        },
    };
    use ferric_browser_runtime::BrowserRuntime;
    use std::{cell::Cell, collections::BTreeMap, fmt::Write, rc::Rc};

    fn fixture(
        purpose: SensitiveOperation,
    ) -> (
        VersioningCoordinator,
        PreviewBook,
        OperationId,
        Rc<Cell<u64>>,
    ) {
        let (runtime, window, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "test").unwrap();
        let profile = runtime.view().windows().get(&window).unwrap().profile;
        let owner = NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            profile,
            ProfileInstanceId::fresh(),
        );
        let scope = runtime
            .view()
            .resolve_versioning_scope(ScopeSelector::Window { window }, owner)
            .unwrap();
        let time = Rc::new(Cell::new(10));
        let clock = Rc::clone(&time);
        let mut coordinator =
            VersioningCoordinator::with_sources(move || clock.get(), OperationId::fresh);
        coordinator.bind_owner(owner);
        let (operation, _) = coordinator
            .admit(
                scope,
                4,
                PrivacyKind::Normal,
                OperationKind::Sensitive(purpose),
                120_000,
            )
            .unwrap();
        coordinator
            .advance(operation, OperationState::Validating)
            .unwrap();
        coordinator
            .advance(operation, OperationState::AwaitingConsent)
            .unwrap();
        (coordinator, PreviewBook::default(), operation, time)
    }

    fn prepare(
        coordinator: &VersioningCoordinator,
        book: &mut PreviewBook,
        operation: OperationId,
        security_revision: u64,
        draft: PreviewDraft,
    ) -> Preview {
        book.prepare(context(coordinator, operation, security_revision), draft)
            .unwrap()
    }

    fn context(
        coordinator: &VersioningCoordinator,
        operation: OperationId,
        security_revision: u64,
    ) -> PreviewContext<'_> {
        let (envelope, state) = coordinator.operation(operation).unwrap();
        PreviewContext {
            envelope,
            state,
            current_owner: coordinator.owner(envelope.owner().runtime_profile()),
            configuration_revision: 4,
            security_revision,
            now_ms: coordinator.now_millis(),
        }
    }

    fn capture(
        coordinator: &VersioningCoordinator,
        book: &mut PreviewBook,
        operation: OperationId,
        proof: NativeDecisionProof,
        security_revision: u64,
    ) -> Result<ConfirmationPermit, PreviewError> {
        book.capture_native_decision(proof, context(coordinator, operation, security_revision))
    }

    fn consume(
        coordinator: &VersioningCoordinator,
        book: &mut PreviewBook,
        operation: OperationId,
        permit: ConfirmationPermit,
        security_revision: u64,
    ) -> Result<AuthorizedPreview, PreviewError> {
        book.consume(permit, context(coordinator, operation, security_revision))
    }

    fn merge_plan(
        base: Option<&[u8]>,
        parent: Option<&[u8]>,
        child: Option<&[u8]>,
    ) -> CookieMergePlan {
        let key = CookieKey::new(CookieKeyInput {
            backend_identity_version: 1,
            name: b"sid".to_vec(),
            domain: "example.test".into(),
            path: "/".into(),
            host_only: true,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Unpartitioned,
            backend_dimensions: BTreeMap::new(),
        })
        .unwrap();
        let snapshot = |value: Option<&[u8]>| {
            CookieSnapshot::new(
                CookieSnapshotMeta {
                    backend_identity_version: 1,
                    required_identity_dimensions: BTreeSet::new(),
                    scope: CookieScope::FullProfile,
                    identity: SnapshotIdentityFidelity::Lossless,
                    completeness: SnapshotCompleteness::Complete,
                    consistency: SnapshotConsistency::Consistent,
                    supported_attributes: BTreeSet::new(),
                },
                value.into_iter().map(|bytes| {
                    CookieObservation::Present(
                        CookieRecord::new(CookieRecordInput {
                            key: key.clone(),
                            value: bytes.to_vec(),
                            expiry: ferric_browser_core::versioning::CookieExpiry::Session,
                            secure: true,
                            http_only: true,
                            same_site: SameSite::Lax,
                            extra_attributes: BTreeMap::new(),
                            fidelity: RecordFidelity::Exact,
                        })
                        .unwrap(),
                    )
                }),
            )
            .unwrap()
        };
        plan_cookie_merge(&snapshot(base), &snapshot(parent), &snapshot(child), 100).unwrap()
    }

    #[test]
    fn partial_page_policy_has_distinct_bounded_disclosure_for_each_opt_in() {
        for (editable, selection, expected) in [
            (false, false, PreviewKind::PageArchivePartial),
            (true, false, PreviewKind::PageArchiveEditablePartial),
            (false, true, PreviewKind::PageArchiveSelectionPartial),
            (true, true, PreviewKind::PageArchiveEditableSelectionPartial),
        ] {
            let draft = PreviewDraft::page_capture_with_failure_policy(
                2,
                editable,
                selection,
                PageCaptureFailurePolicy::AllowPartialPages,
            )
            .expect("two-page partial preview");
            assert_eq!(draft.kind, expected);
            let disclosure = expected.page_capture_disclosure().expect("fixed copy");
            assert!(disclosure.contains("may save the successful pages"));
            assert!(disclosure.contains("record the failure"));
            assert!(disclosure.contains("Saved archives may omit resources"));
            assert!(disclosure.contains("not guaranteed to be atomic snapshots"));
            assert!(disclosure.contains("not encrypted at rest"));
            assert!(disclosure.contains("deterministic, unkeyed digest of the source URL"));
            assert!(disclosure.contains("common URLs may be guessed"));
            assert!(disclosure.contains("correlated across sidecars"));
            assert_eq!(disclosure.contains("editable values"), editable);
            assert_eq!(disclosure.contains("selection positions"), selection);
        }
        assert_eq!(
            PreviewDraft::page_capture_with_failure_policy(
                1,
                false,
                false,
                PageCaptureFailurePolicy::AllowPartialPages,
            ),
            Err(PreviewError::InvalidChoices)
        );
    }

    #[test]
    fn every_page_capture_disclosure_warns_about_incomplete_non_atomic_archives() {
        for kind in [
            PreviewKind::PageArchive,
            PreviewKind::PageArchiveEditable,
            PreviewKind::PageArchiveSelection,
            PreviewKind::PageArchiveEditableSelection,
            PreviewKind::PageArchivePartial,
            PreviewKind::PageArchiveEditablePartial,
            PreviewKind::PageArchiveSelectionPartial,
            PreviewKind::PageArchiveEditableSelectionPartial,
        ] {
            let disclosure = kind.page_capture_disclosure().expect("capture disclosure");
            assert!(disclosure.contains("Saved archives may omit resources"));
            assert!(disclosure.contains("not guaranteed to be atomic snapshots"));
            assert!(disclosure.contains("deterministic, unkeyed digest of the source URL"));
            assert!(disclosure.contains("common URLs may be guessed"));
            assert!(disclosure.contains("correlated across sidecars"));
        }
    }

    #[test]
    fn merge_projection_requires_exact_conflict_choices_and_never_exposes_cookie_bytes() {
        let conflict = merge_plan(
            Some(b"base-secret"),
            Some(b"parent-secret"),
            Some(b"child-secret"),
        );
        let unresolved = PreviewDraft::merge(&conflict, []).unwrap();
        assert!(!unresolved.approvable);
        assert_eq!(unresolved.counts.conflicts, 1);
        assert_eq!(
            PreviewDraft::merge(
                &conflict,
                [PreviewChoice {
                    item: 1,
                    resolution: ResolutionChoice::Branch
                }]
            ),
            Err(PreviewError::InvalidChoices)
        );
        assert_eq!(
            PreviewDraft::merge(
                &conflict,
                [
                    PreviewChoice {
                        item: 0,
                        resolution: ResolutionChoice::Branch
                    },
                    PreviewChoice {
                        item: 0,
                        resolution: ResolutionChoice::Main
                    },
                ]
            ),
            Err(PreviewError::InvalidChoices)
        );
        let resolved = PreviewDraft::merge(
            &conflict,
            [PreviewChoice {
                item: 0,
                resolution: ResolutionChoice::Branch,
            }],
        )
        .unwrap();
        assert!(resolved.approvable);
        let mut text = format!("{conflict:?} {unresolved:?} {resolved:?}");
        let (coordinator, mut book, operation, _) = fixture(SensitiveOperation::MergeApply);
        let preview = prepare(&coordinator, &mut book, operation, 9, resolved);
        write!(text, " {preview:?} {:?}", preview.redacted()).unwrap();
        let ipc = serde_json::to_string(&preview.redacted()).unwrap();
        text.push_str(&ipc);
        for secret in [
            "base-secret",
            "parent-secret",
            "child-secret",
            "sid",
            "example.test",
        ] {
            assert!(
                !text.contains(secret),
                "preview leaked synthetic cookie material"
            );
        }
        assert_eq!(preview.redacted().kind, PreviewKind::Merge);
        assert_eq!(preview.choices()[0].resolution, ResolutionChoice::Branch);
        assert_eq!(preview.id().to_string().parse::<PlanId>(), Ok(preview.id()));
        assert_eq!(
            format!("operation:{}", preview.id().0)
                .parse::<PlanId>()
                .unwrap_err(),
            PreviewError::InvalidPlanId
        );
        assert!(!ipc.contains("plan:"));
        let sink = book.take_native_sink().unwrap();
        let permit = capture(&coordinator, &mut book, operation, sink.accept(&preview), 9).unwrap();
        assert_eq!(
            consume(&coordinator, &mut book, operation, permit, 9),
            Ok(AuthorizedPreview::Merge(conflict))
        );
    }

    #[test]
    fn native_decision_and_permit_bind_exact_preview_and_reject_replay() {
        let (mut coordinator, mut book, operation, _) = fixture(SensitiveOperation::Drop);
        let sink = book.take_native_sink().unwrap();
        assert!(book.take_native_sink().is_none());
        let preview = prepare(
            &coordinator,
            &mut book,
            operation,
            9,
            PreviewDraft::drop(BranchId::fresh(), 2).unwrap(),
        );
        let mut altered = preview.clone();
        altered.draft.counts.deletions = 3;
        assert_eq!(
            capture(&coordinator, &mut book, operation, sink.accept(&altered), 9).unwrap_err(),
            PreviewError::InvalidDecision
        );
        let mut changed_choice = preview.clone();
        changed_choice.draft.choices.push(PreviewChoice {
            item: 0,
            resolution: ResolutionChoice::Main,
        });
        assert_eq!(
            capture(
                &coordinator,
                &mut book,
                operation,
                sink.accept(&changed_choice),
                9
            )
            .unwrap_err(),
            PreviewError::InvalidDecision
        );
        let permit = capture(&coordinator, &mut book, operation, sink.accept(&preview), 9).unwrap();
        assert_eq!(
            capture(&coordinator, &mut book, operation, sink.accept(&preview), 9).unwrap_err(),
            PreviewError::AlreadyDecided
        );
        let forged = ConfirmationPermit {
            plan: preview.id(),
            nonce: Box::new(Uuid::new_v4()),
        };
        assert_eq!(
            consume(&coordinator, &mut book, operation, forged, 9),
            Err(PreviewError::InvalidDecision)
        );
        consume(&coordinator, &mut book, operation, permit, 9).unwrap();
        coordinator.consent(operation, 4).unwrap();
        assert_eq!(
            coordinator.operation(operation).unwrap().1,
            OperationState::Preparing
        );
        assert_eq!(
            capture(&coordinator, &mut book, operation, sink.accept(&preview), 9).unwrap_err(),
            PreviewError::StalePlan
        );
    }

    #[test]
    fn expiry_policy_change_and_owner_replacement_fail_closed() {
        let (mut coordinator, mut book, operation, time) = fixture(SensitiveOperation::Restore);
        let sink = book.take_native_sink().unwrap();
        let preview = prepare(
            &coordinator,
            &mut book,
            operation,
            9,
            PreviewDraft::restore(CommitId::fresh(), 2, true, true).unwrap(),
        );
        assert_eq!(
            capture(
                &coordinator,
                &mut book,
                operation,
                sink.accept(&preview),
                10
            )
            .unwrap_err(),
            PreviewError::StalePlan
        );
        let permit = capture(&coordinator, &mut book, operation, sink.accept(&preview), 9).unwrap();
        time.set(preview.expires_at_ms());
        assert_eq!(
            consume(&coordinator, &mut book, operation, permit, 9),
            Err(PreviewError::Expired)
        );
        time.set(10);
        let owner = coordinator.operation(operation).unwrap().0.owner();
        coordinator.bind_owner(NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            owner.runtime_profile(),
            ProfileInstanceId::fresh(),
        ));
        assert_eq!(
            capture(&coordinator, &mut book, operation, sink.accept(&preview), 9).unwrap_err(),
            PreviewError::WrongState
        );
    }

    #[test]
    fn incomplete_cookie_plan_is_previewable_but_not_approvable() {
        let conflict = merge_plan(Some(b"A"), Some(b"B"), Some(b"C"));
        let (coordinator, mut book, operation, _) = fixture(SensitiveOperation::MergeApply);
        let sink = book.take_native_sink().unwrap();
        let preview = prepare(
            &coordinator,
            &mut book,
            operation,
            9,
            PreviewDraft::merge(&conflict, []).unwrap(),
        );
        assert_eq!(
            capture(&coordinator, &mut book, operation, sink.accept(&preview), 9).unwrap_err(),
            PreviewError::NotApprovable
        );
    }

    #[test]
    fn wrong_sensitive_purpose_cannot_receive_a_preview_or_permit() {
        let (coordinator, mut book, operation, _) = fixture(SensitiveOperation::Drop);
        assert_eq!(
            book.prepare(
                context(&coordinator, operation, 9),
                PreviewDraft::restore(CommitId::fresh(), 1, true, false).unwrap()
            ),
            Err(PreviewError::WrongState)
        );
        assert_eq!(
            book.prepare(
                context(&coordinator, operation, 9),
                PreviewDraft::merge(&merge_plan(None, None, Some(b"x")), []).unwrap()
            ),
            Err(PreviewError::WrongState)
        );
    }

    #[test]
    fn decision_is_bound_to_exact_operation_scope_and_native_authority() {
        let (mut coordinator, mut book, operation, _) = fixture(SensitiveOperation::Drop);
        let sink = book.take_native_sink().unwrap();
        let preview = prepare(
            &coordinator,
            &mut book,
            operation,
            9,
            PreviewDraft::drop(BranchId::fresh(), 1).unwrap(),
        );
        let unrelated = PreviewBook::default().take_native_sink().unwrap();
        assert_eq!(
            capture(
                &coordinator,
                &mut book,
                operation,
                unrelated.accept(&preview),
                9
            )
            .unwrap_err(),
            PreviewError::InvalidDecision
        );
        let scope = coordinator.operation(operation).unwrap().0.scope().clone();
        let (other, _) = coordinator
            .admit(
                scope,
                4,
                PrivacyKind::Normal,
                OperationKind::ReadOnly,
                120_000,
            )
            .unwrap();
        let mut wrong_context = context(&coordinator, other, 9);
        wrong_context.state = OperationState::AwaitingConsent;
        assert_eq!(
            book.capture_native_decision(sink.accept(&preview), wrong_context)
                .unwrap_err(),
            PreviewError::StalePlan
        );
        let mut missing_owner = context(&coordinator, operation, 9);
        missing_owner.current_owner = None;
        assert_eq!(
            book.capture_native_decision(sink.accept(&preview), missing_owner)
                .unwrap_err(),
            PreviewError::WrongOwner
        );
        assert!(capture(&coordinator, &mut book, operation, sink.accept(&preview), 9).is_ok());
    }
}
