use super::{
    BrowserApplication, Config, ConfigReloadWorker, ConfigWatch, ConfigWriteWorker,
    ContextSnapshot, ContextsConfig, CxxQtType, EditorWriteWorker, LoggingConfig, Mode,
    NetworkPolicyWorker, PathBuf, PendingProfileConfiguration, Pin, PrintWorker, PrivacyKind,
    ProfileDeleteWorker, ProfileListWorker, ProfileOpenEffect, ProfileOpenRequest,
    ProfilePersistence, ProfilePreviewWorker, ProfileStorage, QString, QStringList, RootSpec,
    RuntimeOverrides, UserscriptManagerWorker, Uuid, Value, atomic_write_private,
    bootstrap_runtime, bounded_header, cleanup_editor_artifact, cleanup_print_artifact,
    cleanup_staged_downloads, configured_bindings, desktop_portals, discard_config_edit,
    discard_editor_request, finish_pending_selection_operation, hyprland, link_cleaning_policy,
    logging, network_policy, qobject, resolve_browser_roots,
};

impl qobject::BrowserUi {
    #[allow(clippy::too_many_lines)]
    pub(super) fn configure_profile(
        mut self: Pin<&mut Self>,
        private_profile: bool,
        ephemeral_profile: bool,
        profile_label: &QString,
        profile_name: &QString,
        storage_base: &QString,
    ) {
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
            self.as_mut().set_profile_bootstrap_pending(false);
            self.set_status_text(QString::from("Core profile setup failed"));
            return;
        };
        self.as_mut().set_profile_bootstrap_pending(true);
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
        let storage = if private_profile || ephemeral_profile {
            ProfileStorage::Transient
        } else if storage_base.is_empty() {
            ProfileStorage::Durable(RootSpec::Xdg)
        } else {
            ProfileStorage::Durable(RootSpec::Base(PathBuf::from(&storage_base)))
        };
        let mut state = BrowserApplication::from_runtime(runtime, Config::default());
        let request_result = state
            .request_profile_open(ProfileOpenRequest {
                name: profile_name.clone(),
                label: profile_label,
                privacy,
                storage,
                base_configuration,
                initial_profile_overrides,
                command_line_overrides,
                config_path: (!config_path.is_empty()).then(|| PathBuf::from(config_path)),
                contexts,
                context_configuration_error,
            })
            .map_err(|error| error.to_string());
        if let Err(error) = request_result {
            self.as_mut().set_profile_bootstrap_pending(false);
            self.set_status_text(QString::from(format!("Profile bootstrap failed: {error}")));
            return;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_profile_configuration = Some(PendingProfileConfiguration {
            private_profile,
            profile_name,
            storage_base,
            state,
            window,
            tab,
        });
        self.as_mut()
            .set_status_text(QString::from("Profile bootstrap requested"));
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn finish_configure_profile(
        mut self: Pin<&mut Self>,
        pending: PendingProfileConfiguration,
        setup: Result<ProfileOpenEffect, String>,
    ) {
        let PendingProfileConfiguration {
            private_profile,
            profile_name,
            storage_base,
            state,
            window,
            tab,
        } = pending;
        let userscript_roots = resolve_browser_roots(&storage_base).ok();
        let startup_config = serde_json::from_str(&self.as_ref().rust().config_json.to_string())
            .unwrap_or_else(|_| serde_json::to_value(Config::default()).unwrap_or(Value::Null));
        let effect = match setup {
            Ok(effect) => effect,
            Err(error) => {
                self.as_mut().set_profile_bootstrap_pending(false);
                self.as_mut()
                    .set_status_text(QString::from(format!("Profile bootstrap failed: {error}")));
                return;
            }
        };
        let profile_id = effect.profile.id;
        let session_path = effect.session_path;
        let session_state_root = effect.session_state_root;
        let storage_roots = effect.profile.roots.clone();
        let storage_ready = effect.storage_error.is_none();
        let storage_error = effect.storage_error;
        let base_config =
            serde_json::from_str::<Value>(&self.as_ref().rust().config_base_json.to_string())
                .ok()
                .filter(Value::is_object)
                .unwrap_or_else(|| startup_config.clone());
        let cli_overrides = serde_json::from_str::<RuntimeOverrides>(
            &self.as_ref().rust().cli_overrides_json.to_string(),
        )
        .unwrap_or_default();
        let profile_overrides = effect.profile_overrides;
        let runtime_overrides = effect.runtime_overrides;
        let config_path = self.as_ref().rust().config_path.to_string();
        let config_layer_error = effect.configuration_error;
        let presentation_config = effect.configuration.effective;
        let config =
            serde_json::to_value(&presentation_config).unwrap_or_else(|_| startup_config.clone());
        self.as_mut().update_theme_palette(&config);
        self.as_mut()
            .update_chrome_preferences(&presentation_config);
        self.as_mut()
            .update_feature_preferences(&presentation_config);
        self.as_mut().update_settings_presentation(&config);
        let hyprland_config = serde_json::from_value::<ferric_browser_config::HyprlandConfig>(
            config.get("hyprland").cloned().unwrap_or(Value::Null),
        )
        .unwrap_or_default();
        self.as_mut().set_hyprland_status(QString::from(
            hyprland::HyprlandAdapter::from_config(&hyprland_config).status(),
        ));
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
        if !config_path.is_empty() {
            let path = PathBuf::from(&config_path);
            config_watch.set_sources(&path, &effect.config_sources);
        }
        let effective_config_json = serde_json::to_string(&config).unwrap_or_else(|_| "{}".into());
        let desktop_portal_mode = desktop_portals::mode_name(&presentation_config.desktop.portals);
        let profile_overrides_json =
            serde_json::to_string(&profile_overrides).unwrap_or_else(|_| "{}".into());
        let context_config_error = effect.context_error;
        let profile_snapshot = effect.profile;
        let storage_worker_error: Option<String> = None;
        let status = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.state = Some(state);
            this.journey_durable_ids.borrow_mut().clear();
            this.journey_export_payload = None;
            this.journey_export_preview_text = QString::default();
            this.pending_journey_transitions.clear();
            this.pending_journey_parent = None;
            this.pending_journey_mappings.clear();
            this.pending_history_clear = None;
            this.pending_journey_reopen = None;
            this.pending_navigation_urls.clear();
            this.pending_redirect_tabs.clear();
            this.pending_redirect_hops.clear();
            this.ipc_sequence = this.state.as_ref().map_or(0, BrowserApplication::revision);
            this.window = Some(window);
            this.core_window_id = QString::from(window.to_string());
            this.tab = Some(tab);
            this.tab_ids = vec![tab];
            this.popup_tab_ids.clear();
            this.popup_journey_targets.clear();
            this.completion_text = QString::default();
            this.completion_values = QString::default();
            this.completion_start = 0;
            this.completion_end = 0;
            this.completion_selected = -1;
            this.completion_visible = false;
            this.command_history.clear();
            this.command_history_index = None;
            this.command_history_draft.clear();
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
            this.storage_library = None;
            this.storage_library_revision = 0;
            this.storage_library_dirty.set(false);
            this.private_history.clear();
            this.private_history_next_id = -1;
            this.session_names.clear();
            this.session_names_dirty.set(true);
            this.session_restore_preview_pending = false;
            this.session_restore_load_append = None;
            this.session_restore_recovery_pending = false;
            this.session_save_pending = false;
            this.session_save_checkpoint = false;
            this.session_checkpoint_clear_pending = false;
            this.storage_flush_pending = false;
            this.storage_flush_error = None;
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
            finish_pending_selection_operation(this, "cancelled");
            this.pending_caret = None;
            this.pending_spawn = None;
            if let Some(pending) = this.pending_action_target.take() {
                this.operation_states
                    .insert(pending.operation_id, "cancelled".into());
            }
            this.pending_download = None;
            cleanup_staged_downloads(&mut this.staged_downloads);
            this.macro_key_prefix = None;
            this.macro_key_started_ms = None;
            if let Some(pending) = this.pending_userscript.take() {
                this.operation_states.insert(
                    pending.operation_id,
                    "failed (document target invalidated)".into(),
                );
            }
            if let Some(editor) = this.pending_editor.take() {
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
            let old_editor_write_worker = this.editor_write_worker.take();
            drop(old_editor_write_worker);
            if let Some((_, path, private_temporary)) = this.pending_editor_write.take() {
                cleanup_editor_artifact(&path, private_temporary);
            }
            if let Some(editor) = this.pending_config_edit.take() {
                discard_config_edit(editor);
            }
            this.hint_values = QString::default();
            this.hint_visible = false;
            this.hint_links_only = false;
            this.hint_rapid = false;
            this.hint_rapid_target = "current".into();
            this.hint_script = None;
            this.hint_rapid_tabs_created = 0;
            this.caret_selecting = false;
            this.hint_session = None;
            this.config = config;
            this.base_config = base_config;
            this.learning_mode = this
                .config
                .get("discovery")
                .and_then(Value::as_object)
                .and_then(|discovery| discovery.get("learning_mode"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            this.runtime_overrides = runtime_overrides;
            this.temporary_overrides = RuntimeOverrides::default();
            this.cli_overrides = cli_overrides;
            this.profile_overrides = profile_overrides;
            this.config_watch = config_watch;
            this.config_reload_worker = ConfigReloadWorker::spawn().ok();
            this.config_write_worker = ConfigWriteWorker::spawn().ok();
            this.print_worker = PrintWorker::spawn().ok();
            this.editor_write_worker = EditorWriteWorker::spawn().ok();
            this.profile_delete_worker = ProfileDeleteWorker::spawn().ok();
            this.profile_list_worker = ProfileListWorker::spawn().ok();
            this.profile_preview_worker = ProfilePreviewWorker::spawn().ok();
            this.userscript_manager_worker = UserscriptManagerWorker::spawn().ok();
            this.focus_observations.clear();
            this.focus_suppressions.clear();
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
            if let Some(bindings) = configured_bindings(&this.registry, &this.config) {
                this.bindings = Some(bindings);
            }
            this.profile_id = profile_id;
            this.profile_persistence = if profile_snapshot.durable {
                ProfilePersistence::Durable
            } else {
                ProfilePersistence::Transient
            };
            this.session_permissions.clear();
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
            this.userscript_roots = userscript_roots;
            this.session_id = Uuid::new_v4();
            this.session_path = session_path
                .map(|path| path.with_file_name(format!("current-{}.json", this.session_id)));
            this.session_state_root = session_state_root;
            this.core_mode = Mode::Normal;
            if let Some(bindings) = this.bindings.as_mut() {
                bindings.set_mode(Mode::Normal);
            }
            let status = if private_profile {
                QString::from("Private profile ready")
            } else if let Some(error) = storage_error {
                QString::from(format!(
                    "Profile ready (durable storage unavailable: {error})"
                ))
            } else if let Some(error) = storage_worker_error {
                QString::from(format!(
                    "Profile ready (metadata worker unavailable: {error})"
                ))
            } else if let Some(error) = config_layer_error.as_deref() {
                QString::from(format!("Profile ready (configuration issue: {error})"))
            } else if let Some(error) = context_config_error.as_deref() {
                QString::from(format!(
                    "Profile ready (context configuration issue: {error})"
                ))
            } else if storage_ready {
                QString::from("Profile ready")
            } else {
                QString::from("Profile ready (history unavailable)")
            };
            this.status_text = status.clone();
            status
        };
        self.as_mut()
            .set_config_json(QString::from(effective_config_json));
        self.as_mut()
            .set_desktop_portal_mode(QString::from(desktop_portal_mode));
        let learning_mode = self.as_ref().rust().learning_mode;
        self.as_mut().set_learning_mode(learning_mode);
        self.as_mut()
            .set_profile_overrides_json(QString::from(profile_overrides_json));
        self.as_mut().set_blocking_hosts(QStringList::default());
        self.as_mut()
            .set_blocking_exceptions(QStringList::default());
        self.as_mut()
            .set_blocking_rule_hosts(QStringList::default());
        self.as_mut()
            .set_blocking_rule_list_ids(QStringList::default());
        self.as_mut()
            .set_blocking_exception_rule_hosts(QStringList::default());
        self.as_mut()
            .set_blocking_exception_rule_list_ids(QStringList::default());
        self.as_mut().set_blocking_loaded_lists(QString::from("[]"));
        self.as_mut()
            .set_blocking_list_metadata(QString::from("[]"));
        self.as_mut()
            .set_blocking_cosmetic_rule_hosts(QStringList::default());
        self.as_mut()
            .set_blocking_cosmetic_rule_selectors(QStringList::default());
        self.as_mut()
            .set_blocking_cosmetic_exception_hosts(QStringList::default());
        self.as_mut()
            .set_blocking_cosmetic_exception_selectors(QStringList::default());
        self.as_mut()
            .set_blocking_skipped_lists(QString::from("[]"));
        self.as_mut()
            .set_blocking_bypass_sites(QStringList::default());
        self.as_mut().clear_site_experiment_presentation();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .last_site_doctor_result = None;
        self.as_mut().set_blocking_enabled(blocking_enabled);
        self.as_mut().set_blocking_blocked_count(0);
        self.as_mut().set_blocking_active_site_count(0);
        self.as_mut().set_blocking_unknown_context_count(0);
        self.as_mut().clear_blocking_active_evidence();
        self.as_mut().set_active_tab_index(0);
        self.as_mut().set_tab_count(1);
        self.as_mut().set_status_text(status);
        self.as_mut().sync_context_metadata();
        let policy_request = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let policy_roots = this.userscript_roots.clone();
            let policy_config = this.config.clone();
            if this.network_policy_worker.is_none() {
                this.network_policy_worker = NetworkPolicyWorker::spawn().ok();
            }
            this.network_policy_worker
                .as_mut()
                .map(|worker| worker.request(policy_roots, policy_config))
        };
        if !matches!(policy_request, Some(Ok(()))) {
            self.as_mut().set_profile_bootstrap_pending(false);
            self.as_mut().set_status_text(QString::from(
                "Profile ready (network policy worker unavailable)",
            ));
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
