//! The narrow Qt boundary for `Ferric Browser`.
//!
//! Core owns reducer policy; the application owns profile and durable workflows.
//! This crate adapts native facilities, command entry points and presentation
//! projections to the Qt Quick/WebEngine surface.

#![allow(clippy::unnecessary_box_returns)]
// CXX-Qt scalar invokables carry scoped focus facts; its bridge parser does not
// accept per-invokable lint attributes, including on the generated declarations.
#![allow(clippy::too_many_arguments)]
#![allow(
    clippy::assigning_clones,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::drop_non_drop,
    clippy::float_cmp,
    clippy::implicit_clone,
    clippy::let_and_return,
    clippy::manual_let_else,
    clippy::map_unwrap_or,
    clippy::match_same_arms,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::redundant_closure,
    clippy::redundant_guards,
    clippy::semicolon_if_nothing_returned,
    clippy::single_match_else,
    clippy::too_many_lines
)]
// Native declarations live in bridge.rs. This root owns QObject projections;
// handwritten feature state and behavior live in the modules below.

mod action_arguments;
mod action_catalog;
mod action_mapping;
mod action_request_decoder;
mod background_workers;
mod binding_policy;
mod binding_presentation;
mod blocking_evidence;
mod blocking_presentation;
mod browser_ui_browsing_commands;
mod browser_ui_closed_tabs;
mod browser_ui_command_dispatch;
mod browser_ui_command_history;
mod browser_ui_command_workflows;
mod browser_ui_common_commands;
mod browser_ui_context_reload;
use browser_ui_context_reload::{
    ConfigReloadPublication, ContextReloadWorker, PendingContextReload,
};
mod browser_ui_configuration;
mod browser_ui_content_tools;
mod browser_ui_context_commands;
mod browser_ui_context_navigation;
mod browser_ui_contexts;
mod browser_ui_controls;
mod browser_ui_defaults;
mod browser_ui_download_requests;
mod browser_ui_downloads_permissions;
mod browser_ui_edit_commands;
mod browser_ui_external_actions;
mod browser_ui_guide;
mod gui_application;
pub use gui_application::new_gui_application;
mod browser_ui_hint_pointer;
pub use browser_ui_hint_pointer::reject_external_hint_interaction_chain;
mod browser_ui_diagnostics;
mod browser_ui_hint_proxy_configuration;
mod browser_ui_hint_selection;
mod browser_ui_input;
#[allow(clippy::too_many_lines)]
mod browser_ui_ipc;
mod browser_ui_ipc_dispatch;
mod browser_ui_ipc_operations;
mod browser_ui_ipc_preparation;
mod browser_ui_ipc_route_validation;
mod browser_ui_ipc_runtime_dispatch;
mod browser_ui_ipc_switcher_actions;
mod browser_ui_ipc_userscripts;
mod browser_ui_journey_commands;
mod browser_ui_library;
mod browser_ui_switcher_sources;
mod switcher_sources;
use switcher_sources::{CatalogReadState, SourceNeeds, SourceProfile, SwitcherAvailability};
mod browser_ui_download_finalization;
mod browser_ui_download_stream;
mod browser_ui_local_files;
mod browser_ui_maintenance;
mod browser_ui_management_feedback;
mod browser_ui_native_prompts;
mod browser_ui_navigation;
mod browser_ui_navigation_lifecycle;
mod browser_ui_operations;
mod browser_ui_persistence;
mod browser_ui_popup_owner;
mod browser_ui_popups;
mod browser_ui_process_actions;
mod browser_ui_profile_setup;
mod browser_ui_queries;
mod browser_ui_query_bindings;
mod browser_ui_query_configuration;
mod browser_ui_query_diagnostics;
mod browser_ui_query_permissions;
mod browser_ui_query_profiles;
mod browser_ui_query_site;
mod browser_ui_query_switcher;
mod browser_ui_repeat_macros;
mod browser_ui_runtime;
mod browser_ui_runtime_config;
mod browser_ui_runtime_settings;
mod browser_ui_scope_handoff;
mod browser_ui_selector_config;
mod browser_ui_session_save;
mod browser_ui_sessions;
mod browser_ui_site;
mod browser_ui_spatial;
mod browser_ui_start_page;
mod browser_ui_state;
mod browser_ui_storage_writes;
mod browser_ui_tab_state;
mod browser_ui_tabs;
mod browser_ui_target_search;
mod browser_ui_ui_actions;
mod browser_ui_web_actions;
mod chrome_preferences;
mod command_options;
mod compatibility;
mod config_projection;
mod config_watch;
mod configuration_workers;
mod desktop_portals;
mod desktop_preferences;
mod diagnostic_report;
mod diagnostics;
mod diagnostics_overview;
mod diagnostics_presentation;
mod diagnostics_snapshot;
mod download_files;
mod download_finalization;
mod download_presentation;
mod download_stream_policy;
mod download_stream_writer;
mod editor_process;
mod effect_projection;
mod engine_requests;
mod feature_preferences;
mod feature_state;
mod focus_policy;
mod hint_payload;
mod hint_policy;
mod hyprland;
mod input_validation;
mod ipc_command_decoder;
mod ipc_contract;
mod ipc_dispatch_context;
mod ipc_event_projection;
mod ipc_params;
mod ipc_response;
mod ipc_route;
#[cfg(test)]
mod ipc_schema;
mod ipc_transport;
mod library_presentation;
mod link_cleaning_policy;
mod live_ipc_owners;
mod logging;
mod macro_policy;
mod macro_projection;
mod maintenance;
mod mixed_policy_runtime;
mod native_files;
mod native_prompt_lease;
mod navigation_lifecycle;
mod navigation_policy;
mod network_policy;
mod open_policy;
mod operation_policy;
mod operation_projection;
mod page_document_identity;
mod presentation_text;
mod process_output;
mod profile_management;
mod profile_startup;
mod profile_workers;
mod renderer_lifecycle;
mod runtime_bridge;
mod runtime_event_projection;
mod runtime_facts;
mod runtime_guard;
mod selector_validation;
mod settings_presentation;
mod shutdown_control;
mod switcher_index;
mod switcher_policy;
mod switcher_presentation;
mod tab_presentation;
mod theme_presentation;
mod url_presentation;
mod url_safety;
mod userscript_catalog;
mod userscript_presentation;
use ferric_browser_application::{UserscriptManagerRequest, UserscriptManagerResult};
mod view_lifecycle;
mod window_registry;

use action_arguments::decode_ui_action_arguments as ui_action_arguments;
use action_catalog::{
    action_list_value, configured_action_target_supports_subject, configured_action_target_values,
    configured_action_targets, configured_switcher_action_values, external_action_presentations,
    parse_external_action_id, security_deny_rule_count,
};
use action_mapping::parse_action_invocation;
use action_request_decoder::typed_ipc_action;
use background_workers::{
    EditorWriteWorker, HyprlandRequest, HyprlandResponse, HyprlandWorker, NetworkPolicyWorker,
    PrintWorker,
};
use binding_policy::{
    binding_command_parameters, binding_uses_full_command_executor, config_get_command_parameters,
    configured_bindings, configured_undo_limit, ipc_mode_name, is_context_command,
    is_library_command, is_profile_command, is_session_command, learning_mode_request,
    modal_command_prefill, normalize_active_tab_command, parse_ipc_mode, switcher_max_results,
};
use browser_ui_management_feedback::{FeedbackLane, FeedbackState};
use browser_ui_state::{
    ContextSnapshot, PendingContextRoute, PendingProfileConfiguration, PendingSelectorValidation,
    ProfilePersistence, SessionCheckpoint, SessionSaveIntent, SwitcherQueryCache,
};
use chrome_preferences::ChromePreferences;
use command_options::{
    ScrollTargetAction, command_count as ipc_command_count,
    history_clear as parse_history_clear_arguments, is_zoom as is_zoom_command,
    scroll as parse_scroll_options, scroll_target as parse_scroll_target_options,
    search_next as parse_search_next_options, selection_yank_command,
};
use config_projection::{
    config_value_at_path, configured_editor_argv, runtime_override_value, toml_string_array_literal,
};
use config_watch::ConfigWatch;
use configuration_workers::{ConfigReadOperation, ConfigReloadWorker, ConfigWriteWorker};
use desktop_portals::{PortalCapabilities, PortalProbeWorker};
use desktop_preferences::{ReducedMotionProbeWorker, SystemFontScaleProbeWorker};
#[cfg(test)]
use download_files::finalize_staged_download;
#[cfg(test)]
use download_files::parse_user_dirs_download;
use download_files::{
    StagedDownload, cleanup_print_artifact, cleanup_staged_download, cleanup_staged_downloads,
    configured_download_directory_path, path_to_file_url, stage_download_path,
    validate_pdf_output_path, validate_print_pdf_path, validate_save_page_path,
};
use editor_process::{
    EditorCompletion, PendingConfigEdit, PendingEditor, cleanup_editor_artifact,
    discard_config_edit, discard_editor_request, join_editor_stderr, terminate_child_process,
    terminate_editor_process, try_wait_editor_process,
};
use effect_projection::{engine_action_name, restore_entry_line};
use feature_preferences::FeaturePreferences;
use ferric_browser_application::userscript;
use ferric_browser_application::userscript_io::read_bounded;
use ferric_browser_application::{OperationState, OperationUpdate};
use ferric_browser_core::FailureKind;
use focus_policy::{FocusObservation, focus_mode_transition};
use hint_payload::{hint_json, hint_kind_name, parse_hint_candidate, parse_hint_payload};
use hint_policy::{
    HintScopeBinding, LocalInteractionAuthority, LocalInteractionPermit, external_hint_target,
    hint_uses_url_action, parse_hint_options, rapid_hint_keeps_mode,
};
use input_validation::{
    is_bounded_journey_query, is_bounded_untrusted_text, validate_jseval_script,
};
use ipc_command_decoder::{interactive_open_command, typed_ipc_command};
use ipc_contract::{IpcOpenTarget, IpcRoute};
use ipc_dispatch_context::{
    command_context as ipc_command_context, command_invocation as ipc_command_invocation,
};
use ipc_event_projection::is_mutating_ipc_method;
use ipc_response::{
    IpcQueryError, action_failure_category, ipc_action_failure, ipc_action_failure_with_context,
    ipc_command_failure_with_context, ipc_failure,
};
use ipc_transport::publish_ipc_event;
use library_presentation::JourneyGraphEdgePresentation;
use macro_policy::{
    MAX_MACRO_COMMANDS, MAX_MACRO_DEPTH, is_repeatable_command, valid_macro_register,
};
use macro_projection::{state_value as macro_state_value, status_text as macro_status_text};
use native_files::{
    atomic_write_private, bounded_header, resolve_browser_roots, scalar_cursor_to_utf16,
    set_private_directory_permissions, utf16_cursor_to_scalar,
};
use native_prompt_lease::NativePromptLeases;
use navigation_policy::{
    configured_search_url, journey_transition_after_load,
    navigation_context as navigation_context_from_config_and_quickmarks,
    recordable_same_document_change, strip_url_fragment,
};
use open_policy::clean_input as clean_open_input;
use operation_policy::{action_audit_record, action_operation_id};
use operation_projection::{operations_query_value, remember_operation_stderr};
use page_document_identity::{PageDocumentIdentities, PendingSameDocumentPassThrough};
#[cfg(test)]
use presentation_text::MAX_PAGE_TITLE_BYTES;
use presentation_text::{
    bounded_navigation_failure_detail, normalized_navigation_failure_kind, sanitize_untrusted_title,
};
use process_output::sanitize_process_stderr;
use profile_workers::{
    ProfileDeleteWorker, ProfileListWorker, ProfileMutationResult, ProfilePreviewWorker,
    profile_from_list_values,
};
use runtime_guard::{
    captured_target_is_current, current_target, elapsed_ms, live_document_available,
    validate_clipboard_navigation_input,
};
use switcher_index::{
    SwitcherLibraryIndex, SwitcherLibraryIndexResult, build_switcher_library_index,
};
use switcher_policy::{
    action_allowed as switcher_action_allowed, context_boost as switcher_context_boost,
    default_action as switcher_default_action, parse_command as parse_switcher_command,
    parse_generation as parse_switcher_generation,
    validate_generation as validate_switcher_generation,
};
use switcher_presentation::{SwitcherCandidate, select_switcher_page};
use url_presentation::{
    blocking_site_host, canonical_engine_url, display_url, link_preview_presentation,
    link_result_value,
};
use url_safety::{safe_ipc_url, safe_site_host, safe_site_origin};
#[cfg(test)]
use userscript_catalog::{
    action_value as userscript_action_value, is_available as userscript_action_is_available,
};
use userscript_catalog::{
    action_values as userscript_action_values, is_hint_only as userscript_action_is_hint_only,
    subject_argument_name as userscript_action_argument_name,
    subject_target as userscript_subject_target,
};
use window_registry::LiveWindowRegistryEntry;
#[cfg(test)]
use window_registry::decode_live_window_registry;

pub use diagnostics::{action_error_summary, storage_health as diagnostics_storage_health};
pub use ipc_transport::spawn_ipc_server;

/// Returns the display-free diagnostic snapshot. The CLI may perform the
/// bounded probes here; the live Qt surface collects its report off-thread.
/// Qt runtime facts are captured on the GUI thread before collection starts.
#[must_use]
pub fn diagnostics_snapshot() -> Value {
    let mut snapshot = diagnostics::snapshot_with_compositor(diagnostics::hyprland_version_fact());
    apply_qt_webengine_fact(&mut snapshot);
    snapshot
}

pub fn enable_software_rendering() {
    qobject::ferric_browser_enable_software_rendering();
}

pub fn register_internal_scheme() {
    qobject::ferric_browser_register_internal_scheme();
}

pub fn set_desktop_identity() {
    qobject::ferric_browser_set_desktop_identity();
}

#[must_use]
pub fn qt_platform_name() -> String {
    qobject::ferric_browser_qt_platform_name().to_string()
}

#[must_use]
pub fn qt_quick_graphics_api() -> String {
    qobject::ferric_browser_qt_quick_graphics_api().to_string()
}

#[must_use]
pub fn qt_webengine_version() -> String {
    qobject::ferric_browser_qt_webengine_version().to_string()
}

#[must_use]
pub fn qt_chromium_version() -> String {
    qobject::ferric_browser_qt_chromium_version().to_string()
}

#[must_use]
pub fn qt_chromium_security_patch_version() -> String {
    qobject::ferric_browser_qt_chromium_security_patch_version().to_string()
}

fn apply_qt_runtime_facts(snapshot: &mut Value) {
    runtime_facts::apply_runtime_facts(
        snapshot,
        qt_platform_name(),
        qt_quick_graphics_api(),
        qt_webengine_version(),
        qt_chromium_version(),
        qt_chromium_security_patch_version(),
    );
}

fn apply_qt_webengine_fact(snapshot: &mut Value) {
    runtime_facts::apply_webengine_fact(
        snapshot,
        qt_webengine_version(),
        qt_chromium_version(),
        qt_chromium_security_patch_version(),
    );
}

static FORCED_SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Records that the user explicitly crossed the shutdown force boundary.
/// The application bootstrap leaves the crash marker in place for this path.
pub fn mark_forced_shutdown() {
    FORCED_SHUTDOWN.store(true, Ordering::Release);
}

#[must_use]
pub fn shutdown_was_forced() -> bool {
    FORCED_SHUTDOWN.load(Ordering::Acquire)
}

mod bridge;
use bridge::qobject;

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QUrl, QVariant};
use ferric_browser_application::{
    BrowserApplication, ConfigurationLayers, ContextCommand, ProfileOpenEffect, ProfileOpenRequest,
    ProfileStorage, StorageEffect, StorageRequest, StorageTicket,
};
use ferric_browser_config::{
    ActionTargetConfig, Config, ContextsConfig, HyprlandConfig, LoggingConfig, RuntimeOverrides,
    ThemePalette, load_theme_palette, matching_context_routes, matching_site_rules,
    save_contexts_atomic, setting_metadata, setting_metadata_all, setting_supported_scopes,
    setting_supports_site_scope,
};
#[cfg(test)]
use ferric_browser_core::ActionSubject;
use ferric_browser_core::{
    ActionDefinition, ActionRegistry, ActionSource, BindingOutcome, BindingResolver, BindingTrie,
    CleanLinkResult, CommandInvocation, CommandRegistry, CommandSource, CompletionCandidate,
    CompletionCategory, DEFAULT_COMPLETION_LIMIT, Diagnostic, DispatchTarget, Effect,
    EffectiveMixedKeyPolicy, EngineEffect, Event, ExistenceState, GridCell,
    HintAutoFollow as CoreHintAutoFollow, HintInteraction, HintInteractionInput,
    HintInteractionOutcome, HintSession, HintTarget, HintTransport, IdSource, JourneyEdgeKind,
    JourneyNodeId, LoadingState, LogicalRect, Mode, NavigationContext, NavigationError,
    NavigationSource, ParseInput, ParsedCommand, PointerButton, PrivacyKind, ResourceLifecycle,
    SearchCase, SpatialAck, SpatialAction, SpatialCancelReason, SpatialDispatchOutcome,
    SpatialError, SpatialLifecycle, SpatialOwner, SpatialScope, SpatialSession, SpatialTarget,
    SurfaceStamp, TabId, TabTransfer, Target, ValidatedUrl, WindowId, assign_labels_with_options,
    canonical_origin, clean_link, complete, hint_intent_for_candidate, map_css_rect_to_view_local,
    parse_chain, resolve_input, switcher_rank, tokenize_switcher_query,
};
use ferric_browser_ipc::{
    PublicError, Request, Response, has_pending_requests, install_pending_request_waker,
    subscribe_event_stream, take_pending_request,
};
use ferric_browser_runtime::worker::{Poll as WorkerPoll, SingleFlightWorker, SubmitError};
use ferric_browser_runtime::{
    BrowserRuntime, RuntimeCommand, RuntimeDispatch, RuntimeEffect, RuntimeInput,
};
#[cfg(test)]
use ferric_browser_storage::ProfilePrivacy;
#[cfg(test)]
use ferric_browser_storage::ProfileRegistry;
#[cfg(test)]
use ferric_browser_storage::Quickmark;
use ferric_browser_storage::RootSpec;
use ferric_browser_storage::{
    ClosedTabSnapshot, CollisionPolicy, ContextMember, ContextTabDescriptor, DownloadState,
    HistoryRecord, JourneyQuerySnapshot, JourneyWrite, MarkWrite, PermissionRule,
    ProfileLibrarySnapshot, RestoreSafety, SessionSnapshot, SnapshotTabInput, SnapshotWindowInput,
    StorageCompletion, StorageRoots, StorageWorkerError, VisitInput, choose_download_path,
    crash_diagnostics, is_safe_history_url, named_session_path, sanitize_download_filename,
    unix_timestamp,
};
use ipc_params::{
    query_bool_param, query_empty_object_or_null, query_limit, query_object, query_offset,
    query_optional_string,
};
use serde_json::Value;
use std::cell::{Cell, RefCell};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::time::Instant;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use uuid::Uuid;

const PAGE_SCRIPT_DEADLINE: Duration = Duration::from_secs(2);
const SITE_DOCTOR_EXPERIMENT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_UNTRUSTED_ARGUMENT_BYTES: usize = 16 * 1024;
const MAX_USER_DIRS_FILE_BYTES: u64 = 16 * 1024;
const MAX_JOURNEY_REDIRECT_HOPS: u8 = 64;
const MAX_ACTION_AUDIT_RECORDS: usize = 128;
const MAX_DIAGNOSTICS_EXPORT_BYTES: usize = 256 * 1024;
const MAX_PRIVATE_HISTORY: usize = 1_000;

#[allow(clippy::struct_excessive_bools)]
pub struct BrowserUiRust {
    initial_url: QString,
    current_url: QString,
    display_url: QString,
    page_title: QString,
    status_text: QString,
    macro_status_text: QString,
    load_state: QString,
    navigation_failure_kind: QString,
    navigation_failure_requested_url: QString,
    navigation_failure_url: QString,
    navigation_failure_detail: QString,
    navigation_failure_visible: bool,
    mode: QString,
    input_epoch: i32,
    learning_mode: bool,
    view_alive: bool,
    // The application boundary owns reducer state and durable resources; Qt
    // receives only read projections and typed effects.
    state: Option<BrowserApplication>,
    window: Option<WindowId>,
    tab: Option<TabId>,
    registry: Arc<CommandRegistry>,
    bindings: Option<BindingResolver>,
    mixed_policy: EffectiveMixedKeyPolicy,
    core_mode: Mode,
    binding_clock: Instant,
    pending_engine_action: Option<String>,
    pending_engine_actions: VecDeque<engine_requests::NativeRequest>,
    pending_print: Option<PathBuf>,
    pending_history_clear: Option<(Option<i64>, Option<String>)>,
    tab_ids: Vec<TabId>,
    popup_tab_ids: Vec<TabId>,
    active_tab_index: i32,
    tab_count: i32,
    completion_text: QString,
    completion_values: QString,
    completion_start: i32,
    completion_end: i32,
    completion_selected: i32,
    completion_visible: bool,
    command_retryable: bool,
    command_history: Vec<String>,
    command_history_index: Option<usize>,
    // None before recall; Some("") is an active browse with an empty draft.
    command_history_draft: Option<String>,
    queued_command_history: Vec<(String, i64)>,
    command_history_write_pending: bool,
    command_history_write_error: Option<String>,
    start_page_owner_token: QString,
    start_page_eligible_sites: QStringList,
    start_page_eligible_urls: QStringList,
    content_history_revision: u64,
    private_start_state: Arc<Mutex<browser_ui_start_page::PrivateStartState>>,
    start_rank_requested: bool,
    start_page_view: QString,
    start_page_error: QString,
    favourite_action_id: QString,
    favourite_action_state: QString,
    favourite_action_message: QString,
    favourite_action_detail: QString,
    start_page_busy: bool,
    start_page_revision: u64,
    start_page_rank_revision: u64,
    start_page_profile_revision: u64,
    start_page_ready: bool,
    start_page_kinds: QStringList,
    start_page_ids: QStringList,
    start_page_urls: QStringList,
    start_page_titles: QStringList,
    start_page_sites: QStringList,
    start_page_slots: QStringList,
    guide_categories: QStringList,
    guide_titles: QStringList,
    guide_keys: QStringList,
    guide_modes: QStringList,
    guide_commands: QStringList,
    guide_summaries: QStringList,
    guide_ids: QStringList,
    guide_explanations: QStringList,
    guide_arguments: QStringList,
    guide_examples: QStringList,
    guide_recoveries: QStringList,
    guide_counts: QStringList,
    guide_binding_notes: QStringList,
    guide_details: QStringList,
    binding_overlay: QString,
    binding_help_row_kinds: QStringList,
    binding_help_row_titles: QStringList,
    binding_help_row_modes: QStringList,
    binding_help_row_commands: QStringList,
    binding_help_row_descriptions: QStringList,
    binding_help_row_keys: QStringList,
    binding_help_row_sources: QStringList,
    binding_help_row_counts: QStringList,
    switcher_result_kinds: QStringList,
    switcher_result_ids: QStringList,
    switcher_result_generations: QStringList,
    switcher_result_labels: QStringList,
    switcher_result_secondaries: QStringList,
    switcher_result_profiles: QStringList,
    switcher_result_workspaces: QStringList,
    switcher_result_actions: QStringList,
    switcher_result_ranks: QStringList,
    switcher_result_recencies: QStringList,
    switcher_sources_pending: bool,
    switcher_sources_failed: bool,
    switcher_sources_retryable: bool,
    switcher_sources_notice: QString,
    switcher_sources_detail: QString,
    switcher_request_scope: QString,
    switcher_request_query: QString,
    search_text: QString,
    search_backward: bool,
    session_restore_values: QString,
    session_recovery_pending: bool,
    session_recovery_error: QString,
    session_preview_labels: QStringList,
    session_preview_urls: QStringList,
    session_preview_pinned: QStringList,
    session_preview_muted: QStringList,
    session_preview: QString,
    session_preview_request: Option<(String, bool)>,
    bookmark_transfer_id: QString,
    bookmark_transfer_state: QString,
    bookmark_transfer_message: QString,
    bookmark_transfer_detail: QString,
    mark_action_id: QString,
    mark_action_state: QString,
    mark_action_message: QString,
    mark_action_detail: QString,
    library_pending: bool,
    diagnostics_summary: QStringList,
    diagnostics_available: bool,
    diagnostics_pending: bool,
    diagnostics_preview: QString,
    diagnostics_error: QString,
    profile_startup_notice: QString,
    library_error: QString,
    session_catalog_read_state: CatalogReadState,
    journey_read_pending: bool,
    profile_action_id: QString,
    profile_action_state: QString,
    profile_action_message: QString,
    profile_action_detail: QString,
    session_save_id: QString,
    session_save_state: QString,
    session_save_message: QString,
    session_save_detail: QString,
    session_delete_id: QString,
    session_delete_state: QString,
    session_delete_message: QString,
    session_delete_detail: QString,
    profile_list_error: QString,
    profile_preview_error: QString,
    session_names_values: QStringList,
    session_names_pending: bool,
    session_names_error: QString,
    profile_values: QString,
    profile_values_pending: bool,
    userscript_inventory_pending: bool,
    userscript_inventory_ready: bool,
    userscript_inventory_error: QString,
    userscript_inventory_refresh_again: bool,
    userscript_names: QStringList,
    userscript_enabled_values: QStringList,
    userscript_page_world_values: QStringList,
    userscript_action_counts: QStringList,
    userscript_action_ids: QStringList,
    userscript_action_labels: QStringList,
    userscript_action_availability: QStringList,
    page_userscript_names: QStringList,
    page_userscript_sources: QStringList,
    page_userscript_run_at: QStringList,
    page_userscript_runs_on_sub_frames: QStringList,
    site_rule_javascript_set: bool,
    site_rule_javascript_enabled: bool,
    site_rule_images_set: bool,
    site_rule_images_enabled: bool,
    site_rule_force_dark_set: bool,
    site_rule_force_dark_enabled: bool,
    site_rule_autoplay_set: bool,
    site_rule_autoplay: QString,
    site_rule_zoom_set: bool,
    site_rule_zoom: f64,
    external_action_ids: QStringList,
    external_action_labels: QStringList,
    external_action_availability: QStringList,
    userscript_install_state: QString,
    download_desktop_uri: QString,
    download_request_token: QString,
    download_request_url: QString,
    profile_bootstrap_pending: bool,
    profile_list_command_pending: bool,
    pending_profile_open: Option<(String, String)>,
    pending_window_new_profile: Option<(String, bool)>,
    pending_context_create: Option<(String, String, String, Option<String>)>,
    profile_preview_text: QString,
    profile_preview_pending: bool,
    library_kind: QString,
    library_values: QString,
    library_graph_edge_sources: QStringList,
    library_graph_edge_targets: QStringList,
    library_graph_edge_transitions: QStringList,
    journey_export_preview_text: QString,
    journey_export_payload: Option<String>,
    storage_library_revision: i64,
    profile_list_refresh_again: bool,
    pending_permission_reset_session: Option<bool>,
    link_preview_command: QString,
    link_preview_original: QString,
    link_preview_cleaned: QString,
    link_preview_applied_rules: QStringList,
    link_preview_removed_parameters: QStringList,
    link_preview_retained_parameters: QStringList,
    link_preview_explanation: QString,
    link_preview_requires_confirmation: bool,
    link_preview_visible: bool,
    pending_link_navigation: Option<PendingLinkNavigation>,
    external_navigation_uri: QString,
    external_navigation_scheme: QString,
    external_navigation_visible: bool,
    pending_external_navigation: Option<PendingExternalNavigation>,
    hint_values: QString,
    hint_visible: bool,
    target_search_active: bool,
    target_search_scope: QString,
    target_search_query: QString,
    hint_pointer_mode: bool,
    hint_pointer_pending: bool,
    hint_pointer_state: QString,
    hint_pointer_dispatch_request: QString,
    hint_links_only: bool,
    hint_rapid: bool,
    hint_family: QString,
    hint_target: QString,
    hint_proxy_targets_enabled: bool,
    hint_unmatched_policy: QString,
    hint_chrome_available: bool,
    hint_marker_scale: f64,
    hint_rapid_target: String,
    hint_script: Option<String>,
    hint_first: bool,
    hint_index: usize,
    hint_rapid_tabs_created: u8,
    pending_hint_action: Option<String>,
    spatial_visible: bool,
    spatial_selecting: bool,
    spatial_session_id: QString,
    spatial_scope_lease_id: QString,
    spatial_dispatch_request: QString,
    spatial_labels: QStringList,
    spatial_viewport_x: f64,
    spatial_viewport_y: f64,
    spatial_viewport_width: f64,
    spatial_viewport_height: f64,
    spatial_root_x: f64,
    spatial_root_y: f64,
    spatial_root_width: f64,
    spatial_root_height: f64,
    spatial_current_x: f64,
    spatial_current_y: f64,
    spatial_current_width: f64,
    spatial_current_height: f64,
    spatial_crosshair_x: f64,
    spatial_crosshair_y: f64,
    spatial_depth: i32,
    spatial_help_visible: bool,
    caret_selecting: bool,
    caret_request_token: QString,
    caret_request_operation: QString,
    editor_completion_token: QString,
    editor_completion_original: QString,
    editor_completion_updated: QString,
    editor_completion_error: QString,
    editor_completion_stderr: QString,
    clipboard_request: QString,
    clipboard_request_sensitive: bool,
    clipboard_request_primary: bool,
    pending_download: Option<PendingDownload>,
    staged_downloads: BTreeMap<String, StagedDownload>,
    download_finalizer: Option<download_finalization::DownloadFinalizer>,
    download_stream_writer: Option<download_stream_writer::StreamDownloadWriter>,
    download_controller_available: bool,
    pending_save_page: Option<PathBuf>,
    pending_print_private: bool,
    runtime_config_persistence: &'static str,
    config_json: QString,
    selector_validation_request: QString,
    settings_rows: QVariant,
    runtime_setting_error: QString,
    setting_action_id: QString,
    setting_action_state: QString,
    setting_action_message: QString,
    setting_action_detail: QString,
    chrome_font_family: QString,
    chrome_font_size_pt: f64,
    chrome_statusbar_mode: QString,
    chrome_tabs_mode: QString,
    chrome_tab_position: QString,
    chrome_reduced_motion: QString,
    feature_switcher_max_results: i32,
    feature_downloads_ask_destination: bool,
    feature_desktop_notifications_enabled: bool,
    feature_desktop_media_keys_enabled: bool,
    feature_push_service_enabled: bool,
    feature_spellcheck_enabled: bool,
    feature_spellcheck_languages: QStringList,
    feature_blocking_list_ids: QStringList,
    feature_blocking_update_interval_hours: i32,
    feature_link_cleaning_update_source: QString,
    feature_link_cleaning_update_sha256: QString,
    config_base_json: QString,
    cli_overrides_json: QString,
    profile_overrides_json: QString,
    config_path: QString,
    config_source: QString,
    contexts_json: QString,
    context_choice_names: QStringList,
    context_choice_labels: QStringList,
    context_choice_profiles: QStringList,
    context_name: QString,
    context_label: QString,
    context_workspace: QString,
    context_accent: QString,
    context_entry_force_reuse: bool,
    context_route_id: QString,
    context_route_behavior: QString,
    context_route_context: QString,
    context_route_profile: QString,
    context_route_url: QString,
    window_token: QString,
    core_window_id: QString,
    hyprland_status: QString,
    hyprland_clients_json: QString,
    desktop_portal_mode: QString,
    system_reduced_motion_status: QString,
    system_reduced_motion_enabled: bool,
    system_font_scale_status: QString,
    system_font_scale: f64,
    theme_background_color: QString,
    theme_surface_color: QString,
    theme_panel_color: QString,
    theme_primary_text_color: QString,
    theme_secondary_text_color: QString,
    theme_muted_text_color: QString,
    theme_border_color: QString,
    theme_accent_color: QString,
    theme_warning_color: QString,
    theme_error_color: QString,
    theme_success_color: QString,
    theme_private_color: QString,
    theme_mode_insert_color: QString,
    theme_selection_color: QString,
    theme_selection_text_color: QString,
    theme_contrast_status: QString,
    theme_contrast_reason: QString,
    site_status: QString,
    site_experiment_active: bool,
    site_experiment_id: QString,
    site_experiment_kind: QString,
    site_experiment_url: QString,
    site_experiment_remaining_seconds: i64,
    blocking_hosts: QStringList,
    blocking_exceptions: QStringList,
    blocking_rule_hosts: QStringList,
    blocking_rule_list_ids: QStringList,
    blocking_exception_rule_hosts: QStringList,
    blocking_exception_rule_list_ids: QStringList,
    blocking_loaded_lists: QString,
    blocking_list_metadata: QString,
    blocking_skipped_lists: QString,
    blocking_bypass_sites: QStringList,
    blocking_cosmetic_rule_hosts: QStringList,
    blocking_cosmetic_rule_selectors: QStringList,
    blocking_cosmetic_exception_hosts: QStringList,
    blocking_cosmetic_exception_selectors: QStringList,
    blocking_adblock_source_ids: QString,
    blocking_adblock_handle: u64,
    blocking_policy_ready: bool,
    blocking_security_deny_hosts: QStringList,
    blocking_enabled: bool,
    blocking_blocked_count: i64,
    blocking_active_site_count: i64,
    blocking_unknown_context_count: i64,
    blocking_active_evidence: blocking_evidence::BlockingEvidence,
    config: Value,
    config_revision: u64,
    policy_revision: u64,
    portal_capabilities: PortalCapabilities,
    theme_palette: ThemePalette,
    base_config: Value,
    profile_overrides: RuntimeOverrides,
    runtime_overrides: RuntimeOverrides,
    temporary_overrides: RuntimeOverrides,
    cli_overrides: RuntimeOverrides,
    print_worker: Option<PrintWorker>,
    profile_delete_worker: Option<ProfileDeleteWorker>,
    hyprland_worker: Option<HyprlandWorker>,
    portal_probe_worker: Option<PortalProbeWorker>,
    reduced_motion_probe_worker: Option<ReducedMotionProbeWorker>,
    font_scale_probe_worker: Option<SystemFontScaleProbeWorker>,
    pending_profile_configuration: Option<PendingProfileConfiguration>,
    profile_open_effect: Option<ProfileOpenEffect>,
    profile_startup: profile_startup::ProfileStartup,
    profile_storage_base: String,
    profile_list_worker: Option<ProfileListWorker>,
    profile_preview_worker: Option<ProfilePreviewWorker>,
    ipc_sequence: u64,
    ipc_shutdown_gate: bool,
    contexts: Option<ContextSnapshot>,
    context_catalog_revision: u64,
    pending_context_route: Option<PendingContextRoute>,
    profile_name: String,
    profile_id: Option<Uuid>,
    profile_persistence: ProfilePersistence,
    private_history: Vec<HistoryRecord>,
    private_history_next_id: i64,
    storage_roots: Option<StorageRoots>,
    profile_registry_roots: Option<StorageRoots>,
    policy_roots: Option<StorageRoots>,
    session_id: Uuid,
    session_path: Option<PathBuf>,
    session_state_root: Option<PathBuf>,
    recovery_paths: Vec<PathBuf>,
    recovery_placeholder: Option<Target>,
    recovered_session_ids: Vec<Uuid>,
    checkpoint: SessionCheckpoint,
    closed_tabs: Vec<ClosedTabDescriptor>,
    undo_fallback: Option<Target>,
    undo_fallback_ready: bool,
    action_audit: Vec<Value>,
    structured_log: Option<logging::StructuredLogSink>,
    request_resolution_counts: BTreeMap<(String, String), u64>,
    active_site_experiment: Option<SiteExperiment>,
    last_site_doctor_result: Option<Value>,
    last_repeatable: Option<RepeatableCommand>,
    macro_registers: BTreeMap<String, Vec<RepeatableCommand>>,
    recording_macro: Option<(String, Vec<RepeatableCommand>)>,
    macro_key_prefix: Option<String>,
    macro_key_started_ms: Option<u64>,
    macro_depth: u8,
    macro_expanded_commands: usize,
    live_window_registry: Vec<LiveWindowRegistryEntry>,
    ipc_owner: Option<live_ipc_owners::OwnerLease>,
    navigation: feature_state::NavigationState,
    processes: feature_state::ProcessState,
    configuration_io: feature_state::ConfigurationIoState,
    library: feature_state::LibraryState,
    interaction: feature_state::InteractionState,
    diagnostic_work: feature_state::DiagnosticState,
}

#[derive(Clone, Debug)]
struct SiteExperiment {
    id: String,
    kind: String,
    tab: TabId,
    temporary_tab: Option<TabId>,
    origin: String,
    url: String,
    prior_bypass_sites: Vec<String>,
    created_at: Instant,
}

fn site_doctor_remaining_seconds(created_at: Instant, now: Instant) -> Option<u64> {
    let elapsed = now.saturating_duration_since(created_at);
    if elapsed >= SITE_DOCTOR_EXPERIMENT_TIMEOUT {
        None
    } else {
        Some(
            SITE_DOCTOR_EXPERIMENT_TIMEOUT
                .as_secs()
                .saturating_sub(elapsed.as_secs()),
        )
    }
}

#[derive(Clone, Debug)]
struct ClosedTabDescriptor {
    id: Uuid,
    url: String,
    title: String,
    profile: String,
    private: bool,
    closed_at: i64,
    group_index: Option<usize>,
    pinned: bool,
    muted: bool,
    zoom_hundredths: u32,
}

#[derive(Clone, Debug)]
struct RepeatableCommand {
    command: ParsedCommand,
    count: u32,
    // Keep the original authority and routing policy for IPC and typed opens.
    // Local commands continue to resolve against the current keyboard target.
    route: Option<IpcRoute>,
}

#[derive(Clone, Debug)]
struct PendingLinkNavigation {
    target: Option<Target>,
    new_tab_route: Option<IpcRoute>,
    new_window_action: Option<String>,
    journey_parent: Option<JourneyNodeId>,
    repeat_route: Option<IpcRoute>,
    url: ValidatedUrl,
}

#[derive(Clone, Debug)]
struct JourneyReopenRecord {
    id: String,
    url: String,
}

#[derive(Clone, Debug)]
struct PendingExternalNavigation {
    scheme: String,
    url: String,
}

#[derive(Clone, Debug)]
struct PendingSelection {
    token: String,
    target: Target,
    primary: bool,
    issued: bool,
    created_at: Instant,
    search_engine: Option<String>,
    operation_id: Option<String>,
}

fn finish_pending_selection_operation(this: &mut BrowserUiRust, status: OperationUpdate) {
    // Retire the request and every consumer together. Late page replies must
    // never retain authority to launch work after cancellation or replacement.
    let operation_ids = [
        this.processes
            .pending_selection
            .take()
            .and_then(|request| request.operation_id),
        this.processes
            .pending_spawn
            .take()
            .map(|request| request.operation_id),
        this.processes
            .pending_action_target
            .take()
            .map(|request| request.operation_id),
        this.processes
            .pending_userscript
            .take()
            .map(|request| request.operation_id),
    ];
    for operation_id in operation_ids.into_iter().flatten() {
        this.set_operation(operation_id, status.clone());
    }
}

fn cancel_pending_selection_operation(this: &mut BrowserUiRust, operation_id: &str) {
    if this
        .processes
        .pending_selection
        .as_ref()
        .and_then(|request| request.operation_id.as_deref())
        == Some(operation_id)
    {
        finish_pending_selection_operation(this, OperationUpdate::cancelled());
    }
}

fn selection_failure_message(reason: &str) -> &'static str {
    match reason {
        "selection-source-changed" | "selection-source-stale" | "selection-changed" => {
            "The selection changed. Select the text again and retry."
        }
        "selection-source-timeout" | "selection-extraction-timeout" => {
            "Reading the selected text took too long. Try again."
        }
        "password fields are not copied" => "Copying password fields as plain text is disabled.",
        "shadow focus depth is unsupported"
        | "focused selection is unsupported"
        | "closed-shadow selection is unsupported" => {
            "Plain-text selection is unsupported in this control. Use Copy instead."
        }
        "selection is too large" => "Select a smaller passage of text and try again.",
        _ => "Select text on the page and try again.",
    }
}

#[derive(Clone, Debug)]
struct PendingCaret {
    token: String,
    target: Target,
    operation: String,
    issued: bool,
}

#[derive(Clone, Debug)]
struct PendingSpawn {
    token: String,
    target: Target,
    argv: Vec<String>,
    operation_id: String,
}

#[derive(Clone, Debug)]
struct PendingActionTarget {
    token: String,
    target: Target,
    target_name: String,
    target_config: ActionTargetConfig,
    operation_id: String,
    created_at: Instant,
}

#[derive(Clone, Debug)]
struct PendingUserscript {
    token: String,
    target: Target,
    name: String,
    label: String,
    operation_id: String,
}

#[derive(Debug)]
struct PendingDownload {
    operation_id: String,
    created_at: Instant,
    retry: Option<ferric_browser_application::DownloadRetryPermit>,
    token: String,
    target: Target,
    url: String,
    issued: bool,
    visited: std::collections::BTreeSet<String>,
    last_url: Option<String>,
}

#[derive(Clone, Debug)]
struct SpawnCompletion {
    operation_id: String,
    status: OperationUpdate,
}

#[derive(Clone, Debug)]
struct UserscriptCompletion {
    operation_id: String,
    status: OperationUpdate,
    target: Target,
    action: Option<userscript::UserscriptAction>,
    stderr: String,
}

fn bootstrap_runtime(
    privacy: PrivacyKind,
    profile_label: &str,
) -> Option<(BrowserRuntime, WindowId, TabId)> {
    static IDS: std::sync::OnceLock<ferric_browser_core::SharedIdAllocator> =
        std::sync::OnceLock::new();
    BrowserRuntime::bootstrap_with_ids(
        privacy,
        profile_label,
        IdSource::from_shared(IDS.get_or_init(Default::default).clone()),
    )
    .ok()
}

fn ensure_spawn_working_directory(rust: &mut BrowserUiRust) -> Result<PathBuf, String> {
    if let Some(path) = rust.processes.spawn_working_directory.as_ref() {
        return Ok(path.clone());
    }
    let path = rust.storage_roots.as_ref().map_or_else(
        || std::env::temp_dir().join(format!("ferric-browser-spawn-{}", rust.session_id)),
        |roots| roots.runtime.join("spawn"),
    );
    fs::create_dir_all(&path)
        .map_err(|error| format!("could not create process working directory: {error}"))?;
    set_private_directory_permissions(&path)
        .map_err(|error| format!("could not secure process working directory: {error}"))?;
    rust.processes.spawn_working_directory = Some(path.clone());
    Ok(path)
}

impl qobject::BrowserUi {
    /// Collects typed application effects. Qt never borrows the storage worker
    /// or any of its response channels.
    fn poll_storage_effects(mut self: Pin<&mut Self>) -> Vec<StorageEffect> {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        Option::as_mut(&mut this.state)
            .map(BrowserApplication::poll_storage_effects)
            .unwrap_or_default()
    }

    fn storage_is_open(&self) -> bool {
        self.rust()
            .state
            .as_ref()
            .is_some_and(BrowserApplication::storage_is_open)
    }

    fn request_permission_preferences(
        mut self: Pin<&mut Self>,
        origin: &str,
    ) -> Result<Option<StorageTicket>, StorageWorkerError> {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        Option::as_mut(&mut this.state)
            .ok_or(StorageWorkerError::Stopped)?
            .refresh_permission_preferences(origin)
    }

    fn submit_storage(
        mut self: Pin<&mut Self>,
        request: StorageRequest,
    ) -> Result<(), StorageWorkerError> {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        Option::as_mut(&mut this.state)
            .ok_or(StorageWorkerError::Stopped)?
            .submit_storage(request)
            .map(|_| ())
    }

    fn submit_storage_with_ticket(
        mut self: Pin<&mut Self>,
        request: StorageRequest,
    ) -> Result<StorageTicket, StorageWorkerError> {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        Option::as_mut(&mut this.state)
            .ok_or(StorageWorkerError::Stopped)?
            .submit_storage(request)
    }

    fn navigation_context(&self) -> NavigationContext {
        navigation_context_from_config_and_quickmarks(
            &self.rust().config,
            self.rust()
                .library
                .storage_library
                .as_ref()
                .map(|library| library.quickmarks.as_slice())
                .unwrap_or(&[]),
            false,
        )
    }

    fn navigation_context_for_source(&self, source: CommandSource) -> NavigationContext {
        navigation_context_from_config_and_quickmarks(
            &self.rust().config,
            self.rust()
                .library
                .storage_library
                .as_ref()
                .map(|library| library.quickmarks.as_slice())
                .unwrap_or(&[]),
            source == CommandSource::Cli,
        )
    }

    fn active_profile_privacy(&self) -> Option<PrivacyKind> {
        let state = self.rust().state.as_ref()?;
        let tab = state.active_tab()?;
        state
            .profiles()
            .get(&tab.profile)
            .map(|profile| profile.privacy)
    }

    fn active_profile_is_transient(&self) -> bool {
        self.active_profile_privacy()
            .is_some_and(PrivacyKind::is_transient)
    }
}

#[cfg(test)]
mod tests;
