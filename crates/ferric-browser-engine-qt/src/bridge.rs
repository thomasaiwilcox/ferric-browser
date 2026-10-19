//! Generated Qt interface declarations, isolated from application implementation.
#![allow(unsafe_code, clippy::too_many_arguments)]
use crate::BrowserUiRust;

#[cxx_qt::bridge]
pub(crate) mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        include!("cxx-qt-lib/core/qbytearray.h");
        include!("cxx-qt-lib/qstringlist.h");
        include!("cxx-qt-lib/qvariant.h");
        include!("cxx-qt-lib/qurl.h");
        include!("cxx-qt-lib/qguiapplication.h");
        include!("cxx-qt-lib/core/qvector/qvector_QByteArray.h");
        include!("gui_application.h");
        include!("url_display.h");
        type QString = cxx_qt_lib::QString;
        type QByteArray = cxx_qt_lib::QByteArray;
        type QStringList = cxx_qt_lib::QStringList;
        type QVariant = cxx_qt_lib::QVariant;
        type QUrl = cxx_qt_lib::QUrl;
        type QGuiApplication = cxx_qt_lib::QGuiApplication;
        type QVector_QByteArray = cxx_qt_lib::QVector<cxx_qt_lib::QByteArray>;

        fn ferric_browser_gui_application(
            arguments: &QVector_QByteArray,
        ) -> UniquePtr<QGuiApplication>;

        fn ferric_browser_to_ascii_host(host: &QString) -> QString;
        fn ferric_browser_canonicalize_url(value: &QString) -> QString;
        fn ferric_browser_local_file_path(value: &QUrl) -> QString;
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
        #[qproperty(i32, input_epoch)]
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
        #[qproperty(QString, start_page_owner_token)]
        #[qproperty(QStringList, start_page_eligible_sites)]
        #[qproperty(QStringList, start_page_eligible_urls)]
        #[qproperty(u64, content_history_revision)]
        #[qproperty(QString, start_page_view)]
        #[qproperty(QString, start_page_error)]
        #[qproperty(bool, start_page_busy)]
        #[qproperty(u64, start_page_revision)]
        #[qproperty(u64, start_page_rank_revision)]
        #[qproperty(u64, start_page_profile_revision)]
        #[qproperty(bool, start_page_ready)]
        #[qproperty(QStringList, start_page_kinds)]
        #[qproperty(QStringList, start_page_ids)]
        #[qproperty(QStringList, start_page_urls)]
        #[qproperty(QStringList, start_page_titles)]
        #[qproperty(QStringList, start_page_sites)]
        #[qproperty(QStringList, start_page_slots)]
        #[qproperty(QStringList, guide_categories)]
        #[qproperty(QStringList, guide_titles)]
        #[qproperty(QStringList, guide_keys)]
        #[qproperty(QStringList, guide_modes)]
        #[qproperty(QStringList, guide_commands)]
        #[qproperty(QStringList, guide_summaries)]
        #[qproperty(QStringList, guide_ids)]
        #[qproperty(QStringList, guide_explanations)]
        #[qproperty(QStringList, guide_arguments)]
        #[qproperty(QStringList, guide_examples)]
        #[qproperty(QStringList, guide_recoveries)]
        #[qproperty(QStringList, guide_counts)]
        #[qproperty(QStringList, guide_binding_notes)]
        #[qproperty(QStringList, guide_details)]
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
        #[qproperty(bool, switcher_sources_pending)]
        #[qproperty(bool, switcher_sources_failed)]
        #[qproperty(bool, switcher_sources_retryable)]
        #[qproperty(QString, switcher_sources_notice)]
        #[qproperty(QString, switcher_sources_detail)]
        #[qproperty(QString, switcher_request_scope)]
        #[qproperty(QString, switcher_request_query)]
        #[qproperty(QString, search_text)]
        #[qproperty(bool, search_backward)]
        #[qproperty(QString, session_restore_values)]
        #[qproperty(QStringList, session_preview_labels)]
        #[qproperty(QStringList, session_preview_urls)]
        #[qproperty(QStringList, session_preview_pinned)]
        #[qproperty(QStringList, session_preview_muted)]
        #[qproperty(QString, session_preview)]
        #[qproperty(QString, bookmark_transfer_id)]
        #[qproperty(QString, bookmark_transfer_state)]
        #[qproperty(QString, bookmark_transfer_message)]
        #[qproperty(QString, bookmark_transfer_detail)]
        #[qproperty(QString, mark_action_id)]
        #[qproperty(QString, mark_action_state)]
        #[qproperty(QString, mark_action_message)]
        #[qproperty(QString, mark_action_detail)]
        #[qproperty(bool, library_pending)]
        #[qproperty(QStringList, diagnostics_summary)]
        #[qproperty(bool, diagnostics_available)]
        #[qproperty(bool, diagnostics_pending)]
        #[qproperty(QString, diagnostics_preview)]
        #[qproperty(QString, diagnostics_error)]
        #[qproperty(QString, profile_startup_notice)]
        #[qproperty(QString, library_error)]
        #[qproperty(QString, profile_action_id)]
        #[qproperty(QString, profile_action_state)]
        #[qproperty(QString, profile_action_message)]
        #[qproperty(QString, profile_action_detail)]
        #[qproperty(QString, session_save_id)]
        #[qproperty(QString, session_save_state)]
        #[qproperty(QString, session_save_message)]
        #[qproperty(QString, session_save_detail)]
        #[qproperty(QString, session_delete_id)]
        #[qproperty(QString, session_delete_state)]
        #[qproperty(QString, session_delete_message)]
        #[qproperty(QString, session_delete_detail)]
        #[qproperty(QString, profile_list_error)]
        #[qproperty(QString, profile_preview_error)]
        #[qproperty(QStringList, session_names_values)]
        #[qproperty(bool, session_names_pending)]
        #[qproperty(QString, session_names_error)]
        #[qproperty(QString, profile_values)]
        #[qproperty(bool, profile_values_pending)]
        #[qproperty(bool, userscript_inventory_pending)]
        #[qproperty(bool, userscript_inventory_ready)]
        #[qproperty(QString, userscript_inventory_error)]
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
        #[qproperty(bool, target_search_active)]
        #[qproperty(QString, target_search_scope)]
        #[qproperty(QString, target_search_query)]
        #[qproperty(bool, hint_pointer_mode)]
        #[qproperty(bool, hint_pointer_pending)]
        #[qproperty(QString, hint_pointer_state)]
        #[qproperty(QString, hint_pointer_dispatch_request)]
        #[qproperty(bool, hint_links_only)]
        #[qproperty(bool, hint_rapid)]
        #[qproperty(QString, hint_target)]
        #[qproperty(QString, hint_family)]
        #[qproperty(bool, hint_proxy_targets_enabled)]
        #[qproperty(QString, hint_unmatched_policy)]
        #[qproperty(bool, hint_chrome_available)]
        #[qproperty(f64, hint_marker_scale)]
        #[qproperty(bool, spatial_visible)]
        #[qproperty(bool, spatial_selecting)]
        #[qproperty(QString, spatial_session_id)]
        #[qproperty(QString, spatial_scope_lease_id)]
        #[qproperty(QString, spatial_dispatch_request)]
        #[qproperty(QStringList, spatial_labels)]
        #[qproperty(f64, spatial_viewport_x)]
        #[qproperty(f64, spatial_viewport_y)]
        #[qproperty(f64, spatial_viewport_width)]
        #[qproperty(f64, spatial_viewport_height)]
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
        #[qproperty(QString, clipboard_request)]
        #[qproperty(bool, clipboard_request_sensitive)]
        #[qproperty(bool, clipboard_request_primary)]
        #[qproperty(QString, config_json)]
        #[qproperty(QString, selector_validation_request)]
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
        #[qproperty(bool, blocking_policy_ready)]
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
        fn register_ipc_owner(self: Pin<&mut BrowserUi>) -> bool;
        #[qinvokable]
        fn refresh_ipc_owner(self: Pin<&mut BrowserUi>);
        #[qinvokable]
        fn set_ipc_window_active(self: Pin<&mut BrowserUi>, active: bool);
        #[qinvokable]
        fn release_ipc_owner(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn maintenance_delay_ms(self: Pin<&mut BrowserUi>) -> i32;

        #[qinvokable]
        fn refresh_start_page(self: Pin<&mut BrowserUi>) -> bool;
        #[qinvokable]
        fn refresh_start_page_metadata(self: Pin<&mut BrowserUi>);
        #[qinvokable]
        fn set_private_start_owner(self: Pin<&mut BrowserUi>, token: &QString);
        #[qinvokable]
        fn start_page_action(
            self: Pin<&mut BrowserUi>,
            action: &QString,
            slot: i32,
            value: &QString,
            title: &QString,
        ) -> bool;
        #[qinvokable]
        fn refresh_guide(self: Pin<&mut BrowserUi>, search: &QString);
        #[qinvokable]
        fn content_navigated(
            self: Pin<&mut BrowserUi>,
            index: i32,
            url: &QString,
            engine_index: i32,
            traversal: bool,
        );
        #[qinvokable]
        fn content_engine_traversed(
            self: Pin<&mut BrowserUi>,
            index: i32,
            url: &QString,
            engine_index: i32,
        ) -> bool;
        #[qinvokable]
        fn content_history_available(self: Pin<&mut BrowserUi>, index: i32, offset: i32) -> bool;
        #[qinvokable]
        fn content_history_export(self: Pin<&mut BrowserUi>, index: i32) -> QString;
        #[qinvokable]
        fn content_history_import(
            self: Pin<&mut BrowserUi>,
            index: i32,
            snapshot: &QString,
        ) -> bool;
        #[qinvokable]
        fn content_traverse(
            self: Pin<&mut BrowserUi>,
            index: i32,
            offset: i32,
            engine_index: i32,
        ) -> QString;
        #[qinvokable]
        fn start_page_icon_path(self: Pin<&mut BrowserUi>, url: &QString, create: bool) -> QString;
        #[qinvokable]
        fn start_page_key_available(self: Pin<&mut BrowserUi>) -> bool;
        #[qinvokable]
        fn start_page_pick_bookmark(self: Pin<&mut BrowserUi>, id: &QString) -> bool;
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
        fn complete_selector_validation(
            self: Pin<&mut BrowserUi>,
            generation: &QString,
            result: &QString,
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
        fn finish_command_entry(self: Pin<&mut BrowserUi>);

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
        fn tab_pinned_at(&self, index: i32) -> bool;
        #[qinvokable]
        fn tab_muted_at(&self, index: i32) -> bool;

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
        fn refresh_library(self: Pin<&mut BrowserUi>);

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
        fn take_session_preview_request(self: Pin<&mut BrowserUi>) -> QStringList;

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
        fn cancel_target_search(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn complete_hint_scope_handoff(
            self: Pin<&mut BrowserUi>,
            lease_id: &QString,
            receipt_json: &QString,
        ) -> bool;

        #[qinvokable]
        fn complete_grid_scope_handoff(
            self: Pin<&mut BrowserUi>,
            lease_id: &QString,
            receipt_json: &QString,
            geometry_json: &QString,
            serial: &QString,
            revision: &QString,
            view_width: f64,
            view_height: f64,
            zoom_factor: f64,
        ) -> bool;

        #[qinvokable]
        fn hint_selector_policy(self: Pin<&mut BrowserUi>, family: &QString) -> QString;

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
        fn hint_pointer_confirmation_pressed(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn hint_pointer_confirmation_released(
            self: Pin<&mut BrowserUi>,
            surface_serial: &QString,
            surface_revision: &QString,
            view_width: f64,
            view_height: f64,
        ) -> QString;

        #[qinvokable]
        fn complete_hint_pointer_preflight(
            self: Pin<&mut BrowserUi>,
            request_id: &QString,
            preflight_json: &QString,
            surface_serial: &QString,
            surface_revision: &QString,
            view_width: f64,
            view_height: f64,
        ) -> bool;

        #[qinvokable]
        fn hint_pointer_dispatch_ack(
            self: Pin<&mut BrowserUi>,
            request_id: &QString,
            session_id: &QString,
            surface_serial: &QString,
            surface_revision: &QString,
            outcome: &QString,
        );

        #[qinvokable]
        fn hint_pointer_invalidated(self: Pin<&mut BrowserUi>, reason: &QString);

        #[qinvokable]
        fn hint_pointer_timeout(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn cancel_hint_palette(self: Pin<&mut BrowserUi>);

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
        fn request_profile_window(self: Pin<&mut BrowserUi>, name: &QString) -> bool;

        #[qinvokable]
        fn delete_profile(self: Pin<&mut BrowserUi>, name: &QString, confirmed: bool) -> bool;

        #[qinvokable]
        fn default_download_directory(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn allocate_download_id(self: &BrowserUi) -> QString;

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
        #[qsignal]
        fn stream_download_ready(self: Pin<&mut BrowserUi>, id: QString);
        #[qinvokable]
        fn canonical_download_origin(self: &BrowserUi, url: &QString) -> QString;
        #[qinvokable]
        fn set_download_controller_available(self: Pin<&mut BrowserUi>, available: bool);
        #[qinvokable]
        fn admit_download_stream_target(
            self: Pin<&mut BrowserUi>,
            token: &QString,
            url: &QString,
        ) -> bool;
        #[qinvokable]
        fn download_stream_name(
            self: &BrowserUi,
            url: &QString,
            disposition: &QString,
            mime: &QString,
        ) -> QString;
        #[qinvokable]
        fn reserve_stream_download(self: Pin<&mut BrowserUi>, id: &QString) -> bool;
        #[qinvokable]
        fn begin_stream_download(self: Pin<&mut BrowserUi>, id: &QString) -> bool;
        #[qinvokable]
        fn write_stream_download(
            self: Pin<&mut BrowserUi>,
            id: &QString,
            bytes: &QByteArray,
        ) -> bool;
        #[qinvokable]
        fn finish_stream_download(self: Pin<&mut BrowserUi>, id: &QString) -> bool;
        #[qinvokable]
        fn cancel_stream_download(self: Pin<&mut BrowserUi>, id: &QString);
        #[qinvokable]
        fn release_stream_download(self: Pin<&mut BrowserUi>, id: &QString);
        #[qinvokable]
        fn stream_download_state(self: Pin<&mut BrowserUi>, id: &QString) -> i32;
        #[qinvokable]
        fn stream_download_bytes(self: &BrowserUi, id: &QString) -> i64;
        #[qinvokable]
        fn stream_download_error(self: &BrowserUi, id: &QString) -> QString;

        #[qinvokable]
        fn download_finalization_pending(self: &BrowserUi, id: &QString) -> bool;
        #[qinvokable]
        fn has_finalizing_downloads(self: &BrowserUi) -> bool;
        #[qinvokable]
        fn cancel_download_finalization(self: Pin<&mut BrowserUi>, id: &QString) -> bool;
        #[qinvokable]
        fn poll_download_finalizations(self: Pin<&mut BrowserUi>);
        #[qinvokable]
        fn record_download_metrics(
            self: Pin<&mut BrowserUi>,
            id: &QString,
            total: i64,
            code: i32,
            reason: &QString,
        );

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
        fn download_rows(self: Pin<&mut BrowserUi>) -> QVariant;
        #[qinvokable]
        fn download_history_status(self: &BrowserUi) -> QString;
        #[qinvokable]
        fn refresh_download_history(self: Pin<&mut BrowserUi>);

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
        fn download_request_source_index(self: &BrowserUi, token: &QString) -> i32;

        #[qinvokable]
        fn accept_download_request(
            self: Pin<&mut BrowserUi>,
            token: &QString,
            id: &QString,
        ) -> bool;

        #[qinvokable]
        fn discard_download_request(self: Pin<&mut BrowserUi>, token: &QString) -> bool;

        #[qinvokable]
        fn fail_download_request(self: Pin<&mut BrowserUi>, token: &QString) -> bool;

        #[qinvokable]
        fn cancel_download_request(self: Pin<&mut BrowserUi>, token: &QString) -> bool;

        #[qinvokable]
        fn switcher_query(self: Pin<&mut BrowserUi>, query: &QString, scope: &QString) -> bool;

        #[qinvokable]
        fn refresh_switcher_sources(self: Pin<&mut BrowserUi>, scope: &QString) -> bool;

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

        // Called by the native QObject bridge after type/thread/alias checks.
        fn handoff_popup_owner(
            self: Pin<&mut BrowserUi>,
            destination: Pin<&mut BrowserUi>,
            token: &QString,
        ) -> bool;

        fn initialize_window_owner(
            self: Pin<&mut BrowserUi>,
            destination: Pin<&mut BrowserUi>,
            profile_name: &QString,
        ) -> bool;

        // Read-only comparison invoked through the checked native owner bridge.
        fn shares_permission_session(self: &BrowserUi, other: &BrowserUi) -> bool;

        #[qinvokable]
        fn assign_window_context(self: Pin<&mut BrowserUi>, name: &QString) -> bool;

        #[qinvokable]
        fn profile_available(self: &BrowserUi) -> bool;

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
        fn prompt_origin(self: &BrowserUi, url: &QString) -> QString;

        #[qinvokable]
        fn local_file_path(self: &BrowserUi, url: &QUrl) -> QString;

        #[qinvokable]
        fn file_dialog_default_url(self: &BrowserUi, name: &QString, folder: &QUrl) -> QUrl;

        #[qinvokable]
        fn prompt_host(self: &BrowserUi, url: &QString) -> QString;

        #[qinvokable]
        fn client_certificate_origin(self: &BrowserUi, authority: &QString) -> QString;

        #[qinvokable]
        fn reset_permission(
            self: Pin<&mut BrowserUi>,
            origin: &QString,
            permission: &QString,
        ) -> bool;

        #[qinvokable]
        fn refresh_site_status(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn refresh_interaction_status(self: Pin<&mut BrowserUi>) -> QString;

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
        fn profile_startup_details(self: &BrowserUi) -> QString;

        #[qinvokable]
        fn request_diagnostics(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn cancel_diagnostics(self: Pin<&mut BrowserUi>);

        #[qinvokable]
        fn engine_update_notice(self: &BrowserUi) -> QStringList;

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
            epoch: i32,
            context: i32,
            editable: bool,
            user_activated: bool,
        );

        #[qinvokable]
        fn new_prompt_view_identity(self: Pin<&mut BrowserUi>) -> QString;

        #[qinvokable]
        fn register_prompt_view_identity(
            self: Pin<&mut BrowserUi>,
            index: i32,
            view_identity: &QString,
        );

        #[qinvokable]
        fn unregister_prompt_view_identity(
            self: Pin<&mut BrowserUi>,
            index: i32,
            view_identity: &QString,
        );

        #[qinvokable]
        fn begin_native_prompt_lease(
            self: Pin<&mut BrowserUi>,
            index: i32,
            category: &QString,
        ) -> QString;

        #[qinvokable]
        fn complete_native_prompt_lease(self: Pin<&mut BrowserUi>, lease_id: &QString) -> bool;

        #[qinvokable]
        fn finish_native_prompt_focus_probe(self: Pin<&mut BrowserUi>, lease_id: &QString) -> bool;

        #[qinvokable]
        fn has_native_prompt_lease(self: Pin<&mut BrowserUi>) -> bool;

        #[qinvokable]
        fn invalidate_native_prompt_leases(self: Pin<&mut BrowserUi>);

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
        fn page_document_identity_for(
            self: Pin<&mut BrowserUi>,
            index: i32,
            tab_id: &QString,
            document_id: &QString,
            epoch: i32,
        );

        #[qinvokable]
        fn begin_same_document_url_change_for(
            self: Pin<&mut BrowserUi>,
            index: i32,
            tab_id: &QString,
            url: QString,
            document_id: &QString,
            epoch: i32,
        );

        #[qinvokable]
        fn complete_same_document_url_change_for(
            self: Pin<&mut BrowserUi>,
            index: i32,
            tab_id: &QString,
            url: &QString,
            document_id: &QString,
            epoch: i32,
        );

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
        fn mixed_key_route(self: Pin<&mut BrowserUi>, key: &QString) -> QString;

        #[qinvokable]
        fn clipboard_shortcut_browser_owned(self: Pin<&mut BrowserUi>, key: &QString) -> bool;

        #[qinvokable]
        fn clipboard_edit_target_current(
            self: Pin<&mut BrowserUi>,
            tab: &QString,
            generation: &QString,
            document: &QString,
            popup_token: &QString,
        ) -> bool;

        #[qinvokable]
        fn popup_clipboard_edit(
            self: Pin<&mut BrowserUi>,
            token: &QString,
            operation: &QString,
        ) -> QVariant;

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
        fn spatial_scope_geometry_validated(
            self: Pin<&mut BrowserUi>,
            request_id: &QString,
            geometry_json: &QString,
            serial: &QString,
            revision: &QString,
            view_width: f64,
            view_height: f64,
            zoom_factor: f64,
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
        fn take_engine_request(self: Pin<&mut BrowserUi>) -> QVariant;

        #[qinvokable]
        fn request_target_current(
            &self,
            tab: &QString,
            generation: &QString,
            document: &QString,
        ) -> bool;

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
