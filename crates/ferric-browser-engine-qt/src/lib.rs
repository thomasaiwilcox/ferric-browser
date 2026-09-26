//! The narrow Qt boundary for `Ferric Browser`.
//!
//! Browser policy and durable state stay in `ferric-browser-core`. This crate owns
//! only the `QObject` properties and QML registration needed to compose the
//! native Qt Quick/WebEngine surface.

#![allow(unsafe_code)]
#![allow(clippy::unnecessary_box_returns)]
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
// Architecture budget exception: this root may exceed 1,000 lines only because
// it contains the audited CXX-Qt bridge declarations and stable QObject-facing
// state shape. Handwritten implementation logic belongs in the bounded modules
// registered below.

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
mod browser_ui_command_dispatch;
mod browser_ui_command_history;
mod browser_ui_command_workflows;
mod browser_ui_configuration;
mod browser_ui_content_tools;
mod browser_ui_context_commands;
mod browser_ui_context_navigation;
mod browser_ui_contexts;
mod browser_ui_controls;
mod browser_ui_downloads_permissions;
mod browser_ui_edit_commands;
mod browser_ui_external_actions;
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
mod browser_ui_maintenance;
mod browser_ui_navigation;
mod browser_ui_navigation_lifecycle;
mod browser_ui_persistence;
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
mod browser_ui_sessions;
mod browser_ui_site;
mod browser_ui_spatial;
mod browser_ui_state;
mod browser_ui_storage_writes;
mod browser_ui_tab_state;
mod browser_ui_tabs;
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
mod diagnostics;
mod diagnostics_overview;
mod diagnostics_snapshot;
mod download_files;
mod editor_process;
mod effect_projection;
mod feature_preferences;
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
mod logging;
mod macro_policy;
mod macro_projection;
mod maintenance;
mod native_files;
mod navigation_lifecycle;
mod navigation_policy;
mod network_policy;
mod open_policy;
mod operation_policy;
mod operation_projection;
mod presentation_text;
mod process_output;
mod profile_management;
mod profile_workers;
mod renderer_lifecycle;
mod runtime_bridge;
mod runtime_facts;
mod runtime_guard;
mod settings_presentation;
mod shutdown_control;
mod single_flight;
mod switcher_index;
mod switcher_policy;
mod switcher_presentation;
mod tab_presentation;
mod theme_presentation;
mod url_presentation;
mod url_safety;
pub mod userscript;
mod userscript_catalog;
mod userscript_discovery;
mod userscript_install;
mod userscript_io;
mod userscript_manifest;
mod userscript_presentation;
mod userscript_protocol;
mod userscript_storage;
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
    PrintWorker, UserscriptManagerRequest, UserscriptManagerResult, UserscriptManagerWorker,
};
use binding_policy::{
    binding_command_parameters, binding_uses_full_command_executor, config_get_command_parameters,
    configured_bindings, configured_undo_limit, ipc_mode_name, is_context_command,
    is_library_command, is_profile_command, is_session_command, learning_mode_request,
    modal_command_prefill, normalize_active_tab_command, parse_ipc_mode, switcher_max_results,
};
#[cfg(test)]
use browser_ui_state::PermissionLifetimeKey;
use browser_ui_state::{
    ContextSnapshot, PendingContextRoute, PendingProfileConfiguration, PermissionDecisionKey,
    ProfilePersistence, SessionCheckpoint, SwitcherQueryCache, permission_session_key,
};
use chrome_preferences::ChromePreferences;
use command_options::{
    command_count as ipc_command_count, history_clear as parse_history_clear_arguments,
    is_link_clean as is_link_clean_command, is_reopen_in_window as is_reopen_in_window_command,
    is_scroll as is_scroll_command, is_search_next as is_search_next_command,
    is_tab_clone as is_tab_clone_command, is_tab_detach as is_tab_detach_command,
    is_tab_give as is_tab_give_command, is_tab_undo as is_tab_undo_command,
    is_yank as is_yank_command, is_zoom as is_zoom_command, scroll as parse_scroll_options,
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
use download_files::parse_user_dirs_download;
use download_files::{
    StagedDownload, cleanup_print_artifact, cleanup_staged_download, cleanup_staged_downloads,
    configured_download_directory_path, finalize_staged_download, path_to_file_url,
    stage_download_path, validate_pdf_output_path, validate_print_pdf_path,
    validate_save_page_path,
};
use editor_process::{
    EditorCompletion, PendingConfigEdit, PendingEditor, cleanup_editor_artifact,
    discard_config_edit, discard_editor_request, join_editor_stderr, terminate_child_process,
    terminate_editor_process, try_wait_editor_process,
};
use effect_projection::{engine_action_name, restore_entry_line};
use feature_preferences::FeaturePreferences;
use focus_policy::{FocusObservation, focus_mode_transition};
use hint_payload::{hint_json, hint_kind_name, parse_hint_candidate, parse_hint_payload};
use hint_policy::{
    external_hint_target, hint_uses_url_action, parse_hint_options, rapid_hint_keeps_mode,
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
use ipc_transport::{instance_id as ipc_instance_id, publish_ipc_event};
use library_presentation::JourneyGraphEdgePresentation;
use macro_policy::{
    MAX_MACRO_COMMANDS, MAX_MACRO_DEPTH, is_repeatable_command, valid_macro_register,
};
use macro_projection::{state_value as macro_state_value, status_text as macro_status_text};
use native_files::{
    atomic_write_private, bounded_header, resolve_browser_roots, scalar_cursor_to_utf16,
    set_private_directory_permissions, utf16_cursor_to_scalar,
};
use navigation_policy::{
    configured_search_url, journey_transition_after_load,
    navigation_context as navigation_context_from_config_and_quickmarks,
    recordable_same_document_change, strip_url_fragment,
};
use open_policy::clean_input as clean_open_input;
use operation_policy::{
    action_audit_record, action_operation_id, operation_is_terminal, operation_status_kind,
};
use operation_projection::{operations_query_value, remember_operation_stderr};
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
use userscript_io::read_bounded;
use window_registry::LiveWindowRegistryEntry;
#[cfg(test)]
use window_registry::decode_live_window_registry;

pub use diagnostics::{action_error_summary, storage_health as diagnostics_storage_health};
pub use ipc_transport::spawn_ipc_server;

/// Returns the display-free diagnostic snapshot. The CLI may perform the
/// bounded compositor probe here; live Qt diagnostics use `diagnostics::snapshot`
/// and never synchronously query compositor IPC on the GUI thread.
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

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        include!("cxx-qt-lib/qstringlist.h");
        include!("cxx-qt-lib/qvariant.h");
        include!("url_display.h");
        type QString = cxx_qt_lib::QString;
        type QStringList = cxx_qt_lib::QStringList;
        type QVariant = cxx_qt_lib::QVariant;

        fn ferric_browser_to_ascii_host(host: &QString) -> QString;
        fn ferric_browser_canonicalize_url(value: &QString) -> QString;
        fn ferric_browser_read_clipboard() -> QString;
        fn ferric_browser_read_primary_selection() -> QString;
        fn ferric_browser_primary_selection_available() -> bool;
        fn ferric_browser_write_clipboard(value: &QString, primary: bool) -> bool;
        fn ferric_browser_qt_platform_name() -> QString;
        fn ferric_browser_qt_quick_graphics_api() -> QString;
        fn ferric_browser_qt_webengine_version() -> QString;
        fn ferric_browser_qt_chromium_version() -> QString;
        fn ferric_browser_qt_chromium_security_patch_version() -> QString;
        fn ferric_browser_set_desktop_identity();
        fn ferric_browser_enable_software_rendering();
        fn ferric_browser_register_internal_scheme();
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, initial_url)]
        #[qproperty(QString, current_url)]
        #[qproperty(QString, display_url)]
        #[qproperty(QString, page_title)]
        #[qproperty(QString, status_text)]
        #[qproperty(QString, macro_status_text)]
        #[qproperty(QString, load_state)]
        #[qproperty(QString, navigation_failure_kind)]
        #[qproperty(QString, navigation_failure_requested_url)]
        #[qproperty(QString, navigation_failure_url)]
        #[qproperty(QString, navigation_failure_detail)]
        #[qproperty(bool, navigation_failure_visible)]
        #[qproperty(QString, mode)]
        #[qproperty(bool, learning_mode)]
        #[qproperty(bool, view_alive)]
        #[qproperty(i32, active_tab_index)]
        #[qproperty(i32, tab_count)]
        #[qproperty(QString, completion_text)]
        #[qproperty(QString, completion_values)]
        #[qproperty(i32, completion_start)]
        #[qproperty(i32, completion_end)]
        #[qproperty(i32, completion_selected)]
        #[qproperty(bool, completion_visible)]
        #[qproperty(bool, command_retryable)]
        #[qproperty(QString, binding_overlay)]
        #[qproperty(QStringList, binding_help_row_kinds)]
        #[qproperty(QStringList, binding_help_row_titles)]
        #[qproperty(QStringList, binding_help_row_modes)]
        #[qproperty(QStringList, binding_help_row_commands)]
        #[qproperty(QStringList, binding_help_row_descriptions)]
        #[qproperty(QStringList, binding_help_row_keys)]
        #[qproperty(QStringList, binding_help_row_sources)]
        #[qproperty(QStringList, binding_help_row_counts)]
        #[qproperty(QStringList, switcher_result_kinds)]
        #[qproperty(QStringList, switcher_result_ids)]
        #[qproperty(QStringList, switcher_result_generations)]
        #[qproperty(QStringList, switcher_result_labels)]
        #[qproperty(QStringList, switcher_result_secondaries)]
        #[qproperty(QStringList, switcher_result_profiles)]
        #[qproperty(QStringList, switcher_result_workspaces)]
        #[qproperty(QStringList, switcher_result_actions)]
        #[qproperty(QStringList, switcher_result_ranks)]
        #[qproperty(QStringList, switcher_result_recencies)]
        #[qproperty(QString, switcher_request_scope)]
        #[qproperty(QString, switcher_request_query)]
        #[qproperty(QString, search_text)]
        #[qproperty(bool, search_backward)]
        #[qproperty(QString, session_restore_values)]
        #[qproperty(QString, session_preview)]
        #[qproperty(QString, profile_values)]
        #[qproperty(bool, profile_values_pending)]
        #[qproperty(QStringList, userscript_names)]
        #[qproperty(QStringList, userscript_enabled_values)]
        #[qproperty(QStringList, userscript_page_world_values)]
        #[qproperty(QStringList, userscript_action_counts)]
        #[qproperty(QStringList, userscript_action_ids)]
        #[qproperty(QStringList, userscript_action_labels)]
        #[qproperty(QStringList, userscript_action_availability)]
        #[qproperty(QStringList, page_userscript_names)]
        #[qproperty(QStringList, page_userscript_sources)]
        #[qproperty(QStringList, page_userscript_run_at)]
        #[qproperty(QStringList, page_userscript_runs_on_sub_frames)]
        #[qproperty(bool, site_rule_javascript_set)]
        #[qproperty(bool, site_rule_javascript_enabled)]
        #[qproperty(bool, site_rule_images_set)]
        #[qproperty(bool, site_rule_images_enabled)]
        #[qproperty(bool, site_rule_force_dark_set)]
        #[qproperty(bool, site_rule_force_dark_enabled)]
        #[qproperty(bool, site_rule_autoplay_set)]
        #[qproperty(QString, site_rule_autoplay)]
        #[qproperty(bool, site_rule_zoom_set)]
        #[qproperty(f64, site_rule_zoom)]
        #[qproperty(QStringList, external_action_ids)]
        #[qproperty(QStringList, external_action_labels)]
        #[qproperty(QStringList, external_action_availability)]
        #[qproperty(QString, userscript_install_state)]
        #[qproperty(QString, download_desktop_uri)]
        #[qproperty(QString, download_request_token)]
        #[qproperty(QString, download_request_url)]
        #[qproperty(bool, profile_bootstrap_pending)]
        #[qproperty(QString, profile_preview_text)]
        #[qproperty(bool, profile_preview_pending)]
        #[qproperty(QString, library_kind)]
        #[qproperty(QString, library_values)]
        #[qproperty(QStringList, library_graph_edge_sources)]
        #[qproperty(QStringList, library_graph_edge_targets)]
        #[qproperty(QStringList, library_graph_edge_transitions)]
        #[qproperty(QString, journey_export_preview_text)]
        #[qproperty(i64, storage_library_revision)]
        #[qproperty(QString, link_preview_command)]
        #[qproperty(QString, link_preview_original)]
        #[qproperty(QString, link_preview_cleaned)]
        #[qproperty(QStringList, link_preview_applied_rules)]
        #[qproperty(QStringList, link_preview_removed_parameters)]
        #[qproperty(QStringList, link_preview_retained_parameters)]
        #[qproperty(QString, link_preview_explanation)]
        #[qproperty(bool, link_preview_requires_confirmation)]
        #[qproperty(bool, link_preview_visible)]
        #[qproperty(QString, external_navigation_uri)]
        #[qproperty(QString, external_navigation_scheme)]
        #[qproperty(bool, external_navigation_visible)]
        #[qproperty(QString, hint_values)]
        #[qproperty(bool, hint_visible)]
        #[qproperty(bool, hint_links_only)]
        #[qproperty(bool, hint_rapid)]
        #[qproperty(QString, hint_family)]
        #[qproperty(QString, hint_unmatched_policy)]
        #[qproperty(f64, hint_marker_scale)]
        #[qproperty(bool, spatial_visible)]
        #[qproperty(QString, spatial_session_id)]
        #[qproperty(QString, spatial_dispatch_request)]
        #[qproperty(QString, spatial_labels)]
        #[qproperty(f64, spatial_root_x)]
        #[qproperty(f64, spatial_root_y)]
        #[qproperty(f64, spatial_root_width)]
        #[qproperty(f64, spatial_root_height)]
        #[qproperty(f64, spatial_current_x)]
        #[qproperty(f64, spatial_current_y)]
        #[qproperty(f64, spatial_current_width)]
        #[qproperty(f64, spatial_current_height)]
        #[qproperty(f64, spatial_crosshair_x)]
        #[qproperty(f64, spatial_crosshair_y)]
        #[qproperty(i32, spatial_depth)]
        #[qproperty(bool, spatial_help_visible)]
        #[qproperty(bool, caret_selecting)]
        #[qproperty(QString, caret_request_token)]
        #[qproperty(QString, caret_request_operation)]
        #[qproperty(QString, editor_completion_token)]
        #[qproperty(QString, editor_completion_original)]
        #[qproperty(QString, editor_completion_updated)]
        #[qproperty(QString, editor_completion_error)]
        #[qproperty(QString, editor_completion_stderr)]
        #[qproperty(QString, jseval_tab_id)]
        #[qproperty(QString, jseval_world)]
        #[qproperty(QString, jseval_script)]
        #[qproperty(QString, clipboard_request)]
        #[qproperty(bool, clipboard_request_sensitive)]
        #[qproperty(bool, clipboard_request_primary)]
        #[qproperty(QString, config_json)]
        #[qproperty(QVariant, settings_rows)]
        #[qproperty(QString, runtime_setting_error)]
        #[qproperty(QString, chrome_font_family)]
        #[qproperty(f64, chrome_font_size_pt)]
        #[qproperty(QString, chrome_statusbar_mode)]
        #[qproperty(QString, chrome_tabs_mode)]
        #[qproperty(QString, chrome_tab_position)]
        #[qproperty(QString, chrome_reduced_motion)]
        #[qproperty(i32, feature_switcher_max_results)]
        #[qproperty(bool, feature_downloads_ask_destination)]
        #[qproperty(bool, feature_desktop_notifications_enabled)]
        #[qproperty(bool, feature_desktop_media_keys_enabled)]
        #[qproperty(bool, feature_push_service_enabled)]
        #[qproperty(bool, feature_spellcheck_enabled)]
        #[qproperty(QStringList, feature_spellcheck_languages)]
        #[qproperty(QStringList, feature_blocking_list_ids)]
        #[qproperty(i32, feature_blocking_update_interval_hours)]
        #[qproperty(QString, feature_link_cleaning_update_source)]
        #[qproperty(QString, feature_link_cleaning_update_sha256)]
        #[qproperty(QString, config_base_json)]
        #[qproperty(QString, cli_overrides_json)]
        #[qproperty(QString, profile_overrides_json)]
        #[qproperty(QString, config_path)]
        #[qproperty(QString, config_source)]
        #[qproperty(QString, contexts_json)]
        #[qproperty(QStringList, context_choice_names)]
        #[qproperty(QStringList, context_choice_labels)]
        #[qproperty(QStringList, context_choice_profiles)]
        #[qproperty(QString, context_name)]
        #[qproperty(QString, context_label)]
        #[qproperty(QString, context_workspace)]
        #[qproperty(QString, context_accent)]
        #[qproperty(bool, context_entry_force_reuse)]
        #[qproperty(QString, context_route_id)]
        #[qproperty(QString, context_route_behavior)]
        #[qproperty(QString, context_route_context)]
        #[qproperty(QString, context_route_profile)]
        #[qproperty(QString, context_route_url)]
        #[qproperty(QString, window_token)]
        #[qproperty(QString, core_window_id)]
        #[qproperty(QString, hyprland_status)]
        #[qproperty(QString, hyprland_clients_json)]
        #[qproperty(QString, desktop_portal_mode)]
        #[qproperty(QString, system_reduced_motion_status)]
        #[qproperty(bool, system_reduced_motion_enabled)]
        #[qproperty(QString, system_font_scale_status)]
        #[qproperty(f64, system_font_scale)]
        #[qproperty(QString, theme_background_color)]
        #[qproperty(QString, theme_surface_color)]
        #[qproperty(QString, theme_panel_color)]
        #[qproperty(QString, theme_primary_text_color)]
        #[qproperty(QString, theme_secondary_text_color)]
        #[qproperty(QString, theme_muted_text_color)]
        #[qproperty(QString, theme_border_color)]
        #[qproperty(QString, theme_accent_color)]
        #[qproperty(QString, theme_warning_color)]
        #[qproperty(QString, theme_error_color)]
        #[qproperty(QString, theme_success_color)]
        #[qproperty(QString, theme_private_color)]
        #[qproperty(QString, theme_mode_insert_color)]
        #[qproperty(QString, theme_selection_color)]
        #[qproperty(QString, theme_selection_text_color)]
        #[qproperty(QString, theme_contrast_status)]
        #[qproperty(QString, theme_contrast_reason)]
        #[qproperty(QString, site_status)]
        #[qproperty(bool, site_experiment_active)]
        #[qproperty(QString, site_experiment_id)]
        #[qproperty(QString, site_experiment_kind)]
        #[qproperty(QString, site_experiment_url)]
        #[qproperty(i64, site_experiment_remaining_seconds)]
        #[qproperty(QStringList, blocking_hosts)]
        #[qproperty(QStringList, blocking_exceptions)]
        #[qproperty(QStringList, blocking_rule_hosts)]
        #[qproperty(QStringList, blocking_rule_list_ids)]
        #[qproperty(QStringList, blocking_exception_rule_hosts)]
        #[qproperty(QStringList, blocking_exception_rule_list_ids)]
        #[qproperty(QString, blocking_loaded_lists)]
        #[qproperty(QString, blocking_list_metadata)]
        #[qproperty(QString, blocking_skipped_lists)]
        #[qproperty(QStringList, blocking_bypass_sites)]
        #[qproperty(QStringList, blocking_cosmetic_rule_hosts)]
        #[qproperty(QStringList, blocking_cosmetic_rule_selectors)]
        #[qproperty(QStringList, blocking_cosmetic_exception_hosts)]
        #[qproperty(QStringList, blocking_cosmetic_exception_selectors)]
        #[qproperty(QString, blocking_adblock_source_ids)]
        #[qproperty(u64, blocking_adblock_handle)]
        #[qproperty(QStringList, blocking_security_deny_hosts)]
        #[qproperty(bool, blocking_enabled)]
        #[qproperty(i64, blocking_blocked_count)]
        #[qproperty(i64, blocking_active_site_count)]
        #[qproperty(i64, blocking_unknown_context_count)]
        type BrowserUi = super::BrowserUiRust;

        #[qsignal]
        fn runtime_work_available(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn activate_runtime_wake(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn maintenance_delay_ms(self: Pin<&mut BrowserUi>) -> i32;

        #[qinvokable]
        fn navigate(self: Pin<&mut BrowserUi>, input: &QString);

        #[qinvokable]
        fn navigate_initial(
            self: Pin<&mut BrowserUi>,
            input: &QString,
            entry_point: &QString,
            trusted_local_input: bool,
        );

        #[qinvokable]
        fn navigate_without_context_route(self: Pin<&mut BrowserUi>, input: &QString);

        #[qinvokable]
        fn set_blocking_active_evidence(
            self: Pin<&mut BrowserUi>,
            explanation: &QStringList,
            decisions: &QStringList,
        ) -> bool;

        #[qinvokable]
        fn publish_blocking_live_counts(
            self: Pin<&mut BrowserUi>,
            blocked: f64,
            unknown_context: f64,
            active_site: f64,
        ) -> bool;

        #[qinvokable]
        fn clear_blocking_active_evidence(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn set_contexts_configuration(self: Pin<&mut BrowserUi>, input: &QString) -> bool;

        #[qinvokable]
        fn set_context_entry_reuse(self: Pin<&mut BrowserUi>, force_reuse: bool);

        #[qinvokable]
        fn set_startup_configuration(
            self: Pin<&mut BrowserUi>,
            config: &QString,
            base_config: &QString,
            cli_overrides: &QString,
            profile_overrides: &QString,
            path: &QString,
            source: &QString,
        ) -> bool;

        #[qinvokable]
        fn publish_live_window_registry(
            self: Pin<&mut BrowserUi>,
            ids: &QStringList,
            owner_tokens: &QStringList,
            profiles: &QStringList,
            private_flags: &QStringList,
            ephemeral_flags: &QStringList,
            tab_counts: &QStringList,
        ) -> bool;

        #[qinvokable]
        fn note_renderer_process_terminated(self: Pin<&mut BrowserUi>, tab_index: i32) -> bool;

        #[qinvokable]
        fn prepare_renderer_recovery(self: Pin<&mut BrowserUi>, tab_index: i32) -> bool;

        #[qinvokable]
        fn execute_command(self: Pin<&mut BrowserUi>, input: &QString) -> bool;

        #[qinvokable]
        fn execute_interactive_command(self: Pin<&mut BrowserUi>, input: &QString) -> bool;

        #[qinvokable]
        fn execute_secondary_interactive_command(
            self: Pin<&mut BrowserUi>,
            input: &QString,
        ) -> bool;

        #[qinvokable]
        fn command_history_previous(self: Pin<&mut BrowserUi>, current: &QString) -> QString;

        #[qinvokable]
        fn command_history_next(self: Pin<&mut BrowserUi>, current: &QString) -> QString;

        #[qinvokable]
        fn execute_ui_action(
            self: Pin<&mut BrowserUi>,
            action_id: &QString,
            value: &QString,
        ) -> bool;

        #[qinvokable]
        fn select_userscript_action_subject(self: Pin<&mut BrowserUi>, subject: &QString) -> bool;

        #[qinvokable]
        fn refresh_userscript_inventory(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn set_userscript_enabled(self: Pin<&mut BrowserUi>, name: &QString, enabled: bool)
        -> bool;

        #[qinvokable]
        fn remove_userscript(self: Pin<&mut BrowserUi>, name: &QString) -> bool;

        #[qinvokable]
        fn select_external_action_subject(self: Pin<&mut BrowserUi>, subject: &QString) -> bool;

        #[qinvokable]
        fn install_userscript_manifest(self: Pin<&mut BrowserUi>, path: &QString) -> bool;

        #[qinvokable]
        fn journey_export_preview(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn export_journey(self: Pin<&mut BrowserUi>, path: &QString) -> bool;

        #[qinvokable]
        fn paste_open(self: Pin<&mut BrowserUi>, target: &QString) -> bool;

        #[qinvokable]
        fn paste_open_primary(self: Pin<&mut BrowserUi>, target: &QString) -> bool;

        #[qinvokable]
        fn new_tab(self: Pin<&mut BrowserUi>) -> i32;

        #[qinvokable]
        fn select_tab(self: Pin<&mut BrowserUi>, index: i32) -> bool;

        #[qinvokable]
        fn commit_tab_suspend(self: Pin<&mut BrowserUi>, id: &QString) -> bool;

        #[qinvokable]
        fn commit_tab_discard(self: Pin<&mut BrowserUi>, id: &QString) -> bool;

        #[qinvokable]
        fn tab_has_active_operations(self: Pin<&mut BrowserUi>, id: &QString) -> bool;

        #[qinvokable]
        fn tab_index_for_id(self: Pin<&mut BrowserUi>, id: &QString) -> i32;

        #[qinvokable]
        fn tab_id_for_index(self: Pin<&mut BrowserUi>, index: i32) -> QString;

        #[qinvokable]
        fn tab_transfer_payload(self: Pin<&mut BrowserUi>, id: &QString) -> QString;

        #[qinvokable]
        fn adopt_tab_transfer(self: Pin<&mut BrowserUi>, payload: &QString) -> bool;

        #[qinvokable]
        fn complete_tab_transfer(self: Pin<&mut BrowserUi>, id: &QString) -> bool;

        #[qinvokable]
        fn rollback_tab_transfer(self: Pin<&mut BrowserUi>, id: &QString) -> bool;

        #[qinvokable]
        fn set_tab_pinned(self: Pin<&mut BrowserUi>, index: i32, pinned: bool) -> i32;

        #[qinvokable]
        fn set_tab_muted(self: Pin<&mut BrowserUi>, index: i32, muted: bool) -> bool;

        #[qinvokable]
        fn move_tab(self: Pin<&mut BrowserUi>, index: i32, delta: i32) -> i32;

        #[qinvokable]
        fn update_completion(self: Pin<&mut BrowserUi>, input: &QString, cursor: i32) -> bool;

        #[qinvokable]
        fn completion_move(self: Pin<&mut BrowserUi>, delta: i32);

        #[qinvokable]
        fn completion_select(self: Pin<&mut BrowserUi>, index: i32);

        #[qinvokable]
        fn enter_search(self: Pin<&mut BrowserUi>, backward: bool);

        #[qinvokable]
        fn search_changed(self: Pin<&mut BrowserUi>, input: &QString);

        #[qinvokable]
        fn search_next(self: Pin<&mut BrowserUi>, backward: bool);

        #[qinvokable]
        fn accept_search(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn recover_session(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn checkpoint_session(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn set_scroll_position(self: Pin<&mut BrowserUi>, index: i32, x: f64, y: f64) -> bool;

        #[qinvokable]
        fn flush_durable_state(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn clear_current_session_checkpoints(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn save_named_session(self: Pin<&mut BrowserUi>, name: &QString) -> bool;

        #[qinvokable]
        fn request_named_session_load(
            self: Pin<&mut BrowserUi>,
            name: &QString,
            append: bool,
        ) -> bool;

        #[qinvokable]
        fn list_named_sessions(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn delete_named_session(self: Pin<&mut BrowserUi>, name: &QString, confirmed: bool)
        -> bool;

        #[qinvokable]
        fn take_session_restore_values(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn request_session_preview(self: Pin<&mut BrowserUi>, name: &QString) -> bool;

        #[qinvokable]
        fn take_session_preview(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn clear_link_preview(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn confirm_link_navigation(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn confirm_external_navigation(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn cancel_external_navigation(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn begin_hint_session(self: Pin<&mut BrowserUi>, candidates_json: &QString) -> QString;

        #[qinvokable]
        fn update_hint_interaction(
            self: Pin<&mut BrowserUi>,
            action: &QString,
            text: &QString,
        ) -> QString;

        #[qinvokable]
        fn select_hint(
            self: Pin<&mut BrowserUi>,
            label: &QString,
            fresh_candidate_json: &QString,
        ) -> QString;

        #[qinvokable]
        fn select_hint_action(
            self: Pin<&mut BrowserUi>,
            label: &QString,
            fresh_candidate_json: &QString,
            action_id: &QString,
        ) -> QString;

        #[qinvokable]
        fn confirm_rapid_hint_tabs(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn cancel_hints(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn list_profiles(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn request_profile_list(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn queue_bookmark_transfer(
            self: Pin<&mut BrowserUi>,
            url: &QString,
            title: &QString,
        ) -> bool;

        #[qinvokable]
        fn create_profile(self: Pin<&mut BrowserUi>, name: &QString, label: &QString) -> bool;

        #[qinvokable]
        fn rename_profile(self: Pin<&mut BrowserUi>, name: &QString, label: &QString) -> bool;

        #[qinvokable]
        fn profile_delete_preview(self: Pin<&mut BrowserUi>, name: &QString) -> QString;

        #[qinvokable]
        fn request_profile_delete_preview(self: Pin<&mut BrowserUi>, name: &QString) -> bool;

        #[qinvokable]
        fn delete_profile(self: Pin<&mut BrowserUi>, name: &QString, confirmed: bool) -> bool;

        #[qinvokable]
        fn default_download_directory(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn offer_download(
            self: Pin<&mut BrowserUi>,
            id: &QString,
            source_url: &QString,
            suggested_name: &QString,
        ) -> QString;

        #[qinvokable]
        fn accept_download(
            self: Pin<&mut BrowserUi>,
            id: &QString,
            directory: &QString,
            suggested_name: &QString,
        ) -> QString;

        #[qinvokable]
        fn accept_download_path(self: Pin<&mut BrowserUi>, id: &QString, path: &QString)
        -> QString;

        #[qinvokable]
        fn download_staging_directory(self: Pin<&mut BrowserUi>, id: &QString) -> QString;

        #[qinvokable]
        fn finalize_download(self: Pin<&mut BrowserUi>, id: &QString) -> bool;

        #[qinvokable]
        fn discard_download_staging(self: Pin<&mut BrowserUi>, id: &QString);

        #[qinvokable]
        fn update_download(
            self: Pin<&mut BrowserUi>,
            id: &QString,
            state: &QString,
            bytes_received: i64,
            finished: bool,
        ) -> bool;

        #[qinvokable]
        fn list_downloads(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn download_desktop_action(self: Pin<&mut BrowserUi>, id: &QString, reveal: bool) -> bool;

        #[qinvokable]
        fn request_download_action(
            self: Pin<&mut BrowserUi>,
            id: &QString,
            action: &QString,
        ) -> bool;

        #[qinvokable]
        fn request_shutdown(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn begin_shutdown_gate(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn end_shutdown_gate(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn complete_window_focus(
            self: Pin<&mut BrowserUi>,
            operation_id: &QString,
            outcome: &QString,
        ) -> bool;

        #[qinvokable]
        fn complete_transfer_operation(
            self: Pin<&mut BrowserUi>,
            operation_id: &QString,
            succeeded: bool,
        ) -> bool;

        #[qinvokable]
        fn force_quit(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn restart_software_rendering(
            self: Pin<&mut BrowserUi>,
            lock_path: &QString,
            storage_base: &QString,
            temporary_profile: bool,
            safe_mode: bool,
            userscripts_off: bool,
            instance_selector: &QString,
        ) -> bool;

        #[qinvokable]
        fn prepare_print_pdf(self: Pin<&mut BrowserUi>, path: &QString) -> QString;

        #[qinvokable]
        fn prepare_save_page(self: Pin<&mut BrowserUi>, path: &QString) -> QString;

        #[qinvokable]
        fn take_save_page_path(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn finish_print_pdf(self: Pin<&mut BrowserUi>, path: &QString, succeeded: bool) -> bool;

        #[qinvokable]
        fn prepare_print_job(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn finish_print_job(self: Pin<&mut BrowserUi>, path: &QString, succeeded: bool) -> bool;

        #[qinvokable]
        fn take_download_request(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn complete_download_request(
            self: Pin<&mut BrowserUi>,
            token: &QString,
            succeeded: bool,
        ) -> bool;

        #[qinvokable]
        fn switcher_query(self: Pin<&mut BrowserUi>, query: &QString, scope: &QString) -> bool;

        #[qinvokable]
        fn poll_storage_library(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn activate_switcher_result(
            self: Pin<&mut BrowserUi>,
            kind: &QString,
            id: &QString,
            generation: &QString,
        ) -> bool;

        #[qinvokable]
        fn activate_switcher_action(
            self: Pin<&mut BrowserUi>,
            kind: &QString,
            id: &QString,
            action: &QString,
            generation: &QString,
        ) -> bool;

        #[qinvokable]
        fn hyprland_route_workspace(self: Pin<&mut BrowserUi>, workspace: &QString) -> bool;
        fn hyprland_move_active_window(self: Pin<&mut BrowserUi>, workspace: &QString) -> bool;

        #[qinvokable]
        fn hyprland_browser_clients(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn close_tab(self: Pin<&mut BrowserUi>, index: i32) -> bool;

        #[qinvokable]
        fn popup_allowed(
            self: Pin<&mut BrowserUi>,
            requested_url: &QString,
            user_initiated: bool,
        ) -> bool;

        #[qinvokable]
        fn popup_allowed_for(
            self: Pin<&mut BrowserUi>,
            index: i32,
            requested_url: &QString,
            user_initiated: bool,
        ) -> bool;

        #[qinvokable]
        fn take_popup_journey_token(self: Pin<&mut BrowserUi>, requested_url: &QString) -> QString;

        #[qinvokable]
        fn popup_navigation_started(self: Pin<&mut BrowserUi>, token: &QString, url: QString);

        #[qinvokable]
        fn popup_navigation_url_changed(self: Pin<&mut BrowserUi>, token: &QString, url: QString);

        #[qinvokable]
        fn popup_navigation_committed(
            self: Pin<&mut BrowserUi>,
            token: &QString,
            url: QString,
            title: QString,
        );

        #[qinvokable]
        fn popup_navigation_completed(self: Pin<&mut BrowserUi>, token: &QString);

        #[qinvokable]
        fn popup_navigation_failed(self: Pin<&mut BrowserUi>, token: &QString);

        #[qinvokable]
        fn release_popup_journey_token(self: Pin<&mut BrowserUi>, token: &QString);

        #[qinvokable]
        fn close_popup_tab(self: Pin<&mut BrowserUi>, token: &QString) -> bool;

        #[qinvokable]
        fn configure_profile(
            self: Pin<&mut BrowserUi>,
            private_profile: bool,
            ephemeral_profile: bool,
            profile_label: &QString,
            profile_name: &QString,
            storage_base: &QString,
        );

        #[qinvokable]
        fn install_blocking_list(
            self: Pin<&mut BrowserUi>,
            list_id: &QString,
            content: &QString,
            etag: &QString,
            last_modified: &QString,
        ) -> bool;

        #[qinvokable]
        fn accept_link_cleaning_bundle(
            self: Pin<&mut BrowserUi>,
            content: &QString,
            expected_checksum: &QString,
            etag: &QString,
            last_modified: &QString,
        ) -> bool;

        #[qinvokable]
        fn reload_blocking_policy(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn poll_config(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn probe_desktop_portals(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn portal_capability_status(self: Pin<&mut BrowserUi>, capability: &QString) -> QString;

        #[qinvokable]
        fn set_runtime_setting(
            self: Pin<&mut BrowserUi>,
            key: &QString,
            literal: &QString,
            temporary: bool,
        ) -> bool;

        #[qinvokable]
        fn unset_runtime_setting(self: Pin<&mut BrowserUi>, key: &QString, temporary: bool)
        -> bool;

        #[qinvokable]
        fn toggle_blocking_site(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn permission_decision(
            self: Pin<&mut BrowserUi>,
            origin: &QString,
            permission: &QString,
        ) -> QString;

        #[qinvokable]
        fn remember_permission(
            self: Pin<&mut BrowserUi>,
            origin: &QString,
            permission: &QString,
            decision: &QString,
            lifetime: &QString,
        ) -> bool;

        #[qinvokable]
        fn list_permissions(self: Pin<&mut BrowserUi>, origin: &QString) -> QString;

        #[qinvokable]
        fn reset_permission(
            self: Pin<&mut BrowserUi>,
            origin: &QString,
            permission: &QString,
        ) -> bool;

        #[qinvokable]
        fn refresh_site_status(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn site_report(self: Pin<&mut BrowserUi>, include_host: bool) -> QString;

        #[qinvokable]
        fn site_doctor_proposal(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn apply_site_doctor_proposal(
            self: Pin<&mut BrowserUi>,
            proposal_id: &QString,
            confirmed: bool,
        ) -> bool;

        #[qinvokable]
        fn site_data_clear_plan(self: Pin<&mut BrowserUi>, origin: &QString) -> QString;

        #[qinvokable]
        fn site_data_clear(self: Pin<&mut BrowserUi>, origin: &QString, confirmed: bool) -> bool;

        #[qinvokable]
        fn site_data_clear_finished(
            self: Pin<&mut BrowserUi>,
            origin: &QString,
            result: &QString,
        ) -> bool;

        #[qinvokable]
        fn diagnostics_json(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn spellcheck_dictionaries(self: Pin<&mut BrowserUi>) -> QStringList;

        #[qinvokable]
        fn export_diagnostics(self: Pin<&mut BrowserUi>, path: &QString) -> bool;

        #[qinvokable]
        fn record_request_resolution(
            self: Pin<&mut BrowserUi>,
            request_kind: &QString,
            outcome: &QString,
        );

        #[qinvokable]
        fn refresh_binding_help(self: Pin<&mut BrowserUi>, search: &QString) -> bool;

        #[qinvokable]
        fn begin_site_doctor_experiment(self: Pin<&mut BrowserUi>, kind: &QString) -> QString;

        #[qinvokable]
        fn finish_site_doctor_experiment(
            self: Pin<&mut BrowserUi>,
            experiment_id: &QString,
            succeeded: bool,
        ) -> bool;

        #[qinvokable]
        fn tick_site_doctor_experiment(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn select_matching_page_scripts(
            self: Pin<&mut BrowserUi>,
            url: &QString,
            private_profile: bool,
        ) -> bool;

        #[qinvokable]
        fn page_focus_observed_for(
            self: Pin<&mut BrowserUi>,
            index: i32,
            url: &QString,
            frame_path: &QString,
            sequence: i32,
            editable: bool,
            user_activated: bool,
        );

        #[qinvokable]
        fn select_site_rule_settings(self: Pin<&mut BrowserUi>, url: &QString) -> bool;

        #[qinvokable]
        fn navigation_started(self: Pin<&mut BrowserUi>, url: QString);

        #[qinvokable]
        fn navigation_started_for(self: Pin<&mut BrowserUi>, index: i32, url: QString);

        #[qinvokable]
        fn navigation_url_changed(self: Pin<&mut BrowserUi>, url: QString);

        #[qinvokable]
        fn navigation_url_changed_for(self: Pin<&mut BrowserUi>, index: i32, url: QString);

        #[qinvokable]
        fn navigation_committed(self: Pin<&mut BrowserUi>, url: QString, title: QString);

        #[qinvokable]
        fn navigation_committed_for(
            self: Pin<&mut BrowserUi>,
            index: i32,
            url: QString,
            title: QString,
        );

        #[qinvokable]
        fn navigation_completed(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn navigation_completed_for(self: Pin<&mut BrowserUi>, index: i32);

        #[qinvokable]
        fn navigation_failed(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn navigation_failed_for(self: Pin<&mut BrowserUi>, index: i32);

        #[qinvokable]
        fn navigation_failed_with_details(
            self: Pin<&mut BrowserUi>,
            index: i32,
            url: &QString,
            kind: &QString,
            detail: &QString,
        );

        #[qinvokable]
        fn clear_navigation_failure(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn back(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn forward(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn reload(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn stop(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn handle_key(self: Pin<&mut BrowserUi>, key: &QString) -> bool;

        #[qinvokable]
        fn begin_spatial_navigation(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn spatial_surface_ready(
            self: Pin<&mut BrowserUi>,
            width: f64,
            height: f64,
            serial: &QString,
            revision: &QString,
        ) -> bool;

        #[qinvokable]
        fn spatial_dispatch_ack(
            self: Pin<&mut BrowserUi>,
            request_id: &QString,
            session_id: &QString,
            serial: &QString,
            revision: &QString,
            outcome: &QString,
        ) -> bool;

        #[qinvokable]
        fn spatial_invalidated(self: Pin<&mut BrowserUi>, reason: &QString);

        #[qinvokable]
        fn tick_bindings(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn take_engine_action(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn take_clipboard_request(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn write_clipboard(self: Pin<&mut BrowserUi>, value: &QString, primary: bool) -> bool;

        #[qinvokable]
        fn take_selection_request(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn deliver_selection(self: Pin<&mut BrowserUi>, token: &QString, result: &QString) -> bool;

        #[qinvokable]
        fn take_caret_request(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn deliver_caret(self: Pin<&mut BrowserUi>, token: &QString, result: &QString) -> bool;

        #[qinvokable]
        fn take_editor_request(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn deliver_editor(self: Pin<&mut BrowserUi>, token: &QString, result: &QString) -> bool;

        #[qinvokable]
        fn take_editor_completion(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn deliver_editor_apply(
            self: Pin<&mut BrowserUi>,
            token: &QString,
            result: &QString,
        ) -> bool;

        #[qinvokable]
        fn refresh_operations(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn enter_caret(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn poll_ipc(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn view_closed(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn release_transient_resources(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn view_closed_for(self: Pin<&mut BrowserUi>, index: i32) -> bool;

        #[qinvokable]
        fn escape(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn enter_command(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn enter_insert(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn accept_context_route(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn dismiss_context_route(self: Pin<&mut BrowserUi>) -> bool;
    }

    impl cxx_qt::Threading for BrowserUi {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};
use ferric_browser_application::{
    BrowserApplication, ConfigurationLayers, ContextCommand, ProfileOpenEffect, ProfileOpenRequest,
    ProfileStorage, StorageEffect, StorageRequest, StorageTicket,
};
use ferric_browser_config::{
    ActionTargetConfig, Config, ContextsConfig, HyprlandConfig, LoggingConfig, RuntimeOverrides,
    ThemePalette, load_theme_palette, matching_context_routes, matching_site_rules,
    save_contexts_atomic, save_runtime_overrides_atomic, setting_metadata, setting_metadata_all,
    setting_supported_scopes, setting_supports_site_scope, theme_contrast_report,
};
#[cfg(test)]
use ferric_browser_core::ActionSubject;
use ferric_browser_core::{
    ActionDefinition, ActionRegistry, ActionSource, BindingOutcome, BindingResolver, BindingTrie,
    CleanLinkResult, CommandInvocation, CommandRegistry, CommandSource, CompletionCandidate,
    CompletionCategory, DEFAULT_COMPLETION_LIMIT, Diagnostic, DispatchTarget, Effect, EngineEffect,
    Event, ExistenceState, GridCell, HintAutoFollow as CoreHintAutoFollow, HintInteraction,
    HintInteractionInput, HintInteractionOutcome, HintKind, HintSession, HintTarget, IdSource,
    JourneyEdgeKind, JourneyNodeId, LoadingState, LogicalRect, Mode, NavigationContext,
    NavigationError, NavigationSource, ParseInput, ParsedCommand, PointerButton, PrivacyKind,
    ResourceLifecycle, SearchCase, SpatialAction, SpatialCancelReason, SpatialDispatchOutcome,
    SpatialOwner, SpatialSession, SpatialTarget, SurfaceStamp, TabId, TabTransfer, Target,
    ValidatedUrl, WindowId, assign_labels_with_options, canonical_origin, clean_link, complete,
    parse_chain, refresh_labels, resolve_input, switcher_rank, tokenize_switcher_query,
};
use ferric_browser_ipc::{
    EventNotification, PublicError, Request, Response, has_pending_requests,
    install_pending_request_waker, publish_event, subscribe_event_stream, take_pending_request,
};
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
    DownloadUpdate, HistoryRecord, JourneyQuerySnapshot, JourneyWrite, MarkWrite, PermissionRule,
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
use single_flight::{Poll as WorkerPoll, SingleFlightWorker, SubmitError};
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
    learning_mode: bool,
    view_alive: bool,
    // The application boundary owns reducer state and durable resources; Qt
    // receives only read projections and typed effects.
    state: Option<BrowserApplication>,
    window: Option<WindowId>,
    tab: Option<TabId>,
    registry: CommandRegistry,
    bindings: Option<BindingResolver>,
    core_mode: Mode,
    binding_clock: Instant,
    pending_engine_action: Option<String>,
    pending_engine_actions: VecDeque<String>,
    pending_print: Option<PathBuf>,
    pending_journey_transitions: Vec<(Target, JourneyEdgeKind, String)>,
    pending_journey_parent: Option<(Target, JourneyNodeId)>,
    pending_journey_traversal: Option<Target>,
    pending_journey_traversals: VecDeque<Target>,
    pending_journey_mappings: BTreeMap<StorageTicket, (JourneyNodeId, String)>,
    pending_history_clear: Option<(Option<i64>, Option<String>)>,
    pending_journey_reopen: Option<(String, String, Option<TabId>, String)>,
    pending_navigation_urls: BTreeMap<TabId, String>,
    pending_redirect_tabs: BTreeSet<TabId>,
    pending_redirect_hops: BTreeMap<TabId, u8>,
    popup_journey_targets: Vec<(String, Target, String, bool)>,
    tab_ids: Vec<TabId>,
    popup_tab_ids: Vec<TabId>,
    scroll_positions: BTreeMap<TabId, (f64, f64)>,
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
    command_history_draft: String,
    queued_command_history: Vec<(String, i64)>,
    command_history_write_pending: bool,
    command_history_write_error: Option<String>,
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
    switcher_request_scope: QString,
    switcher_request_query: QString,
    search_text: QString,
    search_backward: bool,
    session_restore_values: QString,
    session_preview: QString,
    profile_values: QString,
    profile_values_pending: bool,
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
    storage_library: Option<ProfileLibrarySnapshot>,
    storage_library_revision: i64,
    storage_library_dirty: Cell<bool>,
    switcher_library_index: Option<SwitcherLibraryIndex>,
    switcher_library_index_revision: i64,
    switcher_library_index_building: bool,
    switcher_library_index_cancel: Option<Arc<AtomicBool>>,
    switcher_library_index_result: Arc<Mutex<Option<SwitcherLibraryIndexResult>>>,
    switcher_cache: Option<SwitcherQueryCache>,
    session_names: Vec<String>,
    session_names_dirty: Cell<bool>,
    session_restore_preview_pending: bool,
    session_restore_load_append: Option<bool>,
    session_restore_recovery_pending: bool,
    session_save_pending: bool,
    session_save_checkpoint: bool,
    session_checkpoint_clear_pending: bool,
    storage_flush_pending: bool,
    storage_flush_error: Option<String>,
    download_destination_pending: bool,
    download_create_pending: bool,
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
    hint_links_only: bool,
    hint_rapid: bool,
    hint_family: QString,
    hint_unmatched_policy: QString,
    hint_marker_scale: f64,
    hint_rapid_target: String,
    hint_script: Option<String>,
    hint_first: bool,
    hint_index: usize,
    hint_rapid_tabs_created: u8,
    pending_hint_action: Option<String>,
    spatial_visible: bool,
    spatial_session_id: QString,
    spatial_dispatch_request: QString,
    spatial_labels: QString,
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
    spatial_session: Option<SpatialSession>,
    spatial_ids: IdSource,
    caret_selecting: bool,
    caret_request_token: QString,
    caret_request_operation: QString,
    editor_completion_token: QString,
    editor_completion_original: QString,
    editor_completion_updated: QString,
    editor_completion_error: QString,
    editor_completion_stderr: QString,
    jseval_tab_id: QString,
    jseval_world: QString,
    jseval_script: QString,
    clipboard_request: QString,
    clipboard_request_sensitive: bool,
    clipboard_request_primary: bool,
    pending_selection: Option<PendingSelection>,
    pending_caret: Option<PendingCaret>,
    pending_editor: Option<PendingEditor>,
    pending_editor_write: Option<(String, PathBuf, bool)>,
    editor_completions: Arc<Mutex<Vec<EditorCompletion>>>,
    pending_config_edit: Option<PendingConfigEdit>,
    pending_spawn: Option<PendingSpawn>,
    pending_action_target: Option<PendingActionTarget>,
    pending_userscript: Option<PendingUserscript>,
    pending_download: Option<PendingDownload>,
    staged_downloads: BTreeMap<String, StagedDownload>,
    pending_save_page: Option<PathBuf>,
    pending_print_private: bool,
    spawn_completions: Arc<Mutex<Vec<SpawnCompletion>>>,
    spawn_cancellations: Arc<Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
    spawn_working_directory: Option<PathBuf>,
    userscript_completions: Arc<Mutex<Vec<UserscriptCompletion>>>,
    userscript_cancellations: Arc<Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
    hint_session: Option<HintSession>,
    hint_interaction: Option<HintInteraction>,
    hint_consumed: BTreeSet<(String, u32)>,
    hint_ids: IdSource,
    config_json: QString,
    settings_rows: QVariant,
    runtime_setting_error: QString,
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
    blocking_security_deny_hosts: QStringList,
    blocking_enabled: bool,
    blocking_blocked_count: i64,
    blocking_active_site_count: i64,
    blocking_unknown_context_count: i64,
    blocking_active_evidence: blocking_evidence::BlockingEvidence,
    focus_observations: BTreeMap<(i32, String), FocusObservation>,
    focus_suppressions: BTreeMap<(i32, String), i32>,
    config: Value,
    portal_capabilities: PortalCapabilities,
    theme_palette: ThemePalette,
    base_config: Value,
    pending_config: Option<Value>,
    profile_overrides: RuntimeOverrides,
    runtime_overrides: RuntimeOverrides,
    temporary_overrides: RuntimeOverrides,
    cli_overrides: RuntimeOverrides,
    config_watch: ConfigWatch,
    config_reload_worker: Option<ConfigReloadWorker>,
    config_write_worker: Option<ConfigWriteWorker>,
    print_worker: Option<PrintWorker>,
    editor_write_worker: Option<EditorWriteWorker>,
    profile_delete_worker: Option<ProfileDeleteWorker>,
    network_policy_worker: Option<NetworkPolicyWorker>,
    hyprland_worker: Option<HyprlandWorker>,
    portal_probe_worker: Option<PortalProbeWorker>,
    reduced_motion_probe_worker: Option<ReducedMotionProbeWorker>,
    font_scale_probe_worker: Option<SystemFontScaleProbeWorker>,
    network_policy_reload_pending: bool,
    pending_profile_configuration: Option<PendingProfileConfiguration>,
    profile_list_worker: Option<ProfileListWorker>,
    profile_preview_worker: Option<ProfilePreviewWorker>,
    userscript_manager_worker: Option<UserscriptManagerWorker>,
    theme_watch: ConfigWatch,
    ipc_sequence: u64,
    ipc_shutdown_gate: bool,
    contexts: Option<ContextSnapshot>,
    pending_context_route: Option<PendingContextRoute>,
    profile_name: String,
    profile_id: Option<Uuid>,
    journey_durable_ids: RefCell<BTreeMap<JourneyNodeId, String>>,
    profile_persistence: ProfilePersistence,
    session_permissions: BTreeMap<PermissionDecisionKey, String>,
    private_history: Vec<HistoryRecord>,
    private_history_next_id: i64,
    storage_roots: Option<StorageRoots>,
    profile_registry_roots: Option<StorageRoots>,
    userscript_roots: Option<StorageRoots>,
    session_id: Uuid,
    session_path: Option<PathBuf>,
    session_state_root: Option<PathBuf>,
    checkpoint: SessionCheckpoint,
    closed_tabs: Vec<ClosedTabDescriptor>,
    operation_states: BTreeMap<String, String>,
    operation_stderr: BTreeMap<String, String>,
    action_audit: Vec<Value>,
    structured_log: Option<logging::StructuredLogSink>,
    diagnostics_preview_ready: bool,
    diagnostics_preview_payload: Option<String>,
    request_resolution_counts: BTreeMap<(String, String), u64>,
    active_site_experiment: Option<SiteExperiment>,
    last_site_doctor_result: Option<Value>,
    last_repeatable: Option<ParsedCommand>,
    macro_registers: BTreeMap<String, Vec<ParsedCommand>>,
    recording_macro: Option<(String, Vec<ParsedCommand>)>,
    macro_key_prefix: Option<String>,
    macro_key_started_ms: Option<u64>,
    macro_depth: u8,
    macro_expanded_commands: usize,
    live_window_registry: Vec<LiveWindowRegistryEntry>,
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
}

#[derive(Clone, Debug)]
struct PendingLinkNavigation {
    target: Option<Target>,
    new_window_action: Option<String>,
    journey_parent: Option<JourneyNodeId>,
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

fn finish_pending_selection_operation(this: &mut BrowserUiRust, status: &str) {
    if let Some(request) = this.pending_selection.take()
        && let Some(operation_id) = request.operation_id
    {
        this.operation_states.insert(operation_id, status.into());
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
    operation_id: String,
}

#[derive(Clone, Debug)]
struct PendingDownload {
    token: String,
    target: Target,
    url: String,
    issued: bool,
}

#[derive(Clone, Debug)]
struct SpawnCompletion {
    operation_id: String,
    status: String,
}

#[derive(Clone, Debug)]
struct UserscriptCompletion {
    operation_id: String,
    status: String,
    target: Target,
    action: Option<userscript::UserscriptAction>,
    stderr: String,
}

fn bootstrap_runtime(
    privacy: PrivacyKind,
    profile_label: &str,
) -> Option<(BrowserRuntime, WindowId, TabId)> {
    BrowserRuntime::bootstrap(privacy, profile_label).ok()
}

fn ensure_spawn_working_directory(rust: &mut BrowserUiRust) -> Result<PathBuf, String> {
    if let Some(path) = rust.spawn_working_directory.as_ref() {
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
    rust.spawn_working_directory = Some(path.clone());
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

    fn close_storage(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if let Some(state) = Option::as_mut(&mut this.state) {
            state.close_storage();
        }
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

impl Default for BrowserUiRust {
    #[allow(clippy::too_many_lines)]
    fn default() -> Self {
        let registry = CommandRegistry::default_v1();
        let bindings = BindingTrie::default_v1(registry.clone())
            .ok()
            .map(|trie| BindingResolver::new(trie, Mode::Normal));
        let core = bootstrap_runtime(PrivacyKind::Normal, "default");
        let (state, window, tab) = match core {
            Some((runtime, window, tab)) => (
                Some(BrowserApplication::from_runtime(runtime, Config::default())),
                Some(window),
                Some(tab),
            ),
            None => (None, None, None),
        };
        let theme_palette = ThemePalette::default();
        let chrome_theme = theme_presentation::project(&theme_palette);
        let chrome_preferences = chrome_preferences::project(&Config::default());
        let feature_preferences = feature_preferences::project(&Config::default());
        let settings_config = serde_json::to_value(Config::default()).unwrap_or(Value::Null);
        let settings_rows = settings_presentation::project_variant(&settings_config);
        Self {
            initial_url: QString::from("about:blank"),
            current_url: QString::from("about:blank"),
            display_url: QString::from("about:blank"),
            page_title: QString::default(),
            status_text: QString::from("Ready"),
            macro_status_text: QString::default(),
            load_state: QString::from("idle"),
            navigation_failure_kind: QString::default(),
            navigation_failure_requested_url: QString::default(),
            navigation_failure_url: QString::default(),
            navigation_failure_detail: QString::default(),
            navigation_failure_visible: false,
            mode: QString::from("normal"),
            learning_mode: false,
            view_alive: true,
            state,
            window,
            tab,
            registry,
            bindings,
            core_mode: Mode::Normal,
            binding_clock: Instant::now(),
            pending_engine_action: None,
            pending_engine_actions: VecDeque::new(),
            pending_print: None,
            pending_print_private: false,
            pending_journey_transitions: Vec::new(),
            pending_journey_parent: None,
            pending_journey_traversal: None,
            pending_journey_traversals: VecDeque::new(),
            pending_journey_mappings: BTreeMap::new(),
            pending_history_clear: None,
            pending_journey_reopen: None,
            pending_navigation_urls: BTreeMap::new(),
            pending_redirect_tabs: BTreeSet::new(),
            pending_redirect_hops: BTreeMap::new(),
            popup_journey_targets: Vec::new(),
            tab_ids: tab.into_iter().collect(),
            popup_tab_ids: Vec::new(),
            scroll_positions: BTreeMap::new(),
            active_tab_index: 0,
            tab_count: 1,
            completion_text: QString::default(),
            completion_values: QString::default(),
            completion_start: 0,
            completion_end: 0,
            completion_selected: -1,
            completion_visible: false,
            command_retryable: false,
            command_history: Vec::new(),
            command_history_index: None,
            command_history_draft: String::new(),
            queued_command_history: Vec::new(),
            command_history_write_pending: false,
            command_history_write_error: None,
            binding_overlay: QString::default(),
            binding_help_row_kinds: QStringList::default(),
            binding_help_row_titles: QStringList::default(),
            binding_help_row_modes: QStringList::default(),
            binding_help_row_commands: QStringList::default(),
            binding_help_row_descriptions: QStringList::default(),
            binding_help_row_keys: QStringList::default(),
            binding_help_row_sources: QStringList::default(),
            binding_help_row_counts: QStringList::default(),
            switcher_result_kinds: QStringList::default(),
            switcher_result_ids: QStringList::default(),
            switcher_result_generations: QStringList::default(),
            switcher_result_labels: QStringList::default(),
            switcher_result_secondaries: QStringList::default(),
            switcher_result_profiles: QStringList::default(),
            switcher_result_workspaces: QStringList::default(),
            switcher_result_actions: QStringList::default(),
            switcher_result_ranks: QStringList::default(),
            switcher_result_recencies: QStringList::default(),
            switcher_request_scope: QString::default(),
            switcher_request_query: QString::default(),
            search_text: QString::default(),
            search_backward: false,
            session_restore_values: QString::default(),
            session_preview: QString::default(),
            profile_values: QString::default(),
            profile_values_pending: false,
            userscript_names: QStringList::default(),
            userscript_enabled_values: QStringList::default(),
            userscript_page_world_values: QStringList::default(),
            userscript_action_counts: QStringList::default(),
            userscript_action_ids: QStringList::default(),
            userscript_action_labels: QStringList::default(),
            userscript_action_availability: QStringList::default(),
            external_action_ids: QStringList::default(),
            external_action_labels: QStringList::default(),
            external_action_availability: QStringList::default(),
            page_userscript_names: QStringList::default(),
            page_userscript_sources: QStringList::default(),
            page_userscript_run_at: QStringList::default(),
            page_userscript_runs_on_sub_frames: QStringList::default(),
            site_rule_javascript_set: false,
            site_rule_javascript_enabled: false,
            site_rule_images_set: false,
            site_rule_images_enabled: false,
            site_rule_force_dark_set: false,
            site_rule_force_dark_enabled: false,
            site_rule_autoplay_set: false,
            site_rule_autoplay: QString::default(),
            site_rule_zoom_set: false,
            site_rule_zoom: 0.0,
            userscript_install_state: QString::from("idle"),
            download_desktop_uri: QString::default(),
            download_request_token: QString::default(),
            download_request_url: QString::default(),
            profile_bootstrap_pending: false,
            profile_list_command_pending: false,
            pending_profile_open: None,
            pending_window_new_profile: None,
            pending_context_create: None,
            profile_preview_text: QString::default(),
            profile_preview_pending: false,
            library_kind: QString::default(),
            library_values: QString::default(),
            library_graph_edge_sources: QStringList::default(),
            library_graph_edge_targets: QStringList::default(),
            library_graph_edge_transitions: QStringList::default(),
            journey_export_preview_text: QString::default(),
            journey_export_payload: None,
            storage_library: None,
            storage_library_revision: 0,
            storage_library_dirty: Cell::new(false),
            switcher_library_index: None,
            switcher_library_index_revision: 0,
            switcher_library_index_building: false,
            switcher_library_index_cancel: None,
            switcher_library_index_result: Arc::new(Mutex::new(None)),
            switcher_cache: None,
            session_names: Vec::new(),
            session_names_dirty: Cell::new(true),
            session_restore_preview_pending: false,
            session_restore_load_append: None,
            session_restore_recovery_pending: false,
            session_save_pending: false,
            session_save_checkpoint: false,
            session_checkpoint_clear_pending: false,
            storage_flush_pending: false,
            storage_flush_error: None,
            download_destination_pending: false,
            download_create_pending: false,
            pending_permission_reset_session: None,
            link_preview_command: QString::default(),
            link_preview_original: QString::default(),
            link_preview_cleaned: QString::default(),
            link_preview_applied_rules: QStringList::default(),
            link_preview_removed_parameters: QStringList::default(),
            link_preview_retained_parameters: QStringList::default(),
            link_preview_explanation: QString::default(),
            link_preview_requires_confirmation: false,
            link_preview_visible: false,
            pending_link_navigation: None,
            external_navigation_uri: QString::default(),
            external_navigation_scheme: QString::default(),
            external_navigation_visible: false,
            pending_external_navigation: None,
            hint_values: QString::default(),
            hint_visible: false,
            hint_links_only: false,
            hint_rapid: false,
            hint_family: QString::from("all"),
            hint_unmatched_policy: QString::from("hide"),
            hint_marker_scale: 1.0,
            hint_rapid_target: "current".into(),
            hint_script: None,
            hint_first: false,
            hint_index: 1,
            hint_rapid_tabs_created: 0,
            pending_hint_action: None,
            spatial_visible: false,
            spatial_session_id: QString::default(),
            spatial_dispatch_request: QString::default(),
            spatial_labels: QString::from(
                "[\"1\",\"2\",\"3\",\"4\",\"5\",\"6\",\"7\",\"8\",\"9\"]",
            ),
            spatial_root_x: 0.0,
            spatial_root_y: 0.0,
            spatial_root_width: 0.0,
            spatial_root_height: 0.0,
            spatial_current_x: 0.0,
            spatial_current_y: 0.0,
            spatial_current_width: 0.0,
            spatial_current_height: 0.0,
            spatial_crosshair_x: 0.0,
            spatial_crosshair_y: 0.0,
            spatial_depth: 0,
            spatial_help_visible: false,
            spatial_session: None,
            spatial_ids: IdSource::new(),
            caret_selecting: false,
            caret_request_token: QString::default(),
            caret_request_operation: QString::default(),
            editor_completion_token: QString::default(),
            editor_completion_original: QString::default(),
            editor_completion_updated: QString::default(),
            editor_completion_error: QString::default(),
            editor_completion_stderr: QString::default(),
            jseval_tab_id: QString::default(),
            jseval_world: QString::default(),
            jseval_script: QString::default(),
            clipboard_request: QString::default(),
            clipboard_request_sensitive: false,
            clipboard_request_primary: false,
            pending_selection: None,
            pending_caret: None,
            pending_editor: None,
            pending_editor_write: None,
            editor_completions: Arc::new(Mutex::new(Vec::new())),
            pending_config_edit: None,
            pending_spawn: None,
            pending_action_target: None,
            pending_userscript: None,
            pending_download: None,
            staged_downloads: BTreeMap::new(),
            pending_save_page: None,
            spawn_completions: Arc::new(Mutex::new(Vec::new())),
            spawn_cancellations: Arc::new(Mutex::new(BTreeMap::new())),
            spawn_working_directory: None,
            userscript_completions: Arc::new(Mutex::new(Vec::new())),
            userscript_cancellations: Arc::new(Mutex::new(BTreeMap::new())),
            hint_session: None,
            hint_interaction: None,
            hint_consumed: BTreeSet::new(),
            hint_ids: IdSource::new(),
            config_json: QString::from(
                serde_json::to_string(&Config::default()).unwrap_or_else(|_| "{}".into()),
            ),
            settings_rows,
            runtime_setting_error: QString::default(),
            chrome_font_family: QString::from(chrome_preferences.font_family),
            chrome_font_size_pt: chrome_preferences.font_size_pt,
            chrome_statusbar_mode: QString::from(chrome_preferences.statusbar_mode),
            chrome_tabs_mode: QString::from(chrome_preferences.tabs_mode),
            chrome_tab_position: QString::from(chrome_preferences.tab_position),
            chrome_reduced_motion: QString::from(chrome_preferences.reduced_motion),
            feature_switcher_max_results: feature_preferences.switcher_max_results,
            feature_downloads_ask_destination: feature_preferences.downloads_ask_destination,
            feature_desktop_notifications_enabled: feature_preferences
                .desktop_notifications_enabled,
            feature_desktop_media_keys_enabled: feature_preferences.desktop_media_keys_enabled,
            feature_push_service_enabled: feature_preferences.push_service_enabled,
            feature_spellcheck_enabled: feature_preferences.spellcheck_enabled,
            feature_spellcheck_languages: feature_preferences
                .spellcheck_languages
                .into_iter()
                .map(QString::from)
                .collect(),
            feature_blocking_list_ids: feature_preferences
                .blocking_list_ids
                .into_iter()
                .map(QString::from)
                .collect(),
            feature_blocking_update_interval_hours: feature_preferences
                .blocking_update_interval_hours,
            feature_link_cleaning_update_source: QString::from(
                feature_preferences.link_cleaning_update_source,
            ),
            feature_link_cleaning_update_sha256: QString::from(
                feature_preferences.link_cleaning_update_sha256,
            ),
            config_base_json: QString::from(
                serde_json::to_string(&Config::default()).unwrap_or_else(|_| "{}".into()),
            ),
            cli_overrides_json: QString::from("{}"),
            profile_overrides_json: QString::from("{}"),
            config_path: QString::default(),
            config_source: QString::from("built-in"),
            contexts_json: QString::from(
                serde_json::to_string(&ContextsConfig::default()).unwrap_or_else(|_| "{}".into()),
            ),
            context_choice_names: QStringList::default(),
            context_choice_labels: QStringList::default(),
            context_choice_profiles: QStringList::default(),
            context_name: QString::default(),
            context_label: QString::default(),
            context_workspace: QString::default(),
            context_accent: QString::default(),
            context_entry_force_reuse: false,
            context_route_id: QString::default(),
            context_route_behavior: QString::default(),
            context_route_context: QString::default(),
            context_route_profile: QString::default(),
            context_route_url: QString::default(),
            window_token: QString::from(Uuid::new_v4().to_string()),
            core_window_id: QString::default(),
            hyprland_status: QString::from("disabled"),
            hyprland_clients_json: QString::from("[]"),
            desktop_portal_mode: QString::from("auto"),
            system_reduced_motion_status: QString::from("pending"),
            system_reduced_motion_enabled: false,
            system_font_scale_status: QString::from("pending"),
            system_font_scale: 1.0,
            theme_background_color: QString::from(chrome_theme.background),
            theme_surface_color: QString::from(chrome_theme.surface),
            theme_panel_color: QString::from(chrome_theme.panel),
            theme_primary_text_color: QString::from(chrome_theme.primary_text),
            theme_secondary_text_color: QString::from(chrome_theme.secondary_text),
            theme_muted_text_color: QString::from(chrome_theme.muted_text),
            theme_border_color: QString::from(chrome_theme.border),
            theme_accent_color: QString::from(chrome_theme.accent),
            theme_warning_color: QString::from(chrome_theme.warning),
            theme_error_color: QString::from(chrome_theme.error),
            theme_success_color: QString::from(chrome_theme.success),
            theme_private_color: QString::from(chrome_theme.private),
            theme_mode_insert_color: QString::from(chrome_theme.mode_insert),
            theme_selection_color: QString::from(chrome_theme.selection),
            theme_selection_text_color: QString::from(chrome_theme.selection_text),
            theme_contrast_status: QString::from(chrome_theme.contrast_status),
            theme_contrast_reason: QString::from(chrome_theme.contrast_reason),
            site_status: QString::from("{}"),
            site_experiment_active: false,
            site_experiment_id: QString::default(),
            site_experiment_kind: QString::default(),
            site_experiment_url: QString::default(),
            site_experiment_remaining_seconds: 0,
            blocking_hosts: QStringList::default(),
            blocking_exceptions: QStringList::default(),
            blocking_rule_hosts: QStringList::default(),
            blocking_rule_list_ids: QStringList::default(),
            blocking_exception_rule_hosts: QStringList::default(),
            blocking_exception_rule_list_ids: QStringList::default(),
            blocking_loaded_lists: QString::from("[]"),
            blocking_list_metadata: QString::from("[]"),
            blocking_skipped_lists: QString::from("[]"),
            blocking_bypass_sites: QStringList::default(),
            blocking_cosmetic_rule_hosts: QStringList::default(),
            blocking_cosmetic_rule_selectors: QStringList::default(),
            blocking_cosmetic_exception_hosts: QStringList::default(),
            blocking_cosmetic_exception_selectors: QStringList::default(),
            blocking_adblock_source_ids: QString::from("[]"),
            blocking_adblock_handle: 0,
            blocking_security_deny_hosts: QStringList::default(),
            blocking_enabled: false,
            blocking_blocked_count: 0,
            blocking_active_site_count: 0,
            blocking_unknown_context_count: 0,
            blocking_active_evidence: blocking_evidence::BlockingEvidence::default(),
            focus_observations: BTreeMap::new(),
            focus_suppressions: BTreeMap::new(),
            config: serde_json::to_value(Config::default()).unwrap_or(Value::Null),
            portal_capabilities: PortalCapabilities::pending(),
            theme_palette,
            base_config: serde_json::to_value(Config::default()).unwrap_or(Value::Null),
            pending_config: None,
            profile_overrides: RuntimeOverrides::default(),
            runtime_overrides: RuntimeOverrides::default(),
            temporary_overrides: RuntimeOverrides::default(),
            cli_overrides: RuntimeOverrides::default(),
            config_watch: ConfigWatch::default(),
            config_reload_worker: None,
            config_write_worker: None,
            print_worker: None,
            editor_write_worker: None,
            profile_delete_worker: None,
            network_policy_worker: None,
            hyprland_worker: None,
            portal_probe_worker: None,
            reduced_motion_probe_worker: None,
            font_scale_probe_worker: None,
            network_policy_reload_pending: false,
            pending_profile_configuration: None,
            profile_list_worker: None,
            profile_preview_worker: None,
            userscript_manager_worker: None,
            theme_watch: ConfigWatch::default(),
            ipc_sequence: 3,
            ipc_shutdown_gate: false,
            contexts: None,
            pending_context_route: None,
            profile_name: "default".into(),
            profile_id: None,
            journey_durable_ids: RefCell::new(BTreeMap::new()),
            profile_persistence: ProfilePersistence::Unavailable,
            session_permissions: BTreeMap::new(),
            private_history: Vec::new(),
            private_history_next_id: -1,
            storage_roots: None,
            profile_registry_roots: None,
            userscript_roots: None,
            session_id: Uuid::new_v4(),
            session_path: None,
            checkpoint: SessionCheckpoint::default(),
            session_state_root: None,
            closed_tabs: Vec::new(),
            operation_states: BTreeMap::new(),
            operation_stderr: BTreeMap::new(),
            action_audit: Vec::new(),
            structured_log: None,
            diagnostics_preview_ready: false,
            diagnostics_preview_payload: None,
            request_resolution_counts: BTreeMap::new(),
            active_site_experiment: None,
            last_site_doctor_result: None,
            last_repeatable: None,
            macro_registers: BTreeMap::new(),
            recording_macro: None,
            macro_key_prefix: None,
            macro_key_started_ms: None,
            macro_depth: 0,
            macro_expanded_commands: 0,
            live_window_registry: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
