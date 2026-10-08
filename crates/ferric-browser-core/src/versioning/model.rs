use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt,
};

use crate::{ApplicationState, ExistenceState, ProfileId, ShutdownState, TabId, Target, WindowId};

use super::limits::{MAX_BRANCH_NESTING, MAX_PAGES_PER_WINDOW_CAPTURE, MAX_RETAINED_VIEWS};
use super::{
    ApplicationOwnerId, BranchId, CommitId, LogicalPageId, LogicalWindowId, ProfileInstanceId,
    RetainedViewId, StashId, VersioningReason,
};

/// A live native profile binding supplied by the application/adapter. The core
/// cannot infer this from logical `ApplicationState` alone. Never persist this
/// token or accept it from untrusted IPC as authority.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct NativeProfileOwner {
    application: ApplicationOwnerId,
    runtime_profile: ProfileId,
    profile_instance: ProfileInstanceId,
}

impl fmt::Debug for NativeProfileOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NativeProfileOwner([redacted])")
    }
}

impl NativeProfileOwner {
    #[must_use]
    pub const fn new(
        application: ApplicationOwnerId,
        runtime_profile: ProfileId,
        profile_instance: ProfileInstanceId,
    ) -> Self {
        Self {
            application,
            runtime_profile,
            profile_instance,
        }
    }

    #[must_use]
    pub const fn runtime_profile(self) -> ProfileId {
        self.runtime_profile
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ScopeSelector {
    /// An already captured tab/document, never a deferred "current tab" query.
    Tab { window: WindowId, target: Target },
    /// Membership is copied in the window's stable tab order at admission.
    Window { window: WindowId },
    /// Select the existing normal-profile context containing this exact
    /// invocation window. Application resolution aggregates window scopes.
    Context { window: WindowId },
}

impl fmt::Debug for ScopeSelector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Tab { .. } => "ScopeSelector::Tab([redacted])",
            Self::Window { .. } => "ScopeSelector::Window([redacted])",
            Self::Context { .. } => "ScopeSelector::Context([redacted])",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeError {
    ContextUnavailable,
    OwnerClosed,
    MissingWindow,
    MissingProfile,
    EmptyScope,
    TooManyMembers,
    WrongProfile,
    InconsistentMembership,
    NonLiveMember,
    StaleTarget,
    StaleOwner,
    DocumentChanged,
    NativeOwnerMismatch,
    NotInScope,
}

impl ScopeError {
    #[must_use]
    pub const fn reason(self) -> VersioningReason {
        match self {
            Self::ContextUnavailable => VersioningReason::UnsupportedCapability,
            Self::TooManyMembers => VersioningReason::QuotaExceeded,
            Self::WrongProfile => VersioningReason::WrongProfile,
            Self::DocumentChanged => VersioningReason::DocumentChanged,
            Self::NativeOwnerMismatch
            | Self::OwnerClosed
            | Self::MissingWindow
            | Self::MissingProfile
            | Self::InconsistentMembership
            | Self::NonLiveMember
            | Self::StaleTarget
            | Self::StaleOwner
            | Self::NotInScope
            | Self::EmptyScope => VersioningReason::StaleOwner,
        }
    }
}

impl fmt::Display for ScopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.reason().as_str())
    }
}

impl std::error::Error for ScopeError {}

/// One immutable owner captured at admission, including the exact document.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CapturedOwner {
    native: NativeProfileOwner,
    window: WindowId,
    target: Target,
}

impl fmt::Debug for CapturedOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CapturedOwner([redacted])")
    }
}

impl CapturedOwner {
    #[must_use]
    pub const fn native(&self) -> NativeProfileOwner {
        self.native
    }
    #[must_use]
    pub const fn window(&self) -> WindowId {
        self.window
    }
    #[must_use]
    pub const fn target(&self) -> Target {
        self.target
    }
}

/// Immutable logical input; adding/switching tabs later cannot change members.
#[derive(Clone, Eq, PartialEq)]
pub struct CapturedScope {
    native: NativeProfileOwner,
    window: WindowId,
    revision: u64,
    selected_tab: Option<TabId>,
    members: Box<[CapturedOwner]>,
}

impl fmt::Debug for CapturedScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapturedScope")
            .field("revision", &self.revision)
            .field("has_selected_tab", &self.selected_tab.is_some())
            .field("member_count", &self.members.len())
            .finish()
    }
}

impl CapturedScope {
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    #[must_use]
    pub const fn window(&self) -> WindowId {
        self.window
    }
    #[must_use]
    pub const fn native(&self) -> NativeProfileOwner {
        self.native
    }
    #[must_use]
    pub const fn selected_tab(&self) -> Option<TabId> {
        self.selected_tab
    }
    #[must_use]
    pub fn members(&self) -> &[CapturedOwner] {
        &self.members
    }

    /// Validate a delayed page result against the exact admitted member and
    /// current native binding. Unrelated focus/revision changes are harmless.
    ///
    /// # Errors
    /// Rejects a closed/replaced owner, a non-member, or a replaced document.
    pub fn validate_page_result(
        &self,
        state: &ApplicationState,
        current_native: Option<NativeProfileOwner>,
        target: Target,
    ) -> Result<(), ScopeError> {
        if !self.members.iter().any(|member| member.target == target) {
            return Err(ScopeError::NotInScope);
        }
        let current_native = current_native.ok_or(ScopeError::OwnerClosed)?;
        if current_native != self.native {
            return Err(ScopeError::NativeOwnerMismatch);
        }
        if state.shutdown() != ShutdownState::Running {
            return Err(ScopeError::OwnerClosed);
        }
        let window = state
            .windows()
            .get(&self.window)
            .ok_or(ScopeError::OwnerClosed)?;
        if window.profile != self.native.runtime_profile {
            return Err(ScopeError::WrongProfile);
        }
        let tab = state
            .tabs()
            .get(&target.tab)
            .ok_or(ScopeError::StaleTarget)?;
        if tab.profile != self.native.runtime_profile {
            return Err(ScopeError::WrongProfile);
        }
        if tab.window != self.window || !window.tabs.contains(&target.tab) {
            return Err(ScopeError::StaleOwner);
        }
        if tab.existence != ExistenceState::Live {
            return Err(ScopeError::OwnerClosed);
        }
        if tab.document != target.document {
            return Err(ScopeError::DocumentChanged);
        }
        if tab.generation != target.generation {
            return Err(ScopeError::StaleTarget);
        }
        Ok(())
    }
}

/// Resolve one exact tab or one bounded, single-profile window at admission.
/// The trusted caller must provide the native binding from its owner registry.
///
/// # Errors
/// Rejects unsupported context scope, stale/mixed-profile members and overflow.
pub fn resolve_scope(
    state: &ApplicationState,
    selector: ScopeSelector,
    native: NativeProfileOwner,
) -> Result<CapturedScope, ScopeError> {
    let window_id = match selector {
        ScopeSelector::Tab { window, .. } | ScopeSelector::Window { window } => window,
        ScopeSelector::Context { .. } => return Err(ScopeError::ContextUnavailable),
    };
    if state.shutdown() != ShutdownState::Running {
        return Err(ScopeError::OwnerClosed);
    }
    let window = state
        .windows()
        .get(&window_id)
        .ok_or(ScopeError::MissingWindow)?;
    let profile = state
        .profiles()
        .get(&window.profile)
        .ok_or(ScopeError::MissingProfile)?;
    if profile.id != native.runtime_profile || window.profile != native.runtime_profile {
        return Err(ScopeError::WrongProfile);
    }
    let targets = match selector {
        ScopeSelector::Tab { target, .. } => vec![target],
        ScopeSelector::Window { .. } => {
            if window.tabs.is_empty() {
                return Err(ScopeError::EmptyScope);
            }
            if window.tabs.len() > MAX_PAGES_PER_WINDOW_CAPTURE {
                return Err(ScopeError::TooManyMembers);
            }
            let mut seen = HashSet::with_capacity(window.tabs.len());
            let mut targets = Vec::with_capacity(window.tabs.len());
            for tab_id in &window.tabs {
                if !seen.insert(*tab_id) {
                    return Err(ScopeError::InconsistentMembership);
                }
                let target = state
                    .capture_target(*tab_id)
                    .ok_or(ScopeError::NonLiveMember)?;
                targets.push(target);
            }
            targets
        }
        ScopeSelector::Context { .. } => unreachable!("rejected before window lookup"),
    };
    let mut members = Vec::with_capacity(targets.len());
    for target in targets {
        let tab = state
            .tabs()
            .get(&target.tab)
            .ok_or(ScopeError::StaleTarget)?;
        if tab.profile != window.profile {
            return Err(ScopeError::WrongProfile);
        }
        if tab.window != window_id {
            return Err(ScopeError::StaleOwner);
        }
        if !window.tabs.contains(&target.tab) {
            return Err(ScopeError::InconsistentMembership);
        }
        if tab.existence != ExistenceState::Live {
            return Err(ScopeError::NonLiveMember);
        }
        if tab.document != target.document {
            return Err(ScopeError::DocumentChanged);
        }
        if tab.generation != target.generation {
            return Err(ScopeError::StaleTarget);
        }
        members.push(CapturedOwner {
            native,
            window: window_id,
            target,
        });
    }
    Ok(CapturedScope {
        native,
        window: window_id,
        revision: state.revision(),
        selected_tab: window.active_tab,
        members: members.into_boxed_slice(),
    })
}

/// Bounded display name. Lookup uses a case-folded whitespace-normalized key;
/// names are never treated as identity when an ID is available.
#[derive(Clone, Eq, PartialEq)]
pub struct BranchName {
    display: String,
    key: String,
}

impl BranchName {
    /// # Errors
    /// Rejects empty, control-bearing or overlong names.
    pub fn new(value: &str) -> Result<Self, BranchError> {
        if value.len() > 128 || value.chars().any(char::is_control) {
            return Err(BranchError::InvalidName);
        }
        let display = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if display.is_empty() || display.len() > 128 {
            return Err(BranchError::InvalidName);
        }
        let key = display.to_lowercase();
        if key.len() > 128 {
            return Err(BranchError::InvalidName);
        }
        Ok(Self { display, key })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.display
    }
}

impl fmt::Debug for BranchName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BranchName([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InheritanceRequest {
    None,
    Clean,
    Snapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InheritanceReport {
    None,
    Clean,
    SnapshotVerified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchLifecycle {
    Preparing,
    Live,
    Retained,
    Closed,
    Lost,
}

/// Logical membership projected to the native view pool. It does not contain
/// a Qt object or grant permission to cross a profile boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMembership {
    Visible,
    Retained,
    Transferring,
    Closing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedLifecycle {
    Active,
    Frozen,
    Discarded,
    Lost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchError {
    InvalidName,
    InvalidRoot,
    MissingFork,
    UnknownBranch,
    DuplicateBranch,
    AmbiguousName,
    Cycle,
    DepthExceeded,
    WrongProfile,
    WrongApplication,
    DuplicateLiveOwner,
    InheritanceMismatch,
    InvalidTransition,
    RevisionMismatch,
    RevisionOverflow,
    TooManyPages,
    DuplicatePage,
    DuplicateRetainedView,
    DuplicateStash,
    TooManyRetainedViews,
    RetainedOwnerMismatch,
    UnknownRetainedView,
    LiveWorkOwned,
    RetainedWorkOwned,
    HasChildren,
}

impl BranchError {
    #[must_use]
    pub const fn reason(self) -> VersioningReason {
        match self {
            Self::DepthExceeded | Self::TooManyPages | Self::TooManyRetainedViews => {
                VersioningReason::QuotaExceeded
            }
            Self::WrongProfile => VersioningReason::WrongProfile,
            Self::DuplicateLiveOwner | Self::LiveWorkOwned | Self::RetainedWorkOwned => {
                VersioningReason::Busy
            }
            Self::UnknownBranch
            | Self::UnknownRetainedView
            | Self::RevisionMismatch
            | Self::RetainedOwnerMismatch => VersioningReason::StaleOwner,
            Self::RevisionOverflow => VersioningReason::QuotaExceeded,
            Self::InheritanceMismatch => VersioningReason::UnsupportedCapability,
            Self::InvalidName
            | Self::InvalidRoot
            | Self::MissingFork
            | Self::DuplicateBranch
            | Self::AmbiguousName
            | Self::Cycle
            | Self::InvalidTransition
            | Self::DuplicatePage
            | Self::WrongApplication
            | Self::DuplicateRetainedView
            | Self::DuplicateStash
            | Self::HasChildren => VersioningReason::AmbiguousReference,
        }
    }
}

impl fmt::Display for BranchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.reason().as_str())
    }
}

impl std::error::Error for BranchError {}

/// Domain transitions, admitted through `Event::Versioning` only.
#[derive(Clone, Eq, PartialEq)]
pub enum VersioningEvent {
    BindStructuralIdentity {
        window: WindowId,
        logical_window: LogicalWindowId,
        pages: Vec<(TabId, LogicalPageId)>,
    },
    BindRestoredLineage {
        window: WindowId,
        pages: Vec<(TabId, LogicalPageId)>,
    },
    BindTransientChildLineage {
        window: WindowId,
        logical_window: LogicalWindowId,
        pages: Vec<(TabId, LogicalPageId, LogicalPageId)>,
    },
    PrepareBranch {
        id: BranchId,
        name: BranchName,
        application: ApplicationOwnerId,
        profile: ProfileId,
        parent: Option<BranchId>,
        fork: Option<CommitId>,
        inheritance: InheritanceRequest,
    },
    PublishBranch {
        id: BranchId,
        native: NativeProfileOwner,
        actual: InheritanceReport,
    },
    FailBranch {
        id: BranchId,
    },
    AdvanceWorking {
        id: BranchId,
        expected_revision: u64,
        pages: Vec<LogicalPageId>,
        last_commit: Option<CommitId>,
    },
    PrepareRetained {
        id: BranchId,
        view: RetainedViewId,
        owner: CapturedOwner,
    },
    PrepareStash {
        stash: StashId,
        view: RetainedViewId,
        owner: CapturedOwner,
    },
    /// Atomically acquires both leases and records the observed native freeze.
    ConfirmFrozenStash {
        stash: StashId,
        view: RetainedViewId,
        owner: CapturedOwner,
    },
    /// Undo a native-acknowledged stash before application publication.
    RollbackFrozenStash {
        stash: StashId,
        view: RetainedViewId,
        owner: CapturedOwner,
    },
    AcquireRetained {
        view: RetainedViewId,
    },
    BeginRetainedTransfer {
        view: RetainedViewId,
        destination: NativeProfileOwner,
        window: WindowId,
    },
    CancelRetainedTransfer {
        view: RetainedViewId,
    },
    AdoptRetained {
        view: RetainedViewId,
        destination: NativeProfileOwner,
        window: WindowId,
    },
    BeginRetainedClose {
        view: RetainedViewId,
    },
    ObserveRetainedLifecycle {
        view: RetainedViewId,
        lifecycle: RetainedLifecycle,
    },
    LoseRetained {
        view: RetainedViewId,
    },
    ReleaseRetained {
        view: RetainedViewId,
    },
    RetainBranch {
        id: BranchId,
    },
    ResumeBranch {
        id: BranchId,
    },
    CloseBranch {
        id: BranchId,
    },
    DropBranch {
        id: BranchId,
    },
    SelectBranch {
        id: BranchId,
    },
}

impl fmt::Debug for VersioningEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VersioningEvent")
            .field("kind", &self.kind())
            .field("payload", &"[redacted]")
            .finish()
    }
}

impl VersioningEvent {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::BindStructuralIdentity { .. } => "capture.bind-identity",
            Self::BindRestoredLineage { .. } => "restore.bind-lineage",
            Self::BindTransientChildLineage { .. } => "branch.bind-transient-lineage",
            Self::PrepareBranch { .. } => "branch.prepare",
            Self::PublishBranch { .. } => "branch.publish",
            Self::FailBranch { .. } => "branch.fail",
            Self::AdvanceWorking { .. } => "branch.advance",
            Self::PrepareRetained { .. } => "branch.prepare-view",
            Self::PrepareStash { .. } => "stash.prepare-view",
            Self::ConfirmFrozenStash { .. } => "stash.confirm-frozen",
            Self::RollbackFrozenStash { .. } => "stash.rollback-frozen",
            Self::AcquireRetained { .. } => "branch.acquire-view",
            Self::BeginRetainedTransfer { .. } => "branch.transfer-view",
            Self::CancelRetainedTransfer { .. } => "branch.cancel-transfer-view",
            Self::AdoptRetained { .. } => "branch.adopt-view",
            Self::BeginRetainedClose { .. } => "branch.close-view",
            Self::ObserveRetainedLifecycle { .. } => "branch.observe-view",
            Self::LoseRetained { .. } => "branch.lose-view",
            Self::ReleaseRetained { .. } => "branch.release-view",
            Self::RetainBranch { .. } => "branch.retain",
            Self::ResumeBranch { .. } => "branch.resume",
            Self::CloseBranch { .. } => "branch.close",
            Self::DropBranch { .. } => "branch.drop",
            Self::SelectBranch { .. } => "branch.select",
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BranchRecord {
    id: BranchId,
    name: BranchName,
    application: ApplicationOwnerId,
    profile: ProfileId,
    parent: Option<BranchId>,
    root: BranchId,
    fork: Option<CommitId>,
    depth: usize,
    requested_inheritance: InheritanceRequest,
    actual_inheritance: Option<InheritanceReport>,
    native: Option<NativeProfileOwner>,
    pages: Vec<LogicalPageId>,
    working_revision: u64,
    last_commit: Option<CommitId>,
    retained: BTreeSet<RetainedViewId>,
    lifecycle: BranchLifecycle,
}

impl fmt::Debug for BranchRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BranchRecord")
            .field("depth", &self.depth)
            .field("requested_inheritance", &self.requested_inheritance)
            .field("actual_inheritance", &self.actual_inheritance)
            .field("has_native_owner", &self.native.is_some())
            .field("page_count", &self.pages.len())
            .field("working_revision", &self.working_revision)
            .field("has_last_commit", &self.last_commit.is_some())
            .field("retained_view_count", &self.retained.len())
            .field("lifecycle", &self.lifecycle)
            .finish()
    }
}

impl BranchRecord {
    #[must_use]
    pub const fn id(&self) -> BranchId {
        self.id
    }
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    #[must_use]
    pub const fn application(&self) -> ApplicationOwnerId {
        self.application
    }
    #[must_use]
    pub const fn profile(&self) -> ProfileId {
        self.profile
    }
    #[must_use]
    pub const fn parent(&self) -> Option<BranchId> {
        self.parent
    }
    #[must_use]
    pub const fn root(&self) -> BranchId {
        self.root
    }
    #[must_use]
    pub const fn fork(&self) -> Option<CommitId> {
        self.fork
    }
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }
    #[must_use]
    pub const fn requested_inheritance(&self) -> InheritanceRequest {
        self.requested_inheritance
    }
    #[must_use]
    pub const fn actual_inheritance(&self) -> Option<InheritanceReport> {
        self.actual_inheritance
    }
    #[must_use]
    pub const fn native(&self) -> Option<NativeProfileOwner> {
        self.native
    }
    #[must_use]
    pub fn pages(&self) -> &[LogicalPageId] {
        &self.pages
    }
    #[must_use]
    pub const fn working_revision(&self) -> u64 {
        self.working_revision
    }
    #[must_use]
    pub const fn last_commit(&self) -> Option<CommitId> {
        self.last_commit
    }
    #[must_use]
    pub const fn lifecycle(&self) -> BranchLifecycle {
        self.lifecycle
    }
    #[must_use]
    pub fn retained(&self) -> &BTreeSet<RetainedViewId> {
        &self.retained
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RetentionRecord {
    id: RetainedViewId,
    owner_id: RetentionOwner,
    owner: CapturedOwner,
    membership: ViewMembership,
    lifecycle: RetainedLifecycle,
    profile_lease_count: u8,
    view_lease_count: u8,
    restore_window: Option<WindowId>,
}

impl fmt::Debug for RetentionRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetentionRecord")
            .field("membership", &self.membership)
            .field("lifecycle", &self.lifecycle)
            .field("profile_lease_count", &self.profile_lease_count)
            .field("view_lease_count", &self.view_lease_count)
            .field("has_restore_window", &self.restore_window.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetentionOwner {
    Branch(BranchId),
    Stash(StashId),
}

impl RetentionRecord {
    #[must_use]
    pub const fn id(&self) -> RetainedViewId {
        self.id
    }
    #[must_use]
    pub const fn branch(&self) -> Option<BranchId> {
        match self.owner_id {
            RetentionOwner::Branch(id) => Some(id),
            RetentionOwner::Stash(_) => None,
        }
    }
    #[must_use]
    pub const fn stash(&self) -> Option<StashId> {
        match self.owner_id {
            RetentionOwner::Branch(_) => None,
            RetentionOwner::Stash(id) => Some(id),
        }
    }
    #[must_use]
    pub const fn owner(&self) -> CapturedOwner {
        self.owner
    }
    #[must_use]
    pub const fn membership(&self) -> ViewMembership {
        self.membership
    }
    #[must_use]
    pub const fn lifecycle(&self) -> RetainedLifecycle {
        self.lifecycle
    }
    #[must_use]
    pub const fn profile_lease_count(&self) -> u8 {
        self.profile_lease_count
    }
    #[must_use]
    pub const fn view_lease_count(&self) -> u8 {
        self.view_lease_count
    }
    #[must_use]
    pub const fn restore_window(&self) -> Option<WindowId> {
        self.restore_window
    }
}

/// Read-only to callers; the application reducer owns all mutation.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct VersioningState {
    branches: BTreeMap<BranchId, BranchRecord>,
    retained: BTreeMap<RetainedViewId, RetentionRecord>,
    current: BTreeMap<ApplicationOwnerId, BranchId>,
    logical_windows: BTreeMap<WindowId, LogicalWindowId>,
    logical_pages: BTreeMap<TabId, LogicalPageId>,
    lineage_parents: BTreeMap<TabId, LogicalPageId>,
}

impl fmt::Debug for VersioningState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VersioningState")
            .field("branch_count", &self.branches.len())
            .field("retained_view_count", &self.retained.len())
            .field("current_owner_count", &self.current.len())
            .field("logical_window_count", &self.logical_windows.len())
            .field("logical_page_count", &self.logical_pages.len())
            .field("lineage_count", &self.lineage_parents.len())
            .finish()
    }
}

impl VersioningState {
    #[must_use]
    pub fn branches(&self) -> &BTreeMap<BranchId, BranchRecord> {
        &self.branches
    }
    #[must_use]
    pub fn branch(&self, id: BranchId) -> Option<&BranchRecord> {
        self.branches.get(&id)
    }

    /// Conservatively blocks profile-wide automatic snapshots while any
    /// child-branch record remains not explicitly closed. Window/tab ownership
    /// is not yet recorded here, so callers must not guess which window owns
    /// its pages; even a lost branch stays guarded until it is dropped.
    #[must_use]
    pub fn has_unresolved_child_branch_for_profile(&self, profile: ProfileId) -> bool {
        self.branches.values().any(|branch| {
            branch.profile == profile
                && branch.parent.is_some()
                && branch.lifecycle != BranchLifecycle::Closed
        })
    }

    #[must_use]
    pub fn retained(&self) -> &BTreeMap<RetainedViewId, RetentionRecord> {
        &self.retained
    }
    #[must_use]
    pub fn current_line(&self, application: ApplicationOwnerId) -> Option<BranchId> {
        self.current.get(&application).copied()
    }
    #[must_use]
    pub fn logical_window(&self, window: WindowId) -> Option<LogicalWindowId> {
        self.logical_windows.get(&window).copied()
    }
    #[must_use]
    pub fn logical_page(&self, tab: TabId) -> Option<LogicalPageId> {
        self.logical_pages.get(&tab).copied()
    }

    #[must_use]
    pub fn lineage_parent(&self, tab: TabId) -> Option<LogicalPageId> {
        self.lineage_parents.get(&tab).copied()
    }

    pub(crate) fn forget_structural_page(&mut self, tab: TabId) {
        self.logical_pages.remove(&tab);
        self.lineage_parents.remove(&tab);
    }

    /// Pure view-pool projection. An ordinary live tab is visible; an admitted
    /// retention record takes precedence without exposing native handles.
    #[must_use]
    pub fn membership_for_tab(
        &self,
        state: &ApplicationState,
        tab: TabId,
    ) -> Option<ViewMembership> {
        let tab_state = state.tabs().get(&tab)?;
        if let Some(record) = self
            .retained
            .values()
            .find(|record| record.owner.target().tab == tab)
        {
            return Some(record.membership);
        }
        Some(
            if matches!(
                tab_state.existence,
                ExistenceState::Closing | ExistenceState::Closed
            ) {
                ViewMembership::Closing
            } else {
                ViewMembership::Visible
            },
        )
    }

    /// Ordered tab-strip projection. Retained and transferring pages remain
    /// reducer-owned live tabs, but must not be recreated in the visible view
    /// pool after the native host has accepted their exact handles.
    #[must_use]
    pub fn visible_tabs_for_window(
        &self,
        state: &ApplicationState,
        window: WindowId,
    ) -> Option<Vec<TabId>> {
        let window = state.windows().get(&window)?;
        Some(
            window
                .tabs
                .iter()
                .copied()
                .filter(|tab| self.membership_for_tab(state, *tab) == Some(ViewMembership::Visible))
                .collect(),
        )
    }

    #[must_use]
    pub(crate) fn owns_tab(&self, tab: TabId) -> bool {
        self.retained.values().any(|record| {
            record.owner.target().tab == tab
                && (record.membership == ViewMembership::Visible
                    || record.profile_lease_count != 0
                    || record.view_lease_count != 0)
        })
    }

    /// Called only by `Event::Versioning` in the application reducer. Each
    /// rejected transition leaves this state untouched.
    ///
    /// # Errors
    /// Returns a typed lineage, ownership, lifecycle or resource-bound error.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn apply(&mut self, event: VersioningEvent) -> Result<(), BranchError> {
        match event {
            VersioningEvent::BindStructuralIdentity {
                window,
                logical_window,
                pages,
            } => self.bind_structural_identity(window, logical_window, &pages),
            VersioningEvent::BindRestoredLineage { pages, .. } => {
                self.bind_restored_lineage(&pages)
            }
            VersioningEvent::BindTransientChildLineage {
                window,
                logical_window,
                pages,
            } => {
                let identities = pages
                    .iter()
                    .map(|(tab, child, _)| (*tab, *child))
                    .collect::<Vec<_>>();
                let ancestry = pages
                    .iter()
                    .map(|(tab, _, parent)| (*tab, *parent))
                    .collect::<Vec<_>>();
                let mut candidate = self.clone();
                candidate.bind_structural_identity(window, logical_window, &identities)?;
                candidate.bind_restored_lineage(&ancestry)?;
                *self = candidate;
                Ok(())
            }
            VersioningEvent::PrepareBranch {
                id,
                name,
                application,
                profile,
                parent,
                fork,
                inheritance,
            } => self.prepare_branch(id, name, application, profile, parent, fork, inheritance),
            VersioningEvent::PublishBranch { id, native, actual } => {
                let branch = self.branches.get(&id).ok_or(BranchError::UnknownBranch)?;
                if branch.lifecycle != BranchLifecycle::Preparing {
                    return Err(BranchError::InvalidTransition);
                }
                if let Some(parent_id) = branch.parent {
                    let parent = self
                        .branches
                        .get(&parent_id)
                        .ok_or(BranchError::UnknownBranch)?;
                    if !matches!(
                        parent.lifecycle,
                        BranchLifecycle::Live | BranchLifecycle::Retained
                    ) || parent.native.is_none()
                    {
                        return Err(BranchError::InvalidTransition);
                    }
                    if parent.native == Some(native) {
                        return Err(BranchError::DuplicateLiveOwner);
                    }
                } else {
                    // The root line is in the lineage application's normal
                    // profile. A clean child belongs to a separate ephemeral
                    // application/profile owner; its logical `profile` field
                    // still names the durable parent lineage namespace.
                    if native.application != branch.application {
                        return Err(BranchError::WrongApplication);
                    }
                    if native.runtime_profile != branch.profile {
                        return Err(BranchError::WrongProfile);
                    }
                }
                if !matches!(
                    (branch.requested_inheritance, actual),
                    (InheritanceRequest::None, InheritanceReport::None)
                        | (InheritanceRequest::Clean, InheritanceReport::Clean)
                        | (
                            InheritanceRequest::Snapshot,
                            InheritanceReport::SnapshotVerified
                        )
                ) {
                    return Err(BranchError::InheritanceMismatch);
                }
                if self.branches.values().any(|existing| {
                    existing.id != id
                        && matches!(
                            existing.lifecycle,
                            BranchLifecycle::Live | BranchLifecycle::Retained
                        )
                        && existing.native.is_some_and(|owner| {
                            owner.application == native.application
                                && owner.profile_instance == native.profile_instance
                        })
                }) {
                    return Err(BranchError::DuplicateLiveOwner);
                }
                let branch = self.branches.get_mut(&id).expect("checked above");
                branch.native = Some(native);
                branch.actual_inheritance = Some(actual);
                branch.lifecycle = BranchLifecycle::Live;
                if branch.parent.is_none() {
                    self.current.entry(branch.application).or_insert(id);
                }
                Ok(())
            }
            VersioningEvent::FailBranch { id } => {
                let branch = self
                    .branches
                    .get_mut(&id)
                    .ok_or(BranchError::UnknownBranch)?;
                if matches!(
                    branch.lifecycle,
                    BranchLifecycle::Closed | BranchLifecycle::Lost
                ) {
                    return Err(BranchError::InvalidTransition);
                }
                if !branch.retained.is_empty() {
                    return Err(BranchError::RetainedWorkOwned);
                }
                branch.lifecycle = BranchLifecycle::Lost;
                branch.native = None;
                self.current.retain(|_, current| *current != id);
                Ok(())
            }
            VersioningEvent::AdvanceWorking {
                id,
                expected_revision,
                pages,
                last_commit,
            } => {
                if pages.len() > MAX_PAGES_PER_WINDOW_CAPTURE {
                    return Err(BranchError::TooManyPages);
                }
                let mut seen = HashSet::with_capacity(pages.len());
                if !pages.iter().all(|page| seen.insert(*page)) {
                    return Err(BranchError::DuplicatePage);
                }
                let branch = self
                    .branches
                    .get_mut(&id)
                    .ok_or(BranchError::UnknownBranch)?;
                if branch.lifecycle != BranchLifecycle::Live {
                    return Err(BranchError::InvalidTransition);
                }
                if branch.working_revision != expected_revision {
                    return Err(BranchError::RevisionMismatch);
                }
                let next = expected_revision
                    .checked_add(1)
                    .ok_or(BranchError::RevisionOverflow)?;
                branch.pages = pages;
                branch.working_revision = next;
                if let Some(commit) = last_commit {
                    branch.last_commit = Some(commit);
                }
                Ok(())
            }
            VersioningEvent::PrepareRetained { id, view, owner } => {
                self.prepare_retained(id, view, owner)
            }
            VersioningEvent::PrepareStash { stash, view, owner } => {
                self.prepare_stash(stash, view, owner)
            }
            VersioningEvent::ConfirmFrozenStash { stash, view, owner } => {
                let record = self
                    .retained
                    .get_mut(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if record.owner_id != RetentionOwner::Stash(stash) || record.owner != owner {
                    return Err(BranchError::RetainedOwnerMismatch);
                }
                if record.membership != ViewMembership::Visible
                    || record.lifecycle != RetainedLifecycle::Active
                    || record.profile_lease_count != 0
                    || record.view_lease_count != 0
                {
                    return Err(BranchError::InvalidTransition);
                }
                record.membership = ViewMembership::Retained;
                record.lifecycle = RetainedLifecycle::Frozen;
                record.profile_lease_count = 1;
                record.view_lease_count = 1;
                Ok(())
            }
            VersioningEvent::RollbackFrozenStash { stash, view, owner } => {
                let record = self
                    .retained
                    .get_mut(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if record.owner_id != RetentionOwner::Stash(stash) || record.owner != owner {
                    return Err(BranchError::RetainedOwnerMismatch);
                }
                if record.membership != ViewMembership::Retained
                    || record.lifecycle != RetainedLifecycle::Frozen
                    || record.profile_lease_count != 1
                    || record.view_lease_count != 1
                {
                    return Err(BranchError::InvalidTransition);
                }
                record.membership = ViewMembership::Visible;
                record.lifecycle = RetainedLifecycle::Active;
                record.profile_lease_count = 0;
                record.view_lease_count = 0;
                Ok(())
            }
            VersioningEvent::AcquireRetained { view } => {
                let record = self
                    .retained
                    .get_mut(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if record.membership != ViewMembership::Visible
                    || record.profile_lease_count != 0
                    || record.view_lease_count != 0
                {
                    return Err(BranchError::InvalidTransition);
                }
                record.membership = ViewMembership::Retained;
                record.profile_lease_count = 1;
                record.view_lease_count = 1;
                Ok(())
            }
            VersioningEvent::BeginRetainedTransfer {
                view,
                destination,
                window,
            } => {
                let record = self
                    .retained
                    .get_mut(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if destination != record.owner.native() {
                    return Err(BranchError::WrongProfile);
                }
                if record.membership != ViewMembership::Retained
                    || record.profile_lease_count != 1
                    || record.view_lease_count != 1
                {
                    return Err(BranchError::InvalidTransition);
                }
                record.membership = ViewMembership::Transferring;
                record.restore_window = Some(window);
                Ok(())
            }
            VersioningEvent::CancelRetainedTransfer { view } => {
                let record = self
                    .retained
                    .get_mut(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if record.membership != ViewMembership::Transferring {
                    return Err(BranchError::InvalidTransition);
                }
                record.membership = ViewMembership::Retained;
                record.restore_window = None;
                Ok(())
            }
            VersioningEvent::AdoptRetained {
                view,
                destination,
                window,
            } => {
                let record = self
                    .retained
                    .get(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if destination != record.owner.native() {
                    return Err(BranchError::WrongProfile);
                }
                if record.membership != ViewMembership::Transferring
                    || record.restore_window != Some(window)
                {
                    return Err(BranchError::InvalidTransition);
                }
                self.remove_retained(view)
            }
            VersioningEvent::BeginRetainedClose { view } => {
                let record = self
                    .retained
                    .get_mut(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if record.membership != ViewMembership::Retained {
                    return Err(BranchError::InvalidTransition);
                }
                record.membership = ViewMembership::Closing;
                Ok(())
            }
            VersioningEvent::ObserveRetainedLifecycle { view, lifecycle } => {
                let record = self
                    .retained
                    .get_mut(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if !matches!(
                    record.membership,
                    ViewMembership::Retained | ViewMembership::Transferring
                ) || lifecycle == RetainedLifecycle::Lost
                {
                    return Err(BranchError::InvalidTransition);
                }
                record.lifecycle = lifecycle;
                if lifecycle == RetainedLifecycle::Discarded {
                    record.membership = ViewMembership::Closing;
                    record.profile_lease_count = 0;
                    record.view_lease_count = 0;
                    record.restore_window = None;
                }
                Ok(())
            }
            VersioningEvent::LoseRetained { view } => {
                let record = self
                    .retained
                    .get_mut(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if matches!(
                    record.lifecycle,
                    RetainedLifecycle::Lost | RetainedLifecycle::Discarded
                ) {
                    return Err(BranchError::InvalidTransition);
                }
                record.lifecycle = RetainedLifecycle::Lost;
                record.membership = ViewMembership::Closing;
                record.profile_lease_count = 0;
                record.view_lease_count = 0;
                record.restore_window = None;
                Ok(())
            }
            VersioningEvent::ReleaseRetained { view } => {
                let record = self
                    .retained
                    .get(&view)
                    .ok_or(BranchError::UnknownRetainedView)?;
                if record.membership == ViewMembership::Transferring {
                    return Err(BranchError::InvalidTransition);
                }
                self.remove_retained(view)
            }
            VersioningEvent::RetainBranch { id } => {
                let branch = self
                    .branches
                    .get_mut(&id)
                    .ok_or(BranchError::UnknownBranch)?;
                if branch.lifecycle != BranchLifecycle::Live
                    || !branch.retained.iter().any(|view| {
                        self.retained.get(view).is_some_and(|record| {
                            record.profile_lease_count == 1 && record.view_lease_count == 1
                        })
                    })
                {
                    return Err(BranchError::InvalidTransition);
                }
                branch.lifecycle = BranchLifecycle::Retained;
                Ok(())
            }
            VersioningEvent::ResumeBranch { id } => {
                let branch = self
                    .branches
                    .get_mut(&id)
                    .ok_or(BranchError::UnknownBranch)?;
                if branch.lifecycle != BranchLifecycle::Retained
                    || branch.retained.iter().any(|view| {
                        self.retained.get(view).is_some_and(|record| {
                            matches!(
                                record.lifecycle,
                                RetainedLifecycle::Lost | RetainedLifecycle::Discarded
                            )
                        })
                    })
                {
                    return Err(BranchError::InvalidTransition);
                }
                branch.lifecycle = BranchLifecycle::Live;
                Ok(())
            }
            VersioningEvent::CloseBranch { id } => {
                let branch = self
                    .branches
                    .get_mut(&id)
                    .ok_or(BranchError::UnknownBranch)?;
                if !matches!(
                    branch.lifecycle,
                    BranchLifecycle::Live | BranchLifecycle::Retained
                ) {
                    return Err(BranchError::InvalidTransition);
                }
                if !branch.retained.is_empty() {
                    return Err(BranchError::RetainedWorkOwned);
                }
                branch.lifecycle = BranchLifecycle::Closed;
                branch.native = None;
                self.current.retain(|_, current| *current != id);
                Ok(())
            }
            VersioningEvent::DropBranch { id } => {
                let branch = self.branches.get(&id).ok_or(BranchError::UnknownBranch)?;
                if matches!(
                    branch.lifecycle,
                    BranchLifecycle::Live | BranchLifecycle::Retained | BranchLifecycle::Preparing
                ) {
                    return Err(BranchError::LiveWorkOwned);
                }
                if !branch.retained.is_empty() {
                    return Err(BranchError::RetainedWorkOwned);
                }
                if self
                    .branches
                    .values()
                    .any(|candidate| candidate.parent == Some(id))
                {
                    return Err(BranchError::HasChildren);
                }
                self.branches.remove(&id);
                self.current.retain(|_, current| *current != id);
                Ok(())
            }
            VersioningEvent::SelectBranch { id } => {
                let branch = self.branches.get(&id).ok_or(BranchError::UnknownBranch)?;
                if !matches!(
                    branch.lifecycle,
                    BranchLifecycle::Live | BranchLifecycle::Retained
                ) {
                    return Err(BranchError::InvalidTransition);
                }
                self.current.insert(branch.application, id);
                Ok(())
            }
        }
    }

    fn bind_structural_identity(
        &mut self,
        window: WindowId,
        logical_window: LogicalWindowId,
        pages: &[(TabId, LogicalPageId)],
    ) -> Result<(), BranchError> {
        if pages.is_empty() || pages.len() > MAX_PAGES_PER_WINDOW_CAPTURE {
            return Err(BranchError::TooManyPages);
        }
        if self
            .logical_windows
            .get(&window)
            .is_some_and(|existing| *existing != logical_window)
            || self
                .logical_windows
                .iter()
                .any(|(other, existing)| *other != window && *existing == logical_window)
        {
            return Err(BranchError::InvalidTransition);
        }
        let mut seen_tabs = BTreeSet::new();
        let mut seen_pages = BTreeSet::new();
        for (tab, logical_page) in pages {
            if !seen_tabs.insert(*tab)
                || !seen_pages.insert(*logical_page)
                || self
                    .logical_pages
                    .get(tab)
                    .is_some_and(|existing| *existing != *logical_page)
                || self
                    .logical_pages
                    .iter()
                    .any(|(other, existing)| *other != *tab && *existing == *logical_page)
            {
                return Err(BranchError::DuplicatePage);
            }
        }
        self.logical_windows.insert(window, logical_window);
        self.logical_pages.extend(pages.iter().copied());
        Ok(())
    }

    fn bind_restored_lineage(
        &mut self,
        pages: &[(TabId, LogicalPageId)],
    ) -> Result<(), BranchError> {
        if pages.is_empty() || pages.len() > MAX_PAGES_PER_WINDOW_CAPTURE {
            return Err(BranchError::TooManyPages);
        }
        let mut seen = BTreeSet::new();
        for (tab, parent) in pages {
            if !seen.insert(*tab)
                || self
                    .logical_pages
                    .get(tab)
                    .is_none_or(|logical| logical == parent)
                || self
                    .lineage_parents
                    .get(tab)
                    .is_some_and(|existing| existing != parent)
            {
                return Err(BranchError::InvalidTransition);
            }
        }
        self.lineage_parents.extend(pages.iter().copied());
        Ok(())
    }

    fn prepare_retained(
        &mut self,
        id: BranchId,
        view: RetainedViewId,
        owner: CapturedOwner,
    ) -> Result<(), BranchError> {
        if self.retained.len() >= MAX_RETAINED_VIEWS as usize {
            return Err(BranchError::TooManyRetainedViews);
        }
        if self.retained.contains_key(&view) {
            return Err(BranchError::DuplicateRetainedView);
        }
        let branch = self.branches.get(&id).ok_or(BranchError::UnknownBranch)?;
        if !matches!(
            branch.lifecycle,
            BranchLifecycle::Live | BranchLifecycle::Retained
        ) {
            return Err(BranchError::InvalidTransition);
        }
        if branch.native != Some(owner.native()) {
            return Err(BranchError::RetainedOwnerMismatch);
        }
        if self
            .retained
            .values()
            .any(|record| record.owner.target().tab == owner.target().tab)
        {
            return Err(BranchError::DuplicateLiveOwner);
        }
        self.retained.insert(
            view,
            RetentionRecord {
                id: view,
                owner_id: RetentionOwner::Branch(id),
                owner,
                membership: ViewMembership::Visible,
                lifecycle: RetainedLifecycle::Active,
                profile_lease_count: 0,
                view_lease_count: 0,
                restore_window: None,
            },
        );
        self.branches
            .get_mut(&id)
            .expect("checked above")
            .retained
            .insert(view);
        Ok(())
    }

    fn prepare_stash(
        &mut self,
        stash: StashId,
        view: RetainedViewId,
        owner: CapturedOwner,
    ) -> Result<(), BranchError> {
        if self.retained.len() >= MAX_RETAINED_VIEWS as usize {
            return Err(BranchError::TooManyRetainedViews);
        }
        if self.retained.contains_key(&view) {
            return Err(BranchError::DuplicateRetainedView);
        }
        if self
            .retained
            .values()
            .any(|record| record.stash() == Some(stash))
        {
            return Err(BranchError::DuplicateStash);
        }
        if self
            .retained
            .values()
            .any(|record| record.owner.target().tab == owner.target().tab)
        {
            return Err(BranchError::DuplicateLiveOwner);
        }
        self.retained.insert(
            view,
            RetentionRecord {
                id: view,
                owner_id: RetentionOwner::Stash(stash),
                owner,
                membership: ViewMembership::Visible,
                lifecycle: RetainedLifecycle::Active,
                profile_lease_count: 0,
                view_lease_count: 0,
                restore_window: None,
            },
        );
        Ok(())
    }

    fn remove_retained(&mut self, view: RetainedViewId) -> Result<(), BranchError> {
        let record = self
            .retained
            .remove(&view)
            .ok_or(BranchError::UnknownRetainedView)?;
        if let RetentionOwner::Branch(id) = record.owner_id {
            self.branches
                .get_mut(&id)
                .expect("retention branch invariant")
                .retained
                .remove(&view);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_branch(
        &mut self,
        id: BranchId,
        name: BranchName,
        application: ApplicationOwnerId,
        profile: ProfileId,
        parent: Option<BranchId>,
        fork: Option<CommitId>,
        inheritance: InheritanceRequest,
    ) -> Result<(), BranchError> {
        if self.branches.contains_key(&id) {
            return Err(BranchError::DuplicateBranch);
        }
        let (root, depth) = if let Some(parent_id) = parent {
            if parent_id == id {
                return Err(BranchError::Cycle);
            }
            if fork.is_none() {
                return Err(BranchError::MissingFork);
            }
            if inheritance == InheritanceRequest::None {
                return Err(BranchError::InheritanceMismatch);
            }
            let mut seen = HashSet::new();
            let mut ancestor = Some(parent_id);
            while let Some(candidate) = ancestor {
                if candidate == id || !seen.insert(candidate) {
                    return Err(BranchError::Cycle);
                }
                ancestor = self
                    .branches
                    .get(&candidate)
                    .ok_or(BranchError::UnknownBranch)?
                    .parent;
            }
            let parent = self.branches.get(&parent_id).expect("checked above");
            if parent.application != application {
                return Err(BranchError::WrongApplication);
            }
            if parent.profile != profile {
                return Err(BranchError::WrongProfile);
            }
            if !matches!(
                parent.lifecycle,
                BranchLifecycle::Live | BranchLifecycle::Retained
            ) {
                return Err(BranchError::InvalidTransition);
            }
            let depth = parent
                .depth
                .checked_add(1)
                .ok_or(BranchError::DepthExceeded)?;
            if depth > MAX_BRANCH_NESTING {
                return Err(BranchError::DepthExceeded);
            }
            (parent.root, depth)
        } else {
            if fork.is_some() || inheritance != InheritanceRequest::None || name.key != "main" {
                return Err(BranchError::InvalidRoot);
            }
            if self.branches.values().any(|branch| {
                branch.parent.is_none()
                    && branch.application == application
                    && branch.profile == profile
            }) {
                return Err(BranchError::AmbiguousName);
            }
            (id, 0)
        };
        if self.branches.values().any(|branch| {
            branch.application == application
                && branch.profile == profile
                && branch.root == root
                && branch.name.key == name.key
        }) {
            return Err(BranchError::AmbiguousName);
        }
        self.branches.insert(
            id,
            BranchRecord {
                id,
                name,
                application,
                profile,
                parent,
                root,
                fork,
                depth,
                requested_inheritance: inheritance,
                actual_inheritance: None,
                native: None,
                pages: Vec::new(),
                working_revision: 0,
                last_commit: None,
                retained: BTreeSet::new(),
                lifecycle: BranchLifecycle::Preparing,
            },
        );
        Ok(())
    }

    /// # Errors
    /// Reports the first broken internal lineage or retention invariant.
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        let mut native_owners = HashSet::new();
        let mut names = HashSet::new();
        for (id, branch) in &self.branches {
            if *id != branch.id {
                return Err("versioning branch key mismatch");
            }
            if !names.insert((
                branch.application,
                branch.profile,
                branch.root,
                &branch.name.key,
            )) {
                return Err("versioning branch name is ambiguous");
            }
            if let Some(parent) = branch.parent {
                let parent = self
                    .branches
                    .get(&parent)
                    .ok_or("versioning parent missing")?;
                if parent.root != branch.root
                    || parent.depth + 1 != branch.depth
                    || parent.profile != branch.profile
                    || parent.application != branch.application
                    || branch.fork.is_none()
                {
                    return Err("versioning lineage mismatch");
                }
            } else if branch.root != *id || branch.depth != 0 || branch.fork.is_some() {
                return Err("versioning root mismatch");
            }
            if branch.pages.len() > MAX_PAGES_PER_WINDOW_CAPTURE
                || branch.pages.iter().collect::<HashSet<_>>().len() != branch.pages.len()
            {
                return Err("versioning working pages invalid");
            }
            if matches!(
                branch.lifecycle,
                BranchLifecycle::Live | BranchLifecycle::Retained
            ) {
                let native = branch.native.ok_or("live branch missing native owner")?;
                if let Some(parent_id) = branch.parent {
                    if self.branches.get(&parent_id).and_then(BranchRecord::native) == Some(native)
                    {
                        return Err("child branch reused parent native owner");
                    }
                } else if native.application != branch.application
                    || native.runtime_profile != branch.profile
                {
                    return Err("root native versioning owner mismatch");
                }
                if !native_owners.insert((native.application, native.profile_instance)) {
                    return Err("native versioning owner duplicated");
                }
            } else if branch.native.is_some() {
                return Err("nonlive branch retains native owner");
            }
            for view in &branch.retained {
                if self
                    .retained
                    .get(view)
                    .is_none_or(|record| record.owner_id != RetentionOwner::Branch(*id))
                {
                    return Err("versioning retention membership mismatch");
                }
            }
        }
        self.validate_retained_records()?;
        for (application, id) in &self.current {
            let branch = self
                .branches
                .get(id)
                .ok_or("versioning current branch missing")?;
            if branch.application != *application
                || !matches!(
                    branch.lifecycle,
                    BranchLifecycle::Live | BranchLifecycle::Retained
                )
            {
                return Err("versioning current branch invalid");
            }
        }
        Ok(())
    }

    fn validate_retained_records(&self) -> Result<(), &'static str> {
        let mut retained_tabs = HashSet::new();
        let mut stash_ids = HashSet::new();
        for (view, record) in &self.retained {
            if record.id != *view {
                return Err("versioning retention key mismatch");
            }
            if !retained_tabs.insert(record.owner.target().tab) {
                return Err("versioning view has duplicate retention owners");
            }
            match record.owner_id {
                RetentionOwner::Branch(id) => {
                    let branch = self
                        .branches
                        .get(&id)
                        .ok_or("versioning retention branch missing")?;
                    if !branch.retained.contains(view) {
                        return Err("versioning retention membership mismatch");
                    }
                    if branch.native != Some(record.owner.native()) {
                        return Err("versioning retention native owner mismatch");
                    }
                }
                RetentionOwner::Stash(id) => {
                    if !stash_ids.insert(id) {
                        return Err("versioning stash has duplicate retention owners");
                    }
                }
            }
            let leases = (record.profile_lease_count, record.view_lease_count);
            let valid = match record.membership {
                ViewMembership::Visible => {
                    leases == (0, 0)
                        && record.lifecycle == RetainedLifecycle::Active
                        && record.restore_window.is_none()
                }
                ViewMembership::Retained => {
                    leases == (1, 1)
                        && matches!(
                            record.lifecycle,
                            RetainedLifecycle::Active | RetainedLifecycle::Frozen
                        )
                        && record.restore_window.is_none()
                }
                ViewMembership::Transferring => {
                    leases == (1, 1)
                        && matches!(
                            record.lifecycle,
                            RetainedLifecycle::Active | RetainedLifecycle::Frozen
                        )
                        && record.restore_window.is_some()
                }
                ViewMembership::Closing => {
                    record.restore_window.is_none()
                        && ((leases == (1, 1)
                            && matches!(
                                record.lifecycle,
                                RetainedLifecycle::Active | RetainedLifecycle::Frozen
                            ))
                            || (leases == (0, 0)
                                && matches!(
                                    record.lifecycle,
                                    RetainedLifecycle::Discarded | RetainedLifecycle::Lost
                                )))
                }
            };
            if !valid {
                return Err("versioning retention lease invariant violated");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, PrivacyKind, reduce};

    fn profile_with_window(state: &mut ApplicationState) -> (ProfileId, WindowId) {
        reduce(
            state,
            Event::CreateProfile {
                label: "fixture".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let profile = *state.profiles().keys().last().unwrap();
        reduce(state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().last().unwrap();
        (profile, window)
    }

    fn owner(profile: ProfileId) -> NativeProfileOwner {
        NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            profile,
            ProfileInstanceId::fresh(),
        )
    }

    #[test]
    fn captured_scope_selector_and_versioning_event_debug_redact_identity() {
        let sentinel = "private-branch-name-sentinel-67ca";
        let mut state = ApplicationState::new();
        let (profile, window) = profile_with_window(&mut state);
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = state.windows()[&window].active_tab.unwrap();
        let target = state.capture_target(tab).unwrap();
        let binding = owner(profile);
        let selector = ScopeSelector::Tab { window, target };
        let scope = resolve_scope(&state, selector, binding).unwrap();
        let branch_id = BranchId::fresh();
        let application = ApplicationOwnerId::fresh();
        let event = VersioningEvent::PrepareBranch {
            id: branch_id,
            name: BranchName::new(sentinel).unwrap(),
            application,
            profile,
            parent: None,
            fork: None,
            inheritance: InheritanceRequest::None,
        };

        let projections = [
            format!("{selector:?}"),
            format!("{scope:?}"),
            format!("{:?}", scope.members()[0]),
            format!("{event:?}"),
        ];
        let identities = [
            profile.to_string(),
            window.to_string(),
            target.tab.to_string(),
            target.document.to_string(),
            branch_id.to_string(),
            application.to_string(),
        ];
        for projection in projections {
            assert!(!projection.contains(sentinel), "{projection}");
            for identity in &identities {
                assert!(!projection.contains(identity.as_str()), "{projection}");
            }
        }
    }

    #[test]
    fn selected_tab_switch_does_not_retarget_a_captured_tab() {
        let mut state = ApplicationState::new();
        let (profile, window) = profile_with_window(&mut state);
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let first = state.windows()[&window].active_tab.unwrap();
        let target = state.capture_target(first).unwrap();
        let binding = owner(profile);
        let scope = resolve_scope(&state, ScopeSelector::Tab { window, target }, binding).unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        assert_ne!(state.windows()[&window].active_tab, Some(first));
        assert_eq!(scope.members().len(), 1);
        assert_eq!(scope.members()[0].target(), target);
        assert_eq!(
            scope.validate_page_result(&state, Some(binding), target),
            Ok(())
        );
        let newly_selected = state
            .capture_target(state.windows()[&window].active_tab.unwrap())
            .unwrap();
        assert_eq!(
            scope.validate_page_result(&state, Some(binding), newly_selected),
            Err(ScopeError::NotInScope)
        );
    }

    #[test]
    fn replacement_document_cannot_attach_to_old_owner() {
        let mut state = ApplicationState::new();
        let (profile, window) = profile_with_window(&mut state);
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = state.windows()[&window].active_tab.unwrap();
        let original = state.capture_target(tab).unwrap();
        let binding = owner(profile);
        let scope = resolve_scope(
            &state,
            ScopeSelector::Tab {
                window,
                target: original,
            },
            binding,
        )
        .unwrap();
        let url = crate::ValidatedUrl::parse("https://example.test/next").unwrap();
        reduce(
            &mut state,
            Event::StartNavigation {
                target: original,
                url,
            },
        )
        .unwrap();
        assert_eq!(
            scope.validate_page_result(&state, Some(binding), original),
            Err(ScopeError::DocumentChanged)
        );
    }

    #[test]
    fn admission_rejects_stale_generation_and_closed_tab() {
        let mut state = ApplicationState::new();
        let (profile, window) = profile_with_window(&mut state);
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = state.windows()[&window].active_tab.unwrap();
        let target = state.capture_target(tab).unwrap();
        let stale_target = Target {
            generation: target.generation + 1,
            ..target
        };
        let binding = owner(profile);
        assert_eq!(
            resolve_scope(
                &state,
                ScopeSelector::Tab {
                    window,
                    target: stale_target
                },
                binding
            )
            .unwrap_err(),
            ScopeError::StaleTarget,
        );
        reduce(&mut state, Event::CloseTab { tab }).unwrap();
        assert_eq!(
            resolve_scope(&state, ScopeSelector::Tab { window, target }, binding).unwrap_err(),
            ScopeError::NonLiveMember,
        );
    }

    #[test]
    fn moving_a_member_to_another_window_does_not_retarget_the_capture() {
        let mut state = ApplicationState::new();
        let (profile, original_window) = profile_with_window(&mut state);
        reduce(
            &mut state,
            Event::OpenTab {
                window: original_window,
            },
        )
        .unwrap();
        let tab = state.windows()[&original_window].active_tab.unwrap();
        let target = state.capture_target(tab).unwrap();
        let binding = owner(profile);
        let scope = resolve_scope(
            &state,
            ScopeSelector::Tab {
                window: original_window,
                target,
            },
            binding,
        )
        .unwrap();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let destination = *state.windows().keys().last().unwrap();
        reduce(
            &mut state,
            Event::MoveTab {
                tab,
                to_window: destination,
                index: None,
            },
        )
        .unwrap();
        assert_eq!(scope.window(), original_window);
        assert_eq!(
            scope.validate_page_result(&state, Some(binding), target),
            Err(ScopeError::StaleOwner)
        );
    }

    #[test]
    fn wrong_profile_and_closed_native_owner_are_rejected() {
        let mut state = ApplicationState::new();
        let (first_profile, first_window) = profile_with_window(&mut state);
        let (second_profile, second_window) = profile_with_window(&mut state);
        reduce(
            &mut state,
            Event::OpenTab {
                window: second_window,
            },
        )
        .unwrap();
        let second_tab = state.windows()[&second_window].active_tab.unwrap();
        let target = state.capture_target(second_tab).unwrap();
        let binding = owner(first_profile);
        assert_eq!(
            resolve_scope(
                &state,
                ScopeSelector::Tab {
                    window: first_window,
                    target
                },
                binding
            )
            .unwrap_err(),
            ScopeError::WrongProfile
        );
        reduce(
            &mut state,
            Event::OpenTab {
                window: first_window,
            },
        )
        .unwrap();
        let first_target = state
            .capture_target(state.windows()[&first_window].active_tab.unwrap())
            .unwrap();
        let scope = resolve_scope(
            &state,
            ScopeSelector::Tab {
                window: first_window,
                target: first_target,
            },
            binding,
        )
        .unwrap();
        assert_eq!(
            scope.validate_page_result(&state, None, first_target),
            Err(ScopeError::OwnerClosed)
        );
        assert_eq!(
            scope.validate_page_result(&state, Some(owner(first_profile)), first_target),
            Err(ScopeError::NativeOwnerMismatch)
        );
        let replaced_instance = NativeProfileOwner::new(
            binding.application,
            first_profile,
            ProfileInstanceId::fresh(),
        );
        assert_eq!(
            scope.validate_page_result(&state, Some(replaced_instance), first_target),
            Err(ScopeError::NativeOwnerMismatch)
        );
        let replaced_application = NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            first_profile,
            binding.profile_instance,
        );
        assert_eq!(
            scope.validate_page_result(&state, Some(replaced_application), first_target),
            Err(ScopeError::NativeOwnerMismatch)
        );
        assert_eq!(
            resolve_scope(
                &state,
                ScopeSelector::Window {
                    window: second_window
                },
                binding
            )
            .unwrap_err(),
            ScopeError::WrongProfile
        );
        assert_ne!(first_profile, second_profile);
    }

    #[test]
    fn window_membership_is_ordered_bounded_and_immutable() {
        let mut state = ApplicationState::new();
        let (profile, window) = profile_with_window(&mut state);
        let binding = owner(profile);
        assert_eq!(
            resolve_scope(&state, ScopeSelector::Window { window }, binding).unwrap_err(),
            ScopeError::EmptyScope
        );
        for _ in 0..128 {
            reduce(&mut state, Event::OpenTab { window }).unwrap();
        }
        let order = state.windows()[&window].tabs.clone();
        let scope = resolve_scope(&state, ScopeSelector::Window { window }, binding).unwrap();
        let captured: Vec<_> = scope
            .members()
            .iter()
            .map(|owner| owner.target().tab)
            .collect();
        assert_eq!(captured, order);
        assert_eq!(scope.selected_tab(), state.windows()[&window].active_tab);
        assert_eq!(scope.members().len(), 128);
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        assert_eq!(scope.members().len(), 128);
        assert_eq!(
            resolve_scope(&state, ScopeSelector::Window { window }, binding).unwrap_err(),
            ScopeError::TooManyMembers
        );
        assert_eq!(
            resolve_scope(&state, ScopeSelector::Context { window }, binding,).unwrap_err(),
            ScopeError::ContextUnavailable
        );
    }
}

#[cfg(test)]
mod branch_tests {
    use super::*;
    use crate::{Event, PrivacyKind, ReduceError, reduce};

    fn setup() -> (ApplicationState, ProfileId, ApplicationOwnerId) {
        let mut state = ApplicationState::new();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "normal".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let profile = *state.profiles().keys().next().unwrap();
        (state, profile, ApplicationOwnerId::fresh())
    }

    fn apply(state: &mut ApplicationState, event: VersioningEvent) -> Result<(), ReduceError> {
        reduce(state, Event::Versioning(event)).map(|_| ())
    }

    #[test]
    fn native_profile_owner_debug_redacts_runtime_authority_tokens() {
        let (_, profile, application) = setup();
        let instance = ProfileInstanceId::fresh();
        let owner = NativeProfileOwner::new(application, profile, instance);
        let debug = format!("{owner:?}");

        assert_eq!(debug, "NativeProfileOwner([redacted])");
        assert!(!debug.contains(&application.to_string()));
        assert!(!debug.contains(&profile.to_string()));
        assert!(!debug.contains(&instance.to_string()));
    }

    #[test]
    fn structural_identities_are_stable_unique_and_released_with_runtime_tabs() {
        let (mut state, profile, _) = setup();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().next().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = *state.tabs().keys().next().unwrap();
        let logical_window = LogicalWindowId::fresh();
        let logical_page = LogicalPageId::fresh();
        let bind = VersioningEvent::BindStructuralIdentity {
            window,
            logical_window,
            pages: vec![(tab, logical_page)],
        };
        apply(&mut state, bind.clone()).unwrap();
        apply(&mut state, bind).unwrap();
        assert_eq!(
            state.versioning().logical_window(window),
            Some(logical_window)
        );
        assert_eq!(state.versioning().logical_page(tab), Some(logical_page));
        let revision = state.revision();
        assert!(
            apply(
                &mut state,
                VersioningEvent::BindStructuralIdentity {
                    window,
                    logical_window,
                    pages: vec![(tab, LogicalPageId::fresh())],
                }
            )
            .is_err()
        );
        assert_eq!(state.revision(), revision);
        assert_eq!(state.versioning().logical_page(tab), Some(logical_page));
        reduce(&mut state, Event::TransferTabOut { tab }).unwrap();
        assert_eq!(state.versioning().logical_page(tab), None);
    }

    #[test]
    fn branch_names_are_bounded_and_redacted_in_debug() {
        assert_eq!(BranchName::new(" "), Err(BranchError::InvalidName));
        assert_eq!(BranchName::new("line\nname"), Err(BranchError::InvalidName));
        assert_eq!(
            BranchName::new(&"x".repeat(129)),
            Err(BranchError::InvalidName)
        );
        let name = BranchName::new(" secret=example ").unwrap();
        assert_eq!(name.as_str(), "secret=example");
        assert!(!format!("{name:?}").contains("secret"));
    }

    fn root(
        state: &mut ApplicationState,
        profile: ProfileId,
        application: ApplicationOwnerId,
    ) -> BranchId {
        let id = BranchId::fresh();
        apply(
            state,
            VersioningEvent::PrepareBranch {
                id,
                name: BranchName::new("main").unwrap(),
                application,
                profile,
                parent: None,
                fork: None,
                inheritance: InheritanceRequest::None,
            },
        )
        .unwrap();
        apply(
            state,
            VersioningEvent::PublishBranch {
                id,
                native: NativeProfileOwner::new(application, profile, ProfileInstanceId::fresh()),
                actual: InheritanceReport::None,
            },
        )
        .unwrap();
        id
    }

    fn child(
        state: &mut ApplicationState,
        profile: ProfileId,
        application: ApplicationOwnerId,
        parent: BranchId,
        name: &str,
        fork: CommitId,
    ) -> BranchId {
        let id = BranchId::fresh();
        apply(
            state,
            VersioningEvent::PrepareBranch {
                id,
                name: BranchName::new(name).unwrap(),
                application,
                profile,
                parent: Some(parent),
                fork: Some(fork),
                inheritance: InheritanceRequest::Clean,
            },
        )
        .unwrap();
        apply(
            state,
            VersioningEvent::PublishBranch {
                id,
                native: NativeProfileOwner::new(application, profile, ProfileInstanceId::fresh()),
                actual: InheritanceReport::Clean,
            },
        )
        .unwrap();
        id
    }

    #[test]
    fn clean_child_binds_its_actual_independent_runtime_owner() {
        let (mut state, lineage_profile, application) = setup();
        let main = root(&mut state, lineage_profile, application);
        let main_native = state.versioning().branch(main).unwrap().native().unwrap();
        let child_id = BranchId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareBranch {
                id: child_id,
                name: BranchName::new("trial").unwrap(),
                application,
                profile: lineage_profile,
                parent: Some(main),
                fork: Some(CommitId::fresh()),
                inheritance: InheritanceRequest::Clean,
            },
        )
        .unwrap();
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PublishBranch {
                    id: child_id,
                    native: main_native,
                    actual: InheritanceReport::Clean,
                },
            ),
            Err(ReduceError::Versioning(BranchError::DuplicateLiveOwner))
        );
        assert_eq!(state, before);

        let child_runtime_profile =
            ProfileId::from_display("profileid-999999").expect("separate runtime profile");
        let child_native = NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            child_runtime_profile,
            ProfileInstanceId::fresh(),
        );
        apply(
            &mut state,
            VersioningEvent::PublishBranch {
                id: child_id,
                native: child_native,
                actual: InheritanceReport::Clean,
            },
        )
        .unwrap();
        let child = state.versioning().branch(child_id).unwrap();
        assert_eq!(child.profile(), lineage_profile);
        assert_eq!(child.native(), Some(child_native));
        assert_ne!(child.native(), Some(main_native));
        state.validate().unwrap();
    }

    fn visible_owner(
        state: &mut ApplicationState,
        profile: ProfileId,
        branch: BranchId,
    ) -> (WindowId, TabId, CapturedOwner) {
        reduce(state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().last().unwrap();
        reduce(state, Event::OpenTab { window }).unwrap();
        let tab = state.windows()[&window].active_tab.unwrap();
        let owner = CapturedOwner {
            native: state.versioning().branch(branch).unwrap().native().unwrap(),
            window,
            target: state.capture_target(tab).unwrap(),
        };
        (window, tab, owner)
    }

    #[test]
    fn standalone_stash_owns_one_exact_view_without_creating_a_branch() {
        let (mut state, profile, application) = setup();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().next().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = state.windows()[&window].active_tab.unwrap();
        let owner = CapturedOwner {
            native: NativeProfileOwner::new(application, profile, ProfileInstanceId::fresh()),
            window,
            target: state.capture_target(tab).unwrap(),
        };
        let stash = StashId::fresh();
        let view = RetainedViewId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareStash { stash, view, owner },
        )
        .unwrap();
        assert!(state.versioning().branches().is_empty());
        assert_eq!(state.versioning().retained()[&view].stash(), Some(stash));
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PrepareStash {
                    stash,
                    view: RetainedViewId::fresh(),
                    owner,
                }
            ),
            Err(ReduceError::Versioning(BranchError::DuplicateStash))
        );
        assert_eq!(state, before);
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PrepareStash {
                    stash: StashId::fresh(),
                    view: RetainedViewId::fresh(),
                    owner,
                }
            ),
            Err(ReduceError::Versioning(BranchError::DuplicateLiveOwner))
        );
        assert_eq!(state, before);
        apply(&mut state, VersioningEvent::AcquireRetained { view }).unwrap();
        assert_eq!(
            state.versioning().retained()[&view].profile_lease_count(),
            1
        );
        assert_eq!(state.versioning().retained()[&view].view_lease_count(), 1);
        assert_eq!(
            state.versioning().visible_tabs_for_window(&state, window),
            Some(vec![])
        );
        apply(&mut state, VersioningEvent::ReleaseRetained { view }).unwrap();
        assert_eq!(
            state.versioning().visible_tabs_for_window(&state, window),
            Some(vec![tab])
        );
        assert!(state.versioning().retained().is_empty());
        state.validate().unwrap();
    }

    #[test]
    fn standalone_stash_prepare_rejects_stale_document_without_mutation() {
        let (mut state, profile, application) = setup();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().next().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = state.windows()[&window].active_tab.unwrap();
        let target = state.capture_target(tab).unwrap();
        let native = NativeProfileOwner::new(application, profile, ProfileInstanceId::fresh());
        reduce(
            &mut state,
            Event::StartNavigation {
                target,
                url: crate::ValidatedUrl::parse("https://example.test/changed").unwrap(),
            },
        )
        .unwrap();
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PrepareStash {
                    stash: StashId::fresh(),
                    view: RetainedViewId::fresh(),
                    owner: CapturedOwner {
                        native,
                        window,
                        target,
                    },
                }
            ),
            Err(ReduceError::Versioning(BranchError::RetainedOwnerMismatch))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn frozen_stash_commit_and_rollback_are_atomic_and_exact() {
        let (mut state, profile, application) = setup();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().next().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = state.windows()[&window].active_tab.unwrap();
        let owner = CapturedOwner {
            native: NativeProfileOwner::new(application, profile, ProfileInstanceId::fresh()),
            window,
            target: state.capture_target(tab).unwrap(),
        };
        let stash = StashId::fresh();
        let view = RetainedViewId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareStash { stash, view, owner },
        )
        .unwrap();
        let prepared = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::ConfirmFrozenStash {
                    stash: StashId::fresh(),
                    view,
                    owner,
                }
            ),
            Err(ReduceError::Versioning(BranchError::RetainedOwnerMismatch))
        );
        assert_eq!(state, prepared);
        apply(
            &mut state,
            VersioningEvent::ConfirmFrozenStash { stash, view, owner },
        )
        .unwrap();
        let record = &state.versioning().retained()[&view];
        assert_eq!(record.membership(), ViewMembership::Retained);
        assert_eq!(record.lifecycle(), RetainedLifecycle::Frozen);
        assert_eq!(
            (record.profile_lease_count(), record.view_lease_count()),
            (1, 1)
        );
        assert_eq!(
            state.versioning().visible_tabs_for_window(&state, window),
            Some(vec![])
        );
        let committed = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::ConfirmFrozenStash { stash, view, owner }
            ),
            Err(ReduceError::Versioning(BranchError::InvalidTransition))
        );
        assert_eq!(state, committed);
        apply(
            &mut state,
            VersioningEvent::RollbackFrozenStash { stash, view, owner },
        )
        .unwrap();
        let record = &state.versioning().retained()[&view];
        assert_eq!(record.membership(), ViewMembership::Visible);
        assert_eq!(record.lifecycle(), RetainedLifecycle::Active);
        assert_eq!(
            (record.profile_lease_count(), record.view_lease_count()),
            (0, 0)
        );
        assert_eq!(
            state.versioning().visible_tabs_for_window(&state, window),
            Some(vec![tab])
        );
        state.validate().unwrap();
    }

    #[test]
    fn parent_child_grandchild_keep_immutable_fork_while_parent_evolves() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        assert!(
            !state
                .versioning()
                .has_unresolved_child_branch_for_profile(profile)
        );
        let first_fork = CommitId::fresh();
        let branch = child(&mut state, profile, application, main, "trial", first_fork);
        assert!(
            state
                .versioning()
                .has_unresolved_child_branch_for_profile(profile)
        );
        let second_fork = CommitId::fresh();
        let grandchild = child(
            &mut state,
            profile,
            application,
            branch,
            "deep",
            second_fork,
        );
        assert_eq!(state.versioning().branch(grandchild).unwrap().depth(), 2);
        assert_eq!(
            state.versioning().branch(grandchild).unwrap().parent(),
            Some(branch)
        );
        assert_eq!(
            state.versioning().branch(branch).unwrap().fork(),
            Some(first_fork)
        );
        assert_eq!(
            state
                .versioning()
                .branch(branch)
                .unwrap()
                .requested_inheritance(),
            InheritanceRequest::Clean
        );
        assert_eq!(
            state
                .versioning()
                .branch(branch)
                .unwrap()
                .actual_inheritance(),
            Some(InheritanceReport::Clean)
        );
        let page = LogicalPageId::fresh();
        apply(
            &mut state,
            VersioningEvent::AdvanceWorking {
                id: main,
                expected_revision: 0,
                pages: vec![page],
                last_commit: Some(CommitId::fresh()),
            },
        )
        .unwrap();
        assert_eq!(
            state.versioning().branch(main).unwrap().working_revision(),
            1
        );
        assert_eq!(state.versioning().branch(main).unwrap().pages(), &[page]);
        assert_eq!(
            state.versioning().branch(branch).unwrap().fork(),
            Some(first_fork)
        );
        assert_eq!(
            state.versioning().branch(grandchild).unwrap().fork(),
            Some(second_fork)
        );
        assert_eq!(state.versioning().current_line(application), Some(main));
        state.validate().unwrap();
    }

    #[test]
    fn child_branch_snapshot_guard_tracks_nonterminal_lifecycle() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let id = BranchId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareBranch {
                id,
                name: BranchName::new("trial").unwrap(),
                application,
                profile,
                parent: Some(main),
                fork: Some(CommitId::fresh()),
                inheritance: InheritanceRequest::Clean,
            },
        )
        .unwrap();
        assert!(
            state
                .versioning()
                .has_unresolved_child_branch_for_profile(profile)
        );

        apply(
            &mut state,
            VersioningEvent::PublishBranch {
                id,
                native: NativeProfileOwner::new(application, profile, ProfileInstanceId::fresh()),
                actual: InheritanceReport::Clean,
            },
        )
        .unwrap();
        assert!(
            state
                .versioning()
                .has_unresolved_child_branch_for_profile(profile)
        );

        apply(&mut state, VersioningEvent::CloseBranch { id }).unwrap();
        assert!(
            !state
                .versioning()
                .has_unresolved_child_branch_for_profile(profile)
        );
    }

    #[test]
    fn lost_child_branch_stays_snapshot_guarded_until_dropped() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let branch = child(
            &mut state,
            profile,
            application,
            main,
            "lost-trial",
            CommitId::fresh(),
        );
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "unrelated".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let unrelated_profile = state
            .profiles()
            .keys()
            .copied()
            .find(|candidate| *candidate != profile)
            .unwrap();
        assert!(
            !state
                .versioning()
                .has_unresolved_child_branch_for_profile(unrelated_profile)
        );

        apply(&mut state, VersioningEvent::FailBranch { id: branch }).unwrap();
        assert!(
            state
                .versioning()
                .has_unresolved_child_branch_for_profile(profile)
        );

        apply(&mut state, VersioningEvent::DropBranch { id: branch }).unwrap();
        assert!(
            !state
                .versioning()
                .has_unresolved_child_branch_for_profile(profile)
        );
    }

    #[test]
    fn depth_limit_and_self_cycle_reject_without_mutation() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let mut parent = main;
        for depth in 1..=super::super::limits::MAX_BRANCH_NESTING {
            parent = child(
                &mut state,
                profile,
                application,
                parent,
                &format!("level-{depth}"),
                CommitId::fresh(),
            );
        }
        let before = state.clone();
        let overflow = BranchId::fresh();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PrepareBranch {
                    id: overflow,
                    name: BranchName::new("too-deep").unwrap(),
                    application,
                    profile,
                    parent: Some(parent),
                    fork: Some(CommitId::fresh()),
                    inheritance: InheritanceRequest::Clean,
                }
            ),
            Err(ReduceError::Versioning(BranchError::DepthExceeded))
        );
        assert_eq!(state, before);
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PrepareBranch {
                    id: overflow,
                    name: BranchName::new("self").unwrap(),
                    application,
                    profile,
                    parent: Some(overflow),
                    fork: Some(CommitId::fresh()),
                    inheritance: InheritanceRequest::Clean,
                }
            ),
            Err(ReduceError::Versioning(BranchError::Cycle))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn ambiguous_name_and_duplicate_live_owner_fail_closed() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let trial = child(
            &mut state,
            profile,
            application,
            main,
            "Trial",
            CommitId::fresh(),
        );
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PrepareBranch {
                    id: BranchId::fresh(),
                    name: BranchName::new(" trial ").unwrap(),
                    application,
                    profile,
                    parent: Some(main),
                    fork: Some(CommitId::fresh()),
                    inheritance: InheritanceRequest::Clean,
                }
            ),
            Err(ReduceError::Versioning(BranchError::AmbiguousName))
        );
        assert_eq!(state, before);
        let candidate = BranchId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareBranch {
                id: candidate,
                name: BranchName::new("another").unwrap(),
                application,
                profile,
                parent: Some(main),
                fork: Some(CommitId::fresh()),
                inheritance: InheritanceRequest::Clean,
            },
        )
        .unwrap();
        let owned = state.versioning().branch(trial).unwrap().native().unwrap();
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PublishBranch {
                    id: candidate,
                    native: owned,
                    actual: InheritanceReport::Clean,
                }
            ),
            Err(ReduceError::Versioning(BranchError::DuplicateLiveOwner))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn one_native_profile_instance_cannot_own_two_runtime_profiles() {
        let (mut state, first_profile, application) = setup();
        let first = root(&mut state, first_profile, application);
        let native = state.versioning().branch(first).unwrap().native().unwrap();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "other".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let second_profile = *state.profiles().keys().last().unwrap();
        let second = BranchId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareBranch {
                id: second,
                name: BranchName::new("main").unwrap(),
                application,
                profile: second_profile,
                parent: None,
                fork: None,
                inheritance: InheritanceRequest::None,
            },
        )
        .unwrap();
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PublishBranch {
                    id: second,
                    native: NativeProfileOwner::new(
                        application,
                        second_profile,
                        native.profile_instance
                    ),
                    actual: InheritanceReport::None,
                }
            ),
            Err(ReduceError::Versioning(BranchError::DuplicateLiveOwner))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn live_or_retained_work_and_children_prevent_deletion() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let trial = child(
            &mut state,
            profile,
            application,
            main,
            "trial",
            CommitId::fresh(),
        );
        assert_eq!(
            apply(&mut state, VersioningEvent::DropBranch { id: trial }),
            Err(ReduceError::Versioning(BranchError::LiveWorkOwned))
        );
        apply(&mut state, VersioningEvent::CloseBranch { id: trial }).unwrap();
        apply(&mut state, VersioningEvent::DropBranch { id: trial }).unwrap();
        assert!(state.versioning().branch(trial).is_none());
        assert_eq!(
            apply(&mut state, VersioningEvent::DropBranch { id: main }),
            Err(ReduceError::Versioning(BranchError::LiveWorkOwned))
        );
        state.validate().unwrap();
    }

    #[test]
    fn retained_membership_blocks_close_and_duplicate_ownership() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().next().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let target = state
            .capture_target(state.windows()[&window].active_tab.unwrap())
            .unwrap();
        let native = state.versioning().branch(main).unwrap().native().unwrap();
        let owner = CapturedOwner {
            native,
            window,
            target,
        };
        let view = RetainedViewId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareRetained {
                id: main,
                view,
                owner,
            },
        )
        .unwrap();
        apply(&mut state, VersioningEvent::AcquireRetained { view }).unwrap();
        assert!(
            state
                .versioning()
                .branch(main)
                .unwrap()
                .retained()
                .contains(&view)
        );
        assert_eq!(state.versioning().retained()[&view].owner(), owner);
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::PrepareRetained {
                    id: main,
                    view: RetainedViewId::fresh(),
                    owner,
                }
            ),
            Err(ReduceError::Versioning(BranchError::DuplicateLiveOwner))
        );
        assert_eq!(state, before);
        apply(&mut state, VersioningEvent::RetainBranch { id: main }).unwrap();
        assert_eq!(
            state.versioning().branch(main).unwrap().lifecycle(),
            BranchLifecycle::Retained
        );
        assert_eq!(
            apply(&mut state, VersioningEvent::CloseBranch { id: main }),
            Err(ReduceError::Versioning(BranchError::RetainedWorkOwned))
        );
        apply(&mut state, VersioningEvent::ReleaseRetained { view }).unwrap();
        apply(&mut state, VersioningEvent::ResumeBranch { id: main }).unwrap();
        apply(&mut state, VersioningEvent::CloseBranch { id: main }).unwrap();
        apply(&mut state, VersioningEvent::DropBranch { id: main }).unwrap();
        assert!(state.versioning().retained().is_empty());
        state.validate().unwrap();
    }

    #[test]
    fn leased_view_blocks_logical_tab_close_without_mutating_state() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().next().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = state.windows()[&window].active_tab.unwrap();
        let owner = CapturedOwner {
            native: state.versioning().branch(main).unwrap().native().unwrap(),
            window,
            target: state.capture_target(tab).unwrap(),
        };
        let view = RetainedViewId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareRetained {
                id: main,
                view,
                owner,
            },
        )
        .unwrap();
        apply(&mut state, VersioningEvent::AcquireRetained { view }).unwrap();
        let before = state.clone();
        assert_eq!(
            reduce(&mut state, Event::CloseTab { tab }),
            Err(ReduceError::Versioning(BranchError::RetainedWorkOwned))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn retention_lease_is_acquired_and_released_exactly_once() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let (window, tab, owner) = visible_owner(&mut state, profile, main);
        let view = RetainedViewId::fresh();
        assert_eq!(
            state.versioning().visible_tabs_for_window(&state, window),
            Some(vec![tab])
        );
        assert_eq!(
            state.versioning().membership_for_tab(&state, tab),
            Some(ViewMembership::Visible)
        );
        apply(
            &mut state,
            VersioningEvent::PrepareRetained {
                id: main,
                view,
                owner,
            },
        )
        .unwrap();
        let prepared = &state.versioning().retained()[&view];
        assert_eq!(prepared.membership(), ViewMembership::Visible);
        assert_eq!(
            (prepared.profile_lease_count(), prepared.view_lease_count()),
            (0, 0)
        );
        assert_eq!(
            apply(&mut state, VersioningEvent::RetainBranch { id: main }),
            Err(ReduceError::Versioning(BranchError::InvalidTransition))
        );
        apply(&mut state, VersioningEvent::AcquireRetained { view }).unwrap();
        let acquired = &state.versioning().retained()[&view];
        assert_eq!(acquired.membership(), ViewMembership::Retained);
        assert_eq!(
            (acquired.profile_lease_count(), acquired.view_lease_count()),
            (1, 1)
        );
        assert_eq!(
            state.versioning().membership_for_tab(&state, tab),
            Some(ViewMembership::Retained)
        );
        assert_eq!(
            state.versioning().visible_tabs_for_window(&state, window),
            Some(Vec::new())
        );
        let before = state.clone();
        assert_eq!(
            apply(&mut state, VersioningEvent::AcquireRetained { view }),
            Err(ReduceError::Versioning(BranchError::InvalidTransition))
        );
        assert_eq!(state, before);
        assert_eq!(
            reduce(&mut state, Event::CloseTab { tab }),
            Err(ReduceError::Versioning(BranchError::RetainedWorkOwned))
        );
        assert_eq!(
            apply(&mut state, VersioningEvent::CloseBranch { id: main }),
            Err(ReduceError::Versioning(BranchError::RetainedWorkOwned))
        );
        apply(&mut state, VersioningEvent::BeginRetainedClose { view }).unwrap();
        assert_eq!(
            state.versioning().retained()[&view].membership(),
            ViewMembership::Closing
        );
        assert_eq!(
            state.versioning().membership_for_tab(&state, tab),
            Some(ViewMembership::Closing)
        );
        apply(&mut state, VersioningEvent::ReleaseRetained { view }).unwrap();
        assert_eq!(
            state.versioning().membership_for_tab(&state, tab),
            Some(ViewMembership::Visible)
        );
        assert_eq!(
            state.versioning().visible_tabs_for_window(&state, window),
            Some(vec![tab])
        );
        assert_eq!(
            apply(&mut state, VersioningEvent::ReleaseRetained { view }),
            Err(ReduceError::Versioning(BranchError::UnknownRetainedView))
        );
        apply(&mut state, VersioningEvent::CloseBranch { id: main }).unwrap();
        state.validate().unwrap();
    }

    #[test]
    fn wrong_profile_retention_transfer_and_adoption_are_rejected() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let (_source, _tab, owner) = visible_owner(&mut state, profile, main);
        let view = RetainedViewId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareRetained {
                id: main,
                view,
                owner,
            },
        )
        .unwrap();
        apply(&mut state, VersioningEvent::AcquireRetained { view }).unwrap();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "other".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let other_profile = *state.profiles().keys().last().unwrap();
        reduce(
            &mut state,
            Event::CreateWindow {
                profile: other_profile,
            },
        )
        .unwrap();
        let other_window = *state.windows().keys().last().unwrap();
        let wrong_owner =
            NativeProfileOwner::new(application, other_profile, ProfileInstanceId::fresh());
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::BeginRetainedTransfer {
                    view,
                    destination: wrong_owner,
                    window: other_window,
                }
            ),
            Err(ReduceError::Versioning(BranchError::WrongProfile))
        );
        assert_eq!(state, before);
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let destination_window = *state.windows().keys().last().unwrap();
        apply(
            &mut state,
            VersioningEvent::BeginRetainedTransfer {
                view,
                destination: owner.native(),
                window: destination_window,
            },
        )
        .unwrap();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::AdoptRetained {
                    view,
                    destination: wrong_owner,
                    window: other_window,
                }
            ),
            Err(ReduceError::Versioning(BranchError::WrongProfile))
        );
        state.validate().unwrap();
    }

    #[test]
    fn retention_transfer_can_cancel_and_adopt_only_at_prepared_window() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let (_source, _tab, owner) = visible_owner(&mut state, profile, main);
        let view = RetainedViewId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareRetained {
                id: main,
                view,
                owner,
            },
        )
        .unwrap();
        apply(&mut state, VersioningEvent::AcquireRetained { view }).unwrap();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let destination_window = *state.windows().keys().last().unwrap();
        apply(
            &mut state,
            VersioningEvent::BeginRetainedTransfer {
                view,
                destination: owner.native(),
                window: destination_window,
            },
        )
        .unwrap();
        assert_eq!(
            state.versioning().retained()[&view].membership(),
            ViewMembership::Transferring
        );
        assert_eq!(
            state.versioning().retained()[&view].restore_window(),
            Some(destination_window)
        );
        assert_eq!(
            apply(&mut state, VersioningEvent::ReleaseRetained { view }),
            Err(ReduceError::Versioning(BranchError::InvalidTransition))
        );
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::AdoptRetained {
                    view,
                    destination: owner.native(),
                    window: owner.window(),
                }
            ),
            Err(ReduceError::Versioning(BranchError::InvalidTransition))
        );
        apply(&mut state, VersioningEvent::CancelRetainedTransfer { view }).unwrap();
        assert_eq!(
            state.versioning().retained()[&view].membership(),
            ViewMembership::Retained
        );
        apply(
            &mut state,
            VersioningEvent::BeginRetainedTransfer {
                view,
                destination: owner.native(),
                window: destination_window,
            },
        )
        .unwrap();
        apply(
            &mut state,
            VersioningEvent::AdoptRetained {
                view,
                destination: owner.native(),
                window: destination_window,
            },
        )
        .unwrap();
        assert!(!state.versioning().retained().contains_key(&view));
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::AdoptRetained {
                    view,
                    destination: owner.native(),
                    window: destination_window,
                }
            ),
            Err(ReduceError::Versioning(BranchError::UnknownRetainedView))
        );
        state.validate().unwrap();
    }

    #[test]
    fn lost_retention_is_not_reported_as_a_live_lease() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let (_window, tab, owner) = visible_owner(&mut state, profile, main);
        let view = RetainedViewId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareRetained {
                id: main,
                view,
                owner,
            },
        )
        .unwrap();
        apply(&mut state, VersioningEvent::AcquireRetained { view }).unwrap();
        apply(&mut state, VersioningEvent::RetainBranch { id: main }).unwrap();
        apply(
            &mut state,
            VersioningEvent::ObserveRetainedLifecycle {
                view,
                lifecycle: RetainedLifecycle::Frozen,
            },
        )
        .unwrap();
        assert_eq!(
            state.versioning().retained()[&view].lifecycle(),
            RetainedLifecycle::Frozen
        );
        apply(&mut state, VersioningEvent::LoseRetained { view }).unwrap();
        let lost = &state.versioning().retained()[&view];
        assert_eq!(lost.lifecycle(), RetainedLifecycle::Lost);
        assert_eq!(lost.membership(), ViewMembership::Closing);
        assert_eq!(
            (lost.profile_lease_count(), lost.view_lease_count()),
            (0, 0)
        );
        assert_eq!(
            apply(&mut state, VersioningEvent::ResumeBranch { id: main }),
            Err(ReduceError::Versioning(BranchError::InvalidTransition))
        );
        assert_eq!(
            apply(&mut state, VersioningEvent::LoseRetained { view }),
            Err(ReduceError::Versioning(BranchError::InvalidTransition))
        );
        reduce(&mut state, Event::CloseTab { tab }).unwrap();
        apply(&mut state, VersioningEvent::ReleaseRetained { view }).unwrap();
        state.validate().unwrap();
    }

    #[test]
    fn ordinary_view_removal_is_blocked_and_renderer_loss_marks_lease_lost() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let (window, tab, owner) = visible_owner(&mut state, profile, main);
        let view = RetainedViewId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareRetained {
                id: main,
                view,
                owner,
            },
        )
        .unwrap();
        apply(&mut state, VersioningEvent::AcquireRetained { view }).unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let before = state.clone();
        assert_eq!(
            reduce(&mut state, Event::DiscardTab { tab }),
            Err(ReduceError::Versioning(BranchError::RetainedWorkOwned))
        );
        assert_eq!(
            reduce(&mut state, Event::TransferTabOut { tab }),
            Err(ReduceError::Versioning(BranchError::RetainedWorkOwned))
        );
        assert_eq!(
            reduce(
                &mut state,
                Event::MoveTab {
                    tab,
                    to_window: window,
                    index: None
                }
            ),
            Err(ReduceError::Versioning(BranchError::RetainedWorkOwned))
        );
        assert_eq!(state, before);
        reduce(
            &mut state,
            Event::RendererTerminated {
                target: owner.target(),
            },
        )
        .unwrap();
        let record = &state.versioning().retained()[&view];
        assert_eq!(record.lifecycle(), RetainedLifecycle::Lost);
        assert_eq!(
            (record.profile_lease_count(), record.view_lease_count()),
            (0, 0)
        );
        reduce(&mut state, Event::CloseTab { tab }).unwrap();
        state.validate().unwrap();
    }

    #[test]
    fn new_document_navigation_cannot_keep_an_old_live_retention_claim() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let (_window, _tab, owner) = visible_owner(&mut state, profile, main);
        let view = RetainedViewId::fresh();
        apply(
            &mut state,
            VersioningEvent::PrepareRetained {
                id: main,
                view,
                owner,
            },
        )
        .unwrap();
        apply(&mut state, VersioningEvent::AcquireRetained { view }).unwrap();
        reduce(
            &mut state,
            Event::StartNavigation {
                target: owner.target(),
                url: crate::ValidatedUrl::parse("https://example.test/next").unwrap(),
            },
        )
        .unwrap();
        let record = &state.versioning().retained()[&view];
        assert_eq!(record.lifecycle(), RetainedLifecycle::Lost);
        assert_eq!(record.membership(), ViewMembership::Closing);
        assert_eq!(
            (record.profile_lease_count(), record.view_lease_count()),
            (0, 0)
        );
        state.validate().unwrap();
    }

    #[test]
    fn preparation_failure_and_revision_conflict_have_no_recovery_effect() {
        let (mut state, profile, application) = setup();
        let main = root(&mut state, profile, application);
        let before_revision = state.revision();
        let candidate = BranchId::fresh();
        let effects = reduce(
            &mut state,
            Event::Versioning(VersioningEvent::PrepareBranch {
                id: candidate,
                name: BranchName::new("pending").unwrap(),
                application,
                profile,
                parent: Some(main),
                fork: Some(CommitId::fresh()),
                inheritance: InheritanceRequest::Clean,
            }),
        )
        .unwrap();
        assert!(effects.is_empty());
        assert_eq!(state.revision(), before_revision + 1);
        apply(&mut state, VersioningEvent::FailBranch { id: candidate }).unwrap();
        assert_eq!(
            state.versioning().branch(candidate).unwrap().lifecycle(),
            BranchLifecycle::Lost
        );
        apply(&mut state, VersioningEvent::DropBranch { id: candidate }).unwrap();
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                VersioningEvent::AdvanceWorking {
                    id: main,
                    expected_revision: 1,
                    pages: vec![],
                    last_commit: None,
                }
            ),
            Err(ReduceError::Versioning(BranchError::RevisionMismatch))
        );
        assert_eq!(state, before);
        state.validate().unwrap();
    }

    #[test]
    fn transient_child_lineage_binds_only_live_ephemeral_tabs_atomically() {
        let mut state = ApplicationState::new();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "child".into(),
                privacy: PrivacyKind::Ephemeral,
            },
        )
        .unwrap();
        let profile = *state.profiles().keys().last().unwrap();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().last().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = state.windows()[&window].active_tab.unwrap();
        let logical_window = LogicalWindowId::fresh();
        let logical_child = LogicalPageId::fresh();
        let logical_parent = LogicalPageId::fresh();
        reduce(
            &mut state,
            Event::Versioning(VersioningEvent::BindTransientChildLineage {
                window,
                logical_window,
                pages: vec![(tab, logical_child, logical_parent)],
            }),
        )
        .unwrap();
        assert_eq!(
            state.versioning().logical_window(window),
            Some(logical_window)
        );
        assert_eq!(state.versioning().logical_page(tab), Some(logical_child));
        assert_eq!(state.versioning().lineage_parent(tab), Some(logical_parent));
        let before = state.clone();
        assert!(
            reduce(
                &mut state,
                Event::Versioning(VersioningEvent::BindTransientChildLineage {
                    window,
                    logical_window,
                    pages: vec![(tab, LogicalPageId::fresh(), logical_parent)],
                }),
            )
            .is_err()
        );
        assert_eq!(state, before);
        state.validate().unwrap();
    }
}
