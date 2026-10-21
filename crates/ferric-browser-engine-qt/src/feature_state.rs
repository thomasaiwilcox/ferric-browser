//! Owned native feature state. `QObject` properties remain projections in the bridge.

use super::{
    Arc, AtomicBool, BTreeMap, BTreeSet, CatalogReadState, Cell, CommandSource, ConfigReloadWorker,
    ConfigWatch, ConfigWriteWorker, ContextReloadWorker, EditorCompletion, EditorWriteWorker,
    FeedbackLane, FocusObservation, HintInteraction, HintScopeBinding, HintSession, IdSource,
    Instant, JourneyEdgeKind, JourneyNodeId, LocalInteractionAuthority, LocalInteractionPermit,
    Mutex, NativePromptLeases, NetworkPolicyWorker, PageDocumentIdentities, PathBuf,
    PendingActionTarget, PendingCaret, PendingConfigEdit, PendingContextReload, PendingEditor,
    PendingSameDocumentPassThrough, PendingSelection, PendingSelectorValidation, PendingSpawn,
    PendingUserscript, ProfileLibrarySnapshot, QString, RefCell, SessionSaveIntent, SpatialSession,
    SpawnCompletion, StorageTicket, SwitcherLibraryIndex, SwitcherLibraryIndexResult,
    SwitcherQueryCache, TabId, Target, UserscriptCompletion, Value, VecDeque,
};

pub(super) struct NavigationState {
    pub(super) pending_journey_transitions: Vec<(Target, JourneyEdgeKind, String)>,
    pub(super) pending_journey_parent: Option<(Target, JourneyNodeId)>,
    pub(super) pending_journey_traversal: Option<Target>,
    pub(super) pending_journey_traversals: VecDeque<Target>,
    pub(super) pending_journey_mappings: BTreeMap<StorageTicket, (JourneyNodeId, String)>,
    pub(super) pending_journey_reopen: Option<(String, String, Option<TabId>, String)>,
    pub(super) pending_navigation_urls: BTreeMap<TabId, String>,
    pub(super) page_document_identities: PageDocumentIdentities,
    pub(super) pending_same_document_pass_through: Option<PendingSameDocumentPassThrough>,
    pub(super) pending_redirect_tabs: BTreeSet<TabId>,
    pub(super) pending_redirect_hops: BTreeMap<TabId, u8>,
    pub(super) popup_journey_targets: Vec<(String, Target, String, bool)>,
    pub(super) scroll_positions: BTreeMap<TabId, (f64, f64)>,
    pub(super) content_histories: BTreeMap<TabId, ferric_browser_core::ContentHistory>,
    pub(super) journey_durable_ids: RefCell<BTreeMap<JourneyNodeId, String>>,
}

pub(super) struct ProcessState {
    pub(super) pending_selection: Option<PendingSelection>,
    pub(super) pending_caret: Option<PendingCaret>,
    pub(super) pending_editor: Option<PendingEditor>,
    pub(super) pending_editor_write: Option<(String, PathBuf, bool)>,
    pub(super) editor_completions: Arc<Mutex<Vec<EditorCompletion>>>,
    pub(super) pending_config_edit: Option<PendingConfigEdit>,
    pub(super) pending_spawn: Option<PendingSpawn>,
    pub(super) pending_action_target: Option<PendingActionTarget>,
    pub(super) pending_userscript: Option<PendingUserscript>,
    pub(super) spawn_completions: Arc<Mutex<Vec<SpawnCompletion>>>,
    pub(super) spawn_cancellations: Arc<Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
    pub(super) spawn_working_directory: Option<PathBuf>,
    pub(super) userscript_completions: Arc<Mutex<Vec<UserscriptCompletion>>>,
    pub(super) userscript_cancellations: Arc<Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
    pub(super) editor_write_worker: Option<EditorWriteWorker>,
}

pub(super) struct ConfigurationIoState {
    pub(super) pending_config: Option<Value>,
    pub(super) pending_configuration_publication:
        Option<super::browser_ui_state::PendingConfigurationPublication>,
    pub(super) config_watch: ConfigWatch,
    pub(super) config_reload_worker: Option<ConfigReloadWorker>,
    pub(super) config_reload_requested_by_user: bool,
    pub(super) context_reload_worker: Option<ContextReloadWorker>,
    pub(super) pending_context_reload: Option<PendingContextReload>,
    pub(super) config_write_worker: Option<ConfigWriteWorker>,
    pub(super) network_policy_worker: Option<NetworkPolicyWorker>,
    pub(super) network_policy_reload_pending: bool,
    pub(super) pending_selector_validation: Option<PendingSelectorValidation>,
    pub(super) selector_generation: u64,
    pub(super) selector_config_revision: u64,
    pub(super) active_selector_fingerprint: String,
    pub(super) theme_watch: ConfigWatch,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FavouriteStage {
    Submitting,
    Writing(StorageTicket),
    AwaitingRefresh,
    Reading(StorageTicket),
}

pub(super) struct FavouriteFeedback {
    pub(super) id: QString,
    pub(super) stage: FavouriteStage,
    pub(super) success: &'static str,
}

// Flags belong to independent single-flight catalogs, not one combined state.
#[allow(clippy::struct_excessive_bools)]
pub(super) struct LibraryState {
    pub(super) start_snapshot: ferric_browser_storage::StartPageSnapshot,
    pub(super) start_read_pending: bool,
    pub(super) start_refresh_requested: bool,
    pub(super) start_write_pending: bool,
    pub(super) favourite_feedback: Option<FavouriteFeedback>,
    pub(super) mark_feedback_tickets: BTreeMap<StorageTicket, (FeedbackLane, QString)>,
    pub(super) storage_library_refresh_again: bool,
    pub(super) storage_library_read_state: CatalogReadState,
    pub(super) storage_library: Option<ProfileLibrarySnapshot>,
    pub(super) storage_library_dirty: Cell<bool>,
    pub(super) switcher_library_index: Option<SwitcherLibraryIndex>,
    pub(super) switcher_library_index_revision: i64,
    pub(super) switcher_library_index_building: bool,
    pub(super) switcher_library_index_cancel: Option<Arc<AtomicBool>>,
    pub(super) switcher_library_index_result: Arc<Mutex<Option<SwitcherLibraryIndexResult>>>,
    pub(super) switcher_cache: Option<SwitcherQueryCache>,
    pub(super) session_names: Vec<String>,
    pub(super) session_names_dirty: Cell<bool>,
    pub(super) session_names_refresh_again: bool,
    pub(super) session_restore_preview_pending: bool,
    pub(super) session_restore_load_append: Option<bool>,
    pub(super) session_restore_recovery_pending: bool,
    pub(super) session_saves: BTreeMap<StorageTicket, SessionSaveIntent>,
    pub(super) session_checkpoint_clear_pending: bool,
    pub(super) storage_flush_pending: bool,
    pub(super) storage_flush_error: Option<String>,
}

pub(super) struct InteractionState {
    pub(super) spatial_session: Option<SpatialSession>,
    pub(super) spatial_ids: IdSource,
    pub(super) hint_session: Option<HintSession>,
    pub(super) hint_interaction: Option<HintInteraction>,
    pub(super) pending_hint_scope_handoff: Option<HintScopeBinding>,
    pub(super) active_hint_scope: Option<HintScopeBinding>,
    pub(super) next_hint_scope_lease_id: u64,
    pub(super) hint_consumed: BTreeSet<(String, String, u32)>,
    pub(super) hint_configuration_revision: u64,
    pub(super) pending_hint_palette_permit: Option<LocalInteractionPermit>,
    pub(super) pending_grid_scope_permit: Option<LocalInteractionPermit>,
    pub(super) hint_pointer_source: Option<CommandSource>,
    pub(super) hint_pointer_action: Option<super::browser_ui_hint_pointer::HintPointerAction>,
    pub(super) hint_local_interaction_authority: LocalInteractionAuthority,
    pub(super) hint_ids: IdSource,
    pub(super) focus_observations: BTreeMap<(i32, String), FocusObservation>,
    pub(super) focus_suppressions: BTreeMap<(i32, String), i32>,
    pub(super) prompt_view_identities: BTreeMap<TabId, String>,
    pub(super) native_prompt_leases: NativePromptLeases,
    pub(super) native_prompt_transition: bool,
}

pub(super) struct DiagnosticState {
    pub(super) diagnostics_preview_ready: bool,
    pub(super) diagnostics_preview_payload: Option<String>,
    pub(super) diagnostics_collector: super::diagnostic_report::DiagnosticCollector,
    pub(super) diagnostics_review: Option<(u64, super::diagnostic_report::ReportKey, Instant)>,
    pub(super) pending_diagnostic_requests: Vec<super::browser_ui_diagnostics::PendingDiagnostic>,
}
