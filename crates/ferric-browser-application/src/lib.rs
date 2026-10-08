//! Qt-independent application orchestration for Ferric Browser.
//!
//! This is the only layer allowed to combine the reducer runtime, effective
//! configuration, and Rust-owned durable metadata. Presentation adapters call
//! the domain-specific methods on [`BrowserApplication`] and execute the
//! returned effects; they do not own application state or persistence
//! scheduling.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    time::Instant,
};

use ferric_browser_config::{Config, ConfigError, RuntimeOverrides};
use ferric_browser_core::{
    ApplicationOwnerId, CommandContext, CommandSource, DispatchTarget, JourneyGraph,
    NativeProfileOwner, PrivacyKind, ProfileId, ProfileInstanceId, ProfileState, RetainedViewId,
    RetentionRecord, ScopeSelector, ShutdownState, TabId, TabState, Target, WindowId, WindowState,
};
use ferric_browser_runtime::{
    BrowserRuntime, RuntimeEffect, RuntimeError, RuntimeInput, RuntimeSnapshot, RuntimeView,
};
use ferric_browser_storage::{
    ContextRecord, ContextRegistry, ContextRegistryError, StorageWorkerError,
};
use uuid::Uuid;

mod configuration;
mod profile;
mod profile_open;
mod storage;
pub mod versioning;

use configuration::ConfigurationState;
pub use configuration::{ConfigurationLayers, ConfigurationSnapshot};
pub use ferric_browser_storage::StorageCommand as StorageRequest;
use profile::ActiveProfile;
use profile::ProfileActivation;
pub use profile::{ContextChange, ContextCommand, ContextEffect, ProfileSnapshot};
use profile_open::{PreparedProfile, ProfileOpenCoordinator};
pub use profile_open::{ProfileOpenEffect, ProfileOpenError, ProfileOpenRequest, ProfileStorage};
use storage::StorageCoordinator;
pub use storage::{StorageEffect, StorageTicket};

/// The Qt-independent owner of runtime state, effective configuration, and
/// optional durable metadata work.
pub struct BrowserApplication {
    runtime: BrowserRuntime,
    configuration: ConfigurationState,
    active_profile: Option<ActiveProfile>,
    active_profile_runtime: Option<ProfileId>,
    next_profile_generation: u64,
    storage: StorageCoordinator,
    profile_open: Option<ProfileOpenCoordinator>,
    application_owner: ApplicationOwnerId,
    versioning: versioning::VersioningCoordinator,
    clean_branch_profiles: versioning::CleanBranchProfileLeases,
    pending_clean_branch_fork: Option<versioning::PendingCleanBranchFork>,
    ready_clean_branch_forks:
        BTreeMap<ferric_browser_core::BranchId, versioning::PendingCleanBranchFork>,
    published_clean_branches:
        BTreeMap<ferric_browser_core::BranchId, versioning::PublishedCleanBranch>,
    clean_branch_fork_events: VecDeque<versioning::CleanBranchForkEvent>,
    pending_branch_structural_export: Option<versioning::PendingBranchStructuralExport>,
    previews: versioning::PreviewBook,
    pending_stashes: BTreeMap<ferric_browser_core::OperationId, versioning::PendingStash>,
    pending_page_capture_reservations:
        BTreeMap<StorageTicket, (ferric_browser_core::OperationId, Uuid, PathBuf, bool)>,
    pending_page_capture_stage_effects: BTreeMap<
        StorageTicket,
        (
            StorageTicket,
            ferric_browser_core::OperationId,
            Uuid,
            PathBuf,
            bool,
            usize,
        ),
    >,
    pending_archive_view_manifests: BTreeMap<StorageTicket, versioning::PendingArchiveViewManifest>,
    pending_archive_view_objects: BTreeMap<StorageTicket, versioning::PendingArchiveViewObject>,
    pending_archive_view_cleanups: BTreeMap<StorageTicket, ferric_browser_core::OperationId>,
    active_archive_views: BTreeMap<ferric_browser_core::OperationId, versioning::ActiveArchiveView>,
    pending_page_capture_install_effects: BTreeMap<
        StorageTicket,
        (
            ferric_browser_core::OperationId,
            Uuid,
            PathBuf,
            bool,
            Option<usize>,
        ),
    >,
    pending_window_page_sidecar_effects: BTreeMap<
        StorageTicket,
        (
            ferric_browser_core::OperationId,
            usize,
            ferric_browser_core::versioning::SidecarSummary,
        ),
    >,
    pending_page_capture_abort_effects:
        BTreeMap<StorageTicket, (ferric_browser_core::OperationId, Uuid)>,
    pending_page_capture_publications:
        BTreeMap<StorageTicket, (ferric_browser_core::OperationId, Instant)>,
    page_capture_reservations_held: BTreeMap<ferric_browser_core::OperationId, Uuid>,
    page_capture_abort_requested: BTreeSet<ferric_browser_core::OperationId>,
    ready_page_capture_stages: BTreeMap<ferric_browser_core::OperationId, usize>,
    page_capture_installed_archives: BTreeMap<
        ferric_browser_core::OperationId,
        BTreeMap<usize, ferric_browser_storage::versioning::InstalledObject>,
    >,
    page_capture_installed_sidecars: BTreeMap<
        ferric_browser_core::OperationId,
        BTreeMap<
            usize,
            (
                ferric_browser_storage::versioning::InstalledObject,
                ferric_browser_core::versioning::SidecarSummary,
            ),
        >,
    >,
    page_capture_archive_failures: BTreeMap<
        ferric_browser_core::OperationId,
        BTreeMap<usize, ferric_browser_storage::versioning::ManifestPageArchiveFailure>,
    >,
    page_capture_failure_policies:
        BTreeMap<ferric_browser_core::OperationId, versioning::PageCaptureFailurePolicy>,
    page_capture_stage_tickets: BTreeMap<ferric_browser_core::OperationId, versioning::StageTicket>,
    pending_commit: Option<versioning::PendingCommit>,
    commit_events: VecDeque<versioning::CommitEvent>,
    pending_commit_reads: BTreeMap<StorageTicket, versioning::CommitReadKind>,
    commit_read_events: VecDeque<versioning::CommitReadEvent>,
    pending_commit_deletions: BTreeMap<u64, versioning::PendingCommitDelete>,
    pending_structural_diffs: BTreeMap<u64, versioning::PendingStructuralDiff>,
    pending_restore_plans: BTreeMap<StorageTicket, versioning::PendingRestorePlan>,
    restore_plan_events: VecDeque<versioning::RestorePlanEvent>,
    policy_restore_events: VecDeque<versioning::PolicyRestoreEvent>,
    pending_policy_restore_previews: BTreeMap<u64, versioning::PolicyRestorePreview>,
    context_capture_owners: BTreeMap<Uuid, NativeProfileOwner>,
    pending_restore_sidecars: BTreeMap<StorageTicket, versioning::PendingRestoreSidecar>,
    pending_restore_sidecar_intents: BTreeMap<TabId, versioning::PendingRestoreSidecarIntent>,
    pending_lazy_restore_pages: BTreeMap<TabId, versioning::PendingLazyRestorePage>,
    restore_sidecars: BTreeMap<u64, versioning::StoredRestoreSidecar>,
    restore_sidecar_events: VecDeque<versioning::RestoreSidecarEvent>,
    safe_get_navigation_proofs: BTreeMap<TabId, (Target, String)>,
}

impl BrowserApplication {
    #[must_use]
    pub fn new(configuration: Config) -> Self {
        Self {
            runtime: BrowserRuntime::new(),
            configuration: ConfigurationState::new(configuration),
            active_profile: None,
            active_profile_runtime: None,
            next_profile_generation: 1,
            storage: StorageCoordinator::default(),
            profile_open: None,
            application_owner: ApplicationOwnerId::fresh(),
            versioning: versioning::VersioningCoordinator::default(),
            clean_branch_profiles: versioning::CleanBranchProfileLeases::new(),
            pending_clean_branch_fork: None,
            ready_clean_branch_forks: BTreeMap::new(),
            published_clean_branches: BTreeMap::new(),
            clean_branch_fork_events: VecDeque::new(),
            pending_branch_structural_export: None,
            previews: versioning::PreviewBook::default(),
            pending_stashes: BTreeMap::new(),
            pending_page_capture_reservations: BTreeMap::new(),
            pending_page_capture_stage_effects: BTreeMap::new(),
            pending_archive_view_manifests: BTreeMap::new(),
            pending_archive_view_objects: BTreeMap::new(),
            pending_archive_view_cleanups: BTreeMap::new(),
            active_archive_views: BTreeMap::new(),
            pending_page_capture_install_effects: BTreeMap::new(),
            pending_window_page_sidecar_effects: BTreeMap::new(),
            pending_page_capture_abort_effects: BTreeMap::new(),
            pending_page_capture_publications: BTreeMap::new(),
            page_capture_reservations_held: BTreeMap::new(),
            page_capture_abort_requested: BTreeSet::new(),
            ready_page_capture_stages: BTreeMap::new(),
            page_capture_installed_archives: BTreeMap::new(),
            page_capture_installed_sidecars: BTreeMap::new(),
            page_capture_archive_failures: BTreeMap::new(),
            page_capture_failure_policies: BTreeMap::new(),
            page_capture_stage_tickets: BTreeMap::new(),
            pending_commit: None,
            commit_events: VecDeque::new(),
            pending_commit_reads: BTreeMap::new(),
            commit_read_events: VecDeque::new(),
            pending_commit_deletions: BTreeMap::new(),
            pending_structural_diffs: BTreeMap::new(),
            pending_restore_plans: BTreeMap::new(),
            restore_plan_events: VecDeque::new(),
            policy_restore_events: VecDeque::new(),
            pending_policy_restore_previews: BTreeMap::new(),
            context_capture_owners: BTreeMap::new(),
            pending_restore_sidecars: BTreeMap::new(),
            pending_restore_sidecar_intents: BTreeMap::new(),
            pending_lazy_restore_pages: BTreeMap::new(),
            restore_sidecars: BTreeMap::new(),
            restore_sidecar_events: VecDeque::new(),
            safe_get_navigation_proofs: BTreeMap::new(),
        }
    }

    /// Adopts an already bootstrapped runtime at the application boundary.
    ///
    /// This exists for presentation adapters that create their runtime during
    /// native startup. New entry points should prefer [`Self::bootstrap`].
    #[must_use]
    pub fn from_runtime(runtime: BrowserRuntime, configuration: Config) -> Self {
        Self {
            runtime,
            configuration: ConfigurationState::new(configuration),
            active_profile: None,
            active_profile_runtime: None,
            next_profile_generation: 1,
            storage: StorageCoordinator::default(),
            profile_open: None,
            application_owner: ApplicationOwnerId::fresh(),
            versioning: versioning::VersioningCoordinator::default(),
            clean_branch_profiles: versioning::CleanBranchProfileLeases::new(),
            pending_clean_branch_fork: None,
            ready_clean_branch_forks: BTreeMap::new(),
            published_clean_branches: BTreeMap::new(),
            clean_branch_fork_events: VecDeque::new(),
            pending_branch_structural_export: None,
            previews: versioning::PreviewBook::default(),
            pending_stashes: BTreeMap::new(),
            pending_page_capture_reservations: BTreeMap::new(),
            pending_page_capture_stage_effects: BTreeMap::new(),
            pending_archive_view_manifests: BTreeMap::new(),
            pending_archive_view_objects: BTreeMap::new(),
            pending_archive_view_cleanups: BTreeMap::new(),
            active_archive_views: BTreeMap::new(),
            pending_page_capture_install_effects: BTreeMap::new(),
            pending_window_page_sidecar_effects: BTreeMap::new(),
            pending_page_capture_abort_effects: BTreeMap::new(),
            pending_page_capture_publications: BTreeMap::new(),
            page_capture_reservations_held: BTreeMap::new(),
            page_capture_abort_requested: BTreeSet::new(),
            ready_page_capture_stages: BTreeMap::new(),
            page_capture_installed_archives: BTreeMap::new(),
            page_capture_installed_sidecars: BTreeMap::new(),
            page_capture_archive_failures: BTreeMap::new(),
            page_capture_failure_policies: BTreeMap::new(),
            page_capture_stage_tickets: BTreeMap::new(),
            pending_commit: None,
            commit_events: VecDeque::new(),
            pending_commit_reads: BTreeMap::new(),
            commit_read_events: VecDeque::new(),
            pending_commit_deletions: BTreeMap::new(),
            pending_structural_diffs: BTreeMap::new(),
            pending_restore_plans: BTreeMap::new(),
            restore_plan_events: VecDeque::new(),
            policy_restore_events: VecDeque::new(),
            pending_policy_restore_previews: BTreeMap::new(),
            context_capture_owners: BTreeMap::new(),
            pending_restore_sidecars: BTreeMap::new(),
            pending_restore_sidecar_intents: BTreeMap::new(),
            pending_lazy_restore_pages: BTreeMap::new(),
            restore_sidecars: BTreeMap::new(),
            restore_sidecar_events: VecDeque::new(),
            safe_get_navigation_proofs: BTreeMap::new(),
        }
    }

    /// Creates initial state through the same application boundary used after
    /// startup.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime cannot establish its initial profile,
    /// window, and tab.
    pub fn bootstrap(
        configuration: Config,
        privacy: PrivacyKind,
        profile_label: impl Into<String>,
    ) -> Result<(Self, WindowId, TabId), RuntimeError> {
        let (runtime, window, tab) = BrowserRuntime::bootstrap(privacy, profile_label)?;
        Ok((
            Self {
                runtime,
                configuration: ConfigurationState::new(configuration),
                active_profile: None,
                active_profile_runtime: None,
                next_profile_generation: 1,
                storage: StorageCoordinator::default(),
                profile_open: None,
                application_owner: ApplicationOwnerId::fresh(),
                versioning: versioning::VersioningCoordinator::default(),
                clean_branch_profiles: versioning::CleanBranchProfileLeases::new(),
                pending_clean_branch_fork: None,
                ready_clean_branch_forks: BTreeMap::new(),
                published_clean_branches: BTreeMap::new(),
                clean_branch_fork_events: VecDeque::new(),
                pending_branch_structural_export: None,
                previews: versioning::PreviewBook::default(),
                pending_stashes: BTreeMap::new(),
                pending_page_capture_reservations: BTreeMap::new(),
                pending_page_capture_stage_effects: BTreeMap::new(),
                pending_archive_view_manifests: BTreeMap::new(),
                pending_archive_view_objects: BTreeMap::new(),
                pending_archive_view_cleanups: BTreeMap::new(),
                active_archive_views: BTreeMap::new(),
                pending_page_capture_install_effects: BTreeMap::new(),
                pending_window_page_sidecar_effects: BTreeMap::new(),
                pending_page_capture_abort_effects: BTreeMap::new(),
                pending_page_capture_publications: BTreeMap::new(),
                page_capture_reservations_held: BTreeMap::new(),
                page_capture_abort_requested: BTreeSet::new(),
                ready_page_capture_stages: BTreeMap::new(),
                page_capture_installed_archives: BTreeMap::new(),
                page_capture_installed_sidecars: BTreeMap::new(),
                page_capture_archive_failures: BTreeMap::new(),
                page_capture_failure_policies: BTreeMap::new(),
                page_capture_stage_tickets: BTreeMap::new(),
                pending_commit: None,
                commit_events: VecDeque::new(),
                pending_commit_reads: BTreeMap::new(),
                commit_read_events: VecDeque::new(),
                pending_commit_deletions: BTreeMap::new(),
                pending_structural_diffs: BTreeMap::new(),
                pending_restore_plans: BTreeMap::new(),
                restore_plan_events: VecDeque::new(),
                policy_restore_events: VecDeque::new(),
                pending_policy_restore_previews: BTreeMap::new(),
                context_capture_owners: BTreeMap::new(),
                pending_restore_sidecars: BTreeMap::new(),
                pending_restore_sidecar_intents: BTreeMap::new(),
                pending_lazy_restore_pages: BTreeMap::new(),
                restore_sidecars: BTreeMap::new(),
                restore_sidecar_events: VecDeque::new(),
                safe_get_navigation_proofs: BTreeMap::new(),
            },
            window,
            tab,
        ))
    }

    #[must_use]
    pub const fn runtime_view(&self) -> RuntimeView<'_> {
        self.runtime.view()
    }

    #[must_use]
    pub fn profiles(&self) -> &BTreeMap<ProfileId, ProfileState> {
        self.runtime_view().profiles()
    }

    #[must_use]
    pub fn windows(&self) -> &BTreeMap<WindowId, WindowState> {
        self.runtime_view().windows()
    }

    #[must_use]
    pub fn tabs(&self) -> &BTreeMap<TabId, TabState> {
        self.runtime_view().tabs()
    }

    #[must_use]
    pub fn visible_versioning_tabs_for_window(&self, window: WindowId) -> Option<Vec<TabId>> {
        self.runtime_view()
            .visible_versioning_tabs_for_window(window)
    }

    #[must_use]
    pub fn has_unresolved_child_branch_for_profile(&self, profile: ProfileId) -> bool {
        self.runtime_view()
            .has_unresolved_child_branch_for_profile(profile)
    }

    #[must_use]
    pub fn versioning_retention(&self, view: RetainedViewId) -> Option<&RetentionRecord> {
        self.runtime_view().versioning_retention(view)
    }

    #[must_use]
    pub fn journey(&self) -> &JourneyGraph {
        self.runtime_view().journey()
    }

    #[must_use]
    pub const fn active_window(&self) -> Option<WindowId> {
        self.runtime_view().active_window()
    }

    #[must_use]
    pub const fn last_focused_window(&self) -> Option<WindowId> {
        self.runtime_view().last_focused_window()
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.runtime_view().revision()
    }

    #[must_use]
    pub const fn shutdown(&self) -> ShutdownState {
        self.runtime_view().shutdown()
    }

    #[must_use]
    pub fn active_tab(&self) -> Option<&TabState> {
        self.runtime_view().active_tab()
    }

    #[must_use]
    pub fn capture_target(&self, tab: TabId) -> Option<Target> {
        self.runtime_view().capture_target(tab)
    }

    #[must_use]
    pub fn capture_command_context(
        &self,
        selector: DispatchTarget,
        source: CommandSource,
        count: u32,
        operation_id: Option<String>,
    ) -> CommandContext {
        self.runtime
            .capture_command_context(selector, source, count, operation_id)
    }

    #[must_use]
    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.runtime.snapshot()
    }

    #[must_use]
    pub const fn configuration(&self) -> &Config {
        self.configuration.effective()
    }

    #[must_use]
    pub const fn configuration_revision(&self) -> u64 {
        self.configuration.revision()
    }

    #[must_use]
    pub const fn application_owner(&self) -> ApplicationOwnerId {
        self.application_owner
    }

    #[must_use]
    pub const fn versioning(&self) -> &versioning::VersioningCoordinator {
        &self.versioning
    }

    /// Binds one native profile lifetime to this application owner.
    /// Rebinding invalidates the former lifetime's operations and permits.
    ///
    /// # Errors
    /// Rejects an unknown logical profile.
    pub fn bind_native_profile(
        &mut self,
        profile: ProfileId,
        instance: ProfileInstanceId,
    ) -> Result<versioning::VersioningEffects, versioning::OperationError> {
        if !self.profiles().contains_key(&profile) {
            return Err(versioning::OperationError::OwnerClosed);
        }
        let native = NativeProfileOwner::new(self.application_owner, profile, instance);
        if self.active_profile.is_some() && self.active_profile_runtime.is_none() {
            self.active_profile_runtime = Some(profile);
        }
        let mut runtime_effects = Vec::new();
        if let Some(old) = self.versioning.owner(profile) {
            if old != native {
                self.cancel_page_capture_jobs_for_profile(profile);
                self.schedule_archive_view_cleanup_for_profile(profile);
                self.discard_restore_sidecars_for_owner(old);
                runtime_effects = self
                    .release_pending_stashes_for_profile(profile)
                    .map_err(|_| versioning::OperationError::InvalidTransition)?;
                self.previews.invalidate_owner(old);
            }
        }
        let mut operation_effects = self.versioning.bind_owner(native);
        self.suppress_terminal_page_capture_cleanup_effects(&mut operation_effects);
        Ok(versioning::VersioningEffects {
            runtime_effects,
            operation_effects,
            native_cleanup_pending: Vec::new(),
            cleanup_failed: Vec::new(),
        })
    }

    /// Admits a versioning operation against an explicit, frozen subject set.
    ///
    /// # Errors
    /// Rejects stale owners, unsupported privacy, invalid deadlines, or a full queue.
    pub fn request_versioning_operation(
        &mut self,
        selector: ScopeSelector,
        kind: versioning::OperationKind,
        timeout_ms: u64,
    ) -> Result<
        (
            ferric_browser_core::OperationId,
            Vec<versioning::OperationEffect>,
        ),
        versioning::OperationError,
    > {
        let window = match selector {
            ScopeSelector::Tab { window, .. } | ScopeSelector::Window { window } => window,
            ScopeSelector::Context { .. } => {
                return Err(versioning::OperationError::Scope(
                    ferric_browser_core::ScopeError::ContextUnavailable,
                ));
            }
        };
        let profile = self
            .windows()
            .get(&window)
            .ok_or(versioning::OperationError::OwnerClosed)?
            .profile;
        let privacy = self
            .profiles()
            .get(&profile)
            .ok_or(versioning::OperationError::OwnerClosed)?
            .privacy;
        let native = self
            .versioning
            .owner(profile)
            .ok_or(versioning::OperationError::OwnerClosed)?;
        let scope = self
            .runtime_view()
            .resolve_versioning_scope(selector, native)?;
        self.versioning.admit(
            scope,
            self.configuration_revision(),
            privacy,
            kind,
            timeout_ms,
        )
    }

    /// Advances a previously admitted operation without resolving current focus.
    ///
    /// # Errors
    /// Rejects stale owners, deadlines, and invalid state transitions.
    pub fn advance_versioning_operation(
        &mut self,
        id: ferric_browser_core::OperationId,
        next: versioning::OperationState,
    ) -> Result<(), versioning::OperationError> {
        self.versioning.advance(id, next)
    }

    /// Transfers the non-serializable native-decision capability once. The
    /// trusted UI adapter must keep it away from page, command and IPC paths.
    pub fn take_native_versioning_decision_sink(
        &mut self,
    ) -> Option<versioning::NativeDecisionSink> {
        self.previews.take_native_sink()
    }

    /// Prepares one immutable preview for an operation awaiting native consent.
    /// The security revision is the current effective configuration revision
    /// until the dedicated security-policy revision is introduced.
    ///
    /// # Errors
    /// Rejects stale owners, invalid stage and excess pending previews.
    pub fn prepare_versioning_preview(
        &mut self,
        id: ferric_browser_core::OperationId,
        draft: versioning::PreviewDraft,
    ) -> Result<versioning::Preview, versioning::PreviewError> {
        let (envelope, state) = self
            .versioning
            .operation(id)
            .ok_or(versioning::PreviewError::StalePlan)?;
        self.previews.prepare(
            versioning::PreviewContext {
                envelope,
                state,
                current_owner: self.versioning.owner(envelope.owner().runtime_profile()),
                configuration_revision: self.configuration_revision(),
                security_revision: self.configuration_revision(),
                now_ms: self.versioning.now_millis(),
            },
            draft,
        )
    }

    /// Captures an actual native UI decision for the exact displayed preview.
    /// No IPC or page boolean is accepted as a decision.
    ///
    /// # Errors
    /// Rejects changed previews, stale ownership/policy, expiry and replay.
    pub fn capture_native_versioning_decision(
        &mut self,
        id: ferric_browser_core::OperationId,
        proof: versioning::NativeDecisionProof,
    ) -> Result<versioning::ConfirmationPermit, versioning::PreviewError> {
        let (envelope, state) = self
            .versioning
            .operation(id)
            .ok_or(versioning::PreviewError::StalePlan)?;
        self.previews.capture_native_decision(
            proof,
            versioning::PreviewContext {
                envelope,
                state,
                current_owner: self.versioning.owner(envelope.owner().runtime_profile()),
                configuration_revision: self.configuration_revision(),
                security_revision: self.configuration_revision(),
                now_ms: self.versioning.now_millis(),
            },
        )
    }

    /// Confirms only the original operation after consuming its matching
    /// current native decision permit. A bare operation ID grants nothing.
    ///
    /// # Errors
    /// Rejects expired, stale, replayed or mismatched consent.
    pub fn consent_versioning_operation(
        &mut self,
        id: ferric_browser_core::OperationId,
        permit: versioning::ConfirmationPermit,
    ) -> Result<versioning::AuthorizedPreview, versioning::VersioningConsentError> {
        let (envelope, state) = self
            .versioning
            .operation(id)
            .ok_or(versioning::PreviewError::StalePlan)?;
        let payload = self.previews.consume(
            permit,
            versioning::PreviewContext {
                envelope,
                state,
                current_owner: self.versioning.owner(envelope.owner().runtime_profile()),
                configuration_revision: self.configuration_revision(),
                security_revision: self.configuration_revision(),
                now_ms: self.versioning.now_millis(),
            },
        )?;
        self.versioning.consent(id, self.configuration_revision())?;
        Ok(payload)
    }

    /// Allocates one scoped callback ticket after execution starts.
    ///
    /// # Errors
    /// Rejects a duplicate ticket or a non-executing operation.
    pub fn versioning_stage_ticket(
        &mut self,
        id: ferric_browser_core::OperationId,
    ) -> Result<versioning::StageTicket, versioning::OperationError> {
        self.versioning.stage_ticket(id)
    }

    /// Reconciles a callback against its original ticket, never current focus.
    pub fn acknowledge_versioning_stage(
        &mut self,
        ticket: versioning::StageTicket,
        result: versioning::StageResult,
    ) -> (
        versioning::CallbackDisposition,
        Vec<versioning::OperationEffect>,
    ) {
        self.versioning.acknowledge(ticket, result)
    }

    /// Publishes an already verified operation once.
    ///
    /// # Errors
    /// Rejects stale owners or premature/double completion.
    pub fn complete_versioning_operation(
        &mut self,
        id: ferric_browser_core::OperationId,
    ) -> Result<Vec<versioning::OperationEffect>, versioning::OperationError> {
        let effects = self.versioning.complete(id)?;
        self.previews.invalidate_operation(id);
        self.ready_page_capture_stages.remove(&id);
        self.page_capture_installed_archives.remove(&id);
        self.page_capture_installed_sidecars.remove(&id);
        self.pending_window_page_sidecar_effects
            .retain(|_, (operation, ..)| *operation != id);
        self.page_capture_archive_failures.remove(&id);
        self.page_capture_failure_policies.remove(&id);
        self.page_capture_stage_tickets.remove(&id);
        self.page_capture_reservations_held.remove(&id);
        self.page_capture_abort_requested.remove(&id);
        Ok(effects)
    }

    /// Cancels one exact operation and releases its scheduling slot.
    ///
    /// # Errors
    /// Rejects an unknown or already terminal operation.
    pub fn cancel_versioning_operation(
        &mut self,
        id: ferric_browser_core::OperationId,
    ) -> Result<versioning::VersioningEffects, versioning::OperationError> {
        let publication_pending = self
            .versioning
            .operation(id)
            .is_some_and(|(_, state)| state == versioning::OperationState::Publishing);
        let mut operation_effects = self.versioning.cancel(id)?;
        self.suppress_page_capture_cleanup_effect(id, &mut operation_effects);
        // The metadata transaction is already submitted in Publishing. Keep
        // its reservation until the exact result callback or bounded callback
        // timeout; aborting it here could race a successful durable commit.
        if !publication_pending {
            self.cancel_page_capture_pipeline(id);
        }
        self.previews.invalidate_operation(id);
        let (runtime_effects, native_cleanup_pending, cleanup_failed) =
            if self.pending_stashes.contains_key(&id) {
                match self.release_pending_stash_reservation(id) {
                    Ok(effects) => (effects, Vec::new(), Vec::new()),
                    Err(versioning::StashError::WrongStage) => (Vec::new(), vec![id], Vec::new()),
                    Err(_) => (Vec::new(), Vec::new(), vec![id]),
                }
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
        Ok(versioning::VersioningEffects {
            runtime_effects,
            operation_effects,
            native_cleanup_pending,
            cleanup_failed,
        })
    }

    /// Fails a pre-execution operation only when no native mutation occurred.
    ///
    /// # Errors
    /// Rejects terminal or externally executing operations.
    pub fn fail_versioning_without_mutation(
        &mut self,
        id: ferric_browser_core::OperationId,
    ) -> Result<versioning::VersioningEffects, versioning::OperationError> {
        let mut operation_effects = self.versioning.fail_without_mutation(id)?;
        self.suppress_page_capture_cleanup_effect(id, &mut operation_effects);
        self.cancel_page_capture_pipeline(id);
        self.previews.invalidate_operation(id);
        let (runtime_effects, native_cleanup_pending, cleanup_failed) =
            if self.pending_stashes.contains_key(&id) {
                match self.release_pending_stash_reservation(id) {
                    Ok(effects) => (effects, Vec::new(), Vec::new()),
                    Err(versioning::StashError::WrongStage) => (Vec::new(), vec![id], Vec::new()),
                    Err(_) => (Vec::new(), Vec::new(), vec![id]),
                }
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
        Ok(versioning::VersioningEffects {
            runtime_effects,
            operation_effects,
            native_cleanup_pending,
            cleanup_failed,
        })
    }

    /// Expires overdue operations and returns exact-stage cleanup effects.
    pub fn expire_versioning_operations(&mut self) -> versioning::VersioningEffects {
        let mut operation_effects = self.versioning.expire();
        self.reap_page_capture_pipeline();
        self.suppress_terminal_page_capture_cleanup_effects(&mut operation_effects);
        let ids = self
            .pending_stashes
            .keys()
            .copied()
            .filter(|id| {
                self.versioning
                    .operation(*id)
                    .is_none_or(|(envelope, state)| {
                        self.versioning.now_millis() >= envelope.deadline_ms()
                            || matches!(
                                state,
                                versioning::OperationState::Completed
                                    | versioning::OperationState::Cancelled
                                    | versioning::OperationState::Failed
                                    | versioning::OperationState::Partial
                                    | versioning::OperationState::OutcomeUnknown
                            )
                    })
            })
            .collect::<Vec<_>>();
        let mut runtime_effects = Vec::new();
        let mut native_cleanup_pending = Vec::new();
        let mut cleanup_failed = Vec::new();
        for id in ids {
            match self.release_pending_stash_reservation(id) {
                Ok(effects) => runtime_effects.extend(effects),
                Err(versioning::StashError::WrongStage) => native_cleanup_pending.push(id),
                Err(_) => cleanup_failed.push(id),
            }
        }
        self.previews.reap(self.versioning.now_millis());
        versioning::VersioningEffects {
            runtime_effects,
            operation_effects,
            native_cleanup_pending,
            cleanup_failed,
        }
    }

    /// Invalidate all operations for a closed native profile lifetime.
    pub fn close_native_profile(&mut self, profile: ProfileId) -> versioning::VersioningEffects {
        self.cancel_page_capture_jobs_for_profile(profile);
        self.schedule_archive_view_cleanup_for_profile(profile);
        let runtime_effects = match self.release_pending_stashes_for_profile(profile) {
            Ok(effects) => effects,
            Err(error) => {
                let ids = self
                    .pending_stashes
                    .iter()
                    .filter_map(|(id, pending)| {
                        (pending.owner.native().runtime_profile() == profile).then_some(*id)
                    })
                    .collect();
                let (native_cleanup_pending, cleanup_failed) =
                    if error == versioning::StashError::WrongStage {
                        (ids, Vec::new())
                    } else {
                        (Vec::new(), ids)
                    };
                return versioning::VersioningEffects {
                    runtime_effects: Vec::new(),
                    operation_effects: Vec::new(),
                    native_cleanup_pending,
                    cleanup_failed,
                };
            }
        };
        if let Some(owner) = self.versioning.owner(profile) {
            self.previews.invalidate_owner(owner);
        }
        let mut operation_effects = self.versioning.close_owner(profile);
        self.suppress_terminal_page_capture_cleanup_effects(&mut operation_effects);
        versioning::VersioningEffects {
            runtime_effects,
            operation_effects,
            native_cleanup_pending: Vec::new(),
            cleanup_failed: Vec::new(),
        }
    }

    #[must_use]
    pub fn configuration_snapshot(&self) -> ConfigurationSnapshot {
        self.configuration.snapshot()
    }

    #[must_use]
    pub fn profile_snapshot(&self) -> Option<ProfileSnapshot> {
        self.active_profile.as_ref().map(ActiveProfile::snapshot)
    }

    #[must_use]
    pub fn contexts(&self) -> Option<&[ContextRecord]> {
        self.active_profile
            .as_ref()?
            .contexts
            .as_ref()
            .map(ContextRegistry::contexts)
    }

    /// Applies one validated runtime-override layer without serializing the
    /// live configuration through a presentation format.
    ///
    /// # Errors
    ///
    /// Returns an error when the override violates the configuration schema.
    pub fn apply_configuration_layer(
        &mut self,
        overrides: &RuntimeOverrides,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        self.configuration.replace_temporary(overrides.clone())
    }

    /// Applies one runtime input and returns only runtime effects.
    ///
    /// # Errors
    ///
    /// Returns a typed runtime error when the input violates reducer or
    /// command-dispatch invariants.
    pub fn dispatch_runtime(
        &mut self,
        input: RuntimeInput,
    ) -> Result<Vec<RuntimeEffect>, RuntimeError> {
        let effects = self.runtime.handle(input)?;
        let mut effects = self.route_versioning_effects(effects)?;
        self.advance_restore_sidecar_intents(&effects);
        let lazy_effects = self.advance_lazy_restore_pages(&effects);
        effects.extend(lazy_effects);
        Ok(effects)
    }

    /// Atomically activates every configuration layer during bootstrap.
    ///
    /// # Errors
    ///
    /// Returns a configuration error when the resolved layers are invalid.
    pub fn activate_configuration(
        &mut self,
        layers: ConfigurationLayers,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        self.configuration.activate_layers(layers)
    }

    /// Atomically updates every configuration layer and preserves pending
    /// startup/restart-scoped changes.
    ///
    /// # Errors
    ///
    /// Returns a configuration error without changing active configuration
    /// when the candidate layers are invalid.
    pub fn update_configuration(
        &mut self,
        layers: ConfigurationLayers,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        self.configuration.update_layers(layers)
    }

    /// Replaces the effective configuration for compatibility bootstrap paths.
    ///
    /// # Errors
    ///
    /// Returns a configuration error when the replacement is invalid.
    pub fn replace_configuration(
        &mut self,
        configuration: Config,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        self.configuration.replace_effective(configuration)
    }

    /// Activates a prepared profile and transfers all durable resources to the
    /// application owner.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the profile's durable worker cannot start.
    #[cfg(test)]
    fn activate_profile(
        &mut self,
        activation: ProfileActivation,
    ) -> Result<ProfileSnapshot, StorageWorkerError> {
        let versioning = activation
            .roots
            .as_ref()
            .zip(activation.id)
            .map(|(roots, id)| (roots.state.as_path(), id));
        self.storage
            .replace(activation.storage_path.as_deref(), versioning)?;
        let generation = self.next_profile_generation;
        self.next_profile_generation = self.next_profile_generation.saturating_add(1);
        self.safe_get_navigation_proofs.clear();
        self.pending_lazy_restore_pages.clear();
        self.context_capture_owners.clear();
        self.active_profile = Some(ActiveProfile::activate(generation, activation));
        self.active_profile_runtime = None;
        self.profile_snapshot().ok_or(StorageWorkerError::Stopped)
    }

    /// Starts one bounded, application-owned profile bootstrap operation.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid request, a pending request, or worker
    /// startup/admission failure.
    pub fn request_profile_open(
        &mut self,
        request: ProfileOpenRequest,
    ) -> Result<(), ProfileOpenError> {
        if self.profile_open.is_none() {
            self.profile_open = Some(ProfileOpenCoordinator::spawn()?);
        }
        self.profile_open
            .as_mut()
            .ok_or(ProfileOpenError::WorkerStopped)?
            .submit(request)
    }

    /// Polls profile bootstrap and atomically installs its application state.
    pub fn poll_profile_open(&mut self) -> Option<Result<ProfileOpenEffect, ProfileOpenError>> {
        let prepared = self.profile_open.as_mut()?.poll()?;
        self.profile_open = None;
        Some(prepared.and_then(|prepared| self.install_prepared_profile(prepared)))
    }

    fn install_prepared_profile(
        &mut self,
        mut prepared: PreparedProfile,
    ) -> Result<ProfileOpenEffect, ProfileOpenError> {
        self.pending_policy_restore_previews
            .retain(|_, preview| !preview.is_expired());
        if self.versioning.active_count() != 0
            || !self.pending_stashes.is_empty()
            || self.pending_commit.is_some()
            || !self.pending_commit_reads.is_empty()
            || !self.pending_restore_plans.is_empty()
            || !self.pending_policy_restore_previews.is_empty()
            || !self.pending_restore_sidecars.is_empty()
            || !self.pending_restore_sidecar_intents.is_empty()
            || !self.restore_sidecars.is_empty()
        {
            return Err(ProfileOpenError::ReplacementBlocked);
        }
        self.pending_commit_deletions.clear();
        self.pending_policy_restore_previews.clear();
        self.policy_restore_events.clear();
        self.context_capture_owners.clear();
        let configuration = match self.activate_configuration(prepared.layers) {
            Ok(snapshot) => snapshot,
            Err(_) => {
                prepared.configuration_issue = true;
                self.replace_configuration(prepared.fallback_configuration)
                    .map_err(|_| ProfileOpenError::ConfigurationActivationFailed)?
            }
        };
        if self
            .storage
            .replace(
                prepared.activation.storage_path.as_deref(),
                prepared
                    .activation
                    .roots
                    .as_ref()
                    .zip(prepared.activation.id)
                    .map(|(roots, id)| (roots.state.as_path(), id)),
            )
            .is_err()
        {
            prepared.storage_issue = true;
            prepared.activation.storage_path = None;
            self.storage.close();
        }
        let generation = self.next_profile_generation;
        self.next_profile_generation = self.next_profile_generation.saturating_add(1);
        self.safe_get_navigation_proofs.clear();
        self.pending_lazy_restore_pages.clear();
        self.active_profile = Some(ActiveProfile::activate(generation, prepared.activation));
        self.active_profile_runtime = None;
        let profile = self
            .profile_snapshot()
            .ok_or(ProfileOpenError::WorkerStopped)?;
        Ok(ProfileOpenEffect {
            configuration,
            profile,
            config_sources: prepared.config_sources,
            profile_overrides: prepared.profile_overrides,
            runtime_overrides: prepared.runtime_overrides,
            session_path: prepared.session_path,
            session_state_root: prepared.session_state_root,
            storage_issue: prepared.storage_issue,
            configuration_issue: prepared.configuration_issue,
            context_issue: prepared.context_issue,
        })
    }

    /// Closes the active profile after draining its durable worker.
    pub fn close_profile(&mut self) -> versioning::VersioningEffects {
        let profiles = self
            .pending_stashes
            .values()
            .map(|pending| pending.owner.native().runtime_profile())
            .collect::<std::collections::BTreeSet<_>>();
        let mut runtime_effects = Vec::new();
        let mut native_cleanup_pending = Vec::new();
        let mut cleanup_failed = Vec::new();
        for profile in profiles {
            match self.release_pending_stashes_for_profile(profile) {
                Ok(effects) => runtime_effects.extend(effects),
                Err(versioning::StashError::WrongStage) => native_cleanup_pending.extend(
                    self.pending_stashes.iter().filter_map(|(id, pending)| {
                        (pending.owner.native().runtime_profile() == profile).then_some(*id)
                    }),
                ),
                Err(_) => cleanup_failed.extend(self.pending_stashes.keys().copied()),
            }
        }
        if !native_cleanup_pending.is_empty() || !cleanup_failed.is_empty() {
            return versioning::VersioningEffects {
                runtime_effects,
                operation_effects: Vec::new(),
                native_cleanup_pending,
                cleanup_failed,
            };
        }
        self.cancel_all_page_capture_jobs();
        self.schedule_all_archive_view_cleanup();
        let mut operation_effects = self.versioning.close_all();
        self.suppress_terminal_page_capture_cleanup_effects(&mut operation_effects);
        self.previews.invalidate_all();
        self.storage.close();
        self.pending_commit_deletions.clear();
        self.pending_policy_restore_previews.clear();
        self.context_capture_owners.clear();
        // Retire may complete publication or cancel it. Reconcile exact
        // tickets before dropping the durable profile owner.
        let _ = self.poll_storage_effects();
        self.restore_plan_events.clear();
        self.policy_restore_events.clear();
        self.pending_policy_restore_previews.clear();
        self.pending_restore_sidecars.clear();
        self.pending_restore_sidecar_intents.clear();
        self.restore_sidecars.clear();
        self.restore_sidecar_events.clear();
        self.safe_get_navigation_proofs.clear();
        self.pending_lazy_restore_pages.clear();
        self.active_profile = None;
        self.active_profile_runtime = None;
        versioning::VersioningEffects {
            runtime_effects,
            operation_effects,
            native_cleanup_pending: Vec::new(),
            cleanup_failed: Vec::new(),
        }
    }

    /// Applies a context mutation to the active profile registry.
    ///
    /// # Errors
    ///
    /// Returns a context error when there is no active registry or the
    /// requested mutation is invalid.
    pub fn change_context(
        &mut self,
        command: ContextCommand,
    ) -> Result<ContextEffect, ContextRegistryError> {
        let generation = self
            .active_profile
            .as_ref()
            .map(|profile| profile.generation)
            .ok_or(ContextRegistryError::NotFound)?;
        let profile_name = self.profile_snapshot().map(|profile| profile.name);
        let native_owner = self
            .active_profile_runtime
            .and_then(|profile| self.versioning.owner(profile));
        let (change, context) = {
            let profile = self
                .active_profile
                .as_mut()
                .ok_or(ContextRegistryError::NotFound)?;
            let registry = profile
                .contexts
                .as_mut()
                .ok_or(ContextRegistryError::NotFound)?;
            match command {
                ContextCommand::Create {
                    name,
                    label,
                    profile,
                    workspace,
                } => (
                    ContextChange::Created,
                    registry
                        .create_with_workspace(&name, &label, &profile, workspace.as_deref())?
                        .clone(),
                ),
                ContextCommand::Remove { name } => {
                    (ContextChange::Removed, registry.remove(&name)?)
                }
                ContextCommand::SaveMembership { name, members } => {
                    registry.save_membership(&name, members)?;
                    let context = registry
                        .contexts()
                        .iter()
                        .find(|context| context.name == name)
                        .cloned()
                        .ok_or(ContextRegistryError::NotFound)?;
                    (ContextChange::MembershipSaved, context)
                }
            }
        };
        match change {
            ContextChange::Removed => {
                self.context_capture_owners.remove(&context.id);
            }
            ContextChange::MembershipSaved => {
                self.context_capture_owners.remove(&context.id);
                if profile_name.as_deref() == Some(context.profile.as_str())
                    && let Some(owner) = native_owner
                {
                    // This non-serialized owner binding proves that the
                    // saved runtime IDs were refreshed in this app session.
                    self.context_capture_owners.insert(context.id, owner);
                }
            }
            ContextChange::Created => {}
        }
        Ok(ContextEffect {
            generation,
            change,
            context,
        })
    }

    /// Submits one storage command and returns its application correlation
    /// ticket without exposing the worker.
    ///
    /// # Errors
    ///
    /// Returns a storage error when no worker is active, the request is
    /// invalid, or bounded admission is full.
    pub fn submit_storage(
        &mut self,
        request: StorageRequest,
    ) -> Result<StorageTicket, StorageWorkerError> {
        self.storage.submit(request)
    }

    /// Polls storage lifecycle events in the adapter-facing representation.
    #[must_use]
    pub fn poll_storage_effects(&mut self) -> Vec<StorageEffect> {
        self.retry_page_capture_reservation_releases();
        let storage_effects = self.storage.poll();
        let effects = self.route_page_capture_storage_effects(storage_effects);
        self.retry_page_capture_reservation_releases();
        let effects = self.route_archive_view_storage_effects(effects);
        let effects = self.route_commit_storage_effects(effects);
        self.route_restore_storage_effects(effects)
    }

    /// Starts the single owner thread for one profile metadata database.
    ///
    /// Replacing an existing worker first retires the old profile within the
    /// same bounded durability budget used by shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker cannot be started.
    #[cfg(test)]
    pub fn open_storage(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), StorageWorkerError> {
        self.storage.replace(Some(path.as_ref()), None)
    }

    pub fn close_storage(&mut self) {
        self.storage.close();
    }

    #[must_use]
    pub const fn storage_is_open(&self) -> bool {
        self.storage.is_open()
    }

    /// Reports file-only versioning jobs that must retire before the UI owner
    /// may finish shutdown. Metadata work is covered by the durable flush.
    #[must_use]
    pub fn has_pending_versioning_object_work(&self) -> bool {
        self.storage.has_pending_versioning_object_work()
    }

    /// Restarts the durable worker for the active profile after a recoverable
    /// worker failure.
    ///
    /// # Errors
    ///
    /// Returns [`StorageWorkerError::Stopped`] for transient or closed
    /// profiles, or the worker spawn error for a durable profile.
    pub fn restart_storage(&mut self) -> Result<(), StorageWorkerError> {
        self.pending_policy_restore_previews
            .retain(|_, preview| !preview.is_expired());
        if self.versioning.active_count() != 0
            || !self.pending_stashes.is_empty()
            || self.pending_commit.is_some()
            || !self.pending_commit_reads.is_empty()
            || !self.pending_restore_plans.is_empty()
            || !self.pending_policy_restore_previews.is_empty()
            || !self.pending_restore_sidecars.is_empty()
            || !self.pending_restore_sidecar_intents.is_empty()
            || !self.restore_sidecars.is_empty()
        {
            return Err(StorageWorkerError::Busy);
        }
        self.pending_commit_deletions.clear();
        let profile = self
            .active_profile
            .as_ref()
            .ok_or(StorageWorkerError::Stopped)?;
        let path = profile
            .storage_path()
            .map(std::path::Path::to_owned)
            .ok_or(StorageWorkerError::Stopped)?;
        let versioning = profile.versioning_recovery_scope();
        self.storage.replace(Some(&path), versioning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_core::{
        CommandInvocation, EngineEffect, Event, Mode, NavigationContext, ParsedCommand,
        VersioningEngineEffect,
    };
    use ferric_browser_runtime::{RuntimeCommand, RuntimeDispatch, RuntimeInput};
    use ferric_browser_storage::{RootSpec, StorageCompletion, StorageRoots};
    use std::{fs, thread, time::Duration};
    use uuid::Uuid;

    #[test]
    fn sensitive_versioning_consent_requires_current_native_preview_permit() {
        let (mut application, window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .unwrap();
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .unwrap();
        let native_sink = application.take_native_versioning_decision_sink().unwrap();
        assert!(application.take_native_versioning_decision_sink().is_none());
        let (operation, _) = application
            .request_versioning_operation(
                ScopeSelector::Window { window },
                versioning::OperationKind::Sensitive(versioning::SensitiveOperation::Drop),
                120_000,
            )
            .unwrap();
        application
            .advance_versioning_operation(operation, versioning::OperationState::Validating)
            .unwrap();
        application
            .advance_versioning_operation(operation, versioning::OperationState::AwaitingConsent)
            .unwrap();
        let preview = application
            .prepare_versioning_preview(
                operation,
                versioning::PreviewDraft::drop(ferric_browser_core::BranchId::fresh(), 1).unwrap(),
            )
            .unwrap();
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            versioning::OperationState::AwaitingConsent
        );
        let permit = application
            .capture_native_versioning_decision(operation, native_sink.accept(&preview))
            .unwrap();
        let authorized = application
            .consent_versioning_operation(operation, permit)
            .unwrap();
        assert!(matches!(authorized, versioning::AuthorizedPreview::Drop(_)));
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            versioning::OperationState::Preparing
        );
        assert_eq!(
            application
                .capture_native_versioning_decision(operation, native_sink.accept(&preview))
                .unwrap_err(),
            versioning::PreviewError::StalePlan
        );
    }

    #[test]
    fn replacing_native_owner_invalidates_prepared_preview_and_permit() {
        let (mut application, window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .unwrap();
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .unwrap();
        let native_sink = application.take_native_versioning_decision_sink().unwrap();
        let (operation, _) = application
            .request_versioning_operation(
                ScopeSelector::Window { window },
                versioning::OperationKind::Sensitive(versioning::SensitiveOperation::Restore),
                120_000,
            )
            .unwrap();
        application
            .advance_versioning_operation(operation, versioning::OperationState::Validating)
            .unwrap();
        application
            .advance_versioning_operation(operation, versioning::OperationState::AwaitingConsent)
            .unwrap();
        let preview = application
            .prepare_versioning_preview(
                operation,
                versioning::PreviewDraft::restore(
                    ferric_browser_core::CommitId::fresh(),
                    1,
                    true,
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        let permit = application
            .capture_native_versioning_decision(operation, native_sink.accept(&preview))
            .unwrap();
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .unwrap();
        assert_eq!(
            application.consent_versioning_operation(operation, permit),
            Err(versioning::VersioningConsentError::Preview(
                versioning::PreviewError::StalePlan
            ))
        );
        assert_eq!(
            application.versioning().operation(operation).unwrap().1,
            versioning::OperationState::Cancelled
        );
    }

    #[test]
    fn versioning_admission_captures_explicit_owner_and_subject() {
        let (mut application, window, tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("application bootstrap");
        let profile = application.windows()[&window].profile;
        let selector = ScopeSelector::Tab {
            window,
            target: application.capture_target(tab).expect("live tab target"),
        };
        assert_eq!(
            application.request_versioning_operation(
                selector,
                versioning::OperationKind::Mutating,
                1000
            ),
            Err(versioning::OperationError::OwnerClosed)
        );
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .unwrap();
        let (id, effects) = application
            .request_versioning_operation(selector, versioning::OperationKind::Mutating, 1000)
            .unwrap();
        assert_eq!(effects, vec![versioning::OperationEffect::Start(id)]);
        let (envelope, state) = application.versioning().operation(id).unwrap();
        assert_eq!(state, versioning::OperationState::Requested);
        assert_eq!(envelope.scope().members()[0].target().tab, tab);
        assert_eq!(envelope.owner().runtime_profile(), profile);
        assert_eq!(
            envelope.configuration_revision(),
            application.configuration_revision()
        );
        application.close_native_profile(profile);
        assert_eq!(
            application.versioning().operation(id).unwrap().1,
            versioning::OperationState::Cancelled
        );
    }

    #[test]
    fn runtime_mutation_is_visible_only_as_runtime_effects() {
        let (mut application, window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("application bootstrap");
        let effects = application
            .dispatch_runtime(RuntimeInput::Command(RuntimeCommand::EnterMode {
                window,
                mode: Mode::Command,
            }))
            .expect("runtime input");
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, RuntimeEffect::UiModelUpdated(_)))
        );
        assert_eq!(
            application.runtime_view().windows()[&window].modes.last(),
            Some(&Mode::Command)
        );
    }

    #[test]
    fn versioning_status_routes_through_runtime_without_state_or_storage_mutation() {
        let (mut application, window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("application bootstrap");
        let before = application.revision();
        let effects = application
            .dispatch_runtime(RuntimeInput::Command(RuntimeCommand::Dispatch(Box::new(
                RuntimeDispatch {
                    invocation: CommandInvocation::new(ParsedCommand {
                        name: "versioning-status".into(),
                        arguments: vec![],
                    }),
                    navigation: NavigationContext::default(),
                    target: DispatchTarget::Window(window),
                },
            ))))
            .expect("typed status response");
        assert!(effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::Engine(EngineEffect::Versioning(VersioningEngineEffect::Status {
                production_enabled: false,
                active_operations: 0,
            }))
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::AppliedEvent(Event::VersioningRequest(_))
        )));
        assert!(!effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::Storage(_) | RuntimeEffect::IpcEvent { .. }
        )));
        assert_eq!(application.revision(), before);
    }

    #[test]
    fn unavailable_versioning_commands_return_exact_safe_reasons() {
        let (mut application, window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("application bootstrap");
        let before = application.revision();
        for (name, arguments, reason) in [
            ("commit", vec!["Before trial"], "wrong-profile"),
            ("stash", vec![], "retention-unavailable"),
            ("commit-info", vec!["commit:1234"], "ambiguous-reference"),
            (
                "restore",
                vec!["main", "--mode=live", "--into=current"],
                "retention-unavailable",
            ),
            ("merge", vec!["trial", "--cookies"], "cookie-identity-lossy"),
            ("merge-apply", vec!["plan"], "merge-apply-unavailable"),
        ] {
            let error = application
                .dispatch_runtime(RuntimeInput::Command(RuntimeCommand::Dispatch(Box::new(
                    RuntimeDispatch {
                        invocation: CommandInvocation::new(ParsedCommand {
                            name: name.into(),
                            arguments: arguments.into_iter().map(str::to_owned).collect(),
                        }),
                        navigation: NavigationContext::default(),
                        target: DispatchTarget::Window(window),
                    },
                ))))
                .expect_err("gated command must fail");
            assert!(
                error
                    .diagnostic_context()
                    .contains(&format!("reason={reason}")),
                "{name}: {}",
                error.diagnostic_context()
            );
            assert_eq!(application.revision(), before);
        }
    }

    #[test]
    fn configuration_layers_remain_typed_until_a_presentation_adapter_requests_them() {
        let mut application = BrowserApplication::new(Config::default());
        let mut overrides = RuntimeOverrides::default();
        overrides
            .set_literal("content.zoom", "1.25")
            .expect("valid override");
        let snapshot = application
            .apply_configuration_layer(&overrides)
            .expect("configuration layer");
        assert!((application.configuration().content.zoom - 1.25).abs() < f64::EPSILON);
        assert_eq!(snapshot.revision, 1);
    }

    #[test]
    fn invalid_configuration_update_is_atomic() {
        let mut application = BrowserApplication::new(Config::default());
        let before = application.configuration_snapshot();
        let mut layers = ConfigurationLayers::new(Config::default());
        layers
            .runtime
            .set_literal("content.zoom", "9.0")
            .expect("syntactically valid override");

        let result = application.update_configuration(layers);

        assert!(result.is_err());
        assert_eq!(application.configuration_snapshot(), before);
    }

    #[test]
    fn configuration_update_applies_live_values_and_reports_startup_values() {
        let mut application = BrowserApplication::new(Config::default());
        let mut layers = ConfigurationLayers::new(Config::default());
        layers
            .runtime
            .set_literal("content.zoom", "1.25")
            .expect("live override");
        layers
            .runtime
            .set_literal("session.restore", "always")
            .expect("startup override");

        let snapshot = application
            .update_configuration(layers)
            .expect("configuration update");

        assert!((snapshot.effective.content.zoom - 1.25).abs() < f64::EPSILON);
        assert_eq!(
            snapshot.effective.session.restore,
            Config::default().session.restore
        );
        assert!(
            snapshot.pending_changes.iter().any(|change| {
                change.key == "session.restore" && change.apply_time == "startup"
            })
        );
    }

    #[test]
    fn profile_lifecycle_is_owned_by_the_application() {
        let mut application = BrowserApplication::new(Config::default());
        let snapshot = application
            .activate_profile(ProfileActivation {
                name: "private".into(),
                id: None,
                privacy: PrivacyKind::Private,
                roots: None,
                lock: None,
                contexts: None,
                storage_path: None,
            })
            .expect("activate transient profile");

        assert_eq!(snapshot.generation, 1);
        assert_eq!(snapshot.privacy, PrivacyKind::Private);
        assert!(!snapshot.durable);
        assert!(!application.storage_is_open());

        application.close_profile();
        assert!(application.profile_snapshot().is_none());
    }

    #[test]
    fn profile_open_worker_installs_configuration_and_storage_as_one_application_effect() {
        let base = std::env::temp_dir().join(format!("ferric-app-profile-{}", Uuid::new_v4()));
        let (mut application, _, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("application bootstrap");
        let mut command_line = RuntimeOverrides::default();
        command_line
            .set_literal("content.zoom", "1.25")
            .expect("valid override");
        application
            .request_profile_open(ProfileOpenRequest {
                name: "default".into(),
                label: "Default".into(),
                privacy: PrivacyKind::Normal,
                storage: ProfileStorage::Durable(RootSpec::Base(base.clone())),
                base_configuration: Config::default(),
                initial_profile_overrides: RuntimeOverrides::default(),
                command_line_overrides: command_line,
                config_path: None,
                contexts: toml::from_str(
                    "schema_version = 3\n[[contexts]]\nname = \"worker-context\"\nlabel = \"Worker context\"\nprofile = \"default\"\n",
                )
                .expect("context configuration"),
                context_configuration_issue: false,
            })
            .expect("profile request");

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let effect = loop {
            if let Some(result) = application.poll_profile_open() {
                break result.expect("profile effect");
            }
            assert!(
                std::time::Instant::now() < deadline,
                "profile open timed out"
            );
            thread::yield_now();
        };

        assert!(effect.profile.durable);
        assert!(application.storage_is_open());
        assert!((effect.configuration.effective.content.zoom - 1.25).abs() < f64::EPSILON);
        assert_eq!(application.profile_snapshot(), Some(effect.profile));
        assert_eq!(
            application.contexts().expect("contexts")[0].name,
            "worker-context"
        );
        application.close_profile();
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn context_mutations_cross_the_application_boundary() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        let registry = ContextRegistry::open(&roots).expect("context registry");
        let cleanup_roots = roots.clone();
        let mut application = BrowserApplication::new(Config::default());
        application
            .activate_profile(ProfileActivation {
                name: "default".into(),
                id: Some(Uuid::new_v4()),
                privacy: PrivacyKind::Normal,
                roots: Some(roots),
                lock: None,
                contexts: Some(registry),
                storage_path: None,
            })
            .expect("activate profile");

        let effect = application
            .change_context(ContextCommand::Create {
                name: "research".into(),
                label: "Research".into(),
                profile: "default".into(),
                workspace: Some("special:research".into()),
            })
            .expect("create context");
        assert!(matches!(
            effect,
            ContextEffect {
                generation: 1,
                change: ContextChange::Created,
                context,
            } if context.name == "research"
        ));
        assert_eq!(application.contexts().map(<[_]>::len), Some(1));

        application.close_profile();
        cleanup_roots.cleanup().expect("clean temporary roots");
    }

    #[test]
    fn runtime_dispatch_preserves_runtime_error_identity() {
        let mut application = BrowserApplication::new(Config::default());
        let effects = application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::RequestShutdown))
            .expect("shutdown request");
        assert!(!effects.is_empty());
    }

    #[test]
    fn storage_intents_cannot_bypass_the_application_owner() {
        let mut application = BrowserApplication::new(Config::default());
        let error = application
            .submit_storage(StorageRequest::Library { limit: 1 })
            .expect_err("unopened storage must be rejected");
        assert!(matches!(error, StorageWorkerError::Stopped));
    }

    #[test]
    fn storage_results_return_as_application_effects() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");
        application
            .submit_storage(StorageRequest::Library { limit: 1 })
            .expect("submit library request");

        let mut completed = false;
        for _ in 0..100 {
            let effects = application.poll_storage_effects();
            completed = effects
                .iter()
                .any(|effect| matches!(effect.completion, StorageCompletion::Library(Ok(_))));
            if completed {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            completed,
            "storage completion was not emitted through application"
        );
        drop(application);
        fs::remove_dir_all(root).expect("remove test storage directory");
    }

    #[test]
    fn application_retries_a_busy_storage_intent_without_adapter_queues() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");
        for _ in 0..2 {
            application
                .submit_storage(StorageRequest::Library { limit: 1 })
                .expect("admit library request");
        }

        let mut completed = 0;
        for _ in 0..100 {
            for effect in application.poll_storage_effects() {
                if matches!(effect.completion, StorageCompletion::Library(Ok(_))) {
                    completed += 1;
                }
            }
            if completed == 2 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(completed, 2, "both application-scheduled requests complete");
        drop(application);
        fs::remove_dir_all(root).expect("remove test storage directory");
    }

    #[test]
    fn storage_effects_preserve_the_ticket_of_each_scheduled_intent() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");
        let first = application
            .submit_storage(StorageRequest::Library { limit: 1 })
            .expect("admit first request");
        let second = application
            .submit_storage(StorageRequest::Library { limit: 1 })
            .expect("queue second request");
        assert_ne!(first, second);

        let mut completed = Vec::new();
        for _ in 0..100 {
            for effect in application.poll_storage_effects() {
                if matches!(effect.completion, StorageCompletion::Library(Ok(_))) {
                    completed.push(effect.ticket);
                }
            }
            if completed.len() == 2 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(completed, vec![first, second]);
        drop(application);
        fs::remove_dir_all(root).expect("remove test storage directory");
    }

    #[test]
    fn versioning_reservations_keep_distinct_application_tickets() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        let profile = Uuid::new_v4();
        let directory = root.join("profiles").join(profile.to_string());
        fs::create_dir_all(&directory).expect("test profile directory");
        let database = directory.join("browser.sqlite");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");
        let first = application
            .submit_storage(StorageRequest::VersioningBegin {
                operation: ferric_browser_core::OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("first capture reservation");
        let second = application
            .submit_storage(StorageRequest::VersioningBegin {
                operation: ferric_browser_core::OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("queued capture reservation");
        assert_ne!(first, second);
        let mut completed = Vec::new();
        for _ in 0..100 {
            for effect in application.poll_storage_effects() {
                if matches!(
                    effect.completion,
                    StorageCompletion::VersioningBegin(Ok(()))
                ) {
                    completed.push(effect.ticket);
                }
            }
            if completed.len() == 2 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(completed, vec![first, second]);
        drop(application);
        fs::remove_dir_all(root).expect("remove test storage directory");
    }

    #[test]
    fn flush_completion_is_an_application_effect() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");
        application
            .submit_storage(StorageRequest::Flush)
            .expect("submit flush request");

        let mut completed = false;
        for _ in 0..100 {
            completed = application
                .poll_storage_effects()
                .iter()
                .any(|effect| matches!(effect.completion, StorageCompletion::Flush(Ok(()))));
            if completed {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            completed,
            "flush completion was not emitted through application"
        );
        drop(application);
        fs::remove_dir_all(root).expect("remove test storage directory");
    }

    #[test]
    fn closing_storage_terminates_every_accepted_ticket() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");

        let accepted = (0..10)
            .map(|_| {
                application
                    .submit_storage(StorageRequest::Library { limit: 1 })
                    .expect("admit storage request")
            })
            .collect::<Vec<_>>();

        application.close_storage();
        let mut terminated = application
            .poll_storage_effects()
            .into_iter()
            .map(|effect| effect.ticket)
            .collect::<Vec<_>>();
        terminated.sort_unstable();

        assert_eq!(terminated, accepted);
        assert!(!application.storage_is_open());
        fs::remove_dir_all(root).expect("remove test storage directory");
    }
}
