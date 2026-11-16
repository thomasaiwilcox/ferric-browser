use super::{
    Arc, BrowserApplication, Config, ConfigReloadWorker, ConfigWatch, ConfigWriteWorker,
    ContextSnapshot, ContextsConfig, CxxQtType, EditorWriteWorker, LoggingConfig, Mode, PathBuf,
    PendingProfileConfiguration, Pin, PrintWorker, PrivacyKind, ProfileDeleteWorker,
    ProfileListWorker, ProfileOpenEffect, ProfileOpenRequest, ProfilePersistence,
    ProfilePreviewWorker, ProfileStorage, QString, QStringList, RootSpec, RuntimeOverrides,
    SessionCheckpoint, StorageRoots, Uuid, Value, atomic_write_private, bootstrap_runtime,
    bounded_header, cleanup_editor_artifact, cleanup_print_artifact, cleanup_staged_downloads,
    configured_bindings, desktop_portals, discard_config_edit, discard_editor_request,
    finish_pending_selection_operation, hyprland, link_cleaning_policy, logging, network_policy,
    profile_startup::FailureStage, qobject, resolve_browser_roots,
};
use crate::OperationUpdate;

impl qobject::BrowserUi {
    pub(super) fn profile_available(&self) -> bool {
        self.rust()
            .state
            .as_ref()
            .and_then(BrowserApplication::profile_snapshot)
            .is_some()
    }

    pub(super) fn profile_configuration_request_is_current(&self, request: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.rust().configuration_io.profile_request, request)
    }

    /// Retire only work that has not entered the configuration writer/context
    /// coordinator. A new intent also retires a callback already taken locally
    /// which can reenter here while notifying QML.
    pub(super) fn begin_profile_configuration_request(mut self: Pin<&mut Self>) -> Option<Arc<()>> {
        if !self.as_ref().rust().view_alive || self.as_ref().rust().ipc_shutdown_gate {
            return None;
        }
        let retired = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(generation) = this.configuration_io.selector_generation.checked_add(1) else {
                drop(rust);
                self.as_mut().set_status_text(QString::from(
                    "configuration request generation is exhausted",
                ));
                return None;
            };
            let request = Arc::new(());
            this.configuration_io.selector_generation = generation;
            this.configuration_io.selector_origin_generation = generation;
            this.configuration_io.profile_request = Arc::clone(&request);
            let pending = this.configuration_io.pending_selector_validation.take();
            if let Some(id) = pending
                .as_ref()
                .and_then(|pending| pending.operation_id.as_ref())
                && this.operation(id).is_some_and(|operation| {
                    !operation.state.is_terminal() && operation.cancellable
                })
            {
                this.set_operation(id.clone(), OperationUpdate::cancelled());
            }
            (request, pending, this.pending_profile_configuration.take())
        };
        let (request, selector, profile) = retired;
        drop(profile);
        // Set readiness before any notification can admit unrelated validation.
        if let Some(state) = self.as_mut().rust_mut().as_mut().get_mut().state.as_mut() {
            state.set_policy_owner_preparing(true);
        }
        self.as_mut().set_profile_bootstrap_pending(true);
        if self
            .as_ref()
            .profile_configuration_request_is_current(&request)
        {
            self.as_mut()
                .set_selector_validation_request(QString::default());
        }
        // Always resolve the retired handoff even if clearing the property
        // admitted a newer request. Feedback itself is ID scoped.
        let feedback = selector
            .and_then(super::browser_ui_configuration_preparation::finish_replaced_preparation);
        self.as_mut().finish_setting_feedback(
            feedback,
            Err("The profile changed before settings validation completed."),
        );
        self.as_ref()
            .profile_configuration_request_is_current(&request)
            .then_some(request)
    }

    pub(super) fn clear_policy_owner_preparation(mut self: Pin<&mut Self>) {
        if let Some(state) = self.as_mut().rust_mut().as_mut().get_mut().state.as_mut() {
            state.set_policy_owner_preparing(false);
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn configure_profile(
        mut self: Pin<&mut Self>,
        private_profile: bool,
        ephemeral_profile: bool,
        profile_label: &QString,
        profile_name: &QString,
        storage_base: &QString,
    ) {
        self.as_mut().ensure_owner_runtime_wake();
        let Some(request) = self.as_mut().begin_profile_configuration_request() else {
            return;
        };
        macro_rules! publish {
            ($expression:expr) => {{
                if !self
                    .as_ref()
                    .profile_configuration_request_is_current(&request)
                {
                    return;
                }
                $expression;
                if !self
                    .as_ref()
                    .profile_configuration_request_is_current(&request)
                {
                    return;
                }
            }};
        }
        let context_catalog_revision = super::live_ipc_owners::context_catalog_revision();
        publish!(
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .profile_startup
                .begin()
        );
        publish!(self.as_mut().sync_profile_startup_notice());
        publish!(self.as_mut().cancel_diagnostics());
        publish!(
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .diagnostic_work
                .diagnostics_collector
                .clear()
        );
        let profile_label = profile_label.to_string();
        let profile_name = profile_name.to_string();
        let storage_base = storage_base.to_string();
        let config_path = self.as_ref().rust().config_path.to_string();
        let contexts_json = self.as_ref().rust().contexts_json.to_string();
        let privacy = if ephemeral_profile {
            PrivacyKind::Ephemeral
        } else if private_profile {
            PrivacyKind::Private
        } else {
            PrivacyKind::Normal
        };
        let Some((runtime, window, tab)) = bootstrap_runtime(privacy, &profile_label) else {
            publish!(self.as_mut().clear_policy_owner_preparation());
            publish!(self.as_mut().set_profile_bootstrap_pending(false));
            publish!(
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .profile_startup
                    .fail(
                        FailureStage::Runtime,
                        "The initial browser runtime could not be created",
                    )
            );
            publish!(self.as_mut().sync_profile_startup_notice());
            publish!(self.as_mut().set_status_text(QString::from(
                "The profile couldn't be opened. Open :diagnostics for details.",
            )));
            return;
        };
        publish!(self.as_mut().set_profile_bootstrap_pending(true));
        publish!(self.as_mut().refresh_ipc_owner());
        let startup_config =
            serde_json::from_str::<Config>(&self.as_ref().rust().config_json.to_string())
                .unwrap_or_default();
        let base_configuration =
            serde_json::from_str::<Config>(&self.as_ref().rust().config_base_json.to_string())
                .unwrap_or(startup_config);
        let command_line_overrides = serde_json::from_str::<RuntimeOverrides>(
            &self.as_ref().rust().cli_overrides_json.to_string(),
        )
        .unwrap_or_default();
        let initial_profile_overrides = serde_json::from_str::<RuntimeOverrides>(
            &self.as_ref().rust().profile_overrides_json.to_string(),
        )
        .unwrap_or_default();
        let (contexts, context_configuration_error) =
            match serde_json::from_str::<ContextsConfig>(&contexts_json) {
                Ok(contexts) => (contexts, None),
                Err(error) => (ContextsConfig::default(), Some(error.to_string())),
            };
        let root_spec = if storage_base.is_empty() {
            RootSpec::Xdg
        } else {
            RootSpec::Base(PathBuf::from(&storage_base))
        };
        let script_catalog_root = StorageRoots::resolve(root_spec.clone())
            .ok()
            .map(|roots| roots.config);
        let storage = if private_profile || ephemeral_profile {
            ProfileStorage::Transient
        } else {
            ProfileStorage::Durable(root_spec)
        };
        let mut state = BrowserApplication::from_runtime(runtime, Config::default());
        state.set_policy_owner_preparing(true);
        if let Some(wake) = self.as_ref().rust().runtime_waker.as_ref() {
            state.set_storage_waker(Arc::clone(wake));
        }
        let request_result = state
            .request_profile_open_for_validation(ProfileOpenRequest {
                name: profile_name.clone(),
                label: profile_label,
                privacy,
                storage,
                base_configuration,
                base_authored: self.as_ref().rust().base_authored_input.clone(),
                initial_profile_overrides,
                command_line_overrides,
                config_path: (!config_path.is_empty()).then(|| PathBuf::from(config_path)),
                script_catalog_root,
                contexts,
                context_configuration_error,
            })
            .map_err(|error| error.to_string());
        if let Err(error) = request_result {
            publish!(self.as_mut().clear_policy_owner_preparation());
            publish!(self.as_mut().set_profile_bootstrap_pending(false));
            publish!(
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .profile_startup
                    .fail(FailureStage::ProfileOpen, &error)
            );
            publish!(self.as_mut().sync_profile_startup_notice());
            publish!(self.as_mut().set_status_text(QString::from(
                "The profile couldn't be opened. Open :diagnostics for details.",
            )));
            return;
        }
        publish!(
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_profile_configuration = Some(PendingProfileConfiguration {
                request: Arc::clone(&request),
                selector_receipt: None,
                selector_fingerprint: None,
                context_catalog_revision,
                private_profile,
                profile_name,
                storage_base,
                state,
                window,
                tab,
            })
        );
        publish!(
            self.as_mut()
                .set_status_text(QString::from("Opening profile…"))
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn finish_configure_profile(
        mut self: Pin<&mut Self>,
        pending: PendingProfileConfiguration,
        setup: Result<ProfileOpenEffect, String>,
    ) {
        let PendingProfileConfiguration {
            request,
            selector_receipt,
            selector_fingerprint,
            context_catalog_revision,
            private_profile,
            profile_name,
            storage_base,
            mut state,
            window,
            tab,
        } = pending;
        macro_rules! publish {
            ($expression:expr) => {{
                if !self
                    .as_ref()
                    .profile_configuration_request_is_current(&request)
                {
                    return;
                }
                $expression;
                if !self
                    .as_ref()
                    .profile_configuration_request_is_current(&request)
                {
                    return;
                }
            }};
        }
        if !self
            .as_ref()
            .profile_configuration_request_is_current(&request)
        {
            return;
        }
        let policy_roots = resolve_browser_roots(&storage_base).ok();
        let startup_config = serde_json::from_str(&self.as_ref().rust().config_json.to_string())
            .unwrap_or_else(|_| serde_json::to_value(Config::default()).unwrap_or(Value::Null));
        let effect = match setup {
            Ok(effect) => effect,
            Err(error) => {
                publish!(self.as_mut().clear_policy_owner_preparation());
                publish!(self.as_mut().set_profile_bootstrap_pending(false));
                publish!(
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .profile_startup
                        .fail(FailureStage::ProfileOpen, &error)
                );
                publish!(self.as_mut().sync_profile_startup_notice());
                publish!(self.as_mut().set_status_text(QString::from(
                    "The profile couldn't be opened. Open :diagnostics for details.",
                )));
                return;
            }
        };
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.profile_startup.activate(
                effect.storage_error.as_deref(),
                effect.configuration_error.as_deref(),
                effect.context_error.as_deref(),
            );
        }
        let profile_open_effect = effect.clone();
        publish!(self.as_mut().sync_profile_startup_notice());
        publish!(self.as_mut().clear_session_preview_rows());
        publish!(
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .session_preview_request = None
        );
        publish!(self.as_mut().reset_management_feedback());
        let profile_id = effect.profile.id;
        let session_path = effect.session_path;
        let session_state_root = effect.session_state_root;
        let recovery_paths = effect.recovery_paths;
        let storage_roots = effect.profile.roots.clone();
        // Startup may have discarded profile/runtime/CLI inputs, or CSS may
        // have selected built-in safe layers after the worker returned. The
        // installed application owns every mirror used by later edits.
        let layers = state.configuration_layers();
        let base_config =
            serde_json::to_value(&layers.base).unwrap_or_else(|_| startup_config.clone());
        let cli_overrides = layers.command_line.clone();
        let profile_overrides = layers.profile.clone();
        let runtime_overrides = layers.runtime.clone();
        let temporary_overrides = layers.temporary.clone();
        let config_path = self.as_ref().rust().config_path.to_string();
        let presentation_config = effect.configuration.effective;
        let config =
            serde_json::to_value(&presentation_config).unwrap_or_else(|_| startup_config.clone());
        publish!(self.as_mut().update_theme_palette(&config));
        publish!(
            self.as_mut()
                .update_chrome_preferences(&presentation_config)
        );
        publish!(
            self.as_mut()
                .update_feature_preferences(&presentation_config)
        );
        publish!(self.as_mut().update_hint_preferences(&presentation_config));
        publish!(self.as_mut().update_settings_presentation(&config));
        let hyprland_config = serde_json::from_value::<ferric_browser_config::HyprlandConfig>(
            config.get("hyprland").cloned().unwrap_or(Value::Null),
        )
        .unwrap_or_default();
        publish!(self.as_mut().set_hyprland_status(QString::from(
            hyprland::HyprlandAdapter::from_config(&hyprland_config).status(),
        )));
        let blocking_enabled = config
            .get("blocking")
            .and_then(Value::as_object)
            .is_none_or(|blocking| {
                blocking
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
                    && blocking
                        .get("network_filtering")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
            });
        let mut config_watch = ConfigWatch::default();
        let mut sources = effect.config_sources.clone();
        sources.extend(state.userscript_watch_sources());
        let watch_root = if config_path.is_empty() {
            state
                .userscript_root()
                .map(|root| root.join(".catalog-root-watch"))
        } else {
            Some(PathBuf::from(&config_path))
        };
        if let Some(path) = watch_root {
            config_watch.set_sources(&path, &sources);
        }
        let effective_config_json = serde_json::to_string(&config).unwrap_or_else(|_| "{}".into());
        let desktop_portal_mode = desktop_portals::mode_name(&presentation_config.desktop.portals);
        let profile_overrides_json =
            serde_json::to_string(&profile_overrides).unwrap_or_else(|_| "{}".into());
        let profile_snapshot = effect.profile;
        if !self
            .as_ref()
            .profile_configuration_request_is_current(&request)
        {
            return;
        }
        self.as_mut().detach_site_policy_file();
        self.as_mut()
            .site_policy_preview_cancel(&QString::default());
        let (status, retired_publication) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let retired_publication = this.detach_configuration_publication();
            super::live_ipc_owners::forget_page_order(&this.tab_ids);
            this.page_window_closing = false;
            this.restored_page_windows.clear();
            this.restored_page_group_ids.clear();
            if let Some(wake) = &this.runtime_waker {
                state.set_storage_waker(Arc::clone(wake));
            }
            this.configuration_io
                .selector_receipts
                .published(&state, selector_receipt);
            this.configuration_io.active_selector_fingerprint =
                selector_fingerprint.unwrap_or_default();
            this.configuration_io.selector_config_revision = this
                .configuration_io
                .selector_config_revision
                .wrapping_add(1);
            // CSS and canonical fallback selection are complete. Network
            // bootstrap readiness remains a separate existing consumer gate.
            state.set_policy_owner_preparing(false);
            this.state = Some(state);
            this.configuration_io.published_profile_request = Some(Arc::clone(&request));
            this.profile_open_effect = Some(profile_open_effect);
            this.profile_storage_base.clone_from(&storage_base);
            this.context_catalog_revision = context_catalog_revision;
            this.navigation.journey_durable_ids.borrow_mut().clear();
            this.journey_export_payload = None;
            this.journey_export_preview_text = QString::default();
            this.navigation.pending_journey_transitions.clear();
            this.navigation.pending_journey_parent = None;
            this.navigation.pending_journey_mappings.clear();
            this.pending_history_clear = None;
            this.navigation.pending_journey_reopen = None;
            this.navigation.pending_navigation_urls.clear();
            this.navigation.pending_redirect_tabs.clear();
            this.navigation.pending_redirect_hops.clear();
            this.navigation.page_document_identities.clear();
            this.navigation.pending_same_document_pass_through = None;
            this.ipc_sequence = this.state.as_ref().map_or(0, BrowserApplication::revision);
            this.window = Some(window);
            this.core_window_id = QString::from(window.to_string());
            this.tab = Some(tab);
            this.tab_ids = vec![tab];
            this.popup_tab_ids.clear();
            this.navigation.popup_journey_targets.clear();
            this.completion_text = QString::default();
            this.completion_values = QString::default();
            this.completion_start = 0;
            this.completion_end = 0;
            this.completion_selected = -1;
            this.completion_visible = false;
            this.command_history.clear();
            this.command_history_index = None;
            this.command_history_draft = None;
            this.queued_command_history.clear();
            this.command_history_write_pending = false;
            this.command_history_write_error = None;
            this.search_text = QString::default();
            this.search_backward = false;
            this.session_restore_values = QString::default();
            this.session_preview = QString::default();
            this.profile_values = QString::default();
            this.profile_values_pending = false;
            this.profile_list_command_pending = false;
            this.pending_profile_open = None;
            this.pending_window_new_profile = None;
            this.pending_context_create = None;
            this.profile_preview_text = QString::default();
            this.profile_preview_pending = false;
            this.library_kind = QString::default();
            this.library_values = QString::default();
            this.library_graph_edge_sources = QStringList::default();
            this.library_graph_edge_targets = QStringList::default();
            this.library_graph_edge_transitions = QStringList::default();
            this.library.storage_library = None;
            this.storage_library_revision = 0;
            this.library.storage_library_dirty.set(false);
            this.private_start_state = std::sync::Arc::new(std::sync::Mutex::new(
                super::browser_ui_start_page::PrivateStartState::default(),
            ));
            this.start_rank_requested = false;
            this.library.start_snapshot = ferric_browser_storage::StartPageSnapshot::default();
            this.library.start_read_pending = false;
            this.library.start_write_pending = false;
            this.library.favourite_feedback = None;
            this.library.start_refresh_requested = false;
            this.navigation.content_histories.clear();
            this.private_history.clear();
            this.private_history_next_id = -1;
            this.library.session_names.clear();
            this.library.session_names_dirty.set(true);
            this.library.session_names_refresh_again = false;
            this.library.storage_library_refresh_again = false;
            this.library.storage_library_read_state = super::CatalogReadState::Unrequested;
            this.session_catalog_read_state = super::CatalogReadState::Unrequested;
            this.journey_read_pending = false;
            this.profile_list_refresh_again = false;
            this.library.session_restores.clear();
            this.library.session_saves.clear();
            this.checkpoint = SessionCheckpoint::default();
            this.library.session_checkpoint_clear_pending = false;
            this.library.storage_flush_pending = false;
            this.library.storage_flush_error = None;
            this.action_audit.clear();
            this.link_preview_command = QString::default();
            this.link_preview_original = QString::default();
            this.link_preview_cleaned = QString::default();
            this.link_preview_applied_rules = QStringList::default();
            this.link_preview_removed_parameters = QStringList::default();
            this.link_preview_retained_parameters = QStringList::default();
            this.link_preview_explanation = QString::default();
            this.link_preview_requires_confirmation = false;
            this.link_preview_visible = false;
            this.active_site_experiment = None;
            this.clipboard_request = QString::default();
            this.clipboard_request_sensitive = false;
            this.clipboard_request_primary = false;
            finish_pending_selection_operation(this, OperationUpdate::cancelled());
            this.processes.pending_caret = None;
            super::browser_ui_download_requests::finish_pending_download_operation(
                this,
                OperationUpdate::cancelled(),
            );
            drop(this.download_finalizer.take());
            drop(this.download_stream_writer.take());
            cleanup_staged_downloads(&mut this.staged_downloads);
            this.macro_key_prefix = None;
            this.macro_key_started_ms = None;
            if let Some(editor) = this.processes.pending_editor.take() {
                discard_editor_request(editor);
            }
            let old_print_worker = this.print_worker.take();
            drop(old_print_worker);
            if let Some(path) = this.pending_print.take() {
                let private_temporary = std::mem::take(&mut this.pending_print_private);
                cleanup_print_artifact(&path, private_temporary);
            }
            this.pending_save_page = None;
            // Stop the previous writer before clearing its request metadata.
            // Otherwise a profile replacement could let an in-flight write
            // finish after its path had become unreachable, leaving a stale
            // private scratch file behind.
            let old_editor_write_worker = this.processes.editor_write_worker.take();
            drop(old_editor_write_worker);
            if let Some((_, path, private_temporary)) = this.processes.pending_editor_write.take() {
                cleanup_editor_artifact(&path, private_temporary);
            }
            if let Some(editor) = this.processes.pending_config_edit.take() {
                discard_config_edit(editor);
            }
            this.hint_values = QString::default();
            this.hint_visible = false;
            this.hint_links_only = false;
            this.hint_rapid = false;
            this.hint_family = QString::from("actions");
            this.hint_unmatched_policy = QString::from("hide");
            this.hint_marker_scale = 1.0;
            this.hint_rapid_target = "current".into();
            this.hint_script = None;
            this.hint_first = false;
            this.hint_index = 1;
            this.hint_rapid_tabs_created = 0;
            this.caret_selecting = false;
            this.interaction.hint_session = None;
            this.interaction.hint_interaction = None;
            this.interaction.hint_loading_interaction = None;
            this.interaction.initial_hint = None;
            this.interaction.hint_policy_proof = None;
            this.interaction.confirmed_hint_anchor = None;
            this.interaction.hint_consumed.clear();
            this.config_revision = this.config_revision.wrapping_add(1);
            this.active_tab_projection = None;
            this.url_projection_input = None;
            this.completed_url_projection = None;
            this.policy_revision = this.policy_revision.wrapping_add(1);
            this.config = config;
            crate::page_window_operations::update_policy(this);
            this.base_config = base_config;
            this.learning_mode = this
                .config
                .get("discovery")
                .and_then(Value::as_object)
                .and_then(|discovery| discovery.get("learning_mode"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            this.runtime_overrides = runtime_overrides;
            this.temporary_overrides = temporary_overrides;
            this.cli_overrides = cli_overrides;
            this.profile_overrides = profile_overrides;
            let mixed_resolution = this
                .state
                .as_ref()
                .ok_or_else(|| "application state is unavailable".to_owned())
                .and_then(|state| {
                    let layers = state.configuration_layers();
                    super::mixed_policy_runtime::resolve_for_layers(
                        &layers.base,
                        &layers.profile,
                        &layers.runtime,
                        &layers.command_line,
                        &layers.temporary,
                        state.configuration(),
                        &this.current_url.to_string(),
                    )
                });
            this.replace_mixed_policy(super::mixed_policy_runtime::MixedPolicyState::from_result(
                mixed_resolution,
            ));
            this.binding_overlay = QString::default();
            this.configuration_io.config_watch = config_watch;
            this.configuration_io.pending_context_reload = None;
            this.configuration_io.config_reload_requested_by_user = false;
            this.configuration_io.context_reload_worker = None;
            this.configuration_io.config_reload_worker = Some(ConfigReloadWorker::new());
            this.configuration_io.config_write_worker = Some(ConfigWriteWorker::new());
            this.print_worker = Some(PrintWorker::new());
            this.processes.editor_write_worker = Some(EditorWriteWorker::new());
            this.profile_delete_worker = Some(ProfileDeleteWorker::new());
            this.profile_list_worker = Some(ProfileListWorker::new());
            this.profile_preview_worker = Some(ProfilePreviewWorker::new());
            this.userscript_inventory_refresh_again = false;
            this.interaction.focus_observations.clear();
            this.interaction.focus_suppressions.clear();
            this.profile_name.clone_from(&profile_name);
            this.contexts = this
                .state
                .as_ref()
                .and_then(BrowserApplication::contexts)
                .map(ContextSnapshot::from_records);
            this.pending_context_route = None;
            this.context_route_id = QString::default();
            this.context_route_behavior = QString::default();
            this.context_route_context = QString::default();
            this.context_route_profile = QString::default();
            this.context_route_url = QString::default();
            // Context membership is loaded above; publish the metadata only
            // after the profile/context affinity has been validated.
            this.context_name = QString::default();
            this.context_label = QString::default();
            this.context_workspace = QString::default();
            this.context_accent = QString::default();
            this.context_entry_force_reuse = false;
            let bindings = configured_bindings(&this.registry, &this.config);
            this.replace_input_bindings(bindings);
            this.profile_id = profile_id;
            this.profile_persistence = if profile_snapshot.durable {
                ProfilePersistence::Durable
            } else {
                ProfilePersistence::Transient
            };
            this.pending_permission_reset_session = None;
            let logging_config = serde_json::from_value::<LoggingConfig>(
                this.config.get("logging").cloned().unwrap_or(Value::Null),
            )
            .unwrap_or_default();
            this.structured_log = storage_roots.as_ref().and_then(|roots| {
                logging::StructuredLogSink::open(&roots.state, &logging_config).ok()
            });
            this.storage_roots = storage_roots;
            this.profile_registry_roots = resolve_browser_roots(&storage_base).ok();
            this.policy_roots = policy_roots;
            this.session_id = Uuid::new_v4();
            this.session_path = session_path
                .map(|path| path.with_file_name(format!("current-{}.json", this.session_id)));
            this.session_state_root = session_state_root;
            this.recovery_paths = recovery_paths;
            this.recovery_placeholder = None;
            this.undo_fallback = None;
            this.undo_fallback_ready = false;
            this.recovered_session_ids.clear();
            this.core_mode = Mode::Normal;
            if let Some(bindings) = this.bindings.as_mut() {
                bindings.set_mode(Mode::Normal);
            }
            let mut message = this.profile_startup.ready_message(private_profile);
            if let Some(error) = this.mixed_policy.error() {
                message.push_str("; Mixed input policy unavailable: ");
                message.push_str(error);
            }
            let status = QString::from(message);
            this.status_text = status.clone();
            (status, retired_publication)
        };
        drop(retired_publication);
        let binding_context = self.as_ref().binding_context();
        self.as_mut().input_bindings_changed(binding_context);
        if self.as_ref().in_page_window_mode() {
            let checkpoint = {
                let owner = self.as_ref();
                let rust = owner.rust();
                rust.state
                    .as_ref()
                    .and_then(ferric_browser_application::BrowserApplication::profile_instance_id)
                    .and_then(|profile| {
                        super::live_ipc_owners::page_session_checkpoint(
                            profile,
                            &rust.window_token.to_string(),
                        )
                    })
            };
            if let Some(checkpoint) = checkpoint {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.session_id = checkpoint.session_id;
                this.session_path = checkpoint.path;
                this.recovered_session_ids = checkpoint.recovered;
                this.recovery_paths.clear();
            }
        }
        publish!(self.as_mut().set_session_recovery_pending(false));
        publish!(self.as_mut().set_session_recovery_error(QString::default()));
        publish!(self.as_mut().clear_userscript_inventory_rows());
        publish!(self.as_mut().set_userscript_inventory_ready(false));
        publish!(self.as_mut().set_userscript_inventory_pending(false));
        publish!(
            self.as_mut()
                .set_userscript_inventory_error(QString::default())
        );
        publish!(
            self.as_mut()
                .set_userscript_install_state(QString::from("idle"))
        );
        let revision = self
            .as_ref()
            .rust()
            .start_page_profile_revision
            .wrapping_add(1);
        let history_revision = self
            .as_ref()
            .rust()
            .content_history_revision
            .wrapping_add(1);
        publish!(self.as_mut().set_content_history_revision(history_revision));
        publish!(self.as_mut().set_start_page_ready(false));
        publish!(self.as_mut().set_start_page_profile_revision(revision));
        publish!(self.as_mut().set_start_page_owner_token(QString::default()));
        publish!(self.as_mut().set_start_page_error(QString::default()));
        publish!(self.as_mut().set_start_page_busy(false));
        publish!(self.as_mut().publish_start_page());
        publish!(
            self.as_mut()
                .set_config_json(QString::from(effective_config_json))
        );
        publish!(self.as_mut().note_hint_configuration_change());
        publish!(self.as_mut().update_hint_preferences(&presentation_config));
        publish!(self.as_mut().request_interaction_policy_refresh());
        publish!(
            self.as_mut()
                .set_desktop_portal_mode(QString::from(desktop_portal_mode))
        );
        let learning_mode = self.as_ref().rust().learning_mode;
        publish!(self.as_mut().set_learning_mode(learning_mode));
        publish!(
            self.as_mut()
                .set_profile_overrides_json(QString::from(profile_overrides_json))
        );
        publish!(self.as_mut().set_blocking_policy_ready(false));
        publish!(self.as_mut().set_blocking_hosts(QStringList::default()));
        publish!(
            self.as_mut()
                .set_blocking_exceptions(QStringList::default())
        );
        publish!(
            self.as_mut()
                .set_blocking_rule_hosts(QStringList::default())
        );
        publish!(
            self.as_mut()
                .set_blocking_rule_list_ids(QStringList::default())
        );
        publish!(
            self.as_mut()
                .set_blocking_exception_rule_hosts(QStringList::default())
        );
        publish!(
            self.as_mut()
                .set_blocking_exception_rule_list_ids(QStringList::default())
        );
        publish!(self.as_mut().set_blocking_loaded_lists(QString::from("[]")));
        publish!(
            self.as_mut()
                .set_blocking_list_metadata(QString::from("[]"))
        );
        publish!(
            self.as_mut()
                .set_blocking_cosmetic_rule_hosts(QStringList::default())
        );
        publish!(
            self.as_mut()
                .set_blocking_cosmetic_rule_selectors(QStringList::default())
        );
        publish!(
            self.as_mut()
                .set_blocking_cosmetic_exception_hosts(QStringList::default())
        );
        publish!(
            self.as_mut()
                .set_blocking_cosmetic_exception_selectors(QStringList::default())
        );
        publish!(
            self.as_mut()
                .set_blocking_skipped_lists(QString::from("[]"))
        );
        publish!(
            self.as_mut()
                .set_blocking_bypass_sites(QStringList::default())
        );
        publish!(self.as_mut().clear_site_experiment_presentation());
        publish!(
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .last_site_doctor_result = None
        );
        publish!(self.as_mut().set_blocking_enabled(blocking_enabled));
        publish!(self.as_mut().set_blocking_blocked_count(0));
        publish!(self.as_mut().set_blocking_active_site_count(0));
        publish!(self.as_mut().set_blocking_unknown_context_count(0));
        publish!(self.as_mut().clear_blocking_active_evidence());
        publish!(self.as_mut().set_active_tab_index(0));
        publish!(self.as_mut().set_tab_count(1));
        publish!(self.as_mut().set_status_text(status));
        // The bootstrap snapshot predates watcher installation. Rescan after
        // the watch is armed so edits made during that handoff are not lost.
        publish!(self.as_mut().refresh_userscript_inventory());
        publish!(self.as_mut().sync_context_choices());
        publish!(self.as_mut().sync_context_metadata());
        if !self
            .as_ref()
            .profile_configuration_request_is_current(&request)
        {
            return;
        }
        let policy_request = self.as_mut().request_network_policy(false);
        if !self
            .as_ref()
            .profile_configuration_request_is_current(&request)
        {
            return;
        }
        if let Err(error) = policy_request {
            publish!(
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .profile_startup
                    .network_result(Some(&error))
            );
            publish!(self.as_mut().sync_profile_startup_notice());
            publish!(self.as_mut().clear_policy_owner_preparation());
            publish!(self.as_mut().set_profile_bootstrap_pending(false));
            publish!(self.as_mut().set_status_text(QString::from(
                "Network filtering couldn't be prepared. Open :diagnostics for details.",
            )));
        }
    }

    pub(super) fn install_blocking_list(
        mut self: Pin<&mut Self>,
        list_id: &QString,
        content: &QString,
        etag: &QString,
        last_modified: &QString,
    ) -> bool {
        let id = list_id.to_string();
        if !network_policy::valid_list_id(&id) {
            self.set_status_text(QString::from(format!("Blocklist ID rejected: {id}")));
            return false;
        }
        let Some(roots) = self.as_ref().rust().storage_roots.clone() else {
            self.set_status_text(QString::from(
                "Blocklist update unavailable in a private profile",
            ));
            return false;
        };
        let bytes = content.to_string().into_bytes();
        if network_policy::validate_download(&bytes).is_err() {
            self.set_status_text(QString::from(format!(
                "Blocklist {id} rejected; last-known-good list retained"
            )));
            return false;
        }
        let list_path = roots.cache.join("blocklists").join(format!("{id}.txt"));
        if let Err(error) = atomic_write_private(&list_path, &bytes) {
            self.set_status_text(QString::from(format!(
                "Blocklist {id} could not be installed: {error}"
            )));
            return false;
        }
        let metadata = format!(
            "{}\n{}\n",
            bounded_header(etag),
            bounded_header(last_modified)
        );
        let metadata_path = roots.cache.join("blocklists").join(format!("{id}.meta"));
        if let Err(error) = atomic_write_private(&metadata_path, metadata.as_bytes()) {
            self.as_mut().set_status_text(QString::from(format!(
                "Blocklist {id} installed without cache metadata: {error}"
            )));
        }
        self.as_mut()
            .set_status_text(QString::from(format!("Blocklist {id} installed")));
        true
    }

    pub(super) fn accept_link_cleaning_bundle(
        self: Pin<&mut Self>,
        content: &QString,
        expected_checksum: &QString,
        etag: &QString,
        last_modified: &QString,
    ) -> bool {
        let Some(roots) = self.as_ref().rust().storage_roots.clone() else {
            self.set_status_text(QString::from(
                "Clean-link updates unavailable in a private profile",
            ));
            return false;
        };
        match link_cleaning_policy::accept_downloaded_bundle_with_metadata(
            &roots,
            content.to_string().as_bytes(),
            &expected_checksum.to_string(),
            &etag.to_string(),
            &last_modified.to_string(),
        ) {
            Ok(value) => {
                let revision = value
                    .get("revision")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                self.set_status_text(QString::from(format!(
                    "Clean-link rules accepted: {revision}"
                )));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Clean-link update rejected; last accepted rules retained: {error}"
                )));
                false
            }
        }
    }
}
