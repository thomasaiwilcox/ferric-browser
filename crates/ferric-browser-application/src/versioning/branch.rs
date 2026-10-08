//! Application-side exact owner bookkeeping for a prepared clean branch.
//! Actual Quick profile/view identity is separately checked by the QML lease;
//! neither this book nor its branch token opens a profile store or grants
//! authority to rebind a parent view.

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use ferric_browser_core::{
    BranchId, BranchLifecycle, BranchName, CommitId, Event, ExistenceState, InheritanceReport,
    InheritanceRequest, LoadingState, LogicalPageId, LogicalWindowId, NativeProfileOwner,
    OperationId, PageCaptureInput, PrivacyKind, ProfileId, SafePageUrl, ScopeSelector, TabId,
    Target, VersioningEvent, VersioningFailure, VersioningReason, WindowId,
    versioning::SensitiveCategory,
};
use ferric_browser_runtime::{RuntimeEffect, RuntimeInput};
use ferric_browser_storage::versioning::{
    ManifestPage, ManifestSafeUrl, ManifestScope, ManifestUnavailable, VersionedCaptureManifest,
};

use super::{PreparedStructuralCommit, RestoreNavigation, capture};
use crate::BrowserApplication;

const MAX_CLEAN_BRANCH_OWNERS: usize = 32;
const BRANCH_EXPORT_PREVIEW_TTL: Duration = Duration::from_secs(60);

/// Fixed-copy disclosure data. Its token is only a one-use lookup key; the
/// manifest and source identities remain in the parent application.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BranchStructuralExportPreview {
    pub operation: OperationId,
    pub destination_profile: ProfileId,
    pub page_count: usize,
}

impl std::fmt::Debug for BranchStructuralExportPreview {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BranchStructuralExportPreview")
            .field("operation", &"[redacted]")
            .field("destination_profile", &"[redacted]")
            .field("page_count", &self.page_count)
            .finish()
    }
}

pub(crate) struct PendingBranchStructuralExport {
    branch: BranchId,
    source_window: WindowId,
    source_owner: NativeProfileOwner,
    child_window: WindowId,
    child_owner: NativeProfileOwner,
    child_revision: u64,
    child_tabs: Vec<TabId>,
    child_targets: Vec<Target>,
    child_selected: TabId,
    parent_revision: u64,
    prepared: PreparedStructuralCommit,
    created_at: Instant,
}

#[derive(Clone)]
pub(crate) struct PendingCleanBranchFork {
    branch: BranchId,
    name: BranchName,
    operation: OperationId,
    commit: CommitId,
    source_window: WindowId,
    source_profile: ProfileId,
    source_owner: NativeProfileOwner,
    source: PendingCleanBranchSource,
}

#[derive(Clone)]
enum PendingCleanBranchSource {
    Tab {
        target: Target,
        logical_page: LogicalPageId,
        navigation: RestoreNavigation,
    },
    Window {
        plan: CleanBranchWindowPlan,
        targets: Vec<Target>,
    },
}

/// A published line's source and destination are application identities, not
/// interchangeable local tab numbers. The child application remains the
/// authority for whether its view is still live at native activation time.
#[derive(Clone, Copy)]
pub(crate) struct PublishedCleanBranch {
    source_window: WindowId,
    source_tab: TabId,
    source_owner: NativeProfileOwner,
    child_owner: NativeProfileOwner,
    child_tab: TabId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanBranchSwitchTarget {
    Parent {
        branch: BranchId,
        window: WindowId,
    },
    Child {
        branch: BranchId,
        owner: NativeProfileOwner,
        tab: TabId,
    },
}

/// A bounded, source-ordered window snapshot for one future child profile.
/// This is not a native view lease and cannot be published as a branch until
/// every destination view is allocated and checked by the Qt adapter.
#[derive(Clone, Eq, PartialEq)]
pub struct CleanBranchWindowPage {
    pub source_tab: TabId,
    pub logical_page: LogicalPageId,
    pub navigation: RestoreNavigation,
}

impl std::fmt::Debug for CleanBranchWindowPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CleanBranchWindowPage")
            .field("source_tab", &"[redacted]")
            .field("logical_page", &"[redacted]")
            .field("navigation", &self.navigation)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct CleanBranchWindowPlan {
    pub source_window: WindowId,
    pub source_profile: ProfileId,
    pub source_owner: NativeProfileOwner,
    pub pages: Vec<CleanBranchWindowPage>,
    pub selected_index: usize,
}

impl std::fmt::Debug for CleanBranchWindowPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CleanBranchWindowPlan")
            .field("source", &"[redacted]")
            .field("pages", &self.pages.len())
            .field("selected_index", &self.selected_index)
            .finish()
    }
}

impl CleanBranchSwitchTarget {
    #[must_use]
    pub const fn branch(self) -> BranchId {
        match self {
            Self::Parent { branch, .. } | Self::Child { branch, .. } => branch,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum CleanBranchForkEvent {
    Durable {
        branch: BranchId,
        commit: CommitId,
        navigation: RestoreNavigation,
    },
    WindowDurable {
        branch: BranchId,
        commit: CommitId,
        plan: CleanBranchWindowPlan,
    },
    Failed {
        branch: BranchId,
    },
}

impl std::fmt::Debug for CleanBranchForkEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Durable { navigation, .. } => formatter
                .debug_struct("CleanBranchForkEvent::Durable")
                .field("navigation", navigation)
                .finish_non_exhaustive(),
            Self::WindowDurable { plan, .. } => formatter
                .debug_struct("CleanBranchForkEvent::WindowDurable")
                .field("pages", &plan.pages.len())
                .finish_non_exhaustive(),
            Self::Failed { .. } => formatter.write_str("CleanBranchForkEvent::Failed([redacted])"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanBranchLeaseError {
    AtCapacity,
    DuplicateBranch,
    UnknownBranch,
    AlreadyBound,
    ParentProfileReuse,
    ChildProfileReuse,
    StaleOwner,
    ViewStillOpen,
}

#[derive(Clone, Copy)]
struct Lease {
    parent: NativeProfileOwner,
    child: Option<NativeProfileOwner>,
    initial_view: Option<TabId>,
}

/// Bounded logical handshake for the native branch profile and its first view.
/// A token is only a lookup key; the Qt side must also prove the exact native
/// off-record profile and view objects before it publishes a child window.
#[derive(Default)]
pub struct CleanBranchProfileLeases {
    entries: BTreeMap<BranchId, Lease>,
}

impl CleanBranchProfileLeases {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve a typed branch token without allocating or navigating a child.
    pub fn prepare(
        &mut self,
        branch: BranchId,
        parent: NativeProfileOwner,
    ) -> Result<String, CleanBranchLeaseError> {
        if self.entries.contains_key(&branch) {
            return Err(CleanBranchLeaseError::DuplicateBranch);
        }
        if self.entries.len() >= MAX_CLEAN_BRANCH_OWNERS {
            return Err(CleanBranchLeaseError::AtCapacity);
        }
        self.entries.insert(
            branch,
            Lease {
                parent,
                child: None,
                initial_view: None,
            },
        );
        Ok(branch.to_string())
    }

    /// Bind the profile and initial view observed after a fresh transient
    /// profile bootstrap. Other branches may share a parent, never a child.
    pub fn bind_child(
        &mut self,
        branch: BranchId,
        child: NativeProfileOwner,
        initial_view: TabId,
    ) -> Result<(), CleanBranchLeaseError> {
        self.can_bind_child(branch, child)?;
        let lease = self
            .entries
            .get_mut(&branch)
            .ok_or(CleanBranchLeaseError::UnknownBranch)?;
        lease.child = Some(child);
        lease.initial_view = Some(initial_view);
        Ok(())
    }

    #[must_use = "a failed owner preflight must stop branch publication"]
    pub fn can_bind_child(
        &self,
        branch: BranchId,
        child: NativeProfileOwner,
    ) -> Result<(), CleanBranchLeaseError> {
        let lease = self
            .entries
            .get(&branch)
            .ok_or(CleanBranchLeaseError::UnknownBranch)?;
        if lease.child.is_some() {
            return Err(CleanBranchLeaseError::AlreadyBound);
        }
        if lease.parent == child {
            return Err(CleanBranchLeaseError::ParentProfileReuse);
        }
        if self
            .entries
            .values()
            .any(|existing| existing.child == Some(child))
        {
            return Err(CleanBranchLeaseError::ChildProfileReuse);
        }
        Ok(())
    }

    /// A bound profile cannot be retired while its exact first view is live.
    pub fn close_initial_view(
        &mut self,
        branch: BranchId,
        child: NativeProfileOwner,
        view: TabId,
    ) -> Result<(), CleanBranchLeaseError> {
        let lease = self
            .entries
            .get_mut(&branch)
            .ok_or(CleanBranchLeaseError::UnknownBranch)?;
        if lease.child != Some(child) || lease.initial_view != Some(view) {
            return Err(CleanBranchLeaseError::StaleOwner);
        }
        lease.initial_view = None;
        Ok(())
    }

    pub fn release_child(
        &mut self,
        branch: BranchId,
        child: NativeProfileOwner,
    ) -> Result<(), CleanBranchLeaseError> {
        let lease = self
            .entries
            .get(&branch)
            .ok_or(CleanBranchLeaseError::UnknownBranch)?;
        if lease.child != Some(child) {
            return Err(CleanBranchLeaseError::StaleOwner);
        }
        if lease.initial_view.is_some() {
            return Err(CleanBranchLeaseError::ViewStillOpen);
        }
        self.entries.remove(&branch);
        Ok(())
    }

    /// Roll back an allocation that never acquired a child profile.
    pub fn abort_preparation(&mut self, branch: BranchId) -> Result<(), CleanBranchLeaseError> {
        let lease = self
            .entries
            .get(&branch)
            .ok_or(CleanBranchLeaseError::UnknownBranch)?;
        if lease.child.is_some() {
            return Err(CleanBranchLeaseError::AlreadyBound);
        }
        self.entries.remove(&branch);
        Ok(())
    }

    #[must_use]
    pub fn contains(&self, branch: BranchId) -> bool {
        self.entries.contains_key(&branch)
    }

    #[must_use]
    pub fn parent(&self, branch: BranchId) -> Option<NativeProfileOwner> {
        self.entries.get(&branch).map(|lease| lease.parent)
    }

    #[must_use]
    pub fn child(&self, branch: BranchId) -> Option<(NativeProfileOwner, TabId)> {
        let lease = self.entries.get(&branch)?;
        Some((lease.child?, lease.initial_view?))
    }
}

impl BrowserApplication {
    /// Begin an exact single-tab, structural-only durable fork. The source
    /// remains live and usable; a child must not be allocated or shown merely
    /// because this method returned a queued commit.
    pub fn begin_clean_branch_fork(
        &mut self,
        window: WindowId,
        target: Target,
        name: &str,
    ) -> Result<(BranchId, OperationId, CommitId, Vec<RuntimeEffect>), VersioningFailure> {
        if self.pending_clean_branch_fork.is_some() {
            return Err(branch_failure(VersioningReason::Busy));
        }
        let name = BranchName::new(name)
            .map_err(|_| branch_failure(VersioningReason::UnsupportedCapability))?;
        let source_profile = self
            .windows()
            .get(&window)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?
            .profile;
        let (_, source_owner) = self.structural_commit_owner(window)?;
        let branch = BranchId::fresh();
        self.prepare_clean_branch_profile(branch, source_profile)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        let result = self.prepare_structural_commit(
            ScopeSelector::Tab { window, target },
            "Branch fork".to_owned(),
        );
        let result = result.and_then(|prepared| {
            let source = prepared
                .manifest
                .windows
                .as_slice()
                .first()
                .filter(|_| {
                    prepared.manifest.scope == ManifestScope::Tab
                        && prepared.manifest.windows.len() == 1
                })
                .ok_or_else(|| branch_failure(VersioningReason::UnsafeRestoreDescriptor))?;
            let page = source
                .pages
                .as_slice()
                .first()
                .filter(|_| source.pages.len() == 1)
                .ok_or_else(|| branch_failure(VersioningReason::UnsafeRestoreDescriptor))?;
            let source_logical_page = page
                .logical_page_id
                .parse::<LogicalPageId>()
                .map_err(|_| branch_failure(VersioningReason::UnsafeRestoreDescriptor))?;
            let navigation = clean_branch_navigation(page)?;
            let (operation, commit, effects) = self.submit_prepared_structural_commit(prepared)?;
            self.pending_clean_branch_fork = Some(PendingCleanBranchFork {
                branch,
                name,
                operation,
                commit,
                source_window: window,
                source_profile,
                source_owner,
                source: PendingCleanBranchSource::Tab {
                    target,
                    logical_page: source_logical_page,
                    navigation,
                },
            });
            Ok((branch, operation, commit, effects))
        });
        if result.is_err() {
            let _ = self.abort_clean_branch_profile(branch);
        }
        result
    }

    /// Queue one structural capture for the entire source window. This does
    /// not expose a command or authorize a native child: the completed plan
    /// must still be consumed by an exact multi-view child transaction.
    pub fn begin_clean_branch_window_fork(
        &mut self,
        window: WindowId,
        name: &str,
    ) -> Result<(BranchId, OperationId, CommitId, Vec<RuntimeEffect>), VersioningFailure> {
        if self.pending_clean_branch_fork.is_some() {
            return Err(branch_failure(VersioningReason::Busy));
        }
        let name = BranchName::new(name)
            .map_err(|_| branch_failure(VersioningReason::UnsupportedCapability))?;
        let source_profile = self
            .windows()
            .get(&window)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?
            .profile;
        let (_, source_owner) = self.structural_commit_owner(window)?;
        let branch = BranchId::fresh();
        self.prepare_clean_branch_profile(branch, source_profile)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        let result = self
            .prepare_structural_commit(ScopeSelector::Window { window }, "Branch fork".to_owned());
        let result = result.and_then(|prepared| {
            let plan = self.plan_clean_branch_window(window, &prepared.manifest)?;
            let targets = plan
                .pages
                .iter()
                .map(|page| {
                    self.capture_target(page.source_tab)
                        .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (operation, commit, effects) = self.submit_prepared_structural_commit(prepared)?;
            self.pending_clean_branch_fork = Some(PendingCleanBranchFork {
                branch,
                name,
                operation,
                commit,
                source_window: window,
                source_profile,
                source_owner,
                source: PendingCleanBranchSource::Window { plan, targets },
            });
            Ok((branch, operation, commit, effects))
        });
        if result.is_err() {
            let _ = self.abort_clean_branch_profile(branch);
        }
        result
    }

    /// Validate one already prepared window-scope manifest against the same
    /// current native owner, ordered live tabs and selected logical page.
    /// It intentionally does not allocate a child or admit a command while
    /// the multi-view child-host transaction is not yet wired.
    pub fn plan_clean_branch_window(
        &self,
        window: WindowId,
        manifest: &VersionedCaptureManifest,
    ) -> Result<CleanBranchWindowPlan, VersioningFailure> {
        manifest
            .validate()
            .map_err(|_| branch_failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let source = self
            .windows()
            .get(&window)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        let native = self
            .versioning
            .owner(source.profile)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        let (profile_uuid, checked_owner) = self.structural_commit_owner(window)?;
        if checked_owner != native
            || manifest.profile_uuid != profile_uuid
            || manifest.application_revision != self.revision()
            || manifest.scope != ManifestScope::Window
            || manifest.windows.len() != 1
            || source.tabs.is_empty()
            || source.tabs.len()
                > ferric_browser_core::versioning::limits::MAX_PAGES_PER_WINDOW_CAPTURE
        {
            return Err(branch_failure(VersioningReason::StaleOwner));
        }
        let captured = &manifest.windows[0];
        if captured.pages.len() != source.tabs.len()
            || self
                .runtime_view()
                .logical_window(window)
                .is_none_or(|logical| captured.logical_window_id != logical.to_string())
        {
            return Err(branch_failure(VersioningReason::StaleOwner));
        }
        let mut pages = Vec::with_capacity(captured.pages.len());
        let mut selected_index = None;
        for (index, (&tab, page)) in source.tabs.iter().zip(&captured.pages).enumerate() {
            if self.tabs().get(&tab).is_none_or(|state| {
                state.window != window
                    || state.profile != source.profile
                    || state.existence != ExistenceState::Live
            }) {
                return Err(branch_failure(VersioningReason::StaleOwner));
            }
            let logical_page = page
                .logical_page_id
                .parse::<LogicalPageId>()
                .map_err(|_| branch_failure(VersioningReason::UnsafeRestoreDescriptor))?;
            if self.runtime_view().logical_page(tab) != Some(logical_page) {
                return Err(branch_failure(VersioningReason::StaleOwner));
            }
            if source.active_tab == Some(tab) {
                if captured.selected_logical_page.as_deref() != Some(page.logical_page_id.as_str())
                {
                    return Err(branch_failure(VersioningReason::StaleOwner));
                }
                selected_index = Some(index);
            }
            let navigation = clean_branch_navigation(page)?;
            if let RestoreNavigation::SafeGet(url) = &navigation {
                let target = self
                    .capture_target(tab)
                    .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
                if self.safe_get_navigation_proofs.get(&tab) != Some(&(target, url.clone()))
                    || self.tabs().get(&tab).and_then(|state| state.url.as_deref())
                        != Some(url.as_str())
                {
                    return Err(branch_failure(VersioningReason::UnsafeRestoreDescriptor));
                }
            }
            pages.push(CleanBranchWindowPage {
                source_tab: tab,
                logical_page,
                navigation,
            });
        }
        let selected_index =
            selected_index.ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        Ok(CleanBranchWindowPlan {
            source_window: window,
            source_profile: source.profile,
            source_owner: native,
            pages,
            selected_index,
        })
    }

    pub(crate) fn note_clean_branch_fork_commit(
        &mut self,
        operation: OperationId,
        commit: CommitId,
        durable: bool,
    ) {
        let Some(pending) = self.pending_clean_branch_fork.as_ref() else {
            return;
        };
        if pending.operation != operation || pending.commit != commit {
            return;
        }
        let pending = self
            .pending_clean_branch_fork
            .take()
            .expect("checked above");
        let source_still_exact = self.clean_branch_source_still_exact(&pending);
        if durable && source_still_exact {
            let event = match &pending.source {
                PendingCleanBranchSource::Tab { navigation, .. } => CleanBranchForkEvent::Durable {
                    branch: pending.branch,
                    commit: pending.commit,
                    navigation: navigation.clone(),
                },
                PendingCleanBranchSource::Window { plan, .. } => {
                    CleanBranchForkEvent::WindowDurable {
                        branch: pending.branch,
                        commit: pending.commit,
                        plan: plan.clone(),
                    }
                }
            };
            self.clean_branch_fork_events.push_back(event);
            self.ready_clean_branch_forks
                .insert(pending.branch, pending);
        } else {
            let _ = self.abort_clean_branch_profile(pending.branch);
            self.clean_branch_fork_events
                .push_back(CleanBranchForkEvent::Failed {
                    branch: pending.branch,
                });
        }
    }

    fn clean_branch_source_still_exact(&self, pending: &PendingCleanBranchFork) -> bool {
        let common_source_still_exact = self
            .windows()
            .get(&pending.source_window)
            .is_some_and(|window| window.profile == pending.source_profile)
            && self.versioning.owner(pending.source_profile) == Some(pending.source_owner)
            && self.clean_branch_profiles.parent(pending.branch) == Some(pending.source_owner);
        common_source_still_exact
            && match &pending.source {
                PendingCleanBranchSource::Tab {
                    target,
                    logical_page,
                    navigation,
                } => {
                    self.capture_target(target.tab) == Some(*target)
                        && self.runtime_view().logical_page(target.tab) == Some(*logical_page)
                        && self.clean_branch_navigation_still_exact(*target, navigation)
                }
                PendingCleanBranchSource::Window { plan, targets } => {
                    self.windows()
                        .get(&pending.source_window)
                        .is_some_and(|window| {
                            window.tabs.len() == plan.pages.len()
                                && window.active_tab
                                    == Some(plan.pages[plan.selected_index].source_tab)
                                && window
                                    .tabs
                                    .iter()
                                    .zip(&plan.pages)
                                    .all(|(tab, page)| *tab == page.source_tab)
                        })
                        && targets.len() == plan.pages.len()
                        && targets.iter().zip(&plan.pages).all(|(target, page)| {
                            target.tab == page.source_tab
                                && self.capture_target(target.tab) == Some(*target)
                                && self.runtime_view().logical_page(target.tab)
                                    == Some(page.logical_page)
                                && self
                                    .clean_branch_navigation_still_exact(*target, &page.navigation)
                        })
                }
            }
    }

    fn clean_branch_navigation_still_exact(
        &self,
        target: Target,
        navigation: &RestoreNavigation,
    ) -> bool {
        match navigation {
            RestoreNavigation::SafeGet(url) => {
                self.safe_get_navigation_proofs.get(&target.tab) == Some(&(target, url.clone()))
                    && self
                        .tabs()
                        .get(&target.tab)
                        .and_then(|tab| tab.url.as_deref())
                        == Some(url.as_str())
            }
            RestoreNavigation::Placeholder(_) => true,
        }
    }

    #[must_use]
    pub fn poll_clean_branch_fork_events(&mut self) -> Vec<CleanBranchForkEvent> {
        self.clean_branch_fork_events.drain(..).collect()
    }

    /// Abandon a durable fork that never obtained a child native profile.
    /// The structural commit remains a normal durable user commit.
    pub fn abort_ready_clean_branch_fork(&mut self, branch: BranchId) -> bool {
        if !self.ready_clean_branch_forks.contains_key(&branch)
            || self.abort_clean_branch_profile(branch).is_err()
        {
            return false;
        }
        self.ready_clean_branch_forks.remove(&branch);
        true
    }

    /// Called only after the native child view has closed. Refuses a stale
    /// profile generation and preflights the branch close/drop sequence before
    /// releasing the exact Rust profile lease. The parent root remains live.
    pub fn close_published_clean_branch(
        &mut self,
        branch: BranchId,
        child_owner: NativeProfileOwner,
    ) -> Result<Vec<RuntimeEffect>, VersioningFailure> {
        let (_, first_view) = self
            .clean_branch_profiles
            .child(branch)
            .filter(|(owner, _)| *owner == child_owner)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        let record = self
            .runtime_view()
            .versioning_branch(branch)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        if record.parent().is_none()
            || record.native() != Some(child_owner)
            || record.lifecycle() != BranchLifecycle::Live
        {
            return Err(branch_failure(VersioningReason::StaleOwner));
        }
        let events = [
            VersioningEvent::CloseBranch { id: branch },
            VersioningEvent::DropBranch { id: branch },
        ];
        self.runtime_view()
            .preview_versioning_events(&events)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        self.close_clean_branch_initial_view(branch, child_owner, first_view)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        let mut effects = Vec::new();
        for event in events {
            effects.extend(
                self.dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(event)))
                    .map_err(|_| branch_failure(VersioningReason::StaleOwner))?,
            );
        }
        self.release_clean_branch_profile(branch, child_owner)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        self.published_clean_branches.remove(&branch);
        Ok(effects)
    }

    /// Bind the durable fork to the actual second ephemeral runtime and its
    /// first tab. The caller still owns native Qt profile/view validation and
    /// must leave the child hidden until those exact objects are admitted.
    pub fn publish_clean_branch_lineage(
        &mut self,
        branch: BranchId,
        child: &mut BrowserApplication,
        child_profile: ProfileId,
        child_tab: TabId,
    ) -> Result<(Vec<RuntimeEffect>, Vec<RuntimeEffect>), VersioningFailure> {
        self.publish_clean_branch_pages(branch, child, child_profile, &[child_tab], false)
    }

    /// Publish an ordered group only after the native adapter has created
    /// exactly one child profile and one real view for each child tab. The
    /// first tab is the profile lease's initial view, not an independent line.
    pub fn publish_clean_branch_window_lineage(
        &mut self,
        branch: BranchId,
        child: &mut BrowserApplication,
        child_profile: ProfileId,
        child_tabs: &[TabId],
    ) -> Result<(Vec<RuntimeEffect>, Vec<RuntimeEffect>), VersioningFailure> {
        self.publish_clean_branch_pages(branch, child, child_profile, child_tabs, true)
    }

    fn publish_clean_branch_pages(
        &mut self,
        branch: BranchId,
        child: &mut BrowserApplication,
        child_profile: ProfileId,
        child_tabs: &[TabId],
        window_scope: bool,
    ) -> Result<(Vec<RuntimeEffect>, Vec<RuntimeEffect>), VersioningFailure> {
        let ready = self
            .ready_clean_branch_forks
            .get(&branch)
            .cloned()
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        if !self.clean_branch_source_still_exact(&ready) {
            return Err(branch_failure(VersioningReason::StaleOwner));
        }
        let (source_tabs, source_pages, selected_index) = match (&ready.source, window_scope) {
            (
                PendingCleanBranchSource::Tab {
                    target,
                    logical_page,
                    ..
                },
                false,
            ) => (vec![target.tab], vec![*logical_page], 0),
            (PendingCleanBranchSource::Window { plan, .. }, true) => (
                plan.pages.iter().map(|page| page.source_tab).collect(),
                plan.pages.iter().map(|page| page.logical_page).collect(),
                plan.selected_index,
            ),
            _ => return Err(branch_failure(VersioningReason::UnsupportedCapability)),
        };
        if child_tabs.is_empty() || child_tabs.len() != source_pages.len() {
            return Err(branch_failure(VersioningReason::StaleOwner));
        }
        let child_tab = child_tabs[0];
        let child_native = self
            .checked_clean_branch_child(branch, child, child_profile, child_tab)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        let child_window = child
            .tabs()
            .get(&child_tab)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?
            .window;
        let child_window_state = child
            .windows()
            .get(&child_window)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        if child_window_state.profile != child_profile
            || child_window_state.tabs.as_slice() != child_tabs
            || child_window_state.active_tab != Some(child_tabs[selected_index])
            || child_tabs.iter().any(|tab| {
                child.tabs().get(tab).is_none_or(|state| {
                    state.window != child_window
                        || state.profile != child_profile
                        || state.existence != ExistenceState::Live
                })
            })
        {
            return Err(branch_failure(VersioningReason::StaleOwner));
        }
        let root = self
            .runtime_view()
            .versioning_root_branch(self.application_owner, ready.source_profile)
            .map(|record| {
                (
                    record.id(),
                    record.native(),
                    record.lifecycle(),
                    record.working_revision(),
                )
            });
        if root.as_ref().is_some_and(|(_, native, lifecycle, _)| {
            *native != Some(ready.source_owner) || *lifecycle != BranchLifecycle::Live
        }) {
            return Err(branch_failure(VersioningReason::StaleOwner));
        }

        let child_logical_pages: Vec<_> =
            child_tabs.iter().map(|_| LogicalPageId::fresh()).collect();
        let child_events = [VersioningEvent::BindTransientChildLineage {
            window: child_window,
            logical_window: LogicalWindowId::fresh(),
            pages: child_tabs
                .iter()
                .copied()
                .zip(child_logical_pages.iter().copied())
                .zip(source_pages.iter().copied())
                .map(|((tab, child_page), source_page)| (tab, child_page, source_page))
                .collect(),
        }];
        let mut parent_events = Vec::new();
        let application_owner = self.application_owner;
        let (root_id, root_revision) = if let Some((id, _, _, revision)) = root {
            (id, revision)
        } else {
            let id = BranchId::fresh();
            parent_events.push(VersioningEvent::PrepareBranch {
                id,
                name: BranchName::new("main").expect("fixed root name"),
                application: application_owner,
                profile: ready.source_profile,
                parent: None,
                fork: None,
                inheritance: InheritanceRequest::None,
            });
            parent_events.push(VersioningEvent::PublishBranch {
                id,
                native: ready.source_owner,
                actual: InheritanceReport::None,
            });
            (id, 0)
        };
        parent_events.extend([
            VersioningEvent::AdvanceWorking {
                id: root_id,
                expected_revision: root_revision,
                pages: source_pages,
                last_commit: Some(ready.commit),
            },
            VersioningEvent::PrepareBranch {
                id: branch,
                name: ready.name,
                application: application_owner,
                profile: ready.source_profile,
                parent: Some(root_id),
                fork: Some(ready.commit),
                inheritance: InheritanceRequest::Clean,
            },
            VersioningEvent::PublishBranch {
                id: branch,
                native: child_native,
                actual: InheritanceReport::Clean,
            },
            VersioningEvent::AdvanceWorking {
                id: branch,
                expected_revision: 0,
                pages: child_logical_pages,
                last_commit: None,
            },
        ]);
        self.runtime_view()
            .preview_versioning_events(&parent_events)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        child
            .runtime_view()
            .preview_versioning_events(&child_events)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        self.bind_clean_branch_profile(branch, child, child_profile, child_tab)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        let mut child_effects = Vec::new();
        for event in child_events {
            child_effects.extend(
                child
                    .dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(event)))
                    .map_err(|_| branch_failure(VersioningReason::StaleOwner))?,
            );
        }
        let mut parent_effects = Vec::new();
        for event in parent_events {
            parent_effects.extend(
                self.dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(event)))
                    .map_err(|_| branch_failure(VersioningReason::StaleOwner))?,
            );
        }
        self.ready_clean_branch_forks.remove(&branch);
        self.published_clean_branches.insert(
            branch,
            PublishedCleanBranch {
                source_window: ready.source_window,
                source_tab: source_tabs[0],
                source_owner: ready.source_owner,
                child_owner: child_native,
                child_tab,
            },
        );
        Ok((parent_effects, child_effects))
    }

    /// Prepare a structural-only export from one linked off-record branch.
    /// This leaves both applications and storage unchanged until a separate
    /// exact, one-use confirmation; the returned preview contains no URL or
    /// child profile database material.
    pub fn preview_clean_branch_structural_export(
        &mut self,
        branch: BranchId,
        child: &BrowserApplication,
        destination_window: WindowId,
        child_window: WindowId,
        message: String,
    ) -> Result<BranchStructuralExportPreview, VersioningFailure> {
        if self.pending_commit.is_some()
            || self
                .pending_branch_structural_export
                .as_ref()
                .is_some_and(|pending| pending.created_at.elapsed() <= BRANCH_EXPORT_PREVIEW_TTL)
        {
            return Err(branch_failure(VersioningReason::Busy));
        }
        if !capture::valid_commit_message(&message) {
            return Err(branch_failure(VersioningReason::UnsupportedCapability));
        }
        let line = self
            .published_clean_branches
            .get(&branch)
            .copied()
            .filter(|line| line.source_window == destination_window)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        let (profile_uuid, source_owner) = self.structural_commit_owner(destination_window)?;
        if source_owner != line.source_owner
            || !self.validate_clean_branch_switch_child(branch, child, line.child_tab)
        {
            return Err(branch_failure(VersioningReason::StaleOwner));
        }
        let child_profile = line.child_owner.runtime_profile();
        let child_state = child
            .windows()
            .get(&child_window)
            .filter(|window| {
                window.profile == child_profile
                    && window.tabs.contains(&line.child_tab)
                    && window.active_tab.is_some()
            })
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        if child
            .profiles()
            .get(&child_profile)
            .is_none_or(|profile| profile.privacy != PrivacyKind::Ephemeral)
            || child.versioning.owner(child_profile) != Some(line.child_owner)
        {
            return Err(branch_failure(VersioningReason::PrivatePersistenceDenied));
        }
        let selector = ScopeSelector::Window {
            window: child_window,
        };
        let captured = child
            .runtime_view()
            .resolve_versioning_scope(selector, line.child_owner)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        let logical_window = child
            .runtime_view()
            .logical_window(child_window)
            .unwrap_or_else(LogicalWindowId::fresh);
        let mut inputs = BTreeMap::new();
        let mut targets = Vec::with_capacity(captured.members().len());
        for member in captured.members() {
            let target = member.target();
            let logical = child
                .runtime_view()
                .logical_page(target.tab)
                .unwrap_or_else(LogicalPageId::fresh);
            let mut input = PageCaptureInput::structural_only(logical, target);
            input.lineage_parent = child.runtime_view().lineage_parent(target.tab);
            input.proven_safe_get = child
                .safe_get_navigation_proofs
                .get(&target.tab)
                .is_some_and(|(proved_target, proved_url)| {
                    *proved_target == target
                        && child.tabs()[&target.tab].url.as_deref() == Some(proved_url)
                        && matches!(
                            child.tabs()[&target.tab].loading,
                            LoadingState::Committed | LoadingState::Complete
                        )
                });
            inputs.insert(target.tab, input);
            targets.push(target);
        }
        let structural = child
            .runtime_view()
            .project_structural_scope(&captured, logical_window, &inputs)
            .map_err(|_| branch_failure(VersioningReason::StaleOwner))?;
        let commit = CommitId::fresh();
        let started = capture::unix_millis()?;
        let mut manifest = VersionedCaptureManifest::from_structural(
            capture::structural_header(
                commit,
                profile_uuid,
                message,
                self.configuration_revision(),
                ManifestScope::Window,
                started,
            )?,
            &structural,
        )
        .map_err(|_| branch_failure(VersioningReason::UnsafeRestoreDescriptor))?;
        manifest.application_revision = self.revision();
        manifest
            .validate()
            .map_err(|_| branch_failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let operation = OperationId::fresh();
        let page_count = manifest
            .windows
            .first()
            .map(|window| window.pages.len())
            .filter(|count| *count > 0)
            .ok_or_else(|| branch_failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let preview = BranchStructuralExportPreview {
            operation,
            destination_profile: line.source_owner.runtime_profile(),
            page_count,
        };
        self.pending_branch_structural_export = Some(PendingBranchStructuralExport {
            branch,
            source_window: destination_window,
            source_owner,
            child_window,
            child_owner: line.child_owner,
            child_revision: child.revision(),
            child_tabs: child_state.tabs.clone(),
            child_targets: targets,
            child_selected: child_state.active_tab.expect("checked above"),
            parent_revision: self.revision(),
            prepared: PreparedStructuralCommit {
                operation,
                commit,
                manifest,
                runtime_effects: Vec::new(),
            },
            created_at: Instant::now(),
        });
        Ok(preview)
    }

    /// Declining the native export preview performs no durable write.
    pub fn cancel_clean_branch_structural_export(&mut self, operation: OperationId) -> bool {
        if self
            .pending_branch_structural_export
            .as_ref()
            .is_none_or(|pending| pending.prepared.operation != operation)
        {
            return false;
        }
        self.pending_branch_structural_export.take();
        true
    }

    /// Consume one native confirmation only while both exact owners and all
    /// child pages still match the preview. Publication uses a new normal-
    /// profile commit ID; the child itself never opens durable storage.
    pub fn confirm_clean_branch_structural_export(
        &mut self,
        operation: OperationId,
        child: &BrowserApplication,
        destination_window: WindowId,
        child_window: WindowId,
    ) -> Result<(OperationId, CommitId, Vec<RuntimeEffect>), VersioningFailure> {
        let pending = self
            .pending_branch_structural_export
            .as_ref()
            .filter(|pending| pending.prepared.operation == operation)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        if self.pending_commit.is_some() {
            return Err(branch_failure(VersioningReason::Busy));
        }
        let exact = pending.created_at.elapsed() <= BRANCH_EXPORT_PREVIEW_TTL
            && pending.source_window == destination_window
            && pending.child_window == child_window
            && self.revision() == pending.parent_revision
            && child.revision() == pending.child_revision
            && self
                .structural_commit_owner(destination_window)
                .is_ok_and(|(uuid, owner)| {
                    owner == pending.source_owner && uuid == pending.prepared.manifest.profile_uuid
                })
            && self
                .published_clean_branches
                .get(&pending.branch)
                .is_some_and(|line| {
                    line.source_window == destination_window
                        && line.source_owner == pending.source_owner
                        && line.child_owner == pending.child_owner
                        && self.validate_clean_branch_switch_child(
                            pending.branch,
                            child,
                            line.child_tab,
                        )
                })
            && child.windows().get(&child_window).is_some_and(|window| {
                window.profile == pending.child_owner.runtime_profile()
                    && window.tabs == pending.child_tabs
                    && window.active_tab == Some(pending.child_selected)
            })
            && pending
                .child_targets
                .iter()
                .all(|target| child.capture_target(target.tab) == Some(*target))
            && pending
                .prepared
                .manifest
                .windows
                .first()
                .is_some_and(|window| {
                    window.pages.len() == pending.child_tabs.len()
                        && pending
                            .child_tabs
                            .iter()
                            .zip(&window.pages)
                            .all(|(tab, page)| match &page.safe_url_or_placeholder {
                                ManifestSafeUrl::Replayable(url) => {
                                    child.capture_target(*tab).is_some_and(|target| {
                                        child.safe_get_navigation_proofs.get(tab)
                                            == Some(&(target, url.clone()))
                                            && child
                                                .tabs()
                                                .get(tab)
                                                .and_then(|state| state.url.as_deref())
                                                == Some(url.as_str())
                                    })
                                }
                                ManifestSafeUrl::Placeholder(_) => true,
                            })
                });
        let pending = self
            .pending_branch_structural_export
            .take()
            .expect("matching preview checked above");
        if !exact {
            return Err(branch_failure(VersioningReason::StaleOwner));
        }
        self.submit_prepared_structural_commit(pending.prepared)
    }

    /// Resolve a line only within the exact visible-parent lineage. Names
    /// that identify more than one live line fail rather than taking the
    /// first map entry. The native adapter rechecks the returned owner/view
    /// against the live native window instances immediately before focus.
    pub fn resolve_clean_branch_switch(
        &self,
        source_window: WindowId,
        reference: &str,
    ) -> Result<CleanBranchSwitchTarget, VersioningFailure> {
        let source = self
            .windows()
            .get(&source_window)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        let native = self
            .versioning
            .owner(source.profile)
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        let root = self
            .runtime_view()
            .versioning_root_branch(self.application_owner, source.profile)
            .filter(|record| {
                record.lifecycle() == BranchLifecycle::Live && record.native() == Some(native)
            })
            .ok_or_else(|| branch_failure(VersioningReason::StaleOwner))?;
        let requested_id = reference.parse::<BranchId>().ok();
        let requested_name = if requested_id.is_none() {
            Some(
                BranchName::new(reference)
                    .map_err(|_| branch_failure(VersioningReason::UnsupportedCapability))?
                    .as_str()
                    .to_lowercase(),
            )
        } else {
            None
        };
        let mut found = None;
        let mut consider = |candidate: CleanBranchSwitchTarget, id: BranchId, name: &str| {
            if requested_id.map_or_else(
                || requested_name.as_deref() == Some(name.to_lowercase().as_str()),
                |requested| requested == id,
            ) {
                if found.is_some() {
                    return Err(branch_failure(VersioningReason::AmbiguousReference));
                }
                found = Some(candidate);
            }
            Ok(())
        };
        consider(
            CleanBranchSwitchTarget::Parent {
                branch: root.id(),
                window: source_window,
            },
            root.id(),
            root.name(),
        )?;
        for (&id, line) in &self.published_clean_branches {
            if line.source_window != source_window
                || line.source_owner != native
                || self.tabs().get(&line.source_tab).is_none_or(|tab| {
                    tab.window != source_window
                        || tab.profile != source.profile
                        || tab.existence != ExistenceState::Live
                })
            {
                continue;
            }
            let Some(record) = self.runtime_view().versioning_branch(id) else {
                continue;
            };
            if record.root() != root.id()
                || record.lifecycle() != BranchLifecycle::Live
                || record.native() != Some(line.child_owner)
                || self.clean_branch_profiles.child(id) != Some((line.child_owner, line.child_tab))
            {
                continue;
            }
            consider(
                CleanBranchSwitchTarget::Child {
                    branch: id,
                    owner: line.child_owner,
                    tab: line.child_tab,
                },
                id,
                record.name(),
            )?;
        }
        found.ok_or_else(|| branch_failure(VersioningReason::StaleOwner))
    }

    /// Prove that a child runtime supplied by the native adapter belongs to
    /// the selected published line. Local TabIds alone are never sufficient.
    pub fn validate_clean_branch_switch_child(
        &self,
        branch: BranchId,
        child: &BrowserApplication,
        child_tab: TabId,
    ) -> bool {
        let Some(line) = self.published_clean_branches.get(&branch) else {
            return false;
        };
        let Some(record) = self.runtime_view().versioning_branch(branch) else {
            return false;
        };
        line.child_tab == child_tab
            && record.native() == Some(line.child_owner)
            && record.lifecycle() == BranchLifecycle::Live
            && self.clean_branch_profiles.child(branch) == Some((line.child_owner, child_tab))
            && child.versioning().owner(line.child_owner.runtime_profile())
                == Some(line.child_owner)
            && child.tabs().get(&child_tab).is_some_and(|tab| {
                tab.profile == line.child_owner.runtime_profile()
                    && tab.existence == ExistenceState::Live
                    && child.windows().get(&tab.window).is_some_and(|window| {
                        window.profile == tab.profile && window.tabs.contains(&child_tab)
                    })
            })
    }

    /// Find the source window for a live child only after its application and
    /// initial view have been checked, preventing colliding local TabIds from
    /// being mistaken for a parent or a sibling.
    pub fn clean_branch_source_for_child(
        &self,
        child: &BrowserApplication,
        child_tab: TabId,
    ) -> Option<WindowId> {
        self.published_clean_branches
            .iter()
            .find_map(|(&branch, line)| {
                self.validate_clean_branch_switch_child(branch, child, child_tab)
                    .then_some(line.source_window)
            })
    }

    /// Reserve only against the currently bound normal-profile owner. This
    /// token is not permission to create a window or navigate a child page.
    pub fn prepare_clean_branch_profile(
        &mut self,
        branch: BranchId,
        source_profile: ProfileId,
    ) -> Result<String, CleanBranchLeaseError> {
        if self
            .profiles()
            .get(&source_profile)
            .is_none_or(|profile| profile.privacy != PrivacyKind::Normal)
        {
            return Err(CleanBranchLeaseError::StaleOwner);
        }
        let parent = self
            .versioning
            .owner(source_profile)
            .ok_or(CleanBranchLeaseError::StaleOwner)?;
        self.clean_branch_profiles.prepare(branch, parent)
    }

    /// Accept an actual second BrowserApplication, its live ephemeral runtime
    /// profile and first tab. The parent and child generations must still be
    /// exact when the native UI later publishes its corresponding Qt lease.
    pub fn bind_clean_branch_profile(
        &mut self,
        branch: BranchId,
        child: &BrowserApplication,
        child_profile: ProfileId,
        initial_view: TabId,
    ) -> Result<(), CleanBranchLeaseError> {
        let native = self.checked_clean_branch_child(branch, child, child_profile, initial_view)?;
        self.clean_branch_profiles
            .bind_child(branch, native, initial_view)
    }

    fn checked_clean_branch_child(
        &self,
        branch: BranchId,
        child: &BrowserApplication,
        child_profile: ProfileId,
        initial_view: TabId,
    ) -> Result<NativeProfileOwner, CleanBranchLeaseError> {
        let parent = self
            .clean_branch_profiles
            .parent(branch)
            .ok_or(CleanBranchLeaseError::UnknownBranch)?;
        if self.versioning.owner(parent.runtime_profile()) != Some(parent) {
            return Err(CleanBranchLeaseError::StaleOwner);
        }
        if child.application_owner == self.application_owner {
            return Err(CleanBranchLeaseError::ParentProfileReuse);
        }
        if child
            .profiles()
            .get(&child_profile)
            .is_none_or(|profile| profile.privacy != PrivacyKind::Ephemeral)
            || child.tabs().get(&initial_view).is_none_or(|tab| {
                tab.profile != child_profile
                    || tab.existence != ExistenceState::Live
                    || child.windows().get(&tab.window).is_none_or(|window| {
                        window.profile != child_profile || !window.tabs.contains(&initial_view)
                    })
            })
        {
            return Err(CleanBranchLeaseError::StaleOwner);
        }
        let native = child
            .versioning
            .owner(child_profile)
            .ok_or(CleanBranchLeaseError::StaleOwner)?;
        self.clean_branch_profiles.can_bind_child(branch, native)?;
        Ok(native)
    }

    #[must_use]
    pub fn clean_branch_profiles(&self) -> &CleanBranchProfileLeases {
        &self.clean_branch_profiles
    }

    pub fn close_clean_branch_initial_view(
        &mut self,
        branch: BranchId,
        child: NativeProfileOwner,
        view: TabId,
    ) -> Result<(), CleanBranchLeaseError> {
        self.clean_branch_profiles
            .close_initial_view(branch, child, view)
    }

    pub fn release_clean_branch_profile(
        &mut self,
        branch: BranchId,
        child: NativeProfileOwner,
    ) -> Result<(), CleanBranchLeaseError> {
        self.clean_branch_profiles.release_child(branch, child)
    }

    pub fn abort_clean_branch_profile(
        &mut self,
        branch: BranchId,
    ) -> Result<(), CleanBranchLeaseError> {
        self.clean_branch_profiles.abort_preparation(branch)
    }
}

fn clean_branch_navigation(page: &ManifestPage) -> Result<RestoreNavigation, VersioningFailure> {
    match &page.safe_url_or_placeholder {
        ManifestSafeUrl::Replayable(url) => {
            if SafePageUrl::from_observed(Some(url), true).replay_url() != Some(url.as_str()) {
                return Err(branch_failure(VersioningReason::UnsafeRestoreDescriptor));
            }
            if url.starts_with("http://") || url.starts_with("https://") {
                Ok(RestoreNavigation::SafeGet(url.clone()))
            } else {
                Ok(RestoreNavigation::Placeholder(
                    ManifestUnavailable::Unsupported,
                ))
            }
        }
        ManifestSafeUrl::Placeholder(reason) => Ok(RestoreNavigation::Placeholder(*reason)),
    }
}

fn branch_failure(reason: VersioningReason) -> VersioningFailure {
    VersioningFailure::new(reason, SensitiveCategory::Profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs, thread,
        time::{Duration, Instant},
    };

    use ferric_browser_config::Config;
    use ferric_browser_core::{ApplicationOwnerId, ProfileId, ProfileInstanceId, ValidatedUrl};
    use ferric_browser_storage::{RootSpec, StorageRoots};
    use uuid::Uuid;

    use crate::ProfileActivation;

    fn native(profile: &str) -> NativeProfileOwner {
        NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            ProfileId::from_display(profile).expect("runtime profile"),
            ProfileInstanceId::fresh(),
        )
    }

    fn tab(number: u64) -> TabId {
        TabId::from_display(&format!("tabid-{number}")).expect("runtime tab")
    }

    fn durable_parent() -> (BrowserApplication, StorageRoots, WindowId, TabId) {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        let uuid = Uuid::new_v4();
        let directory = roots.data.join("profiles").join(uuid.to_string());
        fs::create_dir_all(&directory).expect("profile directory");
        let (mut application, window, tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "parent")
                .expect("parent runtime");
        application
            .activate_profile(ProfileActivation {
                name: "parent".into(),
                id: Some(uuid),
                privacy: PrivacyKind::Normal,
                roots: Some(roots.clone()),
                lock: None,
                contexts: None,
                storage_path: Some(directory.join("browser.sqlite")),
            })
            .expect("durable profile");
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("native profile");
        (application, roots, window, tab)
    }

    fn linked_clean_branch() -> (
        BrowserApplication,
        StorageRoots,
        WindowId,
        BrowserApplication,
        WindowId,
        BranchId,
    ) {
        let (mut parent, roots, source_window, source_tab) = durable_parent();
        let target = parent.capture_target(source_tab).expect("source target");
        let (branch, _, _, _) = parent
            .begin_clean_branch_fork(source_window, target, "linked")
            .expect("queued clean fork");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let _ = parent.poll_storage_effects();
            if !parent.poll_commit_events().is_empty() {
                break;
            }
            assert!(Instant::now() < deadline, "clean fork did not finish");
            thread::yield_now();
        }
        assert!(matches!(
            parent.poll_clean_branch_fork_events().as_slice(),
            [CleanBranchForkEvent::Durable { branch: id, .. }] if *id == branch
        ));
        let (mut child, child_window, child_tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Ephemeral, "child")
                .expect("transient child");
        let child_profile = child.windows()[&child_window].profile;
        child
            .bind_native_profile(child_profile, ProfileInstanceId::fresh())
            .expect("native child owner");
        parent
            .publish_clean_branch_lineage(branch, &mut child, child_profile, child_tab)
            .expect("linked child");
        (parent, roots, source_window, child, child_window, branch)
    }

    #[test]
    fn structural_export_requires_linked_owner_and_one_use_confirmation() {
        let (mut parent, _roots, source_window, child, child_window, branch) =
            linked_clean_branch();
        let preview = parent
            .preview_clean_branch_structural_export(
                branch,
                &child,
                source_window,
                child_window,
                "keep structure".to_owned(),
            )
            .expect("linked export preview");
        assert_eq!(preview.page_count, 1);
        assert!(parent.pending_commit.is_none(), "preview must not write");
        assert!(parent.cancel_clean_branch_structural_export(preview.operation));
        assert!(!parent.cancel_clean_branch_structural_export(preview.operation));
        assert!(
            parent
                .confirm_clean_branch_structural_export(
                    preview.operation,
                    &child,
                    source_window,
                    child_window,
                )
                .is_err(),
            "cancelled preview cannot publish"
        );
        let preview = parent
            .preview_clean_branch_structural_export(
                branch,
                &child,
                source_window,
                child_window,
                "keep structure".to_owned(),
            )
            .expect("second preview");
        let wrong_window = WindowId::from_display("windowid-999999").expect("test window");
        assert!(
            parent
                .confirm_clean_branch_structural_export(
                    preview.operation,
                    &child,
                    wrong_window,
                    child_window,
                )
                .is_err(),
            "wrong destination consumes the preview without writing"
        );
        assert!(parent.pending_commit.is_none());
        let (mut private, private_window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Private, "private")
                .expect("ordinary private window");
        let private_profile = private.windows()[&private_window].profile;
        private
            .bind_native_profile(private_profile, ProfileInstanceId::fresh())
            .expect("private owner");
        assert!(
            parent
                .preview_clean_branch_structural_export(
                    branch,
                    &private,
                    source_window,
                    private_window,
                    "private escape".to_owned(),
                )
                .is_err(),
            "unlinked private browsing is not a branch export"
        );
    }

    #[test]
    fn structural_export_refuses_closed_branch_owner() {
        let (mut parent, _roots, source_window, child, child_window, branch) =
            linked_clean_branch();
        let preview = parent
            .preview_clean_branch_structural_export(
                branch,
                &child,
                source_window,
                child_window,
                "keep structure".to_owned(),
            )
            .expect("linked export preview");
        let child_owner = parent
            .published_clean_branches
            .get(&branch)
            .expect("published owner")
            .child_owner;
        parent
            .close_published_clean_branch(branch, child_owner)
            .expect("close line");
        assert!(
            parent
                .confirm_clean_branch_structural_export(
                    preview.operation,
                    &child,
                    source_window,
                    child_window,
                )
                .is_err(),
            "closed branch cannot export"
        );
        assert!(parent.pending_commit.is_none());
    }

    #[test]
    fn confirmed_structural_export_survives_reopen_without_a_live_child() {
        let (mut parent, roots, source_window, mut child, child_window, branch) =
            linked_clean_branch();
        child
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: child_window,
            }))
            .expect("second transient page");
        let profile_uuid = parent
            .profile_snapshot()
            .and_then(|profile| profile.id)
            .expect("durable normal profile");
        let preview = parent
            .preview_clean_branch_structural_export(
                branch,
                &child,
                source_window,
                child_window,
                "keep child layout".to_owned(),
            )
            .expect("structural-only preview");
        assert_eq!(preview.page_count, 2);
        let (operation, commit, effects) = parent
            .confirm_clean_branch_structural_export(
                preview.operation,
                &child,
                source_window,
                child_window,
            )
            .expect("explicitly confirmed export");
        assert!(effects.is_empty());
        assert_eq!(operation, preview.operation);
        assert!(parent.pending_branch_structural_export.is_none());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let _ = parent.poll_storage_effects();
            if let Some(event) = parent.poll_commit_events().into_iter().next() {
                assert_eq!(
                    event,
                    super::super::CommitEvent::Durable { operation, commit }
                );
                break;
            }
            assert!(Instant::now() < deadline, "export commit did not finish");
            thread::yield_now();
        }
        drop(child);
        drop(parent);

        let database = roots
            .data
            .join("profiles")
            .join(profile_uuid.to_string())
            .join("browser.sqlite");
        let connection = rusqlite::Connection::open(&database).expect("reopen normal store");
        let manifest_bytes: Vec<u8> = connection
            .query_row(
                "SELECT manifest_bytes FROM vb_commits WHERE id=?1",
                [commit.to_string()],
                |row| row.get(0),
            )
            .expect("exported commit survives owner teardown");
        let manifest = VersionedCaptureManifest::decode(&manifest_bytes)
            .expect("valid exported structural manifest");
        assert_eq!(manifest.profile_uuid, profile_uuid);
        assert_eq!(manifest.scope, ManifestScope::Window);
        assert_eq!(manifest.windows.len(), 1);
        assert_eq!(manifest.windows[0].pages.len(), 2);
        assert!(
            manifest.windows[0]
                .pages
                .iter()
                .all(|page| { page.archive_object.is_none() && page.sidecar_object.is_none() })
        );
        drop(connection);

        let (mut reopened, new_window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "parent")
                .expect("fresh runtime");
        reopened
            .activate_profile(ProfileActivation {
                name: "parent".into(),
                id: Some(profile_uuid),
                privacy: PrivacyKind::Normal,
                roots: Some(roots),
                lock: None,
                contexts: None,
                storage_path: Some(database),
            })
            .expect("reopen durable profile");
        let reopened_profile = reopened.windows()[&new_window].profile;
        reopened
            .bind_native_profile(reopened_profile, ProfileInstanceId::fresh())
            .expect("fresh native owner");
        reopened
            .request_commit_info(new_window, commit)
            .expect("read exported commit through new owner");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let _ = reopened.poll_storage_effects();
            if let Some(event) = reopened.poll_commit_read_events().into_iter().next() {
                assert!(matches!(
                    event,
                    super::super::CommitReadEvent::Info {
                        commit: Some(_),
                        ..
                    }
                ));
                break;
            }
            assert!(Instant::now() < deadline, "reopened commit read timed out");
            thread::yield_now();
        }
        assert!(
            reopened
                .runtime_view()
                .versioning_root_branch(reopened.application_owner(), reopened_profile)
                .is_none(),
            "a durable representation must not resurrect an authenticated child"
        );
    }

    #[test]
    fn window_branch_plan_is_bounded_ordered_and_owner_exact() {
        let (mut parent, _roots, window, first) = durable_parent();
        for _ in 1..ferric_browser_core::versioning::limits::MAX_PAGES_PER_WINDOW_CAPTURE {
            parent
                .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
                .expect("same-profile source tab");
        }
        let source_tabs = parent.windows()[&window].tabs.clone();
        assert_eq!(source_tabs.len(), 128);
        assert_eq!(source_tabs[0], first);
        let prepared = parent
            .prepare_structural_commit(
                ScopeSelector::Window { window },
                "Window branch plan".to_owned(),
            )
            .expect("bounded structural snapshot");
        let plan = parent
            .plan_clean_branch_window(window, &prepared.manifest)
            .expect("exact window plan");
        assert_eq!(plan.pages.len(), 128);
        assert_eq!(plan.selected_index, 127);
        assert_eq!(
            plan.pages
                .iter()
                .map(|page| page.source_tab)
                .collect::<Vec<_>>(),
            source_tabs
        );
        assert!(
            plan.pages
                .iter()
                .all(|page| matches!(page.navigation, RestoreNavigation::Placeholder(_)))
        );
        let mut wrong_order = prepared.manifest.clone();
        wrong_order.windows[0].pages.reverse();
        assert_eq!(
            parent
                .plan_clean_branch_window(window, &wrong_order)
                .map_err(VersioningFailure::reason),
            Err(VersioningReason::StaleOwner)
        );
        let mut wrong_profile = prepared.manifest.clone();
        wrong_profile.profile_uuid = Uuid::new_v4();
        assert_eq!(
            parent
                .plan_clean_branch_window(window, &wrong_profile)
                .map_err(VersioningFailure::reason),
            Err(VersioningReason::StaleOwner)
        );
        let mut unproved_get = prepared.manifest.clone();
        unproved_get.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/".to_owned());
        assert_eq!(
            parent
                .plan_clean_branch_window(window, &unproved_get)
                .map_err(VersioningFailure::reason),
            Err(VersioningReason::UnsafeRestoreDescriptor)
        );
        parent
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CloseTab { tab: first }))
            .expect("source changed");
        assert_eq!(
            parent
                .plan_clean_branch_window(window, &prepared.manifest)
                .map_err(VersioningFailure::reason),
            Err(VersioningReason::StaleOwner)
        );
    }

    #[test]
    fn window_fork_becomes_ready_only_after_one_durable_group_capture() {
        let (mut parent, _roots, window, first) = durable_parent();
        for _ in 0..2 {
            parent
                .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
                .expect("same-profile tab");
        }
        let source_tabs = parent.windows()[&window].tabs.clone();
        let (branch, _operation, commit, effects) = parent
            .begin_clean_branch_window_fork(window, "group")
            .expect("queued group capture");
        assert!(!effects.is_empty());
        assert!(parent.poll_clean_branch_fork_events().is_empty());
        assert_eq!(source_tabs[0], first);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let _ = parent.poll_storage_effects();
            if !parent.poll_commit_events().is_empty() {
                break;
            }
            assert!(Instant::now() < deadline, "group capture did not finish");
            thread::yield_now();
        }
        let events = parent.poll_clean_branch_fork_events();
        let [
            CleanBranchForkEvent::WindowDurable {
                branch: ready_branch,
                commit: ready_commit,
                plan,
            },
        ] = events.as_slice()
        else {
            panic!("expected durable group event: {events:?}");
        };
        assert_eq!((*ready_branch, *ready_commit), (branch, commit));
        assert_eq!(plan.pages.len(), 3);
        assert_eq!(plan.selected_index, 2);
        assert_eq!(
            plan.pages
                .iter()
                .map(|page| page.source_tab)
                .collect::<Vec<_>>(),
            source_tabs
        );
        assert!(parent.ready_clean_branch_forks.contains_key(&branch));
        let (mut child, child_window, child_tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Ephemeral, "child")
                .expect("transient child");
        let child_profile = child.windows()[&child_window].profile;
        child
            .bind_native_profile(child_profile, ProfileInstanceId::fresh())
            .expect("child owner");
        for _ in 0..2 {
            child
                .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                    window: child_window,
                }))
                .expect("group child tab");
        }
        let child_tabs = child.windows()[&child_window].tabs.clone();
        assert!(
            parent
                .publish_clean_branch_lineage(branch, &mut child, child_profile, child_tab)
                .is_err(),
            "single-view publication must not consume a group fork"
        );
        let mut wrong_order = child_tabs.clone();
        wrong_order.reverse();
        assert!(
            parent
                .publish_clean_branch_window_lineage(
                    branch,
                    &mut child,
                    child_profile,
                    &wrong_order,
                )
                .is_err(),
            "wrong native child order must not publish"
        );
        let (parent_effects, child_effects) = parent
            .publish_clean_branch_window_lineage(branch, &mut child, child_profile, &child_tabs)
            .expect("one ordered group lineage");
        assert!(!parent_effects.is_empty());
        assert!(!child_effects.is_empty());
        let record = parent
            .runtime_view()
            .versioning_branch(branch)
            .expect("published group line");
        assert_eq!(record.pages().len(), source_tabs.len());
        assert_eq!(record.fork(), Some(commit));
        for (source_tab, child_tab) in source_tabs.iter().zip(&child_tabs) {
            assert_eq!(
                child.runtime_view().lineage_parent(*child_tab),
                parent.runtime_view().logical_page(*source_tab)
            );
        }
        assert!(!parent.ready_clean_branch_forks.contains_key(&branch));
    }

    #[test]
    fn window_fork_refuses_source_mutation_before_durability_callback() {
        let (mut parent, _roots, window, _) = durable_parent();
        parent
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("second source tab");
        let (branch, _, _, _) = parent
            .begin_clean_branch_window_fork(window, "group")
            .expect("queued group capture");
        parent
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("source mutated before completion");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let _ = parent.poll_storage_effects();
            if !parent.poll_commit_events().is_empty() {
                break;
            }
            assert!(Instant::now() < deadline, "group capture did not finish");
            thread::yield_now();
        }
        assert_eq!(
            parent.poll_clean_branch_fork_events(),
            vec![CleanBranchForkEvent::Failed { branch }]
        );
        assert!(!parent.clean_branch_profiles().contains(branch));
        assert!(!parent.ready_clean_branch_forks.contains_key(&branch));
    }

    #[test]
    fn two_clean_branches_have_distinct_child_owner_leases() {
        let mut leases = CleanBranchProfileLeases::new();
        let parent = native("profileid-1");
        let first = BranchId::fresh();
        let second = BranchId::fresh();
        let first_child = native("profileid-2");
        let second_child = native("profileid-3");
        assert_eq!(leases.prepare(first, parent), Ok(first.to_string()));
        assert_eq!(leases.prepare(second, parent), Ok(second.to_string()));
        assert_eq!(
            leases.bind_child(first, parent, tab(1)),
            Err(CleanBranchLeaseError::ParentProfileReuse)
        );
        leases
            .bind_child(first, first_child, tab(1))
            .expect("first child");
        assert_eq!(
            leases.bind_child(second, first_child, tab(1)),
            Err(CleanBranchLeaseError::ChildProfileReuse)
        );
        leases
            .bind_child(second, second_child, tab(1))
            .expect("second child");
        assert_eq!(
            leases.release_child(first, first_child),
            Err(CleanBranchLeaseError::ViewStillOpen)
        );
        assert_eq!(
            leases.close_initial_view(first, second_child, tab(1)),
            Err(CleanBranchLeaseError::StaleOwner)
        );
        leases
            .close_initial_view(first, first_child, tab(1))
            .expect("first view closed");
        leases
            .release_child(first, first_child)
            .expect("first profile released");
        assert!(!leases.contains(first));
        assert!(leases.contains(second));
    }

    #[test]
    fn prepared_allocation_aborts_without_touching_a_parent() {
        let mut leases = CleanBranchProfileLeases::new();
        let branch = BranchId::fresh();
        let parent = native("profileid-1");
        leases.prepare(branch, parent).expect("reservation");
        assert_eq!(
            leases.prepare(branch, parent),
            Err(CleanBranchLeaseError::DuplicateBranch)
        );
        leases.abort_preparation(branch).expect("rollback");
        assert!(!leases.contains(branch));
        assert_eq!(
            leases.abort_preparation(branch),
            Err(CleanBranchLeaseError::UnknownBranch)
        );
    }

    #[test]
    fn branch_lease_capacity_matches_native_window_budget() {
        let mut leases = CleanBranchProfileLeases::new();
        let parent = native("profileid-1");
        for _ in 0..MAX_CLEAN_BRANCH_OWNERS {
            leases
                .prepare(BranchId::fresh(), parent)
                .expect("within capacity");
        }
        assert_eq!(
            leases.prepare(BranchId::fresh(), parent),
            Err(CleanBranchLeaseError::AtCapacity)
        );
    }

    #[test]
    fn application_lease_binds_only_a_live_independent_ephemeral_child() {
        let (mut parent, parent_window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "parent")
                .expect("parent runtime");
        let parent_profile = parent.windows()[&parent_window].profile;
        assert!(
            parent
                .bind_native_profile(parent_profile, ProfileInstanceId::fresh())
                .expect("parent native owner")
                .runtime_effects
                .is_empty()
        );
        let (mut child, child_window, child_tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Ephemeral, "child")
                .expect("child runtime");
        let child_profile = child.windows()[&child_window].profile;
        let branch = BranchId::fresh();
        assert_eq!(
            parent.prepare_clean_branch_profile(branch, parent_profile),
            Ok(branch.to_string())
        );
        assert_eq!(
            parent.bind_clean_branch_profile(branch, &child, child_profile, child_tab),
            Err(CleanBranchLeaseError::StaleOwner)
        );
        child
            .bind_native_profile(child_profile, ProfileInstanceId::fresh())
            .expect("child native owner");
        assert_eq!(
            parent.bind_clean_branch_profile(branch, &child, child_profile, tab(999)),
            Err(CleanBranchLeaseError::StaleOwner)
        );
        parent
            .bind_clean_branch_profile(branch, &child, child_profile, child_tab)
            .expect("exact independent child");
        assert!(parent.clean_branch_profiles().contains(branch));
        let child_owner = child
            .versioning()
            .owner(child_profile)
            .expect("child owner");
        assert_eq!(
            parent.release_clean_branch_profile(branch, child_owner),
            Err(CleanBranchLeaseError::ViewStillOpen)
        );
        parent
            .close_clean_branch_initial_view(branch, child_owner, child_tab)
            .expect("first view closed");
        parent
            .release_clean_branch_profile(branch, child_owner)
            .expect("child released");
        assert!(!parent.clean_branch_profiles().contains(branch));
    }

    #[test]
    fn application_lease_refuses_rebound_parent_and_non_ephemeral_child() {
        let (mut parent, parent_window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "parent")
                .expect("parent runtime");
        let parent_profile = parent.windows()[&parent_window].profile;
        parent
            .bind_native_profile(parent_profile, ProfileInstanceId::fresh())
            .expect("parent native owner");
        let branch = BranchId::fresh();
        parent
            .prepare_clean_branch_profile(branch, parent_profile)
            .expect("reservation");
        let (mut normal_child, normal_window, normal_tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "normal child")
                .expect("normal child runtime");
        let normal_profile = normal_child.windows()[&normal_window].profile;
        normal_child
            .bind_native_profile(normal_profile, ProfileInstanceId::fresh())
            .expect("normal child owner");
        assert_eq!(
            parent.bind_clean_branch_profile(branch, &normal_child, normal_profile, normal_tab),
            Err(CleanBranchLeaseError::StaleOwner)
        );
        parent
            .bind_native_profile(parent_profile, ProfileInstanceId::fresh())
            .expect("rebound parent");
        assert_eq!(
            parent.bind_clean_branch_profile(branch, &normal_child, normal_profile, normal_tab),
            Err(CleanBranchLeaseError::StaleOwner)
        );
    }

    #[test]
    fn clean_fork_is_ready_only_after_durable_structural_publication() {
        let (mut parent, _roots, window, source_tab) = durable_parent();
        let target = parent.capture_target(source_tab).expect("source target");
        let (branch, operation, commit, effects) = parent
            .begin_clean_branch_fork(window, target, "trial")
            .expect("queued fork");
        assert!(!effects.is_empty());
        assert!(parent.poll_clean_branch_fork_events().is_empty());
        assert!(parent.clean_branch_profiles().contains(branch));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let _ = parent.poll_storage_effects();
            let commit_events = parent.poll_commit_events();
            if !commit_events.is_empty() {
                assert_eq!(
                    commit_events,
                    vec![super::super::CommitEvent::Durable { operation, commit }]
                );
                break;
            }
            assert!(Instant::now() < deadline, "fork commit did not finish");
            thread::yield_now();
        }
        let events = parent.poll_clean_branch_fork_events();
        assert!(matches!(
            events.as_slice(),
            [CleanBranchForkEvent::Durable {
                branch: ready_branch,
                commit: ready_commit,
                navigation: RestoreNavigation::Placeholder(_),
            }] if *ready_branch == branch && *ready_commit == commit
        ));
        assert!(parent.ready_clean_branch_forks.contains_key(&branch));
        let source_logical_page = parent
            .runtime_view()
            .logical_page(source_tab)
            .expect("source lineage");
        let (mut child, child_window, child_tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Ephemeral, "child")
                .expect("transient child");
        assert_eq!(source_tab, child_tab, "local tab IDs intentionally collide");
        assert_ne!(parent.application_owner(), child.application_owner());
        let child_profile = child.windows()[&child_window].profile;
        child
            .bind_native_profile(child_profile, ProfileInstanceId::fresh())
            .expect("child owner");
        let (parent_effects, child_effects) = parent
            .publish_clean_branch_lineage(branch, &mut child, child_profile, child_tab)
            .expect("published lineage");
        assert!(!parent_effects.is_empty());
        assert!(!child_effects.is_empty());
        let child_record = parent
            .runtime_view()
            .versioning_branch(branch)
            .expect("child branch record");
        assert_eq!(child_record.lifecycle(), BranchLifecycle::Live);
        assert_eq!(
            child_record.native(),
            child.versioning().owner(child_profile)
        );
        assert_eq!(child_record.fork(), Some(commit));
        assert_eq!(
            child.runtime_view().lineage_parent(child_tab),
            Some(source_logical_page)
        );
        assert!(!parent.ready_clean_branch_forks.contains_key(&branch));
        let child_owner = child
            .versioning()
            .owner(child_profile)
            .expect("child owner");
        let root = parent
            .runtime_view()
            .versioning_root_branch(
                parent.application_owner(),
                parent.windows()[&window].profile,
            )
            .expect("root line")
            .id();
        assert_eq!(
            parent.resolve_clean_branch_switch(window, "MAIN"),
            Ok(CleanBranchSwitchTarget::Parent {
                branch: root,
                window
            })
        );
        assert_eq!(
            parent.resolve_clean_branch_switch(window, " trial "),
            Ok(CleanBranchSwitchTarget::Child {
                branch,
                owner: child_owner,
                tab: child_tab,
            })
        );
        assert_eq!(
            parent.resolve_clean_branch_switch(window, &branch.to_string()),
            Ok(CleanBranchSwitchTarget::Child {
                branch,
                owner: child_owner,
                tab: child_tab,
            })
        );
        assert_eq!(
            parent.clean_branch_source_for_child(&child, child_tab),
            Some(window)
        );
        let (mut impostor, impostor_window, impostor_tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Ephemeral, "impostor")
                .expect("independent transient runtime");
        let impostor_profile = impostor.windows()[&impostor_window].profile;
        impostor
            .bind_native_profile(impostor_profile, ProfileInstanceId::fresh())
            .expect("impostor owner");
        assert_eq!(impostor_tab, child_tab);
        assert!(!parent.validate_clean_branch_switch_child(branch, &impostor, impostor_tab));
        assert_eq!(
            parent.clean_branch_source_for_child(&impostor, impostor_tab),
            None
        );
        assert!(parent.validate_clean_branch_switch_child(branch, &child, child_tab));
        let parent_revision = parent.revision();
        assert!(
            parent
                .close_published_clean_branch(branch, native("profileid-999"))
                .is_err()
        );
        assert_eq!(parent.revision(), parent_revision);
        assert!(parent.clean_branch_profiles().contains(branch));
        parent
            .close_published_clean_branch(branch, child_owner)
            .expect("child close and release");
        assert!(parent.runtime_view().versioning_branch(branch).is_none());
        assert!(!parent.clean_branch_profiles().contains(branch));
        assert_eq!(
            parent
                .resolve_clean_branch_switch(window, &branch.to_string())
                .map_err(VersioningFailure::reason),
            Err(VersioningReason::StaleOwner)
        );
        assert!(!parent.validate_clean_branch_switch_child(branch, &child, child_tab));
        assert!(
            parent
                .close_published_clean_branch(branch, child_owner)
                .is_err()
        );
        assert_eq!(parent.capture_target(source_tab), Some(target));
        assert!(
            parent
                .runtime_view()
                .versioning_root_branch(
                    parent.application_owner(),
                    parent.windows()[&window].profile
                )
                .is_some()
        );
    }

    #[test]
    fn parent_generation_change_rejects_durable_fork_publication() {
        let (mut parent, _roots, window, source_tab) = durable_parent();
        let target = parent.capture_target(source_tab).expect("source target");
        let (branch, operation, commit, _) = parent
            .begin_clean_branch_fork(window, target, "trial")
            .expect("queued fork");
        let profile = parent.windows()[&window].profile;
        parent
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("rebound parent");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let _ = parent.poll_storage_effects();
            if parent.poll_commit_events().iter().any(|event| {
                matches!(
                    event,
                    super::super::CommitEvent::Durable {
                        operation: actual_operation,
                        commit: actual_commit,
                    } if *actual_operation == operation && *actual_commit == commit
                )
            }) {
                break;
            }
            assert!(Instant::now() < deadline, "fork commit did not finish");
            thread::yield_now();
        }
        assert_eq!(
            parent.poll_clean_branch_fork_events(),
            vec![CleanBranchForkEvent::Failed { branch }]
        );
        assert!(!parent.clean_branch_profiles().contains(branch));
        assert!(!parent.ready_clean_branch_forks.contains_key(&branch));
    }

    #[test]
    fn invalid_child_name_preflight_does_not_mutate_either_runtime() {
        let (mut parent, _roots, window, source_tab) = durable_parent();
        let target = parent.capture_target(source_tab).expect("source target");
        let (branch, _, _, _) = parent
            .begin_clean_branch_fork(window, target, "main")
            .expect("queued fork");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let _ = parent.poll_storage_effects();
            if !parent.poll_clean_branch_fork_events().is_empty() {
                break;
            }
            assert!(Instant::now() < deadline, "fork commit did not finish");
            thread::yield_now();
        }
        let (mut child, child_window, child_tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Ephemeral, "child")
                .expect("transient child");
        let child_profile = child.windows()[&child_window].profile;
        child
            .bind_native_profile(child_profile, ProfileInstanceId::fresh())
            .expect("child owner");
        let parent_revision = parent.revision();
        let child_revision = child.revision();
        assert!(
            parent
                .publish_clean_branch_lineage(branch, &mut child, child_profile, child_tab)
                .is_err()
        );
        assert_eq!(parent.revision(), parent_revision);
        assert_eq!(child.revision(), child_revision);
        assert!(
            parent
                .runtime_view()
                .versioning_root_branch(
                    parent.application_owner(),
                    parent.windows()[&window].profile
                )
                .is_none()
        );
        assert!(child.runtime_view().logical_page(child_tab).is_none());
        assert!(parent.ready_clean_branch_forks.contains_key(&branch));
        assert!(parent.abort_ready_clean_branch_fork(branch));
        assert!(!parent.clean_branch_profiles().contains(branch));
        assert!(!parent.abort_ready_clean_branch_fork(branch));
    }

    #[test]
    fn post_and_unsafe_url_fork_only_placeholders() {
        let (mut parent, _roots, window, source_tab) = durable_parent();
        let url = "http://127.0.0.1/versioning/form/result";
        let target = parent.capture_target(source_tab).expect("source target");
        parent
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target,
                url: ValidatedUrl::parse(url).expect("safe URL syntax"),
            }))
            .expect("start source navigation");
        let loading = parent.capture_target(source_tab).expect("loading target");
        parent
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: loading,
                url: ValidatedUrl::parse(url).expect("safe URL syntax"),
                title: "synthetic POST result".into(),
            }))
            .expect("commit source navigation");
        assert!(parent.note_typed_safe_get_navigation(source_tab, url));
        parent.invalidate_safe_get_navigation(source_tab);
        let source = parent
            .capture_target(source_tab)
            .expect("POST source target");
        let (branch, _, _, _) = parent
            .begin_clean_branch_fork(window, source, "post-child")
            .expect("structural fork queued");
        let deadline = Instant::now() + Duration::from_secs(5);
        let event = loop {
            let _ = parent.poll_storage_effects();
            if let Some(event) = parent.poll_clean_branch_fork_events().into_iter().next() {
                break event;
            }
            assert!(Instant::now() < deadline, "POST fork did not finish");
            thread::yield_now();
        };
        assert!(matches!(
            event,
            CleanBranchForkEvent::Durable {
                branch: ready,
                navigation: RestoreNavigation::Placeholder(
                    ManifestUnavailable::UnsafeNavigation
                ),
                ..
            } if ready == branch
        ));
        assert_eq!(parent.capture_target(source_tab), Some(source));
        assert_eq!(parent.tabs()[&source_tab].url.as_deref(), Some(url));
        assert!(parent.abort_ready_clean_branch_fork(branch));

        let unsafe_url = "http://127.0.0.1/versioning/document?token=synthetic";
        parent
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: source,
                url: ValidatedUrl::parse(unsafe_url).expect("unsafe URL syntax"),
            }))
            .expect("start unsafe source navigation");
        let loading = parent
            .capture_target(source_tab)
            .expect("unsafe loading target");
        parent
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: loading,
                url: ValidatedUrl::parse(unsafe_url).expect("unsafe URL syntax"),
                title: "synthetic token URL".into(),
            }))
            .expect("commit unsafe source navigation");
        assert!(!parent.note_typed_safe_get_navigation(source_tab, unsafe_url));
        let unsafe_source = parent
            .capture_target(source_tab)
            .expect("unsafe source target");
        let (unsafe_branch, _, _, _) = parent
            .begin_clean_branch_fork(window, unsafe_source, "unsafe-child")
            .expect("unsafe structural fork queued");
        let deadline = Instant::now() + Duration::from_secs(5);
        let event = loop {
            let _ = parent.poll_storage_effects();
            if let Some(event) = parent.poll_clean_branch_fork_events().into_iter().next() {
                break event;
            }
            assert!(Instant::now() < deadline, "unsafe fork did not finish");
            thread::yield_now();
        };
        assert!(matches!(
            event,
            CleanBranchForkEvent::Durable {
                branch: ready,
                navigation: RestoreNavigation::Placeholder(
                    ManifestUnavailable::UnsafeNavigation
                ),
                ..
            } if ready == unsafe_branch
        ));
        assert_eq!(parent.capture_target(source_tab), Some(unsafe_source));
        assert_eq!(parent.tabs()[&source_tab].url.as_deref(), Some(unsafe_url));
        assert!(parent.abort_ready_clean_branch_fork(unsafe_branch));
    }
}
