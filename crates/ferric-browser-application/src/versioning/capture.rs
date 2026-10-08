//! Immutable, structural-only capture preparation at an admitted reducer revision.
//! Publication is a separate storage-worker protocol; this module never reads a
//! native page or infers that a URL was reached by a safe GET.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use ferric_browser_core::{
    CommitId, Event, ExistenceState, LoadingState, LogicalPageId, LogicalWindowId,
    NativeProfileOwner, OperationId, PageCaptureInput, PrivacyKind, ScopeSelector,
    StructuralSnapshot, TabId, VersioningEvent, VersioningFailure, VersioningReason, WindowId,
    versioning::{SensitiveCategory, limits::MAX_LIST_ITEMS},
};
use ferric_browser_runtime::RuntimeInput;
use ferric_browser_storage::versioning::{
    CaptureInterval, InvalidContextMember, InvalidContextMemberReason, ManifestHeader,
    ManifestScope, VersionedCaptureManifest,
};
use ferric_browser_storage::versioning::{
    CommitDeletePreview, CommitDeleteReceipt, CommitInfo, CommitSummary,
};
use ferric_browser_storage::{ContextMember, ContextRecord};
use ferric_browser_storage::{StorageCommand, StorageCompletion};
use uuid::Uuid;

use super::CommitDiffEvent;
use crate::{BrowserApplication, StorageEffect, StorageTicket};

/// Immutable materialization of one exact runtime revision. The manifest is
/// safe to move to the storage worker after the tab, focus or URL changes.
#[derive(Clone)]
pub struct PreparedStructuralCommit {
    pub operation: OperationId,
    pub commit: CommitId,
    pub manifest: VersionedCaptureManifest,
    pub runtime_effects: Vec<ferric_browser_runtime::RuntimeEffect>,
}

/// Source-owned plan for an aggregate assembled from exact owner-issued
/// candidates. The native adapter must bind each candidate at its own owner
/// before presenting this plan for durable publication.
pub struct PreparedContextCandidateCommit {
    source_window: WindowId,
    profile_uuid: Uuid,
    native: NativeProfileOwner,
    source_context: ContextRecord,
    source_bindings: Vec<PlannedContextIdentityBinding>,
    operation: OperationId,
    commit: CommitId,
    manifest: VersionedCaptureManifest,
}

impl std::fmt::Debug for PreparedContextCandidateCommit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedContextCandidateCommit")
            .field("source_window", &self.source_window)
            .field("commit", &self.commit)
            .field("manifest", &"[redacted]")
            .finish_non_exhaustive()
    }
}

impl PreparedContextCandidateCommit {
    #[must_use]
    pub const fn commit(&self) -> CommitId {
        self.commit
    }

    #[must_use]
    pub fn complete_member_count(&self) -> Option<usize> {
        (self.manifest.invalid_context_members.is_empty())
            .then_some(self.manifest.windows.len())
            .filter(|count| u16::try_from(*count).ok() == self.manifest.context_member_count)
    }
}

impl std::fmt::Debug for PreparedStructuralCommit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedStructuralCommit")
            .field("operation", &"[redacted]")
            .field("commit", &"[redacted]")
            .field("manifest", &"[redacted]")
            .field("runtime_effects", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum CommitEvent {
    Durable {
        operation: OperationId,
        commit: CommitId,
    },
    Failed {
        operation: OperationId,
        commit: CommitId,
    },
}

impl std::fmt::Debug for CommitEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Durable { .. } => formatter
                .debug_struct("CommitEvent::Durable")
                .field("operation", &"[redacted]")
                .field("commit", &"[redacted]")
                .finish(),
            Self::Failed { .. } => formatter
                .debug_struct("CommitEvent::Failed")
                .field("operation", &"[redacted]")
                .field("commit", &"[redacted]")
                .finish(),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum CommitReadEvent {
    Listed {
        request_id: u64,
        offset: usize,
        limit: usize,
        has_next: bool,
        commits: Vec<CommitSummary>,
    },
    Info {
        request_id: u64,
        commit: Option<CommitInfo>,
    },
    Failed {
        request_id: u64,
    },
    Diff(CommitDiffEvent),
    DeletePreview {
        request_id: u64,
        preview: Option<CommitDeletePreview>,
    },
    Deleted {
        request_id: u64,
        receipt: CommitDeleteReceipt,
    },
}

impl std::fmt::Debug for CommitReadEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Listed {
                offset,
                limit,
                has_next,
                commits,
                ..
            } => formatter
                .debug_struct("CommitReadEvent::Listed")
                .field("request_id", &"[redacted]")
                .field("offset", offset)
                .field("limit", limit)
                .field("has_next", has_next)
                .field("commit_count", &commits.len())
                .finish(),
            Self::Info { commit, .. } => formatter
                .debug_struct("CommitReadEvent::Info")
                .field("request_id", &"[redacted]")
                .field("available", &commit.is_some())
                .finish(),
            Self::Failed { .. } => formatter
                .debug_struct("CommitReadEvent::Failed")
                .field("request_id", &"[redacted]")
                .finish(),
            Self::Diff(event) => formatter
                .debug_tuple("CommitReadEvent::Diff")
                .field(event)
                .finish(),
            Self::DeletePreview { preview, .. } => formatter
                .debug_struct("CommitReadEvent::DeletePreview")
                .field("request_id", &"[redacted]")
                .field("available", &preview.is_some())
                .finish(),
            Self::Deleted { receipt, .. } => formatter
                .debug_struct("CommitReadEvent::Deleted")
                .field("request_id", &"[redacted]")
                .field("receipt", receipt)
                .finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitReadKind {
    // INVARIANT: every outstanding read is bound to the exact native owner
    // generation as well as the profile UUID; a same-profile rebind must not
    // make an old window's completion current again.
    List {
        offset: usize,
        limit: usize,
        window: WindowId,
        profile_uuid: Uuid,
        native: NativeProfileOwner,
    },
    Info {
        window: WindowId,
        profile_uuid: Uuid,
        native: NativeProfileOwner,
    },
    DeletePreview {
        window: WindowId,
        profile_uuid: Uuid,
        native: NativeProfileOwner,
    },
    Delete {
        window: WindowId,
        profile_uuid: Uuid,
        native: NativeProfileOwner,
    },
    Diff(u64, super::diff::DiffSide),
    Ignore,
}

pub(crate) struct PendingCommitDelete {
    pub(crate) window: WindowId,
    pub(crate) profile_uuid: Uuid,
    pub(crate) native: NativeProfileOwner,
    pub(crate) preview: CommitDeletePreview,
    created_at: Instant,
}

pub(crate) struct PendingCommit {
    operation: OperationId,
    commit: CommitId,
    manifest: VersionedCaptureManifest,
    begin_ticket: StorageTicket,
    publish_ticket: Option<StorageTicket>,
}

struct ContextCaptureSelection {
    context_ref: String,
    member_count: u16,
    windows: Vec<(u16, WindowId)>,
    invalid_members: Vec<InvalidContextMember>,
}

#[derive(Clone)]
struct PlannedContextIdentityBinding {
    window: WindowId,
    logical_window: LogicalWindowId,
    pages: Vec<(TabId, LogicalPageId)>,
}

/// One immutable, owner-issued window capture. It is deliberately not
/// serializable: a native adapter may transfer it only through an in-process
/// Rust handoff, never through QML, IPC, or durable context metadata.
#[derive(Clone)]
pub struct ContextWindowCaptureCandidate {
    profile_uuid: Uuid,
    native: NativeProfileOwner,
    application_revision: u64,
    context_name: String,
    context_ref: String,
    tabs: Vec<TabId>,
    window: ferric_browser_core::StructuralWindow,
    interval: CaptureInterval,
    ordinal: u16,
    binding: PlannedContextIdentityBinding,
}

impl std::fmt::Debug for ContextWindowCaptureCandidate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContextWindowCaptureCandidate")
            .field("window", &"[redacted]")
            .field("context", &"[redacted]")
            .field("tab_count", &self.tabs.len())
            .field("application_revision", &self.application_revision)
            .field("capture", &"[redacted]")
            .finish_non_exhaustive()
    }
}

struct ProjectedContextCapture {
    application_revision: u64,
    windows: Vec<ferric_browser_core::StructuralWindow>,
    intervals: Vec<(u16, CaptureInterval)>,
}

/// Reducer effects produced by a different live application owner during a
/// federated context capture. The native adapter must checkpoint these at the
/// exact originating window, never at the source window.
pub struct FederatedOwnerEffects {
    pub window: WindowId,
    pub effects: Vec<ferric_browser_runtime::RuntimeEffect>,
}

impl BrowserApplication {
    /// Returns the exact native normal-profile owner for a live window in a
    /// named context. Saving membership does not require pre-existing saved
    /// membership, but it must never grant cross-profile capture authority.
    ///
    /// # Errors
    /// Refuses a transient, stale, mismatched-context, or non-native owner.
    pub fn context_membership_owner(
        &self,
        window: WindowId,
        context_name: &str,
    ) -> Result<(Uuid, NativeProfileOwner), VersioningFailure> {
        let owner = self.structural_commit_owner(window)?;
        if context_name.is_empty()
            || context_name.len() > 32
            || self
                .windows()
                .get(&window)
                .is_none_or(|state| state.context.as_deref() != Some(context_name))
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        Ok(owner)
    }

    /// Retains the exact source owner, saved context and local logical binding
    /// expectations for later publication after per-owner binding receipts.
    ///
    /// # Errors
    /// Refuses any invalid candidate aggregate or stale source context.
    pub fn prepare_context_candidate_commit(
        &self,
        source_window: WindowId,
        message: String,
        candidates: &[ContextWindowCaptureCandidate],
    ) -> Result<PreparedContextCandidateCommit, VersioningFailure> {
        let manifest =
            self.prepare_context_manifest_from_candidates(source_window, message, candidates)?;
        let (profile_uuid, native) = self.structural_commit_owner(source_window)?;
        let profile = self
            .profile_snapshot()
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        let context = unique_context_for_source(
            self.contexts()
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?,
            &profile.name,
            &source_window.to_string(),
        )
        .ok_or_else(|| failure(VersioningReason::StaleOwner))?
        .clone();
        let commit = manifest
            .id
            .parse::<CommitId>()
            .map_err(|_| failure(VersioningReason::UnqualifiedRuntime))?;
        let source_bindings = candidates
            .iter()
            .filter(|candidate| self.windows().contains_key(&candidate.binding.window))
            .map(|candidate| candidate.binding.clone())
            .collect();
        Ok(PreparedContextCandidateCommit {
            source_window,
            profile_uuid,
            native,
            source_context: context,
            source_bindings,
            operation: OperationId::fresh(),
            commit,
            manifest,
        })
    }

    /// Submits the already assembled context plan only after its local source
    /// bindings have been accepted by the reducer. A native coordinator must
    /// additionally provide exact peer-binding receipts before calling this.
    ///
    /// # Errors
    /// Refuses stale profile/context/reducer identity or unavailable storage.
    pub fn begin_prepared_context_candidate_commit(
        &mut self,
        plan: PreparedContextCandidateCommit,
    ) -> Result<(OperationId, CommitId), VersioningFailure> {
        if self.pending_commit.is_some()
            || self.structural_commit_owner(plan.source_window)? != (plan.profile_uuid, plan.native)
            || self.revision() != plan.manifest.application_revision
            || self.context_capture_owners.get(&plan.source_context.id) != Some(&plan.native)
            || self.profile_snapshot().is_none_or(|profile| {
                unique_context_for_source(
                    self.contexts().unwrap_or_default(),
                    &profile.name,
                    &plan.source_window.to_string(),
                ) != Some(&plan.source_context)
            })
            || plan.source_bindings.iter().any(|binding| {
                self.runtime_view().logical_window(binding.window) != Some(binding.logical_window)
                    || binding.pages.iter().any(|(tab, logical)| {
                        self.runtime_view().logical_page(*tab) != Some(*logical)
                    })
            })
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let (operation, commit, _) =
            self.submit_prepared_structural_commit(PreparedStructuralCommit {
                operation: plan.operation,
                commit: plan.commit,
                manifest: plan.manifest,
                runtime_effects: Vec::new(),
            })?;
        Ok((operation, commit))
    }
    /// Returns the current saved context identity only for its exact live
    /// normal-profile source window. The ID is routing data, not authority to
    /// capture another owner.
    ///
    /// # Errors
    /// Refuses a stale or transient owner and unsaved context membership.
    pub fn active_context_capture_ref(
        &self,
        source_window: WindowId,
        context_name: &str,
    ) -> Result<String, VersioningFailure> {
        let (_, native) = self.structural_commit_owner(source_window)?;
        let profile = self
            .profile_snapshot()
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        let contexts = self
            .contexts()
            .ok_or_else(|| failure(VersioningReason::UnsupportedCapability))?;
        if contexts.len() > 256
            || self
                .windows()
                .get(&source_window)
                .is_none_or(|window| window.context.as_deref() != Some(context_name))
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let context =
            unique_context_for_source(contexts, &profile.name, &source_window.to_string())
                .filter(|context| context.name == context_name)
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if self.context_capture_owners.get(&context.id) != Some(&native) {
            return Err(failure(VersioningReason::StaleOwner));
        }
        Ok(context.id.to_string())
    }

    /// Returns the saved member order for the exact source context. These
    /// runtime IDs are routing hints only; candidate preparation rechecks
    /// every member against the current saved record and originating owner.
    ///
    /// # Errors
    /// Refuses an unsaved, ambiguous, stale, or transient source context.
    pub fn active_context_capture_members(
        &self,
        source_window: WindowId,
        context_name: &str,
    ) -> Result<Vec<WindowId>, VersioningFailure> {
        let context_ref = self.active_context_capture_ref(source_window, context_name)?;
        let profile = self
            .profile_snapshot()
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        let context = unique_context_for_source(
            self.contexts()
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?,
            &profile.name,
            &source_window.to_string(),
        )
        .filter(|context| context.id.to_string() == context_ref)
        .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if context.members.is_empty() || context.members.len() > 256 {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let mut seen = BTreeSet::new();
        let mut members = Vec::with_capacity(context.members.len());
        for member in &context.members {
            let window = WindowId::from_display(&member.window_id)
                .filter(|window| window.to_string() == member.window_id)
                .filter(|window| seen.insert(*window))
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
            members.push(window);
        }
        if !seen.contains(&source_window) {
            return Err(failure(VersioningReason::StaleOwner));
        }
        Ok(members)
    }
    /// Assembles owner-issued immutable captures against the source's saved
    /// membership. Candidates are not treated as authority until their exact
    /// native owners separately accept their bindings. This step does not
    /// mutate reducer state or storage.
    ///
    /// # Errors
    /// Refuses missing/stale source membership, ambiguous candidates, wrong
    /// profile, duplicate logical identities, or an invalid final manifest.
    #[allow(clippy::too_many_lines)]
    pub fn prepare_context_manifest_from_candidates(
        &self,
        source_window: WindowId,
        message: String,
        candidates: &[ContextWindowCaptureCandidate],
    ) -> Result<VersionedCaptureManifest, VersioningFailure> {
        if !valid_commit_message(&message) || candidates.len() > 256 {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        let (profile_uuid, native) = self.structural_commit_owner(source_window)?;
        let profile = self
            .profile_snapshot()
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        let contexts = self
            .contexts()
            .ok_or_else(|| failure(VersioningReason::UnsupportedCapability))?;
        if contexts.len() > 256 {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        let context =
            unique_context_for_source(contexts, &profile.name, &source_window.to_string())
                .ok_or_else(|| failure(VersioningReason::UnsupportedCapability))?;
        if self.context_capture_owners.get(&context.id) != Some(&native)
            || context.members.is_empty()
            || context.members.len() > 256
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let context_ref = context.id.to_string();
        let member_count = u16::try_from(context.members.len())
            .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
        let mut seen_windows = BTreeSet::new();
        let mut consumed_candidates = BTreeSet::new();
        let mut logical_windows = BTreeSet::new();
        let mut logical_pages = BTreeSet::new();
        let mut runtime_tabs = BTreeSet::new();
        let mut invalid_members = Vec::new();
        let mut projected = ProjectedContextCapture {
            application_revision: self.revision(),
            windows: Vec::new(),
            intervals: Vec::new(),
        };
        let mut source_is_valid = false;
        for (index, member) in context.members.iter().enumerate() {
            let ordinal = u16::try_from(index)
                .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
            let Some(window) = WindowId::from_display(&member.window_id)
                .filter(|window| window.to_string() == member.window_id)
            else {
                invalid_members.push(InvalidContextMember {
                    ordinal,
                    reason: InvalidContextMemberReason::InvalidWindowIdentity,
                });
                continue;
            };
            if !seen_windows.insert(window) {
                invalid_members.push(InvalidContextMember {
                    ordinal,
                    reason: InvalidContextMemberReason::DuplicateWindow,
                });
                continue;
            }
            let matches = candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.binding.window == window)
                .collect::<Vec<_>>();
            let reason = match matches.as_slice() {
                [] => Some(InvalidContextMemberReason::ClosedWindow),
                [(_, candidate)] if candidate.profile_uuid != profile_uuid => {
                    Some(InvalidContextMemberReason::WrongProfile)
                }
                [(_, candidate)]
                    if candidate.context_name != context.name
                        || candidate.context_ref != context_ref
                        || candidate.ordinal != ordinal
                        || candidate.tabs.is_empty()
                        || member.tabs
                            != candidate
                                .tabs
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>() =>
                {
                    Some(InvalidContextMemberReason::ChangedMembership)
                }
                [(_, candidate)]
                    if self.windows().contains_key(&window)
                        && (self.structural_commit_owner(window).ok()
                            != Some((candidate.profile_uuid, candidate.native))
                            || candidate.application_revision != self.revision()
                            || context_member_refusal(
                                self,
                                window,
                                member,
                                &context.name,
                                profile_uuid,
                            )
                            .is_some()) =>
                {
                    Some(InvalidContextMemberReason::InvalidWindowIdentity)
                }
                [_] => None,
                _ => Some(InvalidContextMemberReason::InvalidWindowIdentity),
            };
            if let Some(reason) = reason {
                if let [(_, _)] = matches.as_slice() {
                    consumed_candidates.insert(matches[0].0);
                }
                invalid_members.push(InvalidContextMember { ordinal, reason });
                continue;
            }
            let (candidate_index, candidate) = matches[0];
            if !logical_windows.insert(candidate.window.logical_id)
                || candidate
                    .window
                    .pages
                    .iter()
                    .any(|page| !logical_pages.insert(page.logical_id))
                || candidate
                    .binding
                    .pages
                    .iter()
                    .any(|(tab, _)| !runtime_tabs.insert(*tab))
            {
                return Err(failure(VersioningReason::StaleOwner));
            }
            consumed_candidates.insert(candidate_index);
            projected.windows.push(candidate.window.clone());
            projected.intervals.push((ordinal, candidate.interval));
            if window == source_window {
                source_is_valid = true;
            }
            if self.windows().contains_key(&window) {
                projected.application_revision = projected
                    .application_revision
                    .checked_add(1)
                    .ok_or_else(|| failure(VersioningReason::UnqualifiedRuntime))?;
            }
        }
        if !source_is_valid || consumed_candidates.len() != candidates.len() {
            return Err(failure(VersioningReason::StaleOwner));
        }
        // Candidate projection precedes source-side assembly. The aggregate
        // interval must begin at the first actual owner capture, not when the
        // last handle happened to arrive at the coordinator.
        let capture_started_ms = projected
            .intervals
            .iter()
            .map(|(_, interval)| interval.started_at_ms)
            .min()
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        self.assemble_context_capture_manifest(
            profile_uuid,
            CommitId::fresh(),
            message,
            capture_started_ms,
            ContextCaptureSelection {
                context_ref,
                member_count,
                windows: Vec::new(),
                invalid_members,
            },
            projected,
        )
    }
    /// Projects one exact live window for a later same-process context
    /// handoff. The returned object contains no claim about saved context
    /// membership; the source owner must compare it with its registry before
    /// publishing an aggregate manifest.
    ///
    /// # Errors
    /// Refuses transient, stale, mismatched-context, or empty owners.
    pub fn prepare_context_window_candidate(
        &self,
        window: WindowId,
        context_name: &str,
        context_ref: &str,
        ordinal: u16,
    ) -> Result<ContextWindowCaptureCandidate, VersioningFailure> {
        if context_name.is_empty()
            || context_name.len() > 32
            || context_ref.len() > 64
            || Uuid::parse_str(context_ref).is_err()
        {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        let (profile_uuid, native) = self.structural_commit_owner(window)?;
        let state = self
            .windows()
            .get(&window)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if state.context.as_deref() != Some(context_name) || state.tabs.is_empty() {
            return Err(failure(VersioningReason::StaleOwner));
        }
        if state.tabs.iter().any(|tab| {
            self.tabs().get(tab).is_none_or(|state| {
                state.window != window
                    || state.profile != self.windows()[&window].profile
                    || state.existence != ExistenceState::Live
            })
        }) {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let application_revision = self.revision();
        let selection = ContextCaptureSelection {
            context_ref: context_ref.to_owned(),
            member_count: 1,
            windows: vec![(ordinal, window)],
            invalid_members: Vec::new(),
        };
        let (projected, bindings) = self.project_context_capture(&selection, native)?;
        let mut windows = projected.windows.into_iter();
        let mut intervals = projected.intervals.into_iter();
        let mut bindings = bindings.into_iter();
        let candidate = ContextWindowCaptureCandidate {
            profile_uuid,
            native,
            application_revision,
            context_name: context_name.to_owned(),
            context_ref: context_ref.to_owned(),
            tabs: state.tabs.clone(),
            window: windows
                .next()
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?,
            interval: intervals
                .next()
                .map(|(_, interval)| interval)
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?,
            ordinal,
            binding: bindings
                .next()
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?,
        };
        if windows.next().is_some() || intervals.next().is_some() || bindings.next().is_some() {
            return Err(failure(VersioningReason::StaleOwner));
        }
        Ok(candidate)
    }

    /// Binds only the exact still-current candidate at its originating owner.
    /// A revision, profile instance, context, or tab-order change refuses the
    /// effect. A candidate can be submitted at most once because its revision
    /// is consumed by the reducer binding event.
    ///
    /// # Errors
    /// Refuses a stale or foreign candidate without applying its binding.
    pub fn bind_context_window_candidate(
        &mut self,
        candidate: ContextWindowCaptureCandidate,
    ) -> Result<Vec<ferric_browser_runtime::RuntimeEffect>, VersioningFailure> {
        let window = candidate.binding.window;
        if self.revision() != candidate.application_revision
            || self.structural_commit_owner(window)? != (candidate.profile_uuid, candidate.native)
            || self.windows().get(&window).is_none_or(|state| {
                state.context.as_deref() != Some(candidate.context_name.as_str())
                    || state.tabs != candidate.tabs
            })
            || candidate.tabs.iter().any(|tab| {
                self.tabs().get(tab).is_none_or(|state| {
                    state.window != window || state.existence != ExistenceState::Live
                })
            })
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        self.runtime
            .handle(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::BindStructuralIdentity {
                    window,
                    logical_window: candidate.binding.logical_window,
                    pages: candidate.binding.pages,
                },
            )))
            .map_err(|_| failure(VersioningReason::StaleOwner))
    }
    /// Prepares one context commit from separate, live application owners.
    /// Each peer must be an exact native owner of a saved member window. The
    /// source registry supplies the membership order; no QML or IPC payload
    /// supplies page contents or authority. All owners are preflighted before
    /// any logical identity is bound through their reducers.
    ///
    /// # Errors
    /// Refuses a stale source, ambiguous window owner, wrong profile, changed
    /// membership, or an invalid aggregate manifest.
    #[allow(clippy::too_many_lines)]
    pub fn prepare_federated_context_structural_commit(
        &mut self,
        source_window: WindowId,
        message: String,
        peers: &mut [&mut BrowserApplication],
    ) -> Result<(PreparedStructuralCommit, Vec<FederatedOwnerEffects>), VersioningFailure> {
        if !valid_commit_message(&message) || peers.len() > 255 {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        let (profile_uuid, native) = self.structural_commit_owner(source_window)?;
        let profile = self
            .profile_snapshot()
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        let contexts = self
            .contexts()
            .ok_or_else(|| failure(VersioningReason::UnsupportedCapability))?;
        if contexts.len() > 256 {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        let context =
            unique_context_for_source(contexts, &profile.name, &source_window.to_string())
                .ok_or_else(|| failure(VersioningReason::UnsupportedCapability))?;
        if self.context_capture_owners.get(&context.id) != Some(&native)
            || context.members.is_empty()
            || context.members.len() > 256
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let context_name = context.name.clone();
        let context_ref = context.id.to_string();
        let members = context.members.clone();
        let member_count = u16::try_from(members.len())
            .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
        let mut seen_windows = BTreeSet::new();
        let mut selected = Vec::new();
        let mut invalid_members = Vec::new();
        let mut source_is_valid = false;
        for (index, member) in members.iter().enumerate() {
            let ordinal = u16::try_from(index)
                .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
            let Some(window) = WindowId::from_display(&member.window_id)
                .filter(|window| window.to_string() == member.window_id)
            else {
                invalid_members.push(InvalidContextMember {
                    ordinal,
                    reason: InvalidContextMemberReason::InvalidWindowIdentity,
                });
                continue;
            };
            if !seen_windows.insert(window) {
                invalid_members.push(InvalidContextMember {
                    ordinal,
                    reason: InvalidContextMemberReason::DuplicateWindow,
                });
                continue;
            }
            let owners = usize::from(self.windows().contains_key(&window))
                + peers
                    .iter()
                    .filter(|peer| peer.windows().contains_key(&window))
                    .count();
            let reason = if owners > 1 {
                Some(InvalidContextMemberReason::InvalidWindowIdentity)
            } else if owners == 0 {
                Some(InvalidContextMemberReason::ClosedWindow)
            } else if self.windows().contains_key(&window) {
                context_member_refusal(self, window, member, &context_name, profile_uuid)
            } else {
                let peer = peers
                    .iter()
                    .find(|peer| peer.windows().contains_key(&window))
                    .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
                context_member_refusal(peer, window, member, &context_name, profile_uuid)
            };
            if let Some(reason) = reason {
                invalid_members.push(InvalidContextMember { ordinal, reason });
            } else {
                if window == source_window {
                    source_is_valid = true;
                }
                selected.push((ordinal, window));
            }
        }
        if !source_is_valid {
            return Err(failure(VersioningReason::StaleOwner));
        }

        let capture_started_ms = unix_millis()?;
        let mut projected = ProjectedContextCapture {
            application_revision: self.revision(),
            windows: Vec::with_capacity(selected.len()),
            intervals: Vec::with_capacity(selected.len()),
        };
        let mut planned = Vec::with_capacity(selected.len());
        let mut logical_windows = BTreeSet::new();
        let mut logical_pages = BTreeSet::new();
        let mut runtime_tabs = BTreeSet::new();
        for (ordinal, window) in selected {
            let owner_index = if self.windows().contains_key(&window) {
                None
            } else {
                Some(
                    peers
                        .iter()
                        .position(|peer| peer.windows().contains_key(&window))
                        .ok_or_else(|| failure(VersioningReason::StaleOwner))?,
                )
            };
            let owner: &BrowserApplication = match owner_index {
                None => self,
                Some(index) => peers[index],
            };
            let (_, owner_native) = owner.structural_commit_owner(window)?;
            let selection = ContextCaptureSelection {
                context_ref: context_ref.clone(),
                member_count,
                windows: vec![(ordinal, window)],
                invalid_members: Vec::new(),
            };
            let (one, bindings) = owner.project_context_capture(&selection, owner_native)?;
            let binding = bindings
                .into_iter()
                .next()
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
            if !logical_windows.insert(binding.logical_window)
                || binding.pages.iter().any(|(tab, logical)| {
                    !runtime_tabs.insert(*tab) || !logical_pages.insert(*logical)
                })
            {
                return Err(failure(VersioningReason::StaleOwner));
            }
            projected.windows.extend(one.windows);
            projected.intervals.extend(one.intervals);
            if owner_index.is_none() {
                projected.application_revision = projected
                    .application_revision
                    .checked_add(1)
                    .ok_or_else(|| failure(VersioningReason::UnqualifiedRuntime))?;
            }
            planned.push((owner_index, binding));
        }
        let commit = CommitId::fresh();
        let manifest = self.assemble_context_capture_manifest(
            profile_uuid,
            commit,
            message,
            capture_started_ms,
            ContextCaptureSelection {
                context_ref,
                member_count,
                windows: Vec::new(),
                invalid_members,
            },
            projected,
        )?;
        let mut source_effects = Vec::new();
        let mut peer_effects = Vec::new();
        for (owner_index, binding) in planned {
            let event = RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::BindStructuralIdentity {
                    window: binding.window,
                    logical_window: binding.logical_window,
                    pages: binding.pages,
                },
            ));
            if let Some(index) = owner_index {
                peer_effects.push(FederatedOwnerEffects {
                    window: binding.window,
                    effects: peers[index]
                        .runtime
                        .handle(event)
                        .map_err(|_| failure(VersioningReason::StaleOwner))?,
                });
            } else {
                source_effects.extend(
                    self.runtime
                        .handle(event)
                        .map_err(|_| failure(VersioningReason::StaleOwner))?,
                );
            }
        }
        let prepared = PreparedStructuralCommit {
            operation: OperationId::fresh(),
            commit,
            manifest,
            runtime_effects: source_effects,
        };
        Ok((prepared, peer_effects))
    }
    /// Prepares a normal-profile, structural-only commit without writing it.
    /// The caller must submit a durable reservation and publish this exact
    /// manifest; preparation alone must never be reported as completion.
    ///
    /// # Errors
    /// Refuses transient owners, stale scope members and invalid manifests.
    pub fn prepare_structural_commit(
        &mut self,
        selector: ScopeSelector,
        message: String,
    ) -> Result<PreparedStructuralCommit, VersioningFailure> {
        if !valid_commit_message(&message) {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        if let ScopeSelector::Context { window } = selector {
            return self.prepare_context_structural_commit(window, message);
        }
        let (window, manifest_scope) = structural_scope_for_selector(selector)
            .ok_or_else(|| failure(VersioningReason::UnsupportedCapability))?;
        let (profile_uuid, native) = self.structural_commit_owner(window)?;
        let capture_started_ms = unix_millis()?;
        // Validate the supplied target before writing even a logical-ID event.
        let preliminary = self
            .runtime_view()
            .resolve_versioning_scope(selector, native)
            .map_err(|error| failure(error.reason()))?;
        // Complete every fallible projection and manifest validation before
        // applying the reducer event that binds missing logical identities.
        let bound_revision = self
            .runtime_view()
            .revision()
            .checked_add(1)
            .ok_or_else(|| failure(VersioningReason::UnqualifiedRuntime))?;
        let logical_window = self
            .runtime_view()
            .logical_window(window)
            .unwrap_or_else(LogicalWindowId::fresh);
        let pages = preliminary
            .members()
            .iter()
            .map(|member| {
                let tab = member.target().tab;
                let logical = self
                    .runtime_view()
                    .logical_page(tab)
                    .unwrap_or_else(LogicalPageId::fresh);
                (tab, logical)
            })
            .collect::<Vec<_>>();
        let logical_pages = pages.iter().copied().collect::<BTreeMap<_, _>>();
        let inputs = preliminary
            .members()
            .iter()
            .map(|member| {
                let target = member.target();
                let logical = logical_pages
                    .get(&target.tab)
                    .copied()
                    .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
                let mut input = PageCaptureInput::structural_only(logical, target);
                input.lineage_parent = self.runtime_view().lineage_parent(target.tab);
                input.proven_safe_get = self
                    .safe_get_navigation_proofs
                    .get(&target.tab)
                    .is_some_and(|(proved_target, proved_url)| {
                        *proved_target == target
                            && self.tabs()[&target.tab].url.as_deref() == Some(proved_url)
                            && matches!(
                                self.tabs()[&target.tab].loading,
                                LoadingState::Committed | LoadingState::Complete
                            )
                    });
                Ok((target.tab, input))
            })
            .collect::<Result<BTreeMap<TabId, PageCaptureInput>, VersioningFailure>>()?;
        let structural = self
            .runtime_view()
            .project_structural_scope(&preliminary, logical_window, &inputs)
            .map_err(|_| failure(VersioningReason::StaleOwner))?;
        let commit = CommitId::fresh();
        let mut manifest = VersionedCaptureManifest::from_structural(
            structural_header(
                commit,
                profile_uuid,
                message,
                self.configuration_revision(),
                manifest_scope,
                capture_started_ms,
            )?,
            &structural,
        )
        .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
        // Identity binding is the final fallible step. It advances the
        // application revision exactly once but does not change the already
        // projected scope or page state.
        manifest.application_revision = bound_revision;
        manifest.policy_snapshot = Some(self.allowlisted_policy_snapshot());
        manifest
            .validate()
            .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
        let runtime_effects = self
            .runtime
            .handle(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::BindStructuralIdentity {
                    window,
                    logical_window,
                    pages,
                },
            )))
            .map_err(|_| failure(VersioningReason::StaleOwner))?;
        Ok(PreparedStructuralCommit {
            operation: OperationId::fresh(),
            commit,
            manifest,
            runtime_effects,
        })
    }

    fn prepare_context_structural_commit(
        &mut self,
        source_window: WindowId,
        message: String,
    ) -> Result<PreparedStructuralCommit, VersioningFailure> {
        let (profile_uuid, native) = self.structural_commit_owner(source_window)?;
        let profile = self
            .profile_snapshot()
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        let selection =
            self.resolve_context_capture_selection(source_window, &profile.name, native)?;
        let capture_started_ms = unix_millis()?;
        let (projected, bindings) = self.project_context_capture(&selection, native)?;
        let commit = CommitId::fresh();
        let manifest = self.assemble_context_capture_manifest(
            profile_uuid,
            commit,
            message,
            capture_started_ms,
            selection,
            projected,
        )?;
        // All fallible owner checks, identity allocation, projection and
        // manifest validation are complete before reducer state is changed.
        // The planned bindings are prevalidated as one unique context-wide
        // identity set, then emitted only through the reducer event boundary.
        let mut runtime_effects = Vec::new();
        for binding in bindings {
            runtime_effects.extend(
                self.runtime
                    .handle(RuntimeInput::EngineFact(Event::Versioning(
                        VersioningEvent::BindStructuralIdentity {
                            window: binding.window,
                            logical_window: binding.logical_window,
                            pages: binding.pages,
                        },
                    )))
                    .map_err(|_| failure(VersioningReason::StaleOwner))?,
            );
        }
        Ok(PreparedStructuralCommit {
            operation: OperationId::fresh(),
            commit,
            manifest,
            runtime_effects,
        })
    }

    fn project_context_capture(
        &self,
        selection: &ContextCaptureSelection,
        native: NativeProfileOwner,
    ) -> Result<(ProjectedContextCapture, Vec<PlannedContextIdentityBinding>), VersioningFailure>
    {
        let initial_revision = self.runtime_view().revision();
        let final_revision = initial_revision
            .checked_add(
                u64::try_from(selection.windows.len())
                    .map_err(|_| failure(VersioningReason::UnqualifiedRuntime))?,
            )
            .ok_or_else(|| failure(VersioningReason::UnqualifiedRuntime))?;
        let mut logical_windows = self
            .windows()
            .keys()
            .filter_map(|window| {
                self.runtime_view()
                    .logical_window(*window)
                    .map(|logical| (logical, *window))
            })
            .collect::<BTreeMap<_, _>>();
        let mut logical_pages = self
            .tabs()
            .keys()
            .filter_map(|tab| {
                self.runtime_view()
                    .logical_page(*tab)
                    .map(|logical| (logical, *tab))
            })
            .collect::<BTreeMap<_, _>>();
        let mut projected_capture = ProjectedContextCapture {
            application_revision: final_revision,
            windows: Vec::with_capacity(selection.windows.len()),
            intervals: Vec::with_capacity(selection.windows.len()),
        };
        let mut bindings = Vec::with_capacity(selection.windows.len());
        for (ordinal, window) in &selection.windows {
            let started_at_ms = unix_millis()?;
            let scope = self
                .runtime_view()
                .resolve_versioning_scope(ScopeSelector::Window { window: *window }, native)
                .map_err(|error| failure(error.reason()))?;
            if scope.revision() != initial_revision {
                return Err(failure(VersioningReason::StaleOwner));
            }
            let logical_window = match self.runtime_view().logical_window(*window) {
                Some(logical) if logical_windows.get(&logical) == Some(window) => logical,
                Some(_) => return Err(failure(VersioningReason::StaleOwner)),
                None => fresh_unclaimed_logical_window(&mut logical_windows, *window),
            };
            let pages = scope
                .members()
                .iter()
                .map(|member| {
                    let tab = member.target().tab;
                    let logical = match self.runtime_view().logical_page(tab) {
                        Some(logical) if logical_pages.get(&logical) == Some(&tab) => logical,
                        Some(_) => return Err(failure(VersioningReason::StaleOwner)),
                        None => fresh_unclaimed_logical_page(&mut logical_pages, tab),
                    };
                    Ok((tab, logical))
                })
                .collect::<Result<Vec<_>, VersioningFailure>>()?;
            let page_ids = pages.iter().copied().collect::<BTreeMap<_, _>>();
            let inputs = self.structural_capture_inputs(&scope, &page_ids)?;
            let projected = self
                .runtime_view()
                .project_structural_scope(&scope, logical_window, &inputs)
                .map_err(|_| failure(VersioningReason::StaleOwner))?;
            let finished_at_ms = unix_millis()?;
            let mut projected_window = projected
                .windows
                .into_iter()
                .next()
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
            projected_window.context_ref = Some(selection.context_ref.clone());
            projected_capture.windows.push(projected_window);
            projected_capture.intervals.push((
                *ordinal,
                CaptureInterval {
                    started_at_ms,
                    finished_at_ms,
                },
            ));
            bindings.push(PlannedContextIdentityBinding {
                window: *window,
                logical_window,
                pages,
            });
        }
        Ok((projected_capture, bindings))
    }

    fn structural_capture_inputs(
        &self,
        scope: &ferric_browser_core::CapturedScope,
        logical_pages: &BTreeMap<TabId, LogicalPageId>,
    ) -> Result<BTreeMap<TabId, PageCaptureInput>, VersioningFailure> {
        scope
            .members()
            .iter()
            .map(|member| {
                let target = member.target();
                let logical = logical_pages
                    .get(&target.tab)
                    .copied()
                    .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
                let mut input = PageCaptureInput::structural_only(logical, target);
                input.lineage_parent = self.runtime_view().lineage_parent(target.tab);
                input.proven_safe_get = self
                    .safe_get_navigation_proofs
                    .get(&target.tab)
                    .is_some_and(|(proved_target, proved_url)| {
                        *proved_target == target
                            && self.tabs()[&target.tab].url.as_deref() == Some(proved_url)
                            && matches!(
                                self.tabs()[&target.tab].loading,
                                LoadingState::Committed | LoadingState::Complete
                            )
                    });
                Ok((target.tab, input))
            })
            .collect()
    }

    fn assemble_context_capture_manifest(
        &self,
        profile_uuid: Uuid,
        commit: CommitId,
        message: String,
        capture_started_ms: u64,
        selection: ContextCaptureSelection,
        projected: ProjectedContextCapture,
    ) -> Result<VersionedCaptureManifest, VersioningFailure> {
        let mut manifest = VersionedCaptureManifest::from_structural(
            structural_header(
                commit,
                profile_uuid,
                message,
                self.configuration_revision(),
                ManifestScope::Context,
                capture_started_ms,
            )?,
            &StructuralSnapshot {
                application_revision: projected.application_revision,
                windows: projected.windows,
            },
        )
        .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
        manifest.context_member_count = Some(selection.member_count);
        manifest.invalid_context_members = selection.invalid_members;
        for (window, (ordinal, interval)) in manifest.windows.iter_mut().zip(projected.intervals) {
            window.context_member_ordinal = Some(ordinal);
            window.capture_interval = Some(interval);
        }
        manifest.policy_snapshot = Some(self.allowlisted_policy_snapshot());
        manifest
            .validate()
            .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
        Ok(manifest)
    }

    fn resolve_context_capture_selection(
        &self,
        source_window: WindowId,
        profile_name: &str,
        native: NativeProfileOwner,
    ) -> Result<ContextCaptureSelection, VersioningFailure> {
        const MAX_CONTEXTS_FOR_CAPTURE: usize = 256;
        let contexts = self
            .contexts()
            .ok_or_else(|| failure(VersioningReason::UnsupportedCapability))?;
        if contexts.len() > MAX_CONTEXTS_FOR_CAPTURE {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        let source_id = source_window.to_string();
        let context = unique_context_for_source(contexts, profile_name, &source_id)
            .ok_or_else(|| failure(VersioningReason::UnsupportedCapability))?;
        if self.context_capture_owners.get(&context.id) != Some(&native) {
            return Err(failure(VersioningReason::StaleOwner));
        }
        if context.members.is_empty() || context.members.len() > 256 {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        let member_count = u16::try_from(context.members.len())
            .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
        let source_profile = self
            .windows()
            .get(&source_window)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?
            .profile;
        let mut seen_windows = BTreeSet::new();
        let mut windows = Vec::new();
        let mut invalid_members = Vec::new();
        let mut source_is_valid = false;

        for (index, member) in context.members.iter().enumerate() {
            let ordinal = u16::try_from(index)
                .map_err(|_| failure(VersioningReason::UnsupportedCapability))?;
            let reason = match WindowId::from_display(&member.window_id) {
                Some(window) if window.to_string() == member.window_id => {
                    if !seen_windows.insert(window) {
                        Some(InvalidContextMemberReason::DuplicateWindow)
                    } else if let Some(window_state) = self.windows().get(&window) {
                        if window_state.profile != source_profile {
                            Some(InvalidContextMemberReason::WrongProfile)
                        } else if window_state.context.as_deref() != Some(context.name.as_str()) {
                            Some(InvalidContextMemberReason::ChangedMembership)
                        } else if window_state.tabs.is_empty() {
                            Some(InvalidContextMemberReason::NoLivePages)
                        } else {
                            let parsed_tabs = member
                                .tabs
                                .iter()
                                .map(|value| {
                                    TabId::from_display(value)
                                        .filter(|tab| tab.to_string() == value.as_str())
                                })
                                .collect::<Option<Vec<_>>>();
                            match parsed_tabs {
                                Some(tabs)
                                    if tabs.len() == window_state.tabs.len()
                                        && tabs == window_state.tabs
                                        && tabs.iter().all(|tab| {
                                            self.tabs().get(tab).is_some_and(|state| {
                                                state.window == window
                                                    && state.profile == source_profile
                                                    && state.existence == ExistenceState::Live
                                            })
                                        }) =>
                                {
                                    if window == source_window {
                                        source_is_valid = true;
                                    }
                                    windows.push((ordinal, window));
                                    None
                                }
                                _ => Some(InvalidContextMemberReason::ChangedMembership),
                            }
                        }
                    } else {
                        Some(InvalidContextMemberReason::ClosedWindow)
                    }
                }
                _ => Some(InvalidContextMemberReason::InvalidWindowIdentity),
            };
            if let Some(reason) = reason {
                invalid_members.push(InvalidContextMember { ordinal, reason });
            }
        }
        if !source_is_valid || windows.is_empty() {
            return Err(failure(VersioningReason::StaleOwner));
        }
        Ok(ContextCaptureSelection {
            context_ref: context.id.to_string(),
            member_count,
            windows,
            invalid_members,
        })
    }

    /// Records a native adapter observation only for the exact currently
    /// committed document and URL. The adapter must call this only after a
    /// main-frame typed, bodyless navigation succeeds without a redirect and
    /// the profile interceptor uniquely observes its exact GET request.
    /// Later navigation changes the target or URL and makes this proof stale.
    pub fn note_typed_safe_get_navigation(&mut self, tab: TabId, url: &str) -> bool {
        let Some(target) = self.capture_target(tab) else {
            return false;
        };
        if self.tabs().get(&tab).and_then(|state| state.url.as_deref()) != Some(url)
            || !self.tabs().get(&tab).is_some_and(|state| {
                matches!(
                    state.loading,
                    LoadingState::Committed | LoadingState::Complete
                )
            })
            || ferric_browser_core::SafePageUrl::from_observed(Some(url), true).replay_url()
                != Some(url)
        {
            return false;
        }
        if self.safe_get_navigation_proofs.len() >= 1_024 {
            self.safe_get_navigation_proofs.clear();
        }
        self.safe_get_navigation_proofs
            .insert(tab, (target, url.to_owned()));
        true
    }

    /// Native main-frame callbacks must invalidate prior method evidence even
    /// if the reducer's document token or final URL happens to stay equal.
    pub fn invalidate_safe_get_navigation(&mut self, tab: TabId) {
        self.safe_get_navigation_proofs.remove(&tab);
    }

    /// Starts the durable two-step capture using an immutable prepared DTO.
    /// The returned reducer effects must be delivered through the ordinary
    /// adapter path; only a later `CommitEvent::Durable` confirms publication.
    ///
    /// # Errors
    /// Refuses a second in-flight commit or unavailable storage.
    pub fn begin_structural_commit(
        &mut self,
        selector: ScopeSelector,
        message: String,
    ) -> Result<
        (
            OperationId,
            CommitId,
            Vec<ferric_browser_runtime::RuntimeEffect>,
        ),
        VersioningFailure,
    > {
        if self.pending_commit.is_some() {
            return Err(failure(VersioningReason::Busy));
        }
        let prepared = self.prepare_structural_commit(selector, message)?;
        self.submit_prepared_structural_commit(prepared)
    }

    /// Begins a durable context commit after synchronously capturing separate
    /// native application owners. Peer effects must be checkpointed at their
    /// own adapters; the returned commit is only pending until a durable event.
    ///
    /// # Errors
    /// Refuses stale ownership, invalid aggregate capture, or busy storage.
    pub fn begin_federated_context_structural_commit(
        &mut self,
        source_window: WindowId,
        message: String,
        peers: &mut [&mut BrowserApplication],
    ) -> Result<
        (
            OperationId,
            CommitId,
            Vec<ferric_browser_runtime::RuntimeEffect>,
            Vec<FederatedOwnerEffects>,
        ),
        VersioningFailure,
    > {
        if self.pending_commit.is_some() {
            return Err(failure(VersioningReason::Busy));
        }
        let (prepared, peer_effects) =
            self.prepare_federated_context_structural_commit(source_window, message, peers)?;
        let (operation, commit, effects) = self.submit_prepared_structural_commit(prepared)?;
        Ok((operation, commit, effects, peer_effects))
    }

    pub(crate) fn submit_prepared_structural_commit(
        &mut self,
        prepared: PreparedStructuralCommit,
    ) -> Result<
        (
            OperationId,
            CommitId,
            Vec<ferric_browser_runtime::RuntimeEffect>,
        ),
        VersioningFailure,
    > {
        let profile_uuid = prepared.manifest.profile_uuid;
        let ticket = self
            .storage
            .submit(StorageCommand::VersioningBegin {
                operation: prepared.operation,
                profile_uuid,
                reserved_bytes: 0,
                state_root: None,
            })
            .map_err(|_| failure(VersioningReason::VersioningDisabled))?;
        let ids = (prepared.operation, prepared.commit);
        self.pending_commit = Some(PendingCommit {
            operation: prepared.operation,
            commit: prepared.commit,
            manifest: prepared.manifest,
            begin_ticket: ticket,
            publish_ticket: None,
        });
        Ok((ids.0, ids.1, prepared.runtime_effects))
    }

    /// Requests a bounded, redacted listing for the exact active profile and
    /// native-owner generation.
    /// The returned ticket is correlated with `StorageCompletion::VersioningList`.
    ///
    /// # Errors
    /// Refuses transient/wrong owners, invalid bounds and unavailable storage.
    pub fn request_commit_list(
        &mut self,
        window: WindowId,
        limit: usize,
        offset: usize,
    ) -> Result<StorageTicket, VersioningFailure> {
        if !(1..=MAX_LIST_ITEMS).contains(&limit) || offset > 100_000 {
            return Err(failure(VersioningReason::QuotaExceeded));
        }
        let storage_limit = limit
            .checked_add(1)
            .ok_or_else(|| failure(VersioningReason::QuotaExceeded))?;
        if self.pending_commit_reads.len() >= 16 {
            return Err(failure(VersioningReason::Busy));
        }
        let (profile_uuid, native) = self.structural_commit_owner(window)?;
        let ticket = self
            .storage
            .submit(StorageCommand::VersioningList {
                profile_uuid,
                // Fetch one extra row so the redacted page can report an
                // exact continuation bit without a separate count query.
                limit: storage_limit,
                offset,
            })
            .map_err(|_| failure(VersioningReason::Busy))?;
        self.pending_commit_reads.insert(
            ticket,
            CommitReadKind::List {
                offset,
                limit,
                window,
                profile_uuid,
                native,
            },
        );
        Ok(ticket)
    }

    /// Requests redacted metadata for one exact current-profile commit ID and
    /// native-owner generation.
    ///
    /// # Errors
    /// Refuses transient/wrong owners and unavailable storage.
    pub fn request_commit_info(
        &mut self,
        window: WindowId,
        id: CommitId,
    ) -> Result<StorageTicket, VersioningFailure> {
        if self.pending_commit_reads.len() >= 16 {
            return Err(failure(VersioningReason::Busy));
        }
        let (profile_uuid, native) = self.structural_commit_owner(window)?;
        let ticket = self
            .storage
            .submit(StorageCommand::VersioningInfo { profile_uuid, id })
            .map_err(|_| failure(VersioningReason::Busy))?;
        self.pending_commit_reads.insert(
            ticket,
            CommitReadKind::Info {
                window,
                profile_uuid,
                native,
            },
        );
        Ok(ticket)
    }

    /// Requests a redacted deletion preview for one exact commit. No storage
    /// mutation occurs until the matching native confirmation is consumed.
    ///
    /// # Errors
    /// Refuses a transient or wrong owner, a saturated read queue, or unavailable storage.
    pub fn request_commit_delete_preview(
        &mut self,
        window: WindowId,
        id: CommitId,
    ) -> Result<StorageTicket, VersioningFailure> {
        if self.pending_commit_reads.len() >= 16 {
            return Err(failure(VersioningReason::Busy));
        }
        let (profile_uuid, native) = self.structural_commit_owner(window)?;
        let ticket = self
            .storage
            .submit(StorageCommand::VersioningDeletePreview { profile_uuid, id })
            .map_err(|_| failure(VersioningReason::Busy))?;
        self.pending_commit_reads.insert(
            ticket,
            CommitReadKind::DeletePreview {
                window,
                profile_uuid,
                native,
            },
        );
        Ok(ticket)
    }

    /// Consumes the exact unexpired native deletion preview and schedules a
    /// storage-side snapshot revalidation. The preview is one-shot after
    /// successful submission; stale metadata cannot be deleted silently.
    ///
    /// # Errors
    /// Refuses a missing, expired, stale, or non-deletable preview and unavailable storage.
    pub fn confirm_commit_delete_preview(
        &mut self,
        window: WindowId,
        request_id: u64,
    ) -> Result<StorageTicket, VersioningFailure> {
        let Some(pending) = self.pending_commit_deletions.get(&request_id) else {
            return Err(failure(VersioningReason::ConfirmationRequired));
        };
        if pending.window != window {
            return Err(failure(VersioningReason::StaleOwner));
        }
        if pending.created_at.elapsed() > std::time::Duration::from_secs(60) {
            self.pending_commit_deletions.remove(&request_id);
            return Err(failure(VersioningReason::ConfirmationExpired));
        }
        let (current_profile, current_native) = self.structural_commit_owner(window)?;
        if current_profile != pending.profile_uuid || current_native != pending.native {
            self.pending_commit_deletions.remove(&request_id);
            return Err(failure(VersioningReason::StaleOwner));
        }
        if !pending.preview.can_delete {
            return Err(failure(VersioningReason::ConfirmationRequired));
        }
        let state_root = self
            .active_profile
            .as_ref()
            .and_then(|profile| {
                profile
                    .versioning_recovery_scope()
                    .filter(|(_, owner)| *owner == current_profile)
                    .map(|(root, _)| root.to_owned())
            })
            .ok_or_else(|| failure(VersioningReason::PrivatePersistenceDenied))?;
        let command = StorageCommand::VersioningDelete {
            profile_uuid: pending.profile_uuid,
            id: pending.preview.id,
            expected_snapshot: pending.preview.confirmation_snapshot(),
            state_root,
        };
        let ticket = self
            .storage
            .submit(command)
            .map_err(|_| failure(VersioningReason::Busy))?;
        self.pending_commit_deletions.remove(&request_id);
        self.pending_commit_reads.insert(
            ticket,
            CommitReadKind::Delete {
                window,
                profile_uuid: current_profile,
                native: current_native,
            },
        );
        Ok(ticket)
    }

    /// Cancels only the exact displayed preview from its originating window.
    /// This is UI state cleanup and never mutates durable commits or journal records.
    pub fn cancel_commit_delete_preview(&mut self, window: WindowId, request_id: u64) -> bool {
        if self
            .pending_commit_deletions
            .get(&request_id)
            .is_none_or(|pending| pending.window != window)
        {
            return false;
        }
        self.pending_commit_deletions.remove(&request_id).is_some()
    }

    pub(crate) fn route_commit_storage_effects(
        &mut self,
        effects: Vec<StorageEffect>,
    ) -> Vec<StorageEffect> {
        let mut forwarded = Vec::with_capacity(effects.len());
        for effect in effects {
            let Some(effect) = self.route_commit_read_effect(effect) else {
                continue;
            };
            let Some(pending) = self.pending_commit.as_ref() else {
                forwarded.push(effect);
                continue;
            };
            if effect.ticket == pending.begin_ticket {
                let StorageCompletion::VersioningBegin(result) = effect.completion else {
                    self.fail_pending_commit();
                    continue;
                };
                if result.is_err() {
                    self.fail_pending_commit();
                    continue;
                }
                let (operation, profile_uuid, manifest) = {
                    let pending = self.pending_commit.as_ref().expect("matching begin ticket");
                    (
                        pending.operation,
                        pending.manifest.profile_uuid,
                        pending.manifest.clone(),
                    )
                };
                match self
                    .storage
                    .submit(StorageCommand::VersioningStructuralPublish {
                        operation,
                        profile_uuid,
                        manifest,
                    }) {
                    Ok(ticket) => {
                        if let Some(pending) = self.pending_commit.as_mut() {
                            pending.publish_ticket = Some(ticket);
                        }
                    }
                    Err(_) => self.fail_pending_commit(),
                }
                continue;
            }
            if Some(effect.ticket) == pending.publish_ticket {
                let success = matches!(
                    effect.completion,
                    StorageCompletion::VersioningStructuralPublish(Ok(()))
                );
                let pending = self.pending_commit.take().expect("matching publish ticket");
                self.note_clean_branch_fork_commit(pending.operation, pending.commit, success);
                self.commit_events.push_back(if success {
                    CommitEvent::Durable {
                        operation: pending.operation,
                        commit: pending.commit,
                    }
                } else {
                    CommitEvent::Failed {
                        operation: pending.operation,
                        commit: pending.commit,
                    }
                });
                continue;
            }
            forwarded.push(effect);
        }
        forwarded
    }

    fn route_commit_read_effect(&mut self, effect: StorageEffect) -> Option<StorageEffect> {
        let Some(kind) = self.pending_commit_reads.remove(&effect.ticket) else {
            return Some(effect);
        };
        match kind {
            CommitReadKind::Diff(request_id, side) => {
                self.route_commit_diff_completion(request_id, side, effect.completion);
            }
            CommitReadKind::Ignore => {}
            _ => {
                let event = self.commit_read_event(effect.ticket.value(), kind, effect.completion);
                self.commit_read_events.push_back(event);
            }
        }
        None
    }

    fn commit_read_event(
        &mut self,
        request_id: u64,
        kind: CommitReadKind,
        completion: StorageCompletion,
    ) -> CommitReadEvent {
        let owner_matches = match kind {
            CommitReadKind::List {
                window,
                profile_uuid,
                native,
                ..
            }
            | CommitReadKind::Info {
                window,
                profile_uuid,
                native,
            }
            | CommitReadKind::DeletePreview {
                window,
                profile_uuid,
                native,
            }
            | CommitReadKind::Delete {
                window,
                profile_uuid,
                native,
            } => self.structural_commit_owner(window).is_ok_and(
                |(current_profile, current_native)| {
                    current_profile == profile_uuid && current_native == native
                },
            ),
            CommitReadKind::Diff(_, _) | CommitReadKind::Ignore => true,
        };
        if !owner_matches {
            return CommitReadEvent::Failed { request_id };
        }
        match (kind, completion) {
            (
                CommitReadKind::List { offset, limit, .. },
                StorageCompletion::VersioningList(Ok(mut commits)),
            ) => {
                let has_next = commits.len() > limit;
                commits.truncate(limit);
                CommitReadEvent::Listed {
                    request_id,
                    offset,
                    limit,
                    has_next,
                    commits,
                }
            }
            (CommitReadKind::Info { .. }, StorageCompletion::VersioningInfo(Ok(commit))) => {
                CommitReadEvent::Info { request_id, commit }
            }
            (
                CommitReadKind::DeletePreview {
                    window,
                    profile_uuid,
                    native,
                },
                StorageCompletion::VersioningDeletePreview(Ok(preview)),
            ) => {
                self.commit_delete_preview_event(request_id, window, profile_uuid, native, preview)
            }
            (CommitReadKind::Delete { .. }, StorageCompletion::VersioningDelete(Ok(receipt))) => {
                CommitReadEvent::Deleted {
                    request_id,
                    receipt,
                }
            }
            _ => CommitReadEvent::Failed { request_id },
        }
    }

    fn commit_delete_preview_event(
        &mut self,
        request_id: u64,
        window: WindowId,
        profile_uuid: Uuid,
        expected_native: NativeProfileOwner,
        preview: Option<CommitDeletePreview>,
    ) -> CommitReadEvent {
        let Some(preview) = preview else {
            return CommitReadEvent::DeletePreview {
                request_id,
                preview: None,
            };
        };
        if self.pending_commit_deletions.len() >= 16 {
            return CommitReadEvent::Failed { request_id };
        }
        let Ok((current_profile, native)) = self.structural_commit_owner(window) else {
            return CommitReadEvent::Failed { request_id };
        };
        if current_profile != profile_uuid || native != expected_native {
            return CommitReadEvent::Failed { request_id };
        }
        self.pending_commit_deletions.insert(
            request_id,
            PendingCommitDelete {
                window,
                profile_uuid,
                native,
                preview: preview.clone(),
                created_at: Instant::now(),
            },
        );
        CommitReadEvent::DeletePreview {
            request_id,
            preview: Some(preview),
        }
    }

    fn fail_pending_commit(&mut self) {
        if let Some(pending) = self.pending_commit.take() {
            if let Ok(ticket) = self.storage.submit(StorageCommand::VersioningAbort {
                operation: pending.operation,
                profile_uuid: pending.manifest.profile_uuid,
            }) {
                self.pending_commit_reads
                    .insert(ticket, CommitReadKind::Ignore);
            }
            self.note_clean_branch_fork_commit(pending.operation, pending.commit, false);
            self.commit_events.push_back(CommitEvent::Failed {
                operation: pending.operation,
                commit: pending.commit,
            });
        }
    }

    /// Returns terminal outcomes; no event is emitted for a mere reservation.
    #[must_use]
    pub fn poll_commit_events(&mut self) -> Vec<CommitEvent> {
        self.commit_events.drain(..).collect()
    }

    /// Returns bounded read results correlated with their accepted tickets.
    #[must_use]
    pub fn poll_commit_read_events(&mut self) -> Vec<CommitReadEvent> {
        self.commit_read_events.drain(..).collect()
    }

    pub(crate) fn structural_commit_owner(
        &self,
        window: WindowId,
    ) -> Result<(Uuid, NativeProfileOwner), VersioningFailure> {
        let profile = self
            .windows()
            .get(&window)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?
            .profile;
        if self
            .profiles()
            .get(&profile)
            .is_none_or(|profile| profile.privacy != PrivacyKind::Normal)
        {
            return Err(failure(VersioningReason::PrivatePersistenceDenied));
        }
        if self.active_profile_runtime != Some(profile) {
            return Err(failure(VersioningReason::WrongProfile));
        }
        let active = self
            .profile_snapshot()
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if active.privacy != PrivacyKind::Normal || !active.durable {
            return Err(failure(VersioningReason::PrivatePersistenceDenied));
        }
        let profile_uuid = active
            .id
            .ok_or_else(|| failure(VersioningReason::PrivatePersistenceDenied))?;
        if !self.storage_is_open() {
            return Err(failure(VersioningReason::VersioningDisabled));
        }
        let native = self
            .versioning
            .owner(profile)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        Ok((profile_uuid, native))
    }
}

pub(crate) fn valid_commit_message(message: &str) -> bool {
    !message.is_empty() && message.len() <= 1_024 && !message.chars().any(char::is_control)
}

fn fresh_unclaimed_logical_window(
    owners: &mut BTreeMap<LogicalWindowId, WindowId>,
    window: WindowId,
) -> LogicalWindowId {
    loop {
        let logical = LogicalWindowId::fresh();
        if let std::collections::btree_map::Entry::Vacant(entry) = owners.entry(logical) {
            entry.insert(window);
            return logical;
        }
    }
}

fn fresh_unclaimed_logical_page(
    owners: &mut BTreeMap<LogicalPageId, TabId>,
    tab: TabId,
) -> LogicalPageId {
    loop {
        let logical = LogicalPageId::fresh();
        if let std::collections::btree_map::Entry::Vacant(entry) = owners.entry(logical) {
            entry.insert(tab);
            return logical;
        }
    }
}

fn structural_scope_for_selector(selector: ScopeSelector) -> Option<(WindowId, ManifestScope)> {
    match selector {
        ScopeSelector::Tab { window, .. } => Some((window, ManifestScope::Tab)),
        ScopeSelector::Window { window } => Some((window, ManifestScope::Window)),
        ScopeSelector::Context { .. } => None,
    }
}

fn unique_context_for_source<'a>(
    contexts: &'a [ContextRecord],
    profile_name: &str,
    source_id: &str,
) -> Option<&'a ContextRecord> {
    let mut matching = contexts.iter().filter(|context| {
        context.profile == profile_name
            && context
                .members
                .iter()
                .any(|member| member.window_id == source_id)
    });
    let context = matching.next()?;
    matching.next().is_none().then_some(context)
}

fn context_member_refusal(
    owner: &BrowserApplication,
    window: WindowId,
    member: &ContextMember,
    context_name: &str,
    profile_uuid: Uuid,
) -> Option<InvalidContextMemberReason> {
    let Some(state) = owner.windows().get(&window) else {
        return Some(InvalidContextMemberReason::ClosedWindow);
    };
    if !owner
        .structural_commit_owner(window)
        .is_ok_and(|(id, _)| id == profile_uuid)
    {
        return Some(InvalidContextMemberReason::WrongProfile);
    }
    if state.context.as_deref() != Some(context_name) {
        return Some(InvalidContextMemberReason::ChangedMembership);
    }
    if state.tabs.is_empty() {
        return Some(InvalidContextMemberReason::NoLivePages);
    }
    let Some(tabs) = member
        .tabs
        .iter()
        .map(|value| TabId::from_display(value).filter(|tab| tab.to_string() == value.as_str()))
        .collect::<Option<Vec<_>>>()
    else {
        return Some(InvalidContextMemberReason::ChangedMembership);
    };
    if tabs != state.tabs
        || tabs.iter().any(|tab| {
            owner.tabs().get(tab).is_none_or(|state| {
                state.window != window
                    || state.profile != owner.windows()[&window].profile
                    || state.existence != ExistenceState::Live
            })
        })
    {
        return Some(InvalidContextMemberReason::ChangedMembership);
    }
    None
}

fn failure(reason: VersioningReason) -> VersioningFailure {
    VersioningFailure::new(reason, SensitiveCategory::None)
}

pub(crate) fn structural_header(
    commit: CommitId,
    profile_uuid: Uuid,
    message: String,
    configuration_revision: u64,
    scope: ManifestScope,
    capture_started_ms: u64,
) -> Result<ManifestHeader, VersioningFailure> {
    let now = unix_millis()?;
    Ok(ManifestHeader {
        id: commit,
        profile_uuid,
        created_at_ms: now,
        message,
        parent_commit: None,
        configuration_revision,
        scope,
        capture_interval: CaptureInterval {
            started_at_ms: capture_started_ms,
            finished_at_ms: now,
        },
        engine_fingerprint: "unavailable".into(),
        helper_bundle_version: "none".into(),
    })
}

pub(crate) fn unix_millis() -> Result<u64, VersioningFailure> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| failure(VersioningReason::UnqualifiedRuntime))?
        .as_millis();
    u64::try_from(now).map_err(|_| failure(VersioningReason::UnqualifiedRuntime))
}

#[cfg(test)]
mod tests {
    use super::super::PageCaptureProvenance;
    use super::*;
    use crate::versioning::{OperationState, VersioningCoordinator};
    use crate::{ContextCommand, ProfileActivation};
    use ferric_browser_config::Config;
    use ferric_browser_core::{
        CommandInvocation, CommandSource, DispatchTarget, EngineEffect, NavigationContext,
        ParsedCommand, ProfileInstanceId, ValidatedUrl, VersioningEngineEffect, WindowId,
    };
    use ferric_browser_runtime::{BrowserRuntime, RuntimeCommand, RuntimeDispatch, RuntimeEffect};
    use ferric_browser_storage::{
        ContextMember, ContextRegistry, RootSpec, StorageRoots,
        versioning::{ManifestMutationEvidence, ManifestPageCapture},
    };
    use std::{
        cell::Cell,
        fs,
        path::{Path, PathBuf},
        rc::Rc,
        thread,
        time::{Duration, Instant},
    };
    use uuid::Uuid;

    #[test]
    fn versioning_events_debug_redact_durable_and_request_identity() {
        let commit = CommitId::fresh();
        let commit_text = commit.to_string();
        let read = CommitReadEvent::Listed {
            request_id: 987_654_321,
            offset: 0,
            limit: 1,
            has_next: false,
            commits: vec![CommitSummary {
                id: commit,
                created_at_ms: 1,
                pinned: false,
                object_links: 0,
            }],
        };
        let commit_event = CommitEvent::Durable {
            operation: OperationId::fresh(),
            commit,
        };
        for debug in [format!("{read:?}"), format!("{commit_event:?}")] {
            assert!(!debug.contains(commit_text.as_str()));
            assert!(!debug.contains("987654321"));
        }
    }

    fn durable_application() -> (BrowserApplication, StorageRoots, Uuid, WindowId, TabId) {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        let uuid = Uuid::new_v4();
        let directory = roots.data.join("profiles").join(uuid.to_string());
        fs::create_dir_all(&directory).expect("profile directory");
        let path = directory.join("browser.sqlite");
        let (mut application, window, tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("normal runtime");
        application
            .activate_profile(ProfileActivation {
                name: "default".into(),
                id: Some(uuid),
                privacy: PrivacyKind::Normal,
                roots: Some(roots.clone()),
                lock: None,
                contexts: None,
                storage_path: Some(path),
            })
            .expect("durable profile");
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("native owner");
        (application, roots, uuid, window, tab)
    }

    fn durable_process_unique_application()
    -> (BrowserApplication, StorageRoots, Uuid, WindowId, TabId) {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        let uuid = Uuid::new_v4();
        let path = roots
            .data
            .join("profiles")
            .join(uuid.to_string())
            .join("browser.sqlite");
        fs::create_dir_all(path.parent().expect("profile directory")).expect("profile directory");
        let (runtime, window, tab) =
            BrowserRuntime::bootstrap_process_unique(PrivacyKind::Normal, "default")
                .expect("normal process-unique runtime");
        let mut application = BrowserApplication::from_runtime(runtime, Config::default());
        application
            .activate_profile(ProfileActivation {
                name: "default".into(),
                id: Some(uuid),
                privacy: PrivacyKind::Normal,
                roots: Some(roots.clone()),
                lock: None,
                contexts: None,
                storage_path: Some(path),
            })
            .expect("durable profile");
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("native owner");
        (application, roots, uuid, window, tab)
    }

    fn stored_archive_object_path(
        roots: &StorageRoots,
        profile_uuid: Uuid,
        object: &ferric_browser_storage::versioning::InstalledObject,
    ) -> PathBuf {
        let digest = object.digest_hex();
        roots
            .state
            .join("versioning")
            .join(profile_uuid.hyphenated().to_string())
            .join("objects/sha256")
            .join(&digest[..2])
            .join(&digest[2..])
    }

    fn install_context_registry(application: &mut BrowserApplication, roots: &StorageRoots) {
        application
            .active_profile
            .as_mut()
            .expect("durable active profile")
            .contexts = Some(ContextRegistry::open(roots).expect("context registry"));
    }

    fn context_member(window: WindowId, tabs: Vec<String>) -> ContextMember {
        ContextMember {
            window_id: window.to_string(),
            tabs,
            selected_tab: None,
            tab_descriptors: Vec::new(),
            selected_index: None,
        }
    }

    fn wait_for_read(application: &mut BrowserApplication, request_id: u64) -> CommitReadEvent {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if let Some(event) = application
                .poll_commit_read_events()
                .into_iter()
                .find(|event| match event {
                    CommitReadEvent::Listed {
                        request_id: event_id,
                        ..
                    }
                    | CommitReadEvent::Info {
                        request_id: event_id,
                        ..
                    }
                    | CommitReadEvent::DeletePreview {
                        request_id: event_id,
                        ..
                    }
                    | CommitReadEvent::Deleted {
                        request_id: event_id,
                        ..
                    }
                    | CommitReadEvent::Failed {
                        request_id: event_id,
                    }
                    | CommitReadEvent::Diff(
                        super::CommitDiffEvent::Compared {
                            request_id: event_id,
                            ..
                        }
                        | super::CommitDiffEvent::NotFound {
                            request_id: event_id,
                        }
                        | super::CommitDiffEvent::Failed {
                            request_id: event_id,
                        },
                    ) => *event_id == request_id,
                })
            {
                return event;
            }
            assert!(Instant::now() < deadline, "storage ticket did not complete");
            thread::yield_now();
        }
    }

    fn wait_for_storage_completion(
        application: &mut BrowserApplication,
        ticket: StorageTicket,
    ) -> StorageCompletion {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(completion) = application
                .poll_storage_effects()
                .into_iter()
                .find_map(|effect| (effect.ticket == ticket).then_some(effect.completion))
            {
                return completion;
            }
            assert!(Instant::now() < deadline, "storage ticket did not complete");
            thread::yield_now();
        }
    }

    fn wait_for_policy_restore(
        application: &mut BrowserApplication,
        request_id: u64,
    ) -> crate::versioning::PolicyRestoreEvent {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if let Some(event) =
                application
                    .poll_policy_restore_events()
                    .into_iter()
                    .find(|event| match event {
                        crate::versioning::PolicyRestoreEvent::PreviewReady {
                            request_id: event_id,
                            ..
                        }
                        | crate::versioning::PolicyRestoreEvent::Failed {
                            request_id: event_id,
                        } => *event_id == request_id,
                    })
            {
                return event;
            }
            assert!(Instant::now() < deadline, "policy preview did not complete");
            thread::yield_now();
        }
    }

    fn stored_manifest(database: &Path, commit: CommitId) -> VersionedCaptureManifest {
        let connection = rusqlite::Connection::open(database).expect("inspect disposable profile");
        let bytes: Vec<u8> = connection
            .query_row(
                "SELECT manifest_bytes FROM vb_commits WHERE id=?1",
                [commit.to_string()],
                |row| row.get(0),
            )
            .expect("stored immutable manifest");
        VersionedCaptureManifest::decode(&bytes).expect("valid stored manifest")
    }

    fn dispatch_versioning_command(
        application: &mut BrowserApplication,
        window: WindowId,
        name: &str,
        arguments: &[&str],
        source: CommandSource,
    ) -> Result<Vec<RuntimeEffect>, ferric_browser_runtime::RuntimeError> {
        let context =
            application.capture_command_context(DispatchTarget::Window(window), source, 1, None);
        application.dispatch_runtime(RuntimeInput::Command(RuntimeCommand::Dispatch(Box::new(
            RuntimeDispatch {
                invocation: CommandInvocation::new(ParsedCommand {
                    name: name.into(),
                    arguments: arguments
                        .iter()
                        .map(|argument| (*argument).into())
                        .collect(),
                })
                .with_context(context),
                navigation: NavigationContext::default(),
                target: DispatchTarget::Window(window),
            },
        ))))
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn context_commit_aggregates_exact_live_profile_members_without_layout_data() {
        let (mut application, roots, _, source_window, source_tab) = durable_application();
        install_context_registry(&mut application, &roots);
        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("second same-profile window");
        let second_window = application
            .runtime_view()
            .active_window()
            .expect("active window");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: second_window,
            }))
            .expect("second window tab");
        let second_tab = application.windows()[&second_window]
            .active_tab
            .expect("second live tab");
        let closed_window = WindowId::from_display("windowid-18446744073709551615")
            .expect("synthetic closed window ID");

        application
            .change_context(ContextCommand::Create {
                name: "research".into(),
                label: "Research".into(),
                profile: "default".into(),
                workspace: Some("do-not-serialize-workspace".into()),
            })
            .expect("context creation");
        for window in [source_window, second_window] {
            application
                .dispatch_runtime(RuntimeInput::EngineFact(Event::SetWindowContext {
                    window,
                    context: Some("research".into()),
                }))
                .expect("context-bind live window");
        }
        application
            .change_context(ContextCommand::SaveMembership {
                name: "research".into(),
                members: vec![
                    context_member(source_window, vec![source_tab.to_string()]),
                    context_member(second_window, vec![second_tab.to_string()]),
                    context_member(closed_window, Vec::new()),
                ],
            })
            .expect("context membership");

        let previous_revision = application.revision();
        let prepared = application
            .prepare_structural_commit(
                ScopeSelector::Context {
                    window: source_window,
                },
                "context checkpoint".into(),
            )
            .expect("exact context capture");
        let manifest = prepared.manifest;
        assert_eq!(manifest.scope, ManifestScope::Context);
        assert_eq!(
            manifest.application_revision,
            previous_revision
                .checked_add(2)
                .expect("two context members must leave room for identity binding")
        );
        assert_eq!(application.revision(), manifest.application_revision);
        assert_eq!(
            prepared
                .runtime_effects
                .iter()
                .filter(|effect| matches!(
                    effect,
                    RuntimeEffect::AppliedEvent(Event::Versioning(
                        VersioningEvent::BindStructuralIdentity { .. }
                    ))
                ))
                .count(),
            2,
            "both context identity bindings are emitted after projection"
        );
        assert!(
            application
                .runtime_view()
                .logical_window(source_window)
                .is_some()
        );
        assert!(
            application
                .runtime_view()
                .logical_window(second_window)
                .is_some()
        );
        assert!(
            application
                .runtime_view()
                .logical_page(source_tab)
                .is_some()
        );
        assert!(
            application
                .runtime_view()
                .logical_page(second_tab)
                .is_some()
        );
        assert_eq!(manifest.context_member_count, Some(3));
        assert_eq!(manifest.windows.len(), 2);
        assert!(
            manifest
                .windows
                .iter()
                .flat_map(|window| &window.pages)
                .all(|page| page.archive_object.is_none()
                    && page.sidecar_object.is_none()
                    && page.page_capture.is_none()),
            "structural context commits must never include page archives or sidecars"
        );
        assert_eq!(
            manifest
                .windows
                .iter()
                .map(|window| window.context_member_ordinal)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1)]
        );
        assert!(
            manifest
                .windows
                .iter()
                .all(|window| window.capture_interval.is_some())
        );
        assert_eq!(
            manifest.invalid_context_members,
            vec![InvalidContextMember {
                ordinal: 2,
                reason: InvalidContextMemberReason::ClosedWindow,
            }]
        );
        let encoded = serde_json::to_string(&manifest).expect("serialize bounded context DTO");
        assert!(!encoded.contains(&source_window.to_string()));
        assert!(!encoded.contains(&second_window.to_string()));
        assert!(!encoded.contains(&source_tab.to_string()));
        assert!(!encoded.contains(&second_tab.to_string()));
        assert!(!encoded.contains("do-not-serialize-workspace"));
        application.context_capture_owners.clear();
        assert!(
            application
                .prepare_structural_commit(
                    ScopeSelector::Context {
                        window: source_window,
                    },
                    "stale context checkpoint".into(),
                )
                .is_err(),
            "persisted runtime IDs alone are not current-session authority"
        );

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn context_capture_ref_requires_the_current_saved_native_owner() {
        let (mut application, roots, _, window, tab) = durable_application();
        install_context_registry(&mut application, &roots);
        let context = application
            .change_context(ContextCommand::Create {
                name: "research".into(),
                label: "Research".into(),
                profile: "default".into(),
                workspace: None,
            })
            .expect("context creation")
            .context;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::SetWindowContext {
                window,
                context: Some("research".into()),
            }))
            .expect("exact context binding");
        assert!(
            application
                .context_membership_owner(window, "research")
                .is_ok(),
            "live native membership capture does not require an earlier save"
        );
        assert!(
            application
                .context_membership_owner(window, "other")
                .is_err()
        );
        assert!(
            application
                .active_context_capture_ref(window, "research")
                .is_err(),
            "unsaved runtime membership is not capture authority"
        );
        assert!(
            application
                .active_context_capture_members(window, "research")
                .is_err(),
            "unsaved members must not be offered for routing"
        );
        application
            .change_context(ContextCommand::SaveMembership {
                name: "research".into(),
                members: vec![context_member(window, vec![tab.to_string()])],
            })
            .expect("saved membership");
        assert_eq!(
            application
                .active_context_capture_ref(window, "research")
                .expect("current owner routing ID"),
            context.id.to_string()
        );
        assert_eq!(
            application
                .active_context_capture_members(window, "research")
                .expect("saved member routing order"),
            vec![window]
        );
        assert!(
            application
                .active_context_capture_ref(window, "other")
                .is_err()
        );
        application.context_capture_owners.clear();
        assert!(
            application
                .active_context_capture_ref(window, "research")
                .is_err(),
            "persisted registry IDs alone cannot revive old native authority"
        );
        assert!(
            application
                .active_context_capture_members(window, "research")
                .is_err(),
            "a stale native owner cannot route saved members"
        );
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn context_commit_aggregates_separate_application_owners() {
        let (mut source, roots, profile_uuid, source_window, source_tab) =
            durable_process_unique_application();
        install_context_registry(&mut source, &roots);
        let (peer_runtime, peer_window, peer_tab) =
            BrowserRuntime::bootstrap_process_unique(PrivacyKind::Normal, "default")
                .expect("second normal runtime");
        let mut peer = BrowserApplication::from_runtime(peer_runtime, Config::default());
        peer.activate_profile(ProfileActivation {
            name: "default".into(),
            id: Some(profile_uuid),
            privacy: PrivacyKind::Normal,
            roots: Some(roots.clone()),
            lock: None,
            contexts: None,
            storage_path: Some(
                roots
                    .data
                    .join("profiles")
                    .join(profile_uuid.to_string())
                    .join("browser.sqlite"),
            ),
        })
        .expect("same durable profile");
        let peer_profile = peer.windows()[&peer_window].profile;
        peer.bind_native_profile(peer_profile, ProfileInstanceId::fresh())
            .expect("independent native owner");
        source
            .change_context(ContextCommand::Create {
                name: "research".into(),
                label: "Research".into(),
                profile: "default".into(),
                workspace: None,
            })
            .expect("context creation");
        for (application, window) in [(&mut source, source_window), (&mut peer, peer_window)] {
            application
                .dispatch_runtime(RuntimeInput::EngineFact(Event::SetWindowContext {
                    window,
                    context: Some("research".into()),
                }))
                .expect("context-bind exact owner");
        }
        source
            .change_context(ContextCommand::SaveMembership {
                name: "research".into(),
                members: vec![
                    context_member(source_window, vec![source_tab.to_string()]),
                    context_member(peer_window, vec![peer_tab.to_string()]),
                ],
            })
            .expect("saved cross-owner membership");
        assert_eq!(
            source
                .active_context_capture_members(source_window, "research")
                .expect("source-owned saved member order"),
            vec![source_window, peer_window]
        );
        let source_revision = source.revision();
        let peer_revision = peer.revision();
        let (prepared, peer_effects) = source
            .prepare_federated_context_structural_commit(
                source_window,
                "two native owners".into(),
                &mut [&mut peer],
            )
            .expect("federated context preflight");
        assert_eq!(prepared.manifest.scope, ManifestScope::Context);
        assert_eq!(prepared.manifest.windows.len(), 2);
        assert_eq!(prepared.manifest.context_member_count, Some(2));
        assert!(prepared.manifest.invalid_context_members.is_empty());
        assert_eq!(source.revision(), source_revision + 1);
        assert_eq!(peer.revision(), peer_revision + 1);
        assert_eq!(prepared.manifest.application_revision, source.revision());
        assert_eq!(peer_effects.len(), 1);
        assert_eq!(peer_effects[0].window, peer_window);
        assert!(source.runtime_view().logical_page(source_tab).is_some());
        assert!(peer.runtime_view().logical_page(peer_tab).is_some());
        assert_eq!(
            prepared
                .manifest
                .windows
                .iter()
                .map(|window| window.context_member_ordinal)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1)]
        );
        assert!(
            prepared
                .manifest
                .windows
                .iter()
                .all(|window| window.capture_interval.is_some())
        );
        let (operation, commit, source_effects, peer_effects) = source
            .begin_federated_context_structural_commit(
                source_window,
                "durable two-owner context".into(),
                &mut [&mut peer],
            )
            .expect("reserve federated context commit");
        assert!(!source_effects.is_empty());
        assert_eq!(peer_effects.len(), 1);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = source.poll_storage_effects();
            let outcomes = source.poll_commit_events();
            if !outcomes.is_empty() {
                assert_eq!(outcomes, vec![CommitEvent::Durable { operation, commit }]);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "federated commit did not publish"
            );
            thread::yield_now();
        }
        let database = roots
            .data
            .join("profiles")
            .join(profile_uuid.to_string())
            .join("browser.sqlite");
        let stored = stored_manifest(&database, commit);
        assert_eq!(stored.scope, ManifestScope::Context);
        assert_eq!(stored.windows.len(), 2);
        assert_eq!(stored.context_member_count, Some(2));
        let context_ref = source
            .contexts()
            .expect("source registry")
            .iter()
            .find(|context| context.name == "research")
            .expect("saved context")
            .id
            .to_string();
        let source_candidate = source
            .prepare_context_window_candidate(source_window, "research", &context_ref, 0)
            .expect("source-issued candidate");
        let peer_candidate = peer
            .prepare_context_window_candidate(peer_window, "research", &context_ref, 1)
            .expect("peer-issued candidate");
        let candidates = [source_candidate, peer_candidate];
        let closed_peer_manifest = source
            .prepare_context_manifest_from_candidates(
                source_window,
                "closed peer candidate".into(),
                &candidates[..1],
            )
            .expect("missing peer is reported without widening capture");
        assert_eq!(closed_peer_manifest.windows.len(), 1);
        assert_eq!(
            closed_peer_manifest.invalid_context_members,
            vec![InvalidContextMember {
                ordinal: 1,
                reason: InvalidContextMemberReason::ClosedWindow,
            }]
        );
        let candidate_manifest = source
            .prepare_context_manifest_from_candidates(
                source_window,
                "detached owner candidates".into(),
                &candidates,
            )
            .expect("source validates exact owner-issued candidates");
        assert_eq!(candidate_manifest.windows.len(), 2);
        assert_eq!(candidate_manifest.scope, ManifestScope::Context);
        let candidate_plan = source
            .prepare_context_candidate_commit(
                source_window,
                "durable detached candidates".into(),
                &candidates,
            )
            .expect("source retains exact candidate plan");
        assert_eq!(candidate_plan.complete_member_count(), Some(2));
        let [source_candidate, peer_candidate] = candidates;
        assert!(
            !peer
                .bind_context_window_candidate(peer_candidate)
                .expect("peer accepts its own current candidate")
                .is_empty()
        );
        assert!(
            !source
                .bind_context_window_candidate(source_candidate)
                .expect("source accepts its own current candidate")
                .is_empty()
        );
        let (candidate_operation, candidate_commit) = source
            .begin_prepared_context_candidate_commit(candidate_plan)
            .expect("publish after exact owner bindings");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = source.poll_storage_effects();
            let outcomes = source.poll_commit_events();
            if !outcomes.is_empty() {
                assert_eq!(
                    outcomes,
                    vec![CommitEvent::Durable {
                        operation: candidate_operation,
                        commit: candidate_commit,
                    }]
                );
                break;
            }
            assert!(
                Instant::now() < deadline,
                "candidate commit did not publish"
            );
            thread::yield_now();
        }
        assert_eq!(
            stored_manifest(&database, candidate_commit).windows.len(),
            2
        );
        let stale_peer_candidate = peer
            .prepare_context_window_candidate(peer_window, "research", &context_ref, 1)
            .expect("fresh peer candidate before later tab change");
        peer.dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
            window: peer_window,
        }))
        .expect("change peer membership after candidate capture");
        let changed_revision = peer.revision();
        let failure = peer
            .bind_context_window_candidate(stale_peer_candidate)
            .expect_err("stale peer candidate cannot bind identity");
        assert_eq!(failure.reason(), VersioningReason::StaleOwner);
        assert_eq!(peer.revision(), changed_revision);
        source.close_profile();
        peer.close_profile();
        drop(source);
        drop(peer);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn federated_context_preflight_never_binds_stale_or_wrong_profile_peer() {
        let (mut source, roots, _, source_window, source_tab) =
            durable_process_unique_application();
        install_context_registry(&mut source, &roots);
        let (mut peer, peer_roots, peer_uuid, _, _) = durable_application();
        let (peer_runtime, peer_window, peer_tab) =
            BrowserRuntime::bootstrap_process_unique(PrivacyKind::Normal, "default")
                .expect("distinct peer runtime identities");
        peer.close_profile();
        peer = BrowserApplication::from_runtime(peer_runtime, Config::default());
        peer.activate_profile(ProfileActivation {
            name: "default".into(),
            id: Some(peer_uuid),
            privacy: PrivacyKind::Normal,
            roots: Some(peer_roots.clone()),
            lock: None,
            contexts: None,
            storage_path: Some(
                peer_roots
                    .data
                    .join("profiles")
                    .join(peer_uuid.to_string())
                    .join("browser.sqlite"),
            ),
        })
        .expect("other durable profile");
        let peer_profile = peer.windows()[&peer_window].profile;
        peer.bind_native_profile(peer_profile, ProfileInstanceId::fresh())
            .expect("other native owner");
        source
            .change_context(ContextCommand::Create {
                name: "research".into(),
                label: "Research".into(),
                profile: "default".into(),
                workspace: None,
            })
            .expect("context creation");
        for (application, window) in [(&mut source, source_window), (&mut peer, peer_window)] {
            application
                .dispatch_runtime(RuntimeInput::EngineFact(Event::SetWindowContext {
                    window,
                    context: Some("research".into()),
                }))
                .expect("context-bind exact owner");
        }
        source
            .change_context(ContextCommand::SaveMembership {
                name: "research".into(),
                members: vec![
                    context_member(source_window, vec![source_tab.to_string()]),
                    context_member(peer_window, vec![peer_tab.to_string()]),
                ],
            })
            .expect("saved foreign-profile member");
        let peer_revision = peer.revision();
        let (prepared, peer_effects) = source
            .prepare_federated_context_structural_commit(
                source_window,
                "wrong profile excluded".into(),
                &mut [&mut peer],
            )
            .expect("valid source still capturable");
        assert!(peer_effects.is_empty());
        assert_eq!(peer.revision(), peer_revision);
        assert_eq!(prepared.manifest.windows.len(), 1);
        assert_eq!(
            prepared.manifest.invalid_context_members,
            vec![InvalidContextMember {
                ordinal: 1,
                reason: InvalidContextMemberReason::WrongProfile,
            }]
        );
        source.close_profile();
        peer.close_profile();
        drop(source);
        drop(peer);
        roots.cleanup().expect("temporary roots cleanup");
        peer_roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn context_commit_records_changed_and_wrong_profile_members() {
        let (mut application, roots, _, source_window, source_tab) = durable_application();
        install_context_registry(&mut application, &roots);
        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("second same-profile window");
        let changed_window = application
            .runtime_view()
            .active_window()
            .expect("active window");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: changed_window,
            }))
            .expect("changed-member tab");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateProfile {
                label: "other".into(),
                privacy: PrivacyKind::Normal,
            }))
            .expect("second runtime profile");
        let other_profile = *application
            .profiles()
            .keys()
            .next_back()
            .expect("other profile");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow {
                profile: other_profile,
            }))
            .expect("other-profile window");
        let other_window = application
            .runtime_view()
            .active_window()
            .expect("other window");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: other_window,
            }))
            .expect("other-profile tab");
        let other_tab = application.windows()[&other_window]
            .active_tab
            .expect("other-profile tab ID");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateProfile {
                label: "private branch".into(),
                privacy: PrivacyKind::Private,
            }))
            .expect("private branch profile");
        let private_profile = *application
            .profiles()
            .keys()
            .next_back()
            .expect("private profile");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow {
                profile: private_profile,
            }))
            .expect("private branch window");
        let private_window = application
            .runtime_view()
            .active_window()
            .expect("private branch window ID");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: private_window,
            }))
            .expect("private branch tab");
        let private_tab = application.windows()[&private_window]
            .active_tab
            .expect("private branch tab ID");
        let private_start = application
            .capture_target(private_tab)
            .expect("private branch navigation target");
        let private_url = ValidatedUrl::parse(
            "https://private-secret.example.test/?token=private-query-sentinel",
        )
        .expect("private branch URL");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: private_start,
                url: private_url.clone(),
            }))
            .expect("start private branch navigation");
        let private_committing = application
            .capture_target(private_tab)
            .expect("private branch committing target");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: private_committing,
                url: private_url,
                title: "private-title-sentinel".into(),
            }))
            .expect("commit private branch navigation");

        application
            .change_context(ContextCommand::Create {
                name: "research".into(),
                label: "Research".into(),
                profile: "default".into(),
                workspace: None,
            })
            .expect("context creation");
        for window in [source_window, changed_window] {
            application
                .dispatch_runtime(RuntimeInput::EngineFact(Event::SetWindowContext {
                    window,
                    context: Some("research".into()),
                }))
                .expect("context-bind current-profile window");
        }
        application
            .change_context(ContextCommand::SaveMembership {
                name: "research".into(),
                members: vec![
                    context_member(source_window, vec![source_tab.to_string()]),
                    context_member(changed_window, vec!["tab-18446744073709551615".into()]),
                    context_member(other_window, vec![other_tab.to_string()]),
                    context_member(private_window, vec![private_tab.to_string()]),
                ],
            })
            .expect("context membership");

        let prepared = application
            .prepare_structural_commit(
                ScopeSelector::Context {
                    window: source_window,
                },
                "context checkpoint".into(),
            )
            .expect("capture skips invalid context members");
        assert_eq!(prepared.manifest.windows.len(), 1);
        assert_eq!(
            prepared.manifest.invalid_context_members,
            vec![
                InvalidContextMember {
                    ordinal: 1,
                    reason: InvalidContextMemberReason::ChangedMembership,
                },
                InvalidContextMember {
                    ordinal: 2,
                    reason: InvalidContextMemberReason::WrongProfile,
                },
                InvalidContextMember {
                    ordinal: 3,
                    reason: InvalidContextMemberReason::WrongProfile,
                },
            ]
        );
        let encoded =
            serde_json::to_string(&prepared.manifest).expect("normal-profile context manifest");
        assert!(!encoded.contains("private-secret.example.test"));
        assert!(!encoded.contains("private-query-sentinel"));
        assert!(!encoded.contains("private-title-sentinel"));

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn transient_window_and_context_capture_are_rejected() {
        let (mut application, window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Private, "private")
                .expect("private runtime");
        let window_failure = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "private window".into())
            .expect_err("private window cannot create a durable structural commit");
        assert_eq!(
            window_failure.reason(),
            VersioningReason::PrivatePersistenceDenied
        );
        let context_failure = application
            .prepare_structural_commit(ScopeSelector::Context { window }, "private context".into())
            .expect_err("private context cannot create a durable structural commit");
        assert_eq!(
            context_failure.reason(),
            VersioningReason::PrivatePersistenceDenied
        );
        assert_eq!(
            application
                .context_membership_owner(window, "private")
                .expect_err("private membership cannot enter the native handoff")
                .reason(),
            VersioningReason::PrivatePersistenceDenied
        );
    }

    #[test]
    fn page_capture_consent_uses_fixed_disclosure_and_cancel_invalidates_it() {
        let (mut application, roots, _, window, _) = durable_application();
        let consent = application
            .prepare_page_capture_consent(ScopeSelector::Window { window }, false)
            .expect("normal owner can prepare a consent preview");
        let redacted = consent.preview.redacted();
        assert_eq!(redacted.kind, crate::versioning::PreviewKind::PageArchive);
        assert_eq!(redacted.counts.additions, 1);
        let disclosure = redacted
            .kind
            .page_capture_disclosure()
            .expect("fixed archive disclosure");
        assert!(disclosure.contains("embedded secrets"));
        assert!(disclosure.contains("bounded same-origin page-state metadata"));
        assert!(disclosure.contains("supported control state"));
        assert!(disclosure.contains("does not sanitize"));
        assert!(disclosure.contains("Saved archives may omit resources"));
        assert!(disclosure.contains("not guaranteed to be atomic snapshots"));
        assert!(disclosure.contains("not encrypted at rest"));
        assert!(!format!("{:?}", consent.preview).contains("https://"));

        let sink = application
            .take_native_versioning_decision_sink()
            .expect("one native decision authority");
        application
            .cancel_versioning_operation(consent.operation)
            .expect("declined consent cancels exact operation");
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .unwrap()
                .1,
            crate::versioning::OperationState::Cancelled
        );
        assert!(
            application
                .capture_native_versioning_decision(
                    consent.operation,
                    sink.accept(&consent.preview),
                )
                .is_err()
        );
        assert!(application.poll_commit_events().is_empty());

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn page_capture_consent_binds_editable_opt_in_and_rejects_changed_scope() {
        let (mut application, roots, _, window, _) = durable_application();
        let consent = application
            .prepare_page_capture_consent(ScopeSelector::Window { window }, true)
            .expect("explicit editable request prepares distinct preview");
        assert_eq!(
            consent.preview.redacted().kind,
            crate::versioning::PreviewKind::PageArchiveEditable
        );
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("native acceptance of exact preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("frozen scope receives one-use capture authority");
        assert!(authorization.editable());
        assert_eq!(authorization.scope().members().len(), 1);
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .unwrap()
                .1,
            crate::versioning::OperationState::Preparing
        );
        assert!(
            application
                .capture_native_versioning_decision(
                    consent.operation,
                    sink.accept(&consent.preview),
                )
                .is_err()
        );
        application
            .cancel_versioning_operation(consent.operation)
            .expect("unconsumed capture job can be cancelled");
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");

        let (mut stale, roots, _, window, _) = durable_application();
        let consent = stale
            .prepare_page_capture_consent(ScopeSelector::Window { window }, false)
            .expect("fresh exact window scope");
        let sink = stale
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = stale
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("preview is current before the scope changes");
        stale
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("change the captured scope revision");
        let changed = stale
            .confirm_page_capture_consent(consent.operation, permit)
            .expect_err("scope changes invalidate one-use consent");
        assert_eq!(changed.reason(), VersioningReason::DocumentChanged);
        assert!(stale.poll_commit_events().is_empty());
        stale.close_profile();
        drop(stale);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn page_capture_selection_is_a_separate_one_use_opt_in() {
        let (mut application, roots, _, window, _) = durable_application();
        let consent = application
            .prepare_page_capture_consent_with_options(
                ScopeSelector::Window { window },
                false,
                true,
            )
            .expect("selection-position request prepares its own preview");
        assert_eq!(
            consent.preview.redacted().kind,
            crate::versioning::PreviewKind::PageArchiveSelection
        );
        let disclosure = consent
            .preview
            .redacted()
            .kind
            .page_capture_disclosure()
            .expect("selection disclosure copy");
        assert!(disclosure.contains("selection position"));
        assert!(disclosure.contains("Selection text itself is not stored"));

        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("native acceptance of exact selection preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("selection flag remains bound to one-use authority");
        assert!(authorization.selection());
        assert!(!authorization.editable());
        application
            .cancel_versioning_operation(consent.operation)
            .expect("unconsumed capture job can be cancelled");
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One end-to-end test preserves the preview-to-partial-publication chain.
    fn partial_window_capture_preserves_frozen_identity_through_view_loss_and_publish() {
        use crate::versioning::PageCaptureFailurePolicy;

        let (mut application, roots, profile_uuid, window, tab) = durable_application();
        let single = application
            .prepare_page_capture_consent_with_failure_policy(
                ScopeSelector::Window { window },
                false,
                false,
                PageCaptureFailurePolicy::AllowPartialPages,
            )
            .expect_err("one page cannot opt into partial-window publication");
        assert_eq!(single.reason(), VersioningReason::UnsupportedCapability);
        let tab_target = application.capture_target(tab).expect("live tab target");
        let tab_scope = application
            .prepare_page_capture_consent_with_failure_policy(
                ScopeSelector::Tab {
                    window,
                    target: tab_target,
                },
                false,
                false,
                PageCaptureFailurePolicy::AllowPartialPages,
            )
            .expect_err("tab capture remains strict");
        assert_eq!(tab_scope.reason(), VersioningReason::UnsupportedCapability);

        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("second live tab");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("third live tab");
        let prepared = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "partial window".into())
            .expect("three-page structural manifest");
        let consent = application
            .prepare_page_capture_consent_with_failure_policy(
                ScopeSelector::Window { window },
                false,
                false,
                PageCaptureFailurePolicy::AllowPartialPages,
            )
            .expect("three-page window can request an explicit partial policy");
        let redacted = consent.preview.redacted();
        assert_eq!(
            redacted.kind,
            crate::versioning::PreviewKind::PageArchivePartial
        );
        assert_eq!(redacted.counts.additions, 3);
        let disclosure = redacted
            .kind
            .page_capture_disclosure()
            .expect("partial policy has fixed disclosure");
        assert!(disclosure.contains("may save the successful pages"));
        assert!(disclosure.contains("record the failure"));
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("accept exact partial disclosure");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use policy remains bound to three-page scope");
        assert_eq!(authorization.scope().members().len(), 3);
        assert_eq!(
            authorization.failure_policy(),
            PageCaptureFailurePolicy::AllowPartialPages
        );
        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("reserve every page before the first save");
        assert!(matches!(
            wait_for_storage_completion(&mut application, stage_ticket),
            StorageCompletion::VersioningCaptureStage(Ok(_))
        ));
        let failed_tab = authorization.scope().members()[0].target();
        let closed_saved_tab = authorization.scope().members()[1].target();
        application
            .record_window_page_capture_failure(
                &authorization,
                0,
                ferric_browser_storage::versioning::ManifestPageArchiveFailure::NativeViewLost,
            )
            .expect("partial consent admits a page-local view loss");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CloseTab {
                tab: failed_tab.tab,
            }))
            .expect("failed first tab begins closing");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::ViewClosed {
                tab: failed_tab.tab,
                generation: failed_tab.generation,
            }))
            .expect("failed first tab view is gone");
        assert!(
            application
                .runtime_view()
                .logical_page(failed_tab.tab)
                .is_none()
        );
        let next_ticket = application
            .request_window_page_capture_stage(&authorization, 1)
            .expect("failed first page does not prevent the next page");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) =
            wait_for_storage_completion(&mut application, next_ticket)
        else {
            panic!("second stage required");
        };
        fs::write(stage.path(), b"second archive").expect("second native save");
        let install_ticket = application
            .request_window_page_capture_object_install(&authorization, 1, None, None)
            .expect("install successful second page");
        assert!(matches!(
            wait_for_storage_completion(&mut application, install_ticket),
            StorageCompletion::VersioningObjectInstall(Ok(_))
        ));
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CloseTab {
                tab: closed_saved_tab.tab,
            }))
            .expect("saved second tab begins closing");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::ViewClosed {
                tab: closed_saved_tab.tab,
                generation: closed_saved_tab.generation,
            }))
            .expect("saved second page view is gone");
        assert!(
            application
                .runtime_view()
                .logical_page(closed_saved_tab.tab)
                .is_none()
        );
        let third_ticket = application
            .request_window_page_capture_stage(&authorization, 2)
            .expect("closed saved page does not prevent the final page");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) =
            wait_for_storage_completion(&mut application, third_ticket)
        else {
            panic!("third page stage required");
        };
        fs::write(stage.path(), b"third archive").expect("third native save");
        application
            .record_window_page_capture_failure(
                &authorization,
                2,
                ferric_browser_storage::versioning::ManifestPageArchiveFailure::Timeout,
            )
            .expect("a later page may fail after an earlier page was installed");
        assert_eq!(
            application.page_capture_archive_failures[&consent.operation][&0],
            ferric_browser_storage::versioning::ManifestPageArchiveFailure::NativeViewLost
        );
        assert_eq!(
            application.page_capture_archive_failures[&consent.operation][&2],
            ferric_browser_storage::versioning::ManifestPageArchiveFailure::Timeout
        );
        assert!(application.page_capture_installed_archives[&consent.operation].contains_key(&1));
        assert!(!application.page_capture_installed_archives[&consent.operation].contains_key(&2));
        let mut manifest = prepared.manifest;
        manifest.requested_capture.allow_partial_pages = true;
        let started_at_ms = manifest.capture_interval.started_at_ms;
        let page_capture = ManifestPageCapture {
            archive_interval: Some(CaptureInterval {
                started_at_ms,
                finished_at_ms: started_at_ms,
            }),
            sidecar_interval: None,
            archive_monotonic_interval: Some(
                ferric_browser_storage::versioning::CaptureMonotonicInterval {
                    started_offset_ms: 0,
                    finished_offset_ms: 0,
                },
            ),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        };
        for index in [0, 2] {
            let mut wrong_page = manifest.clone();
            wrong_page.windows[0].pages[index].logical_page_id = LogicalPageId::fresh().to_string();
            assert!(
                application
                    .request_window_page_capture_object_publish(
                        &authorization,
                        wrong_page,
                        vec![None, Some(page_capture.clone()), None],
                        PageCaptureProvenance::new(None, None),
                    )
                    .is_err(),
                "failed page {index} cannot be rebound to another logical page"
            );
        }
        let published_commit = manifest.id.parse::<CommitId>().expect("partial commit ID");
        let publish_ticket = application
            .request_window_page_capture_object_publish(
                &authorization,
                manifest,
                vec![None, Some(page_capture), None],
                PageCaptureProvenance::new(None, None),
            )
            .expect("one saved page and two failed pages may publish atomically");
        assert!(matches!(
            wait_for_storage_completion(&mut application, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        application
            .complete_versioning_operation(consent.operation)
            .expect("complete durable partial window capture");
        application.close_profile();
        drop(application);
        let database = roots
            .data
            .join("profiles")
            .join(profile_uuid.to_string())
            .join("browser.sqlite");
        let committed = stored_manifest(&database, published_commit);
        let pages = &committed.windows[0].pages;
        assert_eq!(
            pages[0].archive_failure,
            Some(ferric_browser_storage::versioning::ManifestPageArchiveFailure::NativeViewLost)
        );
        assert!(pages[0].archive_object.is_none() && pages[0].page_capture.is_none());
        assert!(pages[1].archive_failure.is_none());
        assert!(pages[1].archive_object.is_some());
        assert!(
            pages[1]
                .page_capture
                .as_ref()
                .is_some_and(|capture| capture.archive_interval.is_some())
        );
        assert_eq!(
            pages[2].archive_failure,
            Some(ferric_browser_storage::versioning::ManifestPageArchiveFailure::Timeout)
        );
        assert!(pages[2].archive_object.is_none() && pages[2].page_capture.is_none());
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One test follows the two-page staging and reservation boundary.
    fn window_page_capture_reserves_and_installs_each_bound_member() {
        let (mut application, roots, profile_uuid, window, _) = durable_application();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("second live tab");
        let prepared = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "strict window".into())
            .expect("two-page structural manifest");
        let consent = application
            .prepare_page_capture_consent(ScopeSelector::Window { window }, false)
            .expect("strict two-page window preview");
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("accept exact window preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use window authorization");
        assert_eq!(authorization.scope().members().len(), 2);
        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("reserve for the whole window before any native save");
        let StorageCompletion::VersioningCaptureStage(Ok(first_stage)) =
            wait_for_storage_completion(&mut application, stage_ticket)
        else {
            panic!("first page stage required");
        };
        let database = roots
            .data
            .join("profiles")
            .join(profile_uuid.to_string())
            .join("browser.sqlite");
        let connection = rusqlite::Connection::open(database).expect("inspect capture reservation");
        let marker: String = connection
            .query_row(
                "SELECT safe_recovery_metadata FROM vb_operations WHERE id=?1",
                [consent.operation.to_string()],
                |row| row.get(0),
            )
            .expect("durable reservation marker");
        assert_eq!(
            marker,
            format!(
                "capture-reserved-v1:{}",
                2 * ferric_browser_storage::versioning::MAX_CAPTURE_RESERVATION_BYTES
            )
        );
        assert!(
            application
                .request_window_page_capture_stage(&authorization, 1)
                .is_err(),
            "the next page cannot steal a still-ready target"
        );
        assert!(
            application
                .request_window_page_capture_object_install(&authorization, 1, Some(14), None)
                .is_err(),
            "page one cannot install from page zero's target"
        );
        fs::write(first_stage.path(), b"first archive").expect("first native save");
        let install_ticket = application
            .request_window_page_capture_object_install(&authorization, 0, Some(13), None)
            .expect("install authorized page zero");
        let StorageCompletion::VersioningObjectInstall(Ok(first_object)) =
            wait_for_storage_completion(&mut application, install_ticket)
        else {
            panic!("first archive object required");
        };
        assert!(
            application
                .request_window_page_capture_sidecar_install(
                    &authorization,
                    1,
                    Vec::new(),
                    "https://example.test",
                    "wrong-index",
                    "wrong-index",
                    "wrong-index",
                    false,
                )
                .is_err(),
            "a page cannot claim another member's archive receipt"
        );
        let first_tab = authorization.scope().members()[0].target().tab;
        let second_tab = authorization.scope().members()[1].target().tab;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::ActivateTab {
                window,
                tab: first_tab,
            }))
            .expect("focus first captured tab");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::ActivateTab {
                window,
                tab: second_tab,
            }))
            .expect("focus second captured tab");
        assert_eq!(application.windows()[&window].active_tab, Some(second_tab));
        let next_ticket = application
            .request_window_page_capture_stage(&authorization, 1)
            .expect("prepare next exact member");
        let StorageCompletion::VersioningCaptureStage(Ok(second_stage)) =
            wait_for_storage_completion(&mut application, next_ticket)
        else {
            panic!("second page stage required");
        };
        assert_ne!(first_stage.path(), second_stage.path());
        assert!(
            application
                .request_window_page_capture_object_install(&authorization, 0, Some(13), None)
                .is_err(),
            "page zero cannot consume page one's target"
        );
        fs::write(second_stage.path(), b"second archive").expect("second native save");
        let install_ticket = application
            .request_window_page_capture_object_install(&authorization, 1, Some(14), None)
            .expect("install authorized page one");
        let StorageCompletion::VersioningObjectInstall(Ok(second_object)) =
            wait_for_storage_completion(&mut application, install_ticket)
        else {
            panic!("second archive object required");
        };
        assert_ne!(first_object.sha256, second_object.sha256);
        let started_at_ms = prepared.manifest.capture_interval.started_at_ms;
        let page_capture = ManifestPageCapture {
            archive_interval: Some(CaptureInterval {
                started_at_ms,
                finished_at_ms: started_at_ms,
            }),
            sidecar_interval: None,
            archive_monotonic_interval: Some(
                ferric_browser_storage::versioning::CaptureMonotonicInterval {
                    started_offset_ms: 0,
                    finished_offset_ms: 0,
                },
            ),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        };
        let mut wrong_order = prepared.manifest.clone();
        wrong_order.windows[0].pages.swap(0, 1);
        assert!(
            application
                .request_window_page_capture_object_publish(
                    &authorization,
                    wrong_order,
                    vec![Some(page_capture.clone()), Some(page_capture.clone())],
                    PageCaptureProvenance::new(None, None),
                )
                .is_err(),
            "the page receipts cannot be attached to reordered logical pages"
        );
        assert!(
            application
                .request_window_page_capture_object_publish(
                    &authorization,
                    prepared.manifest.clone(),
                    vec![Some(page_capture.clone()), None],
                    PageCaptureProvenance::new(None, None),
                )
                .is_err(),
            "strict publication cannot omit an installed page result"
        );
        let publish_ticket = application
            .request_window_page_capture_object_publish(
                &authorization,
                prepared.manifest,
                vec![Some(page_capture.clone()), Some(page_capture)],
                PageCaptureProvenance::new(None, None),
            )
            .expect("strict window publishes only after both archives are installed");
        assert!(matches!(
            wait_for_storage_completion(&mut application, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        application
            .complete_versioning_operation(consent.operation)
            .expect("complete durable strict window capture");
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One regression follows document replacement through partial publication.
    fn partial_window_records_changed_document_without_reusing_its_stage() {
        use crate::versioning::PageCaptureFailurePolicy;

        let (mut application, roots, _, window, _) = durable_application();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("second live tab");
        let prepared = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "changed page".into())
            .expect("pre-navigation window manifest");
        let consent = application
            .prepare_page_capture_consent_with_failure_policy(
                ScopeSelector::Window { window },
                false,
                false,
                PageCaptureFailurePolicy::AllowPartialPages,
            )
            .expect("partial window preview");
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("accept exact partial preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use partial authorization");
        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("reserve both pages");
        assert!(matches!(
            wait_for_storage_completion(&mut application, stage_ticket),
            StorageCompletion::VersioningCaptureStage(Ok(_))
        ));
        let first = authorization.scope().members()[0].target();
        let new_url =
            ValidatedUrl::parse("https://changed.example.test/").expect("safe navigation fixture");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: first,
                url: new_url.clone(),
            }))
            .expect("first page starts a new document");
        let changed = application
            .capture_target(first.tab)
            .expect("replacement document target");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: changed,
                url: new_url,
                title: "changed".into(),
            }))
            .expect("replacement document commits");
        assert_ne!(application.capture_target(first.tab), Some(first));
        application
            .record_window_page_capture_failure(
                &authorization,
                0,
                ferric_browser_storage::versioning::ManifestPageArchiveFailure::DocumentChanged,
            )
            .expect("partial window records changed page as a failure");
        let next_ticket = application
            .request_window_page_capture_stage(&authorization, 1)
            .expect("unaffected page may still stage");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) =
            wait_for_storage_completion(&mut application, next_ticket)
        else {
            panic!("unaffected page stage required");
        };
        fs::write(stage.path(), b"second archive").expect("unaffected native save");
        let install_ticket = application
            .request_window_page_capture_object_install(&authorization, 1, Some(14), None)
            .expect("install unaffected archive");
        assert!(matches!(
            wait_for_storage_completion(&mut application, install_ticket),
            StorageCompletion::VersioningObjectInstall(Ok(_))
        ));
        let mut manifest = prepared.manifest;
        manifest.requested_capture.allow_partial_pages = true;
        let started_at_ms = manifest.capture_interval.started_at_ms;
        let page_capture = ManifestPageCapture {
            archive_interval: Some(CaptureInterval {
                started_at_ms,
                finished_at_ms: started_at_ms,
            }),
            sidecar_interval: None,
            archive_monotonic_interval: Some(
                ferric_browser_storage::versioning::CaptureMonotonicInterval {
                    started_offset_ms: 0,
                    finished_offset_ms: 0,
                },
            ),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        };
        let publish_ticket = application
            .request_window_page_capture_object_publish(
                &authorization,
                manifest,
                vec![None, Some(page_capture)],
                PageCaptureProvenance::new(None, None),
            )
            .expect("changed page cannot prevent consented partial publication");
        assert!(matches!(
            wait_for_storage_completion(&mut application, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        application
            .complete_versioning_operation(consent.operation)
            .expect("complete durable changed-page partial capture");
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn partial_window_records_changed_child_frame_with_live_top_document() {
        use crate::versioning::PageCaptureFailurePolicy;

        let (mut application, roots, _, window, _) = durable_application();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("second live tab");
        let consent = application
            .prepare_page_capture_consent_with_failure_policy(
                ScopeSelector::Window { window },
                false,
                false,
                PageCaptureFailurePolicy::AllowPartialPages,
            )
            .expect("partial window preview");
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("accept exact partial preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use partial authorization");
        let first = authorization.scope().members()[0].target();
        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("reserve and stage first page");
        assert!(matches!(
            wait_for_storage_completion(&mut application, stage_ticket),
            StorageCompletion::VersioningCaptureStage(Ok(_))
        ));
        assert_eq!(application.capture_target(first.tab), Some(first));
        application
            .record_window_page_capture_failure(
                &authorization,
                0,
                ferric_browser_storage::versioning::ManifestPageArchiveFailure::DocumentChanged,
            )
            .expect("collector-observed child-frame change is a page failure");
        let second_stage_ticket = application
            .request_window_page_capture_stage(&authorization, 1)
            .expect("other page remains eligible");
        assert!(matches!(
            wait_for_storage_completion(&mut application, second_stage_ticket),
            StorageCompletion::VersioningCaptureStage(Ok(_))
        ));
        application
            .cancel_versioning_operation(consent.operation)
            .expect("cancel test operation");
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn transient_page_capture_consent_is_rejected_before_preview_creation() {
        for privacy in [PrivacyKind::Private, PrivacyKind::Ephemeral] {
            let (mut application, window, _) =
                BrowserApplication::bootstrap(Config::default(), privacy, "transient")
                    .expect("transient runtime");
            let failure = application
                .prepare_page_capture_consent(ScopeSelector::Window { window }, false)
                .expect_err("transient page cannot create a durable capture preview");
            assert_eq!(failure.reason(), VersioningReason::PrivatePersistenceDenied);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One test follows the real stage, failure and reservation cleanup.
    fn missing_staged_archive_cancels_without_commit_or_reservation() {
        let (mut application, roots, profile_uuid, window, tab) = durable_application();
        let source_url = "https://archive.example.test/missing-stage";
        let navigation_target = application.capture_target(tab).expect("live page target");
        let validated_url = ValidatedUrl::parse(source_url).expect("safe source URL");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: navigation_target,
                url: validated_url.clone(),
            }))
            .expect("start synthetic navigation");
        let committed_target = application.capture_target(tab).expect("loading target");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: committed_target,
                url: validated_url,
                title: "synthetic archive failure".into(),
            }))
            .expect("commit synthetic navigation");
        assert!(application.note_typed_safe_get_navigation(tab, source_url));

        let consent = application
            .prepare_page_capture_consent(
                ScopeSelector::Tab {
                    window,
                    target: application.capture_target(tab).expect("live tab target"),
                },
                false,
            )
            .expect("normal-profile page capture preview");
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("confirm exact preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use authorization");
        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("bounded reservation and staging request");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) =
            wait_for_storage_completion(&mut application, stage_ticket)
        else {
            panic!("exact archive staging request must succeed");
        };
        let stage_path = stage.path().to_owned();
        assert!(stage_path.exists());
        assert!(
            application
                .page_capture_reservations_held
                .contains_key(&consent.operation)
        );

        fs::remove_file(&stage_path).expect("simulate exact native stage disappearing");
        let install_ticket = application
            .request_page_capture_object_install(&authorization, Some(1), None)
            .expect("submit bounded install of missing stage");
        assert!(matches!(
            wait_for_storage_completion(&mut application, install_ticket),
            StorageCompletion::VersioningObjectInstall(Err(_))
        ));
        assert!(
            application
                .cancel_versioning_operation(consent.operation)
                .is_ok()
        );
        let cleanup_deadline = Instant::now() + Duration::from_secs(5);
        while application
            .page_capture_reservations_held
            .contains_key(&consent.operation)
        {
            let _ = application.poll_storage_effects();
            assert!(
                Instant::now() < cleanup_deadline,
                "capture reservation was not released after missing-stage failure"
            );
            thread::yield_now();
        }
        assert!(
            !application
                .ready_page_capture_stages
                .contains_key(&consent.operation)
        );
        assert!(!stage_path.exists());
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .expect("cancelled capture operation")
                .1,
            OperationState::Cancelled
        );
        let database = roots
            .data
            .join("profiles")
            .join(profile_uuid.to_string())
            .join("browser.sqlite");
        let connection = rusqlite::Connection::open(database).expect("inspect disposable profile");
        let visible_commits: i64 = connection
            .query_row("SELECT COUNT(*) FROM vb_commits", [], |row| row.get(0))
            .expect("count visible commits");
        assert_eq!(visible_commits, 0);

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One end-to-end regression follows corrupt bytes through abort.
    fn corrupted_installed_archive_is_rejected_before_commit_and_releases_reservation() {
        let (mut application, roots, profile_uuid, window, tab) = durable_application();
        let source_url = "https://archive.example.test/corrupted-object";
        let navigation_target = application.capture_target(tab).expect("live page target");
        let validated_url = ValidatedUrl::parse(source_url).expect("safe source URL");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: navigation_target,
                url: validated_url.clone(),
            }))
            .expect("start synthetic source navigation");
        let committed_target = application
            .capture_target(tab)
            .expect("loading page target");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: committed_target,
                url: validated_url,
                title: "synthetic corrupted archive".into(),
            }))
            .expect("commit synthetic source navigation");
        assert!(application.note_typed_safe_get_navigation(tab, source_url));

        let prepared = application
            .prepare_structural_commit(
                ScopeSelector::Tab {
                    window,
                    target: application.capture_target(tab).expect("current tab target"),
                },
                "corrupted archive capture".into(),
            )
            .expect("structural manifest at the capture revision");
        let consent = application
            .prepare_page_capture_consent(
                ScopeSelector::Tab {
                    window,
                    target: application.capture_target(tab).expect("current tab target"),
                },
                false,
            )
            .expect("normal-profile page capture preview");
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("confirm exact preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use page archive authorization");

        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("bounded reservation and staging request");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) =
            wait_for_storage_completion(&mut application, stage_ticket)
        else {
            panic!("the exact archive staging request must succeed");
        };
        let archive_bytes = b"synthetic archive with a corrupt object reference";
        fs::write(stage.path(), archive_bytes).expect("write the exact staged archive");
        let install_ticket = application
            .request_page_capture_object_install(
                &authorization,
                Some(archive_bytes.len() as u64),
                None,
            )
            .expect("bounded archive object install");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for_storage_completion(&mut application, install_ticket)
        else {
            panic!("the original archive object must install successfully");
        };

        let object_path = stored_archive_object_path(&roots, profile_uuid, &archive);
        let mut corrupted_bytes = archive_bytes.to_vec();
        corrupted_bytes[0] ^= 0xff;
        fs::write(&object_path, &corrupted_bytes)
            .expect("corrupt the installed object without changing its length");

        let started_at_ms = prepared.manifest.capture_interval.started_at_ms;
        let page_capture = ManifestPageCapture {
            archive_interval: Some(CaptureInterval {
                started_at_ms,
                finished_at_ms: started_at_ms.saturating_add(1),
            }),
            sidecar_interval: None,
            archive_monotonic_interval: Some(
                ferric_browser_storage::versioning::CaptureMonotonicInterval {
                    started_offset_ms: 0,
                    finished_offset_ms: 1,
                },
            ),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        };
        let publish_ticket = application
            .request_page_capture_object_publish(
                &authorization,
                prepared.manifest,
                archive,
                None,
                page_capture,
                PageCaptureProvenance::new(None, None),
            )
            .expect("submit publication for the exact authorized page");
        assert!(matches!(
            wait_for_storage_completion(&mut application, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Err(_))
        ));

        let release_deadline = Instant::now() + Duration::from_secs(5);
        while application
            .page_capture_reservations_held
            .contains_key(&consent.operation)
        {
            let _ = application.poll_storage_effects();
            assert!(
                Instant::now() < release_deadline,
                "corrupt-object publication did not release its capture reservation"
            );
            thread::yield_now();
        }
        assert!(
            !application
                .page_capture_abort_requested
                .contains(&consent.operation),
            "reservation abort callback must settle"
        );
        assert!(
            !application
                .pending_page_capture_publications
                .values()
                .any(|(operation, _)| *operation == consent.operation)
        );
        application
            .cancel_versioning_operation(consent.operation)
            .expect("retire the failed publication conservatively");
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .expect("retained terminal operation")
                .1,
            OperationState::OutcomeUnknown,
            "the UI-facing operation remains conservative after publication began"
        );

        let database = roots
            .data
            .join("profiles")
            .join(profile_uuid.to_string())
            .join("browser.sqlite");
        let connection = rusqlite::Connection::open(database).expect("inspect disposable profile");
        let visible_commits: i64 = connection
            .query_row("SELECT COUNT(*) FROM vb_commits", [], |row| row.get(0))
            .expect("count visible commits");
        assert_eq!(
            visible_commits, 0,
            "corrupt archive bytes are never published"
        );
        drop(connection);

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One end-to-end test documents the stage-to-publication contract.
    fn page_archive_publish_advances_exact_stage_ticket_before_durable_completion() {
        let (mut application, roots, profile_uuid, window, tab) = durable_application();
        let owner = application
            .versioning
            .owner(application.windows()[&window].profile)
            .expect("bound native owner");
        let clock = Rc::new(Cell::new(1_000_u64));
        let clock_source = Rc::clone(&clock);
        let mut versioning =
            VersioningCoordinator::with_sources(move || clock_source.get(), OperationId::fresh);
        versioning.bind_owner(owner);
        application.versioning = versioning;
        let source_url = "https://archive.example.test/article";
        let navigation_target = application
            .capture_target(tab)
            .expect("live page target before source navigation");
        let validated_url = ValidatedUrl::parse(source_url).expect("safe fixture source URL");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: navigation_target,
                url: validated_url.clone(),
            }))
            .expect("start synthetic source navigation");
        let committed_target = application
            .capture_target(tab)
            .expect("loading page target");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: committed_target,
                url: validated_url,
                title: "synthetic captured source".into(),
            }))
            .expect("commit synthetic source navigation");
        assert!(application.note_typed_safe_get_navigation(tab, source_url));
        let prepared = application
            .prepare_structural_commit(
                ScopeSelector::Tab {
                    window,
                    target: application.capture_target(tab).expect("live tab target"),
                },
                "archive capture base".into(),
            )
            .expect("structural manifest at the capture revision");
        let commit = prepared.commit;
        let consent = application
            .prepare_page_capture_consent(
                ScopeSelector::Tab {
                    window,
                    target: application.capture_target(tab).expect("live tab target"),
                },
                false,
            )
            .expect("normal-profile page capture preview");
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("confirm exact preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use page archive authorization");

        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("bounded reservation and staging request");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) =
            wait_for_storage_completion(&mut application, stage_ticket)
        else {
            panic!("the exact archive staging request must succeed");
        };
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .unwrap()
                .1,
            crate::versioning::OperationState::Executing
        );

        let bytes = b"synthetic bounded archive";
        fs::write(stage.path(), bytes).expect("synthetic archive in its exact stage");
        let install_ticket = application
            .request_page_capture_object_install(&authorization, Some(bytes.len() as u64), None)
            .expect("bounded archive object install");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for_storage_completion(&mut application, install_ticket)
        else {
            panic!("the validated archive object must be installed");
        };
        let archive_object_path = stored_archive_object_path(&roots, profile_uuid, &archive);
        let unexpected_stage_leaf = stage
            .path()
            .parent()
            .expect("capture is inside its operation directory")
            .join("unexpected.tmp");
        fs::write(&unexpected_stage_leaf, b"retain unknown stage data")
            .expect("unknown stage entry for cleanup refusal");

        let interval_start = prepared.manifest.capture_interval.started_at_ms;
        let page_capture = ManifestPageCapture {
            archive_interval: Some(CaptureInterval {
                started_at_ms: interval_start,
                finished_at_ms: interval_start.saturating_add(1),
            }),
            sidecar_interval: None,
            archive_monotonic_interval: Some(
                ferric_browser_storage::versioning::CaptureMonotonicInterval {
                    started_offset_ms: 0,
                    finished_offset_ms: 1,
                },
            ),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        };
        clock.set(350_000);
        let mut wrong_page = prepared.manifest.clone();
        let unrelated_page = ferric_browser_core::LogicalPageId::fresh().to_string();
        wrong_page.windows[0].pages[0].logical_page_id = unrelated_page.clone();
        wrong_page.windows[0].selected_logical_page = Some(unrelated_page);
        assert!(
            application
                .request_page_capture_object_publish(
                    &authorization,
                    wrong_page,
                    archive,
                    None,
                    page_capture.clone(),
                    PageCaptureProvenance::new(None, None),
                )
                .is_err(),
            "a valid object cannot be attached to another logical page"
        );
        let mut wrong_window = prepared.manifest.clone();
        wrong_window.windows[0].logical_window_id =
            ferric_browser_core::LogicalWindowId::fresh().to_string();
        assert!(
            application
                .request_page_capture_object_publish(
                    &authorization,
                    wrong_window,
                    archive,
                    None,
                    page_capture.clone(),
                    PageCaptureProvenance::new(None, None),
                )
                .is_err(),
            "a valid object cannot be attached to another logical window"
        );
        let publish_ticket = application
            .request_page_capture_object_publish(
                &authorization,
                prepared.manifest,
                archive,
                None,
                page_capture,
                PageCaptureProvenance::new(None, Some(
                    "qtwebengine=6.11.2;chromium=140.0.7339.225;security-patch=151.0.7922.71;platform=wayland",
                )),
            )
            .expect("exact page capture should enter durable publication");
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .unwrap()
                .1,
            OperationState::Publishing
        );
        clock.set(361_000);
        assert!(application.expire_versioning_operations().is_empty());
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .unwrap()
                .1,
            OperationState::Publishing,
            "a pending durable publication outlives its preparation deadline"
        );
        assert!(
            application
                .page_capture_reservations_held
                .contains_key(&consent.operation)
        );
        assert!(
            !application
                .page_capture_abort_requested
                .contains(&consent.operation)
        );
        assert!(matches!(
            wait_for_storage_completion(&mut application, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: false,
                },
            ))
        ));
        assert_eq!(
            fs::read(&unexpected_stage_leaf).expect("unknown stage data remains for recovery"),
            b"retain unknown stage data"
        );
        assert!(
            !application
                .page_capture_reservations_held
                .contains_key(&consent.operation)
        );
        assert!(
            !application
                .page_capture_abort_requested
                .contains(&consent.operation),
            "durable publication with deferred cleanup is not an abort"
        );
        assert!(
            !application
                .pending_page_capture_publications
                .values()
                .any(|(operation, _)| *operation == consent.operation)
        );
        application
            .complete_versioning_operation(consent.operation)
            .expect("durable page archive callback completes its original stage ticket");
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .unwrap()
                .1,
            OperationState::Completed
        );
        let database = roots
            .data
            .join("profiles")
            .join(profile_uuid.to_string())
            .join("browser.sqlite");
        let stored = stored_manifest(&database, commit);
        assert_eq!(
            stored.engine_fingerprint,
            "qtwebengine=6.11.2;chromium=140.0.7339.225;security-patch=151.0.7922.71;platform=wayland"
        );

        let archive_view_ticket = application
            .request_versioned_archive_view(window, commit)
            .expect("one committed archive resolves to an isolated-view file");
        let StorageCompletion::VersioningArchiveView(Ok(archive_view)) =
            wait_for_storage_completion(&mut application, archive_view_ticket)
        else {
            panic!("exact digest-verified archive-view completion expected");
        };
        assert_eq!(
            fs::read(archive_view.path()).expect("materialized archive bytes"),
            bytes
        );
        let metadata = application
            .versioned_archive_view_metadata(window, archive_view.operation())
            .expect("exact active archive has inert provenance metadata");
        assert!(
            application
                .versioned_archive_view_metadata(
                    WindowId::from_display("windowid-18446744073709551615")
                        .expect("distinct synthetic window ID"),
                    archive_view.operation(),
                )
                .is_none()
        );
        assert_eq!(metadata.source_url.as_deref(), Some(source_url));
        assert_eq!(metadata.captured_at_ms, interval_start.saturating_add(1));
        assert!(
            metadata
                .fidelity_label
                .contains("Archive: complete for request")
        );
        assert!(metadata.fidelity_label.contains("page state:"));
        assert!(
            metadata
                .fidelity_label
                .contains("mutation evidence: unavailable")
        );
        let metadata_debug = format!("{metadata:?}");
        assert!(!metadata_debug.contains(source_url));
        assert!(!metadata_debug.contains(&metadata.captured_at_ms.to_string()));
        assert!(!metadata_debug.contains("complete for request"));
        assert!(
            application
                .request_versioned_archive_view(window, commit)
                .is_err(),
            "a second isolated archive view exceeds the one-view bound"
        );
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("native owner change must not remove an exposed viewer file before blank");
        assert!(
            !application
                .pending_archive_view_cleanups
                .values()
                .any(|pending| *pending == archive_view.operation()),
            "exposed archive must not be scheduled for removal before blank"
        );
        application.schedule_all_archive_view_cleanup();
        assert!(
            !application
                .pending_archive_view_cleanups
                .values()
                .any(|pending| *pending == archive_view.operation()),
            "whole-profile teardown must also retain an exposed viewer until blank"
        );
        assert!(
            archive_view.path().exists(),
            "exposed viewer file remains available"
        );
        let cleanup_ticket = application
            .close_versioned_archive_view(window, archive_view.operation())
            .expect("blanked view may remove its exact file after native owner change");
        assert!(matches!(
            wait_for_storage_completion(&mut application, cleanup_ticket),
            StorageCompletion::VersioningArchiveViewCleanup(Ok(()))
        ));
        assert!(!archive_view.path().exists());

        let duplicate_cleanup_view_ticket = application
            .request_versioned_archive_view(window, commit)
            .expect("another exact archive view for teardown cleanup");
        let StorageCompletion::VersioningArchiveView(Ok(duplicate_cleanup_view)) =
            wait_for_storage_completion(&mut application, duplicate_cleanup_view_ticket)
        else {
            panic!("another digest-verified archive view expected");
        };
        let profile = application.windows()[&window].profile;
        application
            .pending_archive_view_cleanups
            .insert(cleanup_ticket, duplicate_cleanup_view.operation());
        application.schedule_archive_view_cleanup_for_profile(profile);
        assert_eq!(
            application.pending_archive_view_cleanups.len(),
            1,
            "an already-pending exact cleanup must not be scheduled again"
        );
        application
            .pending_archive_view_cleanups
            .remove(&cleanup_ticket);
        application.schedule_archive_view_cleanup_for_profile(profile);
        application.schedule_archive_view_cleanup_for_profile(profile);
        assert_eq!(
            application.pending_archive_view_cleanups.len(),
            1,
            "repeated profile teardown must not schedule duplicate removal of one file"
        );
        let cleanup_ticket = *application
            .pending_archive_view_cleanups
            .keys()
            .next()
            .expect("one exact teardown cleanup ticket");
        assert!(matches!(
            wait_for_storage_completion(&mut application, cleanup_ticket),
            StorageCompletion::VersioningArchiveViewCleanup(Ok(()))
        ));
        assert!(!duplicate_cleanup_view.path().exists());

        let stale_view_ticket = application
            .request_versioned_archive_view(window, commit)
            .expect("a later exact-view request after cleanup");
        let manifest_deadline = Instant::now() + Duration::from_secs(5);
        let manifest_effect = loop {
            if let Some(effect) = application
                .storage
                .poll()
                .into_iter()
                .find(|effect| effect.ticket == stale_view_ticket)
            {
                break effect;
            }
            assert!(
                Instant::now() < manifest_deadline,
                "archive manifest read did not complete"
            );
            thread::yield_now();
        };
        assert!(
            application
                .route_archive_view_storage_effects(vec![manifest_effect])
                .is_empty()
        );
        let materialize_ticket = *application
            .pending_archive_view_objects
            .keys()
            .next()
            .expect("manifest callback schedules exact object materialization");
        let materialize_deadline = Instant::now() + Duration::from_secs(5);
        let materialized_effect = loop {
            if let Some(effect) = application
                .storage
                .poll()
                .into_iter()
                .find(|effect| effect.ticket == materialize_ticket)
            {
                break effect;
            }
            assert!(
                Instant::now() < materialize_deadline,
                "archive materialization did not complete"
            );
            thread::yield_now();
        };
        let stale_file = match &materialized_effect.completion {
            StorageCompletion::VersioningArchiveView(Ok(file)) => file.path().to_owned(),
            _ => panic!("digest-verified archive file is required for stale-owner cleanup"),
        };
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("archive callback must be tested after same-profile native rebind");
        let stale_result =
            application.route_archive_view_storage_effects(vec![materialized_effect]);
        assert_eq!(stale_result.len(), 1);
        assert_eq!(stale_result[0].ticket, stale_view_ticket);
        assert!(matches!(
            &stale_result[0].completion,
            StorageCompletion::VersioningArchiveView(Err(_))
        ));
        assert!(application.active_archive_views.is_empty());
        let cleanup_deadline = Instant::now() + Duration::from_secs(5);
        while stale_file.exists() {
            let effects = application.storage.poll();
            if !effects.is_empty() {
                let _ = application.route_archive_view_storage_effects(effects);
            }
            assert!(
                Instant::now() < cleanup_deadline,
                "stale materialized archive file was not removed"
            );
            thread::yield_now();
        }

        let mut corrupt_archive = bytes.to_vec();
        corrupt_archive[0] ^= 1;
        fs::write(&archive_object_path, &corrupt_archive)
            .expect("corrupt exact object for archive-view failure path");
        let corrupt_view_ticket = application
            .request_versioned_archive_view(window, commit)
            .expect("corrupt archive is attempted only through exact materialization");
        assert!(matches!(
            wait_for_storage_completion(&mut application, corrupt_view_ticket),
            StorageCompletion::VersioningArchiveView(Err(_))
        ));

        fs::remove_file(&archive_object_path).expect("remove exact object for missing fixture");
        let missing_view_ticket = application
            .request_versioned_archive_view(window, commit)
            .expect("missing archive remains an unavailable exact object");
        assert!(matches!(
            wait_for_storage_completion(&mut application, missing_view_ticket),
            StorageCompletion::VersioningArchiveView(Err(_))
        ));

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One test holds a durable callback across the publication deadline.
    fn late_page_archive_publish_callback_keeps_durable_commit_but_not_success_claim() {
        let (mut application, roots, profile_uuid, window, tab) = durable_application();
        let owner = application
            .versioning
            .owner(application.windows()[&window].profile)
            .expect("bound native owner");
        let clock = Rc::new(Cell::new(1_000_u64));
        let clock_source = Rc::clone(&clock);
        let mut versioning =
            VersioningCoordinator::with_sources(move || clock_source.get(), OperationId::fresh);
        versioning.bind_owner(owner);
        application.versioning = versioning;
        let source_url = "https://archive.example.test/late-publication";
        let target = application.capture_target(tab).expect("live page target");
        let url = ValidatedUrl::parse(source_url).expect("safe fixture URL");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target,
                url: url.clone(),
            }))
            .expect("start synthetic navigation");
        let target = application
            .capture_target(tab)
            .expect("loading page target");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target,
                url,
                title: "late publication fixture".into(),
            }))
            .expect("commit synthetic navigation");
        assert!(application.note_typed_safe_get_navigation(tab, source_url));
        let selector = ScopeSelector::Tab {
            window,
            target: application.capture_target(tab).expect("live tab target"),
        };
        let prepared = application
            .prepare_structural_commit(selector, "late publication".into())
            .expect("structural capture manifest");
        let commit = prepared.commit;
        let consent = application
            .prepare_page_capture_consent(selector, false)
            .expect("normal-profile preview");
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("confirm exact preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use authorization");
        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("bounded reservation and stage");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) =
            wait_for_storage_completion(&mut application, stage_ticket)
        else {
            panic!("exact archive stage must succeed");
        };
        let bytes = b"synthetic archive with delayed callback";
        fs::write(stage.path(), bytes).expect("write exact staged archive");
        let install_ticket = application
            .request_page_capture_object_install(&authorization, Some(bytes.len() as u64), None)
            .expect("bounded object installation");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for_storage_completion(&mut application, install_ticket)
        else {
            panic!("validated object must be installed");
        };
        let interval_start = prepared.manifest.capture_interval.started_at_ms;
        let page_capture = ManifestPageCapture {
            archive_interval: Some(CaptureInterval {
                started_at_ms: interval_start,
                finished_at_ms: interval_start.saturating_add(1),
            }),
            sidecar_interval: None,
            archive_monotonic_interval: Some(
                ferric_browser_storage::versioning::CaptureMonotonicInterval {
                    started_offset_ms: 0,
                    finished_offset_ms: 1,
                },
            ),
            sidecar_monotonic_interval: None,
            mutation_evidence: ManifestMutationEvidence::NotObserved,
            categories: Vec::new(),
        };
        let publish_ticket = application
            .request_page_capture_object_publish(
                &authorization,
                prepared.manifest,
                archive,
                None,
                page_capture,
                PageCaptureProvenance::new(None, None),
            )
            .expect("submit one atomic publication");
        let storage_deadline = Instant::now() + Duration::from_secs(5);
        let published_effect = loop {
            if let Some(effect) = application
                .storage
                .poll()
                .into_iter()
                .find(|effect| effect.ticket == publish_ticket)
            {
                break effect;
            }
            assert!(
                Instant::now() < storage_deadline,
                "publication worker stalled"
            );
            thread::yield_now();
        };
        assert!(matches!(
            published_effect.completion,
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        let database = roots
            .data
            .join("profiles")
            .join(profile_uuid.to_string())
            .join("browser.sqlite");
        assert_eq!(stored_manifest(&database, commit).id, commit.to_string());

        clock.set(361_000);
        application
            .pending_page_capture_publications
            .get_mut(&publish_ticket)
            .expect("exact callback reservation")
            .1 = Instant::now()
            .checked_sub(Duration::from_millis(1))
            .expect("monotonic clock has at least one millisecond of history");
        assert!(application.expire_versioning_operations().is_empty());
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .unwrap()
                .1,
            OperationState::OutcomeUnknown
        );
        let abort_deadline = Instant::now() + Duration::from_secs(5);
        while application
            .page_capture_reservations_held
            .contains_key(&consent.operation)
        {
            let _ = application.poll_storage_effects();
            assert!(
                Instant::now() < abort_deadline,
                "reservation abort callback stalled"
            );
            thread::yield_now();
        }
        assert_eq!(stored_manifest(&database, commit).id, commit.to_string());

        let forwarded = application.route_page_capture_storage_effects(vec![published_effect]);
        assert_eq!(forwarded.len(), 1);
        assert!(matches!(
            forwarded[0].completion,
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(
            application
                .complete_versioning_operation(consent.operation)
                .is_err()
        );
        assert_eq!(
            application
                .versioning()
                .operation(consent.operation)
                .unwrap()
                .1,
            OperationState::OutcomeUnknown
        );
        assert!(application.pending_page_capture_publications.is_empty());
        assert_eq!(stored_manifest(&database, commit).id, commit.to_string());

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn profile_teardown_cancels_page_archive_hashing_and_cleans_temporary_object() {
        let (mut application, roots, profile_uuid, window, _) = durable_application();
        let consent = application
            .prepare_page_capture_consent(ScopeSelector::Window { window }, false)
            .expect("normal-profile page capture preview");
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("confirm exact preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use page archive authorization");
        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("bounded reservation and staging request");
        let stage_deadline = Instant::now() + Duration::from_secs(3);
        let stage = loop {
            if let Some(stage) = application
                .poll_storage_effects()
                .into_iter()
                .find_map(|effect| {
                    (effect.ticket == stage_ticket)
                        .then_some(effect.completion)
                        .and_then(|completion| match completion {
                            StorageCompletion::VersioningCaptureStage(Ok(stage)) => Some(stage),
                            _ => None,
                        })
                })
            {
                break stage;
            }
            assert!(
                Instant::now() < stage_deadline,
                "archive stage did not complete"
            );
            thread::yield_now();
        };
        let bytes = vec![b'x'; 32 * 1024 * 1024];
        fs::write(stage.path(), &bytes).expect("synthetic archive in the exact stage");
        application
            .request_page_capture_object_install(&authorization, Some(bytes.len() as u64), None)
            .expect("bounded file-only hashing request");
        let temporary_object = stage
            .path()
            .parent()
            .expect("operation staging directory")
            .join("object.tmp");
        let deadline = Instant::now() + Duration::from_secs(2);
        while !temporary_object.exists() {
            assert!(Instant::now() < deadline, "object hashing did not begin");
            thread::yield_now();
        }

        // Profile retirement cancels this exact hash job and joins its file
        // worker. The temporary object must be removed and no commit published.
        application.close_profile();
        assert!(!temporary_object.exists());
        let objects_root = roots
            .state
            .join("versioning")
            .join(profile_uuid.hyphenated().to_string())
            .join("objects/sha256");
        for shard in fs::read_dir(objects_root).expect("private object root") {
            assert_eq!(
                fs::read_dir(shard.expect("object shard").path())
                    .expect("object shard contents")
                    .count(),
                0,
                "cancelled hashing must not install a partial object"
            );
        }
        assert!(application.poll_commit_events().is_empty());
        drop(application);
        roots
            .cleanup()
            .expect("temporary roots cleanup after worker retirement");
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One end-to-end test documents late archive completion ownership.
    fn native_profile_rebind_rejects_late_page_archive_hash_completion() {
        let (mut application, roots, profile_uuid, window, tab) = durable_application();
        let capture_target = application
            .capture_target(tab)
            .expect("live page target for archive capture");
        let prepared = application
            .prepare_structural_commit(
                ScopeSelector::Tab {
                    window,
                    target: capture_target,
                },
                "late archive callback base".into(),
            )
            .expect("exact structural manifest for the capture target");
        let consent = application
            .prepare_page_capture_consent(
                ScopeSelector::Tab {
                    window,
                    target: application
                        .capture_target(tab)
                        .expect("same live page target for capture consent"),
                },
                false,
            )
            .expect("normal-profile page capture preview");
        let sink = application
            .take_native_versioning_decision_sink()
            .expect("native decision authority");
        let permit = application
            .capture_native_versioning_decision(consent.operation, sink.accept(&consent.preview))
            .expect("confirm exact preview");
        let authorization = application
            .confirm_page_capture_consent(consent.operation, permit)
            .expect("one-use page archive authorization");
        let stage_ticket = application
            .request_page_capture_stage(&authorization)
            .expect("bounded reservation and staging request");
        let stage_deadline = Instant::now() + Duration::from_secs(3);
        let stage = loop {
            if let Some(stage) = application
                .poll_storage_effects()
                .into_iter()
                .find_map(|effect| {
                    (effect.ticket == stage_ticket)
                        .then_some(effect.completion)
                        .and_then(|completion| match completion {
                            StorageCompletion::VersioningCaptureStage(Ok(stage)) => Some(stage),
                            _ => None,
                        })
                })
            {
                break stage;
            }
            assert!(
                Instant::now() < stage_deadline,
                "archive stage did not complete"
            );
            thread::yield_now();
        };
        let bytes = vec![b'y'; 32 * 1024 * 1024];
        fs::write(stage.path(), &bytes).expect("synthetic archive in the exact stage");
        let install_ticket = application
            .request_page_capture_object_install(&authorization, Some(bytes.len() as u64), None)
            .expect("bounded file-only hashing request");
        let temporary_object = stage
            .path()
            .parent()
            .expect("operation staging directory")
            .join("object.tmp");
        let deadline = Instant::now() + Duration::from_secs(2);
        while !temporary_object.exists() {
            assert!(Instant::now() < deadline, "object hashing did not begin");
            thread::yield_now();
        }

        // Collect the worker result without routing it through the application
        // yet. This models a completion queued immediately before the native
        // profile owner is replaced, then delivered after the replacement.
        let completion_deadline = Instant::now() + Duration::from_secs(10);
        let install_effect = loop {
            if let Some(effect) = application
                .storage
                .poll()
                .into_iter()
                .find(|effect| effect.ticket == install_ticket)
            {
                break effect;
            }
            assert!(
                Instant::now() < completion_deadline,
                "late object completion did not arrive"
            );
            thread::yield_now();
        };
        assert!(matches!(
            &install_effect.completion,
            StorageCompletion::VersioningObjectInstall(Ok(_))
        ));

        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("rebind to a new native profile owner");
        assert!(
            application
                .versioning()
                .operation(consent.operation)
                .is_none_or(|(_, state)| state.terminal())
        );

        let capture_started_at_ms = prepared.manifest.capture_interval.started_at_ms;
        let mut routed = application.route_page_capture_storage_effects(vec![install_effect]);
        assert_eq!(
            routed.len(),
            1,
            "the correlated late completion is surfaced once"
        );
        let archive = match routed.pop().expect("one routed effect").completion {
            StorageCompletion::VersioningObjectInstall(Ok(archive)) => archive,
            other => panic!("the completed worker result remains successful: {other:?}"),
        };
        let installed_path = stored_archive_object_path(&roots, profile_uuid, &archive);
        let stale_publish = application.request_page_capture_object_publish(
            &authorization,
            prepared.manifest,
            archive,
            None,
            ManifestPageCapture {
                archive_interval: Some(CaptureInterval {
                    started_at_ms: capture_started_at_ms,
                    finished_at_ms: capture_started_at_ms.saturating_add(1),
                }),
                sidecar_interval: None,
                archive_monotonic_interval: Some(
                    ferric_browser_storage::versioning::CaptureMonotonicInterval {
                        started_offset_ms: 0,
                        finished_offset_ms: 1,
                    },
                ),
                sidecar_monotonic_interval: None,
                mutation_evidence: ManifestMutationEvidence::NotObserved,
                categories: Vec::new(),
            },
            PageCaptureProvenance::new(None, None),
        );
        assert!(
            stale_publish.is_err(),
            "an installed object callback from the old owner cannot publish"
        );
        let objects_root = roots
            .state
            .join("versioning")
            .join(profile_uuid.hyphenated().to_string())
            .join("objects/sha256");
        assert!(installed_path.starts_with(&objects_root));
        assert!(
            installed_path.exists(),
            "worker completion delivered a full object"
        );
        application.close_profile();
        assert!(!temporary_object.exists());
        assert!(
            objects_root.exists(),
            "the immutable object store remains profile-local"
        );
        assert!(application.poll_commit_events().is_empty());
        drop(application);
        roots
            .cleanup()
            .expect("temporary roots cleanup after worker retirement");
    }

    #[test]
    fn typed_get_proof_is_exact_document_and_url_scoped() {
        let (mut application, roots, _, window, tab) = durable_application();
        let original = application.capture_target(tab).unwrap();
        let url = "https://example.test/typed";
        let parsed = ValidatedUrl::parse(url).unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target: original,
                url: parsed.clone(),
            }))
            .unwrap();
        let loading = application.capture_target(tab).unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: loading,
                url: parsed,
                title: "synthetic".into(),
            }))
            .unwrap();
        assert!(!application.note_typed_safe_get_navigation(tab, "https://other.test/"));
        assert!(application.note_typed_safe_get_navigation(tab, url));
        let prepared = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "typed".into())
            .unwrap();
        assert_eq!(
            prepared.manifest.windows[0].pages[0].safe_url_or_placeholder,
            ferric_browser_storage::versioning::ManifestSafeUrl::Replayable(url.into())
        );
        // A page-initiated POST can commit the same URL without advancing the
        // reducer target. Its native load callback must still revoke proof.
        application.invalidate_safe_get_navigation(tab);
        let same_url = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "same URL".into())
            .unwrap();
        assert!(matches!(
            same_url.manifest.windows[0].pages[0].safe_url_or_placeholder,
            ferric_browser_storage::versioning::ManifestSafeUrl::Placeholder(_)
        ));
        assert!(application.note_typed_safe_get_navigation(tab, url));
        let target = application.capture_target(tab).unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target,
                url: ValidatedUrl::parse("https://example.test/next").unwrap(),
            }))
            .unwrap();
        let stale = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "stale".into())
            .unwrap();
        assert!(matches!(
            stale.manifest.windows[0].pages[0].safe_url_or_placeholder,
            ferric_browser_storage::versioning::ManifestSafeUrl::Placeholder(_)
        ));
        application.close_profile();
        drop(application);
        roots.cleanup().unwrap();
    }

    #[test]
    fn structural_preparation_binds_identity_after_projecting_the_exact_revision() {
        let (mut application, roots, _, window, tab) = durable_application();
        let previous_revision = application.revision();
        assert!(application.runtime_view().logical_window(window).is_none());
        assert!(application.runtime_view().logical_page(tab).is_none());
        let target = application
            .capture_target(tab)
            .expect("live target for structural projection");

        let prepared = application
            .prepare_structural_commit(
                ScopeSelector::Tab { window, target },
                "project before identity binding".into(),
            )
            .expect("structural projection should bind only after validation");

        assert_eq!(
            application.revision(),
            previous_revision
                .checked_add(1)
                .expect("the test revision must leave room for identity binding")
        );
        assert_eq!(
            prepared.manifest.application_revision,
            application.revision()
        );
        assert!(application.runtime_view().logical_window(window).is_some());
        assert!(application.runtime_view().logical_page(tab).is_some());
        assert!(prepared.runtime_effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::AppliedEvent(Event::Versioning(
                VersioningEvent::BindStructuralIdentity { .. }
            ))
        )));

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary profile roots cleanup");
    }

    #[test]
    fn allowlisted_policy_restore_is_explicit_revision_checked_and_schema_bounded() {
        use ferric_browser_config::RuntimeOverrides;

        let (mut application, roots, _, window, _) = durable_application();
        let prepared = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "policy preview".into())
            .expect("structural capture");
        assert_eq!(
            prepared
                .manifest
                .policy_snapshot
                .as_ref()
                .map(|policy| policy.schema_sha256.as_str()),
            Some(super::super::ALLOWLISTED_POLICY_SCHEMA_SHA256)
        );
        let mut stored = prepared.manifest.clone();
        stored
            .policy_snapshot
            .as_mut()
            .expect("captured allowlisted policy")
            .javascript_enabled = false;

        // Ordinary structural restore planning is descriptive and leaves
        // current policy untouched.
        let _ = application
            .plan_new_window_restore(window, &stored)
            .expect("ordinary structural plan");
        assert!(application.configuration().content.javascript);

        let stale_preview = application
            .preview_allowlisted_policy_restore(window, &stored)
            .expect("policy preview");
        assert_eq!(stale_preview.change.key, "content.javascript");
        assert!(stale_preview.change.current_enabled);
        assert!(!stale_preview.change.captured_enabled);
        let mut overrides = RuntimeOverrides::default();
        overrides
            .set_setting("content.zoom", toml::Value::Float(1.25))
            .expect("valid unrelated config change");
        application
            .apply_configuration_layer(&overrides)
            .expect("advance the current configuration revision");
        assert!(
            application
                .apply_allowlisted_policy_restore(&stale_preview)
                .is_err()
        );
        assert!(application.configuration().content.javascript);

        let mut obsolete = stored.clone();
        obsolete
            .policy_snapshot
            .as_mut()
            .expect("policy snapshot")
            .schema_sha256 = "0".repeat(64);
        assert!(
            application
                .preview_allowlisted_policy_restore(window, &obsolete)
                .is_err()
        );

        let preview = application
            .preview_allowlisted_policy_restore(window, &stored)
            .expect("fresh confirmed preview");
        let replay = preview.clone();
        let applied = application
            .apply_allowlisted_policy_restore(&preview)
            .expect("explicit validated application");
        assert!(!applied.effective.content.javascript);
        assert!(
            application
                .apply_allowlisted_policy_restore(&replay)
                .is_err(),
            "configuration revision invalidates a replayed preview"
        );

        let mut unknown = serde_json::to_value(&stored).expect("serialize fixture manifest");
        unknown["policy_snapshot"]["unreviewed_setting"] = serde_json::Value::Bool(true);
        let bytes = serde_json::to_vec(&unknown).expect("serialize unknown field fixture");
        assert!(VersionedCaptureManifest::decode(&bytes).is_err());

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn restore_policies_command_previews_without_mutating_until_confirmation() {
        use crate::versioning::PolicyRestoreEvent;
        use ferric_browser_config::RuntimeOverrides;

        let (mut application, roots, _, window, _) = durable_application();
        let mut disabled = RuntimeOverrides::default();
        disabled
            .set_setting("content.javascript", toml::Value::Boolean(false))
            .expect("valid allowlisted capture policy");
        application
            .apply_configuration_layer(&disabled)
            .expect("set captured policy");
        let (operation, commit, _) = application
            .begin_structural_commit(ScopeSelector::Window { window }, "captured policy".into())
            .expect("capture admission");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if application.poll_commit_events() == vec![CommitEvent::Durable { operation, commit }]
            {
                break;
            }
            assert!(Instant::now() < deadline, "policy fixture commit timed out");
            thread::yield_now();
        }

        let mut enabled = RuntimeOverrides::default();
        enabled
            .set_setting("content.javascript", toml::Value::Boolean(true))
            .expect("valid current policy");
        application
            .apply_configuration_layer(&enabled)
            .expect("restore current policy for preview");
        assert!(application.configuration().content.javascript);

        let effects = dispatch_versioning_command(
            &mut application,
            window,
            "restore",
            &[&commit.to_string(), "--policies"],
            CommandSource::Ui,
        )
        .expect("policy restore preview route");
        let request_id = effects
            .iter()
            .find_map(|effect| match effect {
                RuntimeEffect::Engine(EngineEffect::Versioning(
                    VersioningEngineEffect::PolicyRestorePreviewQueued { request_id },
                )) => Some(*request_id),
                _ => None,
            })
            .expect("policy preview request");
        let PolicyRestoreEvent::PreviewReady { preview, .. } =
            wait_for_policy_restore(&mut application, request_id)
        else {
            panic!("stored allowlisted policy was not previewed");
        };
        assert_eq!(preview.change.key, "content.javascript");
        assert!(preview.change.current_enabled);
        assert!(!preview.change.captured_enabled);
        assert!(application.configuration().content.javascript);

        let other_window = WindowId::from_display("windowid-18446744073709551615")
            .expect("synthetic non-owner window ID");
        assert!(!application.cancel_allowlisted_policy_restore(other_window, request_id));
        assert!(
            application
                .confirm_allowlisted_policy_restore(other_window, request_id)
                .is_err()
        );

        let (confirmed_commit, applied) = application
            .confirm_allowlisted_policy_restore(window, request_id)
            .expect("explicit policy confirmation");
        assert_eq!(confirmed_commit, commit);
        assert!(!applied.effective.content.javascript);
        assert!(!application.configuration().content.javascript);
        assert!(
            application
                .confirm_allowlisted_policy_restore(window, request_id)
                .is_err(),
            "confirmed preview is one-shot"
        );

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn command_route_tracks_durable_result_for_normal_profile() {
        let (mut application, roots, _, window, _) = durable_application();
        let effects = dispatch_versioning_command(
            &mut application,
            window,
            "commit",
            &["note"],
            CommandSource::Keyboard,
        )
        .expect("typed command admitted");
        assert!(effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::AppliedEvent(Event::Versioning(
                VersioningEvent::BindStructuralIdentity { .. }
            ))
        )));
        let (operation, commit) = effects
            .iter()
            .find_map(|effect| match effect {
                RuntimeEffect::Engine(EngineEffect::Versioning(
                    VersioningEngineEffect::CommitQueued { operation, commit },
                )) => Some((*operation, *commit)),
                _ => None,
            })
            .expect("queued operation ID");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            let outcomes = application.poll_commit_events();
            if !outcomes.is_empty() {
                assert_eq!(outcomes, vec![CommitEvent::Durable { operation, commit }]);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "command capture did not complete"
            );
            thread::yield_now();
        }
        let list = dispatch_versioning_command(
            &mut application,
            window,
            "commit-list",
            &[],
            CommandSource::Ui,
        )
        .expect("action listing admitted");
        let list_request = list
            .iter()
            .find_map(|effect| match effect {
                RuntimeEffect::Engine(EngineEffect::Versioning(
                    VersioningEngineEffect::CommitListQueued { request_id },
                )) => Some(*request_id),
                _ => None,
            })
            .expect("listing ticket");
        let CommitReadEvent::Listed { commits, .. } = wait_for_read(&mut application, list_request)
        else {
            panic!("action listing did not complete");
        };
        assert_eq!(commits[0].id, commit);
        let info = dispatch_versioning_command(
            &mut application,
            window,
            "commit-info",
            &[&commit.to_string()],
            CommandSource::Ipc,
        )
        .expect("IPC info admitted");
        let info_request = info
            .iter()
            .find_map(|effect| match effect {
                RuntimeEffect::Engine(EngineEffect::Versioning(
                    VersioningEngineEffect::CommitInfoQueued { request_id },
                )) => Some(*request_id),
                _ => None,
            })
            .expect("info ticket");
        let CommitReadEvent::Info {
            commit: Some(summary),
            ..
        } = wait_for_read(&mut application, info_request)
        else {
            panic!("IPC info did not complete");
        };
        assert_eq!(summary.summary.id, commit);
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[cfg(not(feature = "unqualified-clean-branch-test"))]
    #[test]
    fn default_build_keeps_clean_branch_command_unqualified() {
        let (mut application, roots, _, window, _) = durable_application();
        assert!(
            dispatch_versioning_command(
                &mut application,
                window,
                "branch",
                &["trial", "--clean"],
                CommandSource::Keyboard,
            )
            .is_err()
        );
        assert!(application.pending_clean_branch_fork.is_none());
        assert!(application.poll_clean_branch_fork_events().is_empty());
        assert!(
            dispatch_versioning_command(
                &mut application,
                window,
                "switch",
                &["main"],
                CommandSource::Keyboard,
            )
            .is_err()
        );
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[cfg(feature = "unqualified-clean-branch-test")]
    #[test]
    fn opt_in_clean_branch_command_queues_only_a_durable_fork() {
        let (mut application, roots, _, window, _) = durable_application();
        let effects = dispatch_versioning_command(
            &mut application,
            window,
            "branch",
            &["trial", "--clean"],
            CommandSource::Keyboard,
        )
        .expect("test-only clean branch admission");
        assert!(effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::Engine(EngineEffect::Versioning(
                VersioningEngineEffect::BranchQueued { .. }
            ))
        )));
        assert!(application.poll_clean_branch_fork_events().is_empty());
        let deadline = Instant::now() + Duration::from_secs(3);
        let branch = loop {
            let _ = application.poll_storage_effects();
            if let Some(super::super::CleanBranchForkEvent::Durable { branch, .. }) = application
                .poll_clean_branch_fork_events()
                .into_iter()
                .next()
            {
                break branch;
            }
            assert!(Instant::now() < deadline, "branch fork did not publish");
            thread::yield_now();
        };
        assert!(application.abort_ready_clean_branch_fork(branch));
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn sensitive_page_command_requires_native_request_policy_and_durable_normal_owner() {
        use ferric_browser_config::RuntimeOverrides;

        let (mut application, roots, _, window, _) = durable_application();
        let pending = dispatch_versioning_command(
            &mut application,
            window,
            "commit",
            &["archive", "--pages"],
            CommandSource::Keyboard,
        )
        .expect_err("archive pipeline is still unavailable even when policy asks");
        assert!(
            pending
                .diagnostic_context()
                .contains("reason=unqualified-runtime;category=page")
        );
        assert!(application.poll_commit_events().is_empty());

        let non_native = dispatch_versioning_command(
            &mut application,
            window,
            "commit",
            &["archive", "--pages"],
            CommandSource::Ipc,
        )
        .expect_err("IPC cannot stand in for a current native page request");
        assert!(
            non_native
                .diagnostic_context()
                .contains("reason=confirmation-required;category=page")
        );

        let mut denied = RuntimeOverrides::default();
        denied
            .set_setting(
                "versioning.page_capture",
                toml::Value::String("deny".into()),
            )
            .expect("typed deny setting");
        application
            .apply_configuration_layer(&denied)
            .expect("apply deny policy");
        let denied_result = dispatch_versioning_command(
            &mut application,
            window,
            "commit",
            &["archive", "--pages"],
            CommandSource::Keyboard,
        )
        .expect_err("deny policy blocks the requested archive");
        assert!(
            denied_result
                .diagnostic_context()
                .contains("reason=sensitive-capture-denied;category=page")
        );

        let mut editable_denied = RuntimeOverrides::default();
        editable_denied
            .set_setting(
                "versioning.editable_capture",
                toml::Value::String("deny".into()),
            )
            .expect("typed editable deny setting");
        application
            .apply_configuration_layer(&editable_denied)
            .expect("apply editable deny policy");
        let editable_result = dispatch_versioning_command(
            &mut application,
            window,
            "commit",
            &["archive", "--pages", "--editable"],
            CommandSource::Keyboard,
        )
        .expect_err("editable capture requires its own positive opt-in");
        assert!(
            editable_result
                .diagnostic_context()
                .contains("reason=sensitive-capture-denied;category=editable")
        );

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");

        let (mut private, window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Private, "private")
                .expect("private runtime");
        let denied = dispatch_versioning_command(
            &mut private,
            window,
            "commit",
            &["archive", "--pages"],
            CommandSource::Keyboard,
        )
        .expect_err("private origin cannot persist an archive");
        assert!(
            denied
                .diagnostic_context()
                .contains("reason=private-persistence-denied")
        );
    }

    #[test]
    fn commit_deletion_requires_live_preview_confirmation_and_is_one_shot() {
        let (mut application, roots, _, window, _) = durable_application();
        let (operation, commit, _) = application
            .begin_structural_commit(ScopeSelector::Window { window }, "delete test".into())
            .expect("commit admission");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if application.poll_commit_events() == vec![CommitEvent::Durable { operation, commit }]
            {
                break;
            }
            assert!(Instant::now() < deadline, "test commit did not complete");
            thread::yield_now();
        }

        let first_ticket = application
            .request_commit_delete_preview(window, commit)
            .expect("deletion preview request");
        let CommitReadEvent::DeletePreview {
            request_id: first_request,
            preview: Some(preview),
        } = wait_for_read(&mut application, first_ticket.value())
        else {
            panic!("deletion preview did not complete");
        };
        assert!(preview.can_delete);
        let other_window = WindowId::from_display("windowid-18446744073709551615")
            .expect("synthetic non-owner window ID");
        assert!(!application.cancel_commit_delete_preview(other_window, first_request));
        assert!(
            application
                .confirm_commit_delete_preview(other_window, first_request)
                .is_err()
        );
        assert!(application.cancel_commit_delete_preview(window, first_request));
        assert!(
            application
                .confirm_commit_delete_preview(window, first_request)
                .is_err()
        );

        let second_ticket = application
            .request_commit_delete_preview(window, commit)
            .expect("fresh deletion preview");
        let CommitReadEvent::DeletePreview {
            request_id: second_request,
            preview: Some(preview),
        } = wait_for_read(&mut application, second_ticket.value())
        else {
            panic!("fresh deletion preview did not complete");
        };
        assert!(preview.can_delete);
        let deletion_ticket = application
            .confirm_commit_delete_preview(window, second_request)
            .expect("explicit one-shot confirmation");
        let CommitReadEvent::Deleted { receipt, .. } =
            wait_for_read(&mut application, deletion_ticket.value())
        else {
            panic!("confirmed deletion did not complete");
        };
        assert_eq!(receipt.id, commit);
        assert!(!receipt.cleanup_pending);
        assert!(
            application
                .confirm_commit_delete_preview(window, second_request)
                .is_err()
        );
        let info_ticket = application
            .request_commit_info(window, commit)
            .expect("verify deleted commit is absent");
        assert!(matches!(
            wait_for_read(&mut application, info_ticket.value()),
            CommitReadEvent::Info { commit: None, .. }
        ));

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn exact_manifest_diff_is_profile_scoped_and_redacted() {
        let (mut application, roots, _, window, tab) = durable_application();
        let (left_operation, left, _) = application
            .begin_structural_commit(
                ScopeSelector::Window { window },
                "left private message".into(),
            )
            .expect("first structural commit");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if application.poll_commit_events()
                == vec![CommitEvent::Durable {
                    operation: left_operation,
                    commit: left,
                }]
            {
                break;
            }
            assert!(Instant::now() < deadline, "first commit did not complete");
            thread::yield_now();
        }
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::SetTabPinned {
                tab,
                pinned: true,
            }))
            .expect("pin current tab");
        let (right_operation, right, _) = application
            .begin_structural_commit(
                ScopeSelector::Window { window },
                "right private message".into(),
            )
            .expect("second structural commit");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if application.poll_commit_events()
                == vec![CommitEvent::Durable {
                    operation: right_operation,
                    commit: right,
                }]
            {
                break;
            }
            assert!(Instant::now() < deadline, "second commit did not complete");
            thread::yield_now();
        }

        let ticket = application
            .request_commit_diff(window, left, right, ferric_browser_core::DiffKind::Page, 0)
            .expect("exact same-profile diff");
        let CommitReadEvent::Diff(super::CommitDiffEvent::Compared {
            left: reported_left,
            right: reported_right,
            kind,
            page,
            unsupported_categories,
            ..
        }) = wait_for_read(&mut application, ticket.value())
        else {
            panic!("structural diff did not complete");
        };
        assert_eq!(reported_left, left);
        assert_eq!(reported_right, right);
        assert_eq!(kind, ferric_browser_core::DiffKind::Page);
        assert!(
            page.events.iter().any(|event| {
                event.change == ferric_browser_core::StructuralChange::PinnedChanged
            })
        );
        assert!(page.total_events >= page.events.len());
        assert_eq!(unsupported_categories.len(), 1);
        assert!(unsupported_categories[0].contains("DOM/page-body content"));
        let serialized = serde_json::to_string(&page).expect("serialize redacted change page");
        assert!(!serialized.contains("left private message"));
        assert!(!serialized.contains("right private message"));
        assert!(!serialized.contains("https://"));

        let cookie_ticket = application
            .request_commit_diff(
                window,
                left,
                right,
                ferric_browser_core::DiffKind::Cookies,
                0,
            )
            .expect("coverage-only cookie diff");
        let CommitReadEvent::Diff(super::CommitDiffEvent::Compared {
            unsupported_categories,
            page,
            ..
        }) = wait_for_read(&mut application, cookie_ticket.value())
        else {
            panic!("cookie category did not return its unsupported projection");
        };
        assert!(page.events.is_empty());
        assert_eq!(unsupported_categories.len(), 1);
        assert!(unsupported_categories[0].contains("values are not captured"));
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn late_commit_diff_results_are_rejected_after_native_owner_rebind() {
        let (mut application, roots, _, window, _) = durable_application();
        let prepared = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "synthetic diff".into())
            .expect("valid disposable structural manifest");
        let manifest_id = prepared
            .manifest
            .id
            .parse::<CommitId>()
            .expect("manifest commit id");
        let ticket = application
            .request_commit_diff(
                window,
                manifest_id,
                manifest_id,
                ferric_browser_core::DiffKind::Page,
                0,
            )
            .expect("profile-scoped diff request");
        let request_id = ticket.value();
        let diff_tickets = application
            .pending_commit_reads
            .iter()
            .filter_map(|(ticket, kind)| {
                matches!(kind, CommitReadKind::Diff(id, _) if *id == request_id).then_some(*ticket)
            })
            .collect::<Vec<_>>();
        assert_eq!(diff_tickets.len(), 2);

        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("new native owner generation");
        for (index, diff_ticket) in diff_tickets.into_iter().enumerate() {
            let forwarded = application.route_commit_storage_effects(vec![StorageEffect {
                ticket: diff_ticket,
                completion: StorageCompletion::VersioningManifest(Ok(Some(
                    prepared.manifest.clone(),
                ))),
            }]);
            assert!(forwarded.is_empty());
            let events = application.poll_commit_read_events();
            if index == 0 {
                assert_eq!(
                    events,
                    vec![CommitReadEvent::Diff(CommitDiffEvent::Failed {
                        request_id
                    })]
                );
                assert!(application.pending_structural_diffs.is_empty());
            } else {
                assert!(events.is_empty(), "second stale result is ignored");
            }
        }
        assert!(application.pending_structural_diffs.is_empty());

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary profile roots cleanup");
    }

    #[test]
    fn durable_commit_keeps_admitted_revision_and_survives_worker_restart() {
        let (mut application, roots, profile_uuid, window, tab) = durable_application();
        let (operation, commit, admission_effects) = application
            .begin_structural_commit(ScopeSelector::Window { window }, "before pin".into())
            .expect("admit structural commit");
        assert!(admission_effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::AppliedEvent(Event::Versioning(
                VersioningEvent::BindStructuralIdentity { .. }
            ))
        )));
        let admitted = application
            .pending_commit
            .as_ref()
            .unwrap()
            .manifest
            .clone();
        assert_eq!(admitted.windows[0].pages.len(), 1);
        assert!(!admitted.windows[0].pages[0].pinned);
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::SetTabPinned {
                tab,
                pinned: true,
            }))
            .expect("later tab mutation");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            let outcomes = application.poll_commit_events();
            if !outcomes.is_empty() {
                assert_eq!(outcomes, vec![CommitEvent::Durable { operation, commit }]);
                break;
            }
            assert!(Instant::now() < deadline, "capture did not complete");
            thread::yield_now();
        }
        assert!(application.tabs()[&tab].pinned);
        assert_eq!(admitted.application_revision + 1, application.revision());
        let list_ticket = application
            .request_commit_list(window, 10, 0)
            .expect("bounded current-profile listing");
        let CommitReadEvent::Listed { commits, .. } =
            wait_for_read(&mut application, list_ticket.value())
        else {
            panic!("unexpected listing result");
        };
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].id, commit);
        let info_ticket = application
            .request_commit_info(window, commit)
            .expect("current-profile commit info");
        let CommitReadEvent::Info {
            commit: Some(info), ..
        } = wait_for_read(&mut application, info_ticket.value())
        else {
            panic!("unexpected info result");
        };
        assert_eq!(info.summary.id, commit);
        application.close_profile();
        let database = roots
            .data
            .join("profiles")
            .join(profile_uuid.to_string())
            .join("browser.sqlite");
        let stored = stored_manifest(&database, commit);
        assert_eq!(stored.application_revision, admitted.application_revision);
        assert!(!stored.windows[0].pages[0].pinned);
        let mut worker = ferric_browser_storage::ProfileStoreWorker::spawn(&database)
            .expect("reopen durable store");
        worker
            .try_submit(StorageCommand::VersioningList {
                profile_uuid,
                limit: 10,
                offset: 0,
            })
            .expect("list after restart");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(completion) = worker.poll_events().into_iter().next() {
                let StorageCompletion::VersioningList(Ok(commits)) = completion else {
                    panic!("unexpected list completion");
                };
                assert_eq!(commits.len(), 1);
                assert_eq!(commits[0].id, commit);
                break;
            }
            assert!(Instant::now() < deadline, "list did not complete");
            thread::yield_now();
        }
        drop(worker);
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn late_commit_list_result_is_rejected_after_native_owner_rebind() {
        let (mut application, roots, _, window, _) = durable_application();
        let ticket = application
            .request_commit_list(window, 10, 0)
            .expect("bounded listing request");
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("new native owner generation");

        let forwarded = application.route_commit_storage_effects(vec![StorageEffect {
            ticket,
            completion: StorageCompletion::VersioningList(Ok(Vec::new())),
        }]);
        assert!(forwarded.is_empty());
        assert_eq!(
            application.poll_commit_read_events(),
            vec![CommitReadEvent::Failed {
                request_id: ticket.value(),
            }]
        );

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn commit_list_uses_one_row_lookahead_for_exact_pagination() {
        let (mut application, roots, _, window, _) = durable_application();
        let requested_limit = 2;
        let first_ticket = application
            .request_commit_list(window, requested_limit, 0)
            .expect("bounded listing request");
        let row = || CommitSummary {
            id: CommitId::fresh(),
            created_at_ms: 1,
            pinned: false,
            object_links: 0,
        };
        let forwarded = application.route_commit_storage_effects(vec![StorageEffect {
            ticket: first_ticket,
            completion: StorageCompletion::VersioningList(Ok(vec![row(), row(), row()])),
        }]);
        assert!(forwarded.is_empty());
        let first_events = application.poll_commit_read_events();
        let [
            CommitReadEvent::Listed {
                commits,
                limit,
                has_next,
                ..
            },
        ] = first_events.as_slice()
        else {
            panic!("lookahead listing event");
        };
        assert_eq!(*limit, requested_limit);
        assert_eq!(commits.len(), requested_limit);
        assert!(*has_next);

        let final_ticket = application
            .request_commit_list(window, requested_limit, requested_limit)
            .expect("second bounded listing request");
        let forwarded = application.route_commit_storage_effects(vec![StorageEffect {
            ticket: final_ticket,
            completion: StorageCompletion::VersioningList(Ok(vec![row(), row()])),
        }]);
        assert!(forwarded.is_empty());
        let final_events = application.poll_commit_read_events();
        let [
            CommitReadEvent::Listed {
                commits, has_next, ..
            },
        ] = final_events.as_slice()
        else {
            panic!("final page listing event");
        };
        assert_eq!(commits.len(), requested_limit);
        assert!(!has_next);

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn late_commit_info_and_delete_results_are_rejected_after_native_owner_rebind() {
        let (mut application, roots, _, window, _) = durable_application();
        let info_ticket = application
            .request_commit_info(window, CommitId::fresh())
            .expect("commit info request");
        let preview_ticket = application
            .request_commit_delete_preview(window, CommitId::fresh())
            .expect("commit delete preview request");
        let delete_ticket = application
            .request_commit_info(window, CommitId::fresh())
            .expect("ticket for confirmed-delete completion");
        let (profile_uuid, native) = application
            .structural_commit_owner(window)
            .expect("current exact native owner");
        // A confirmed delete uses this same ticket-bound read kind. Reusing a
        // harmless pending info ticket lets the test inject its late storage
        // completion without mutating the disposable profile's commit graph.
        application.pending_commit_reads.insert(
            delete_ticket,
            CommitReadKind::Delete {
                window,
                profile_uuid,
                native,
            },
        );

        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("new native owner generation");
        let receipt = CommitDeleteReceipt {
            id: CommitId::fresh(),
            reclaimable_objects: 0,
            reclaimable_bytes: 0,
            cleanup_pending: false,
            quarantined_staging_operations: 0,
            quarantined_orphan_objects: 0,
            quarantined_corrupt_objects: 0,
            missing_objects: 0,
        };
        let forwarded = application.route_commit_storage_effects(vec![
            StorageEffect {
                ticket: info_ticket,
                completion: StorageCompletion::VersioningInfo(Ok(None)),
            },
            StorageEffect {
                ticket: preview_ticket,
                completion: StorageCompletion::VersioningDeletePreview(Ok(None)),
            },
            StorageEffect {
                ticket: delete_ticket,
                completion: StorageCompletion::VersioningDelete(Ok(receipt)),
            },
        ]);
        assert!(forwarded.is_empty());
        assert_eq!(
            application.poll_commit_read_events(),
            vec![
                CommitReadEvent::Failed {
                    request_id: info_ticket.value(),
                },
                CommitReadEvent::Failed {
                    request_id: preview_ticket.value(),
                },
                CommitReadEvent::Failed {
                    request_id: delete_ticket.value(),
                },
            ]
        );

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary profile roots cleanup");
    }

    #[test]
    fn transient_profiles_cannot_prepare_or_persist_a_commit() {
        for privacy in [PrivacyKind::Private, PrivacyKind::Ephemeral] {
            let (mut application, window, _) =
                BrowserApplication::bootstrap(Config::default(), privacy, "transient")
                    .expect("transient runtime");
            application
                .activate_profile(ProfileActivation {
                    name: "transient".into(),
                    id: None,
                    privacy,
                    roots: None,
                    lock: None,
                    contexts: None,
                    storage_path: None,
                })
                .expect("transient profile");
            let profile = application.windows()[&window].profile;
            application
                .bind_native_profile(profile, ProfileInstanceId::fresh())
                .expect("native owner");
            assert_eq!(
                application
                    .begin_structural_commit(ScopeSelector::Window { window }, "forbidden".into(),)
                    .unwrap_err()
                    .reason(),
                VersioningReason::PrivatePersistenceDenied
            );
            assert!(application.pending_commit.is_none());
            assert!(application.poll_commit_events().is_empty());
        }
    }

    #[test]
    fn another_runtime_profile_cannot_write_the_current_profile_database() {
        let (mut application, roots, _, _, _) = durable_application();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateProfile {
                label: "other".into(),
                privacy: PrivacyKind::Normal,
            }))
            .expect("second runtime profile");
        let other = *application.profiles().keys().next_back().unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow {
                profile: other,
            }))
            .expect("second runtime window");
        let window = *application.windows().keys().next_back().unwrap();
        application
            .bind_native_profile(other, ProfileInstanceId::fresh())
            .expect("second native owner");
        assert_eq!(
            application
                .begin_structural_commit(ScopeSelector::Window { window }, "wrong profile".into(),)
                .unwrap_err()
                .reason(),
            VersioningReason::WrongProfile
        );
        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn qualified_command_listing_discovers_a_commit_after_application_restart() {
        let (mut application, roots, profile_uuid, window, _) = durable_application();
        let (_, commit, _) = application
            .begin_structural_commit(ScopeSelector::Window { window }, "restart listing".into())
            .expect("capture admitted");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if let Some(event) = application.poll_commit_events().into_iter().next() {
                assert!(matches!(event, CommitEvent::Durable { commit: id, .. } if id == commit));
                break;
            }
            assert!(Instant::now() < deadline, "capture did not complete");
            thread::yield_now();
        }
        application.close_profile();
        drop(application);

        let (mut reopened, reopened_window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("restarted runtime");
        reopened
            .activate_profile(ProfileActivation {
                name: "default".into(),
                id: Some(profile_uuid),
                privacy: PrivacyKind::Normal,
                roots: Some(roots.clone()),
                lock: None,
                contexts: None,
                storage_path: Some(
                    roots
                        .data
                        .join("profiles")
                        .join(profile_uuid.to_string())
                        .join("browser.sqlite"),
                ),
            })
            .expect("restarted durable owner");
        let runtime_profile = reopened.windows()[&reopened_window].profile;
        reopened
            .bind_native_profile(runtime_profile, ProfileInstanceId::fresh())
            .expect("restarted native owner");
        let effects = dispatch_versioning_command(
            &mut reopened,
            reopened_window,
            "commit-list",
            &[],
            CommandSource::Ui,
        )
        .expect("restarted command listing");
        let request_id = effects
            .iter()
            .find_map(|effect| match effect {
                RuntimeEffect::Engine(EngineEffect::Versioning(
                    VersioningEngineEffect::CommitListQueued { request_id },
                )) => Some(*request_id),
                _ => None,
            })
            .expect("listing request ID");
        let CommitReadEvent::Listed { commits, .. } = wait_for_read(&mut reopened, request_id)
        else {
            panic!("listing did not complete");
        };
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].id, commit);
        reopened.close_profile();
        drop(reopened);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn ui_action_and_ipc_sourced_commits_reach_durable_publication() {
        for source in [CommandSource::Ui, CommandSource::Ipc] {
            let (mut application, roots, _, window, _) = durable_application();
            let effects = dispatch_versioning_command(
                &mut application,
                window,
                "commit",
                &["source routing"],
                source,
            )
            .expect("source-scoped commit admission");
            let (operation, commit) = effects
                .iter()
                .find_map(|effect| match effect {
                    RuntimeEffect::Engine(EngineEffect::Versioning(
                        VersioningEngineEffect::CommitQueued { operation, commit },
                    )) => Some((*operation, *commit)),
                    _ => None,
                })
                .expect("queued source-scoped commit");
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let _ = application.poll_storage_effects();
                if let Some(event) = application.poll_commit_events().into_iter().next() {
                    assert_eq!(event, CommitEvent::Durable { operation, commit });
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "source-scoped commit did not complete"
                );
                thread::yield_now();
            }
            application.close_profile();
            drop(application);
            roots.cleanup().expect("temporary roots cleanup");
        }
    }
}
