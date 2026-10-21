use super::browser_ui_state::PendingSelectorAction;
use super::selector_validation::selector_validation_items;
use super::{
    Config, ConfigReadOperation, ConfigReloadWorker, ConfigWriteWorker, ConfigurationLayers,
    CxxQtType, Duration, FeedbackLane, FeedbackState, HyprlandResponse, HyprlandWorker, Instant,
    NetworkPolicyWorker, ParseInput, ParsedCommand, PathBuf, Pin, PortalProbeWorker, PrintWorker,
    ProfileDeleteWorker, ProfileListWorker, ProfileMutationResult, ProfilePreviewWorker, QString,
    ReducedMotionProbeWorker, RuntimeOverrides, SiteExperiment, SystemFontScaleProbeWorker,
    UserscriptManagerResult, Value, cleanup_print_artifact, configured_bindings, desktop_portals,
    desktop_preferences, parse_chain, parse_ipc_mode, profile_from_list_values, qobject,
    safe_ipc_url,
};

impl qobject::BrowserUi {
    pub(super) fn poll_config(mut self: Pin<&mut Self>) {
        self.as_mut().poll_configuration_publication();
        self.as_mut().poll_peer_configuration();
        self.as_mut().expire_selector_validation();
        self.as_mut().poll_diagnostics();
        self.as_mut().poll_context_configuration_reload();
        let reduced_motion_result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.reduced_motion_probe_worker.is_none() {
                this.reduced_motion_probe_worker = ReducedMotionProbeWorker::spawn().ok();
            }
            if let Some(worker) = this.reduced_motion_probe_worker.as_mut() {
                let _ = worker.request_once();
                worker.poll()
            } else {
                None
            }
        };
        if let Some(result) = reduced_motion_result {
            let presentation = desktop_preferences::reduced_motion(&result);
            self.as_mut()
                .set_system_reduced_motion_status(QString::from(presentation.status));
            self.as_mut()
                .set_system_reduced_motion_enabled(presentation.enabled);
        }
        let font_scale_result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.font_scale_probe_worker.is_none() {
                this.font_scale_probe_worker = SystemFontScaleProbeWorker::spawn().ok();
            }
            if let Some(worker) = this.font_scale_probe_worker.as_mut() {
                let _ = worker.request_once();
                worker.poll()
            } else {
                None
            }
        };
        if let Some(result) = font_scale_result {
            let presentation = desktop_preferences::font_scale(&result);
            self.as_mut()
                .set_system_font_scale_status(QString::from(presentation.status));
            self.as_mut().set_system_font_scale(presentation.scale);
        }
        let portal_result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .portal_probe_worker
            .as_mut()
            .and_then(PortalProbeWorker::poll);
        if let Some(result) = portal_result {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .portal_capabilities = desktop_portals::project(&result);
        }
        let hyprland_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .hyprland_worker
                .as_mut()
                .and_then(HyprlandWorker::poll)
        };
        if let Some(result) = hyprland_result {
            match result {
                HyprlandResponse::RouteWorkspace(result) => match result {
                    Ok(()) => self
                        .as_mut()
                        .set_status_text(QString::from("Hyprland workspace routing completed")),
                    Err(error) => self.as_mut().set_status_text(QString::from(format!(
                        "Hyprland workspace routing failed: {error}"
                    ))),
                },
                HyprlandResponse::MoveActiveWindow(result) => match result {
                    Ok(()) => self
                        .as_mut()
                        .set_status_text(QString::from("Hyprland workspace move completed")),
                    Err(error) => self.as_mut().set_status_text(QString::from(format!(
                        "Hyprland workspace move failed: {error}"
                    ))),
                },
                HyprlandResponse::BrowserClients(result) => match result {
                    Ok(clients) => {
                        let serialized =
                            serde_json::to_string(&clients).unwrap_or_else(|_| "[]".into());
                        self.as_mut()
                            .set_hyprland_clients_json(QString::from(serialized));
                        self.as_mut().set_status_text(QString::from(
                            "Hyprland browser-client query completed",
                        ));
                    }
                    Err(error) => self.as_mut().set_status_text(QString::from(format!(
                        "Hyprland browser-client query failed: {error}"
                    ))),
                },
            }
        }
        let profile_setup_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .pending_profile_configuration
                .as_mut()
                .and_then(|pending| pending.state.poll_profile_open())
        };
        if let Some(result) = profile_setup_result {
            let pending = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_profile_configuration
                .take();
            if let Some(pending) = pending {
                match result.map_err(|error| error.to_string()) {
                    Ok(effect) => {
                        // The Rust active base still contains safe defaults
                        // during bootstrap. Validate the authored startup input,
                        // not that prior active projection.
                        let base = serde_json::from_str::<Config>(
                            &self.as_ref().rust().config_base_json.to_string(),
                        )
                        .unwrap_or_default();
                        let cli = serde_json::from_str::<RuntimeOverrides>(
                            &self.as_ref().rust().cli_overrides_json.to_string(),
                        )
                        .unwrap_or_default();
                        let layers = ConfigurationLayers {
                            base,
                            profile: effect.profile_overrides.clone(),
                            runtime: effect.runtime_overrides.clone(),
                            command_line: cli,
                            temporary: self.as_ref().rust().temporary_overrides.clone(),
                        };
                        self.as_mut()
                            .queue_profile_selector_validation(pending, effect, layers);
                    }
                    Err(error) => self.as_mut().finish_configure_profile(pending, Err(error)),
                }
            }
        }
        let network_policy_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .configuration_io
                .network_policy_worker
                .as_mut()
                .and_then(NetworkPolicyWorker::poll)
        };
        if let Some(result) = network_policy_result {
            let reload_requested = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .configuration_io
                .network_policy_reload_pending;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .configuration_io
                .network_policy_reload_pending = false;
            match result {
                Ok(policy) => {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .profile_startup
                        .network_result(None);
                    self.as_mut().sync_profile_startup_notice();
                    self.as_mut().apply_network_policy_snapshot(policy);
                    if reload_requested {
                        self.as_mut()
                            .set_status_text(QString::from("Network filtering updated"));
                    }
                }
                Err(error) => {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .profile_startup
                        .network_result(Some(&error));
                    self.as_mut().sync_profile_startup_notice();
                    self.as_mut().set_status_text(QString::from(
                        "Network filtering couldn't be prepared. Open :diagnostics for details.",
                    ));
                }
            }
            self.as_mut().set_profile_bootstrap_pending(false);
        }
        let userscript_manager_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .state
                .as_mut()
                .and_then(super::BrowserApplication::poll_userscript_job)
        };
        if let Some(result) = userscript_manager_result {
            match result {
                UserscriptManagerResult::Inventory(result, _) => {
                    self.as_mut().set_userscript_inventory_pending(false);
                    match result {
                        Ok(scripts) => {
                            if self.as_ref().active_profile_is_transient() {
                                self.as_mut().clear_userscript_inventory_rows();
                            } else {
                                self.as_mut().set_userscript_inventory_rows(&scripts);
                            }
                        }
                        Err(error) => {
                            self.as_mut().userscript_inventory_failed(&error);
                        }
                    }
                }
                UserscriptManagerResult::Installed(result) => self
                    .as_mut()
                    .finish_userscript_change(result, "installed", "Userscript installed"),
                UserscriptManagerResult::Removed(result) => {
                    self.as_mut()
                        .finish_userscript_change(result, "removed", "Userscript removed")
                }
                UserscriptManagerResult::SetEnabled { enabled, result } => {
                    self.as_mut().finish_userscript_change(
                        result,
                        if enabled { "enabled" } else { "disabled" },
                        if enabled {
                            "Userscript enabled; it applies on the next navigation"
                        } else {
                            "Userscript disabled; it applies on the next navigation"
                        },
                    );
                }
            }
            let refresh_again = {
                let mut rust = self.as_mut().rust_mut();
                std::mem::take(&mut rust.as_mut().get_mut().userscript_inventory_refresh_again)
            };
            if refresh_again {
                self.as_mut().set_userscript_inventory_pending(false);
                self.as_mut().refresh_userscript_inventory();
            }
        }
        let profile_list_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .profile_list_worker
                .as_mut()
                .and_then(ProfileListWorker::poll)
        };
        let profile_list_result =
            if self.as_ref().rust().profile_list_refresh_again && profile_list_result.is_some() {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .profile_list_refresh_again = false;
                self.as_mut().set_profile_values_pending(false);
                let command_request = self.as_ref().rust().profile_list_command_pending;
                self.as_mut()
                    .request_profile_list_with_mode(command_request);
                None
            } else {
                profile_list_result
            };
        if let Some(result) = profile_list_result {
            let command_request = self.as_ref().rust().profile_list_command_pending;
            let pending_profile_open = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_profile_open
                .take();
            let pending_window_new_profile = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_window_new_profile
                .take();
            let pending_context_create = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_context_create
                .take();
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .profile_list_command_pending = false;
            self.as_mut().set_profile_values_pending(false);
            match result {
                Ok(values) => {
                    self.as_mut().set_profile_list_error(QString::default());
                    let status_values = values.clone();
                    self.as_mut().set_profile_values(QString::from(values));
                    if let Some((name, url)) = pending_profile_open {
                        if let Some((profile_name, profile_label)) =
                            profile_from_list_values(&status_values, &name)
                        {
                            self.as_mut()
                                .rust_mut()
                                .as_mut()
                                .get_mut()
                                .pending_engine_action = Some(format!(
                                "profile-window\t{}\t{}\t{}",
                                profile_name,
                                profile_label,
                                safe_ipc_url(url.as_str())
                            ));
                            self.as_mut().set_status_text(QString::from(format!(
                                "Opening profile window: {profile_label}"
                            )));
                        } else {
                            self.as_mut().set_status_text(QString::from(format!(
                                "Profile not found: {name}"
                            )));
                        }
                    }
                    if let Some((name, private)) = pending_window_new_profile {
                        if let Some((profile_name, _profile_label)) =
                            profile_from_list_values(&status_values, &name)
                        {
                            self.as_mut()
                                .rust_mut()
                                .as_mut()
                                .get_mut()
                                .pending_engine_action =
                                Some(super::browser_ui_ipc_preparation::window_open_action(
                                    private,
                                    &profile_name,
                                    None,
                                    false,
                                    "about:blank",
                                ));
                            self.as_mut().set_status_text(QString::from(if private {
                                "Private window requested"
                            } else {
                                "New window requested"
                            }));
                        } else {
                            self.as_mut().set_status_text(QString::from(format!(
                                "Profile not found: {name}"
                            )));
                        }
                    }
                    if let Some((name, label, profile, workspace)) = pending_context_create {
                        if profile_from_list_values(&status_values, &profile).is_some() {
                            match self.as_mut().create_context_from_parts(
                                &name,
                                &label,
                                &profile,
                                workspace.as_deref(),
                            ) {
                                Ok(_) => self.as_mut().set_status_text(QString::from(format!(
                                    "Context created: {name}"
                                ))),
                                Err(error) => self.as_mut().set_status_text(QString::from(
                                    format!("Context creation failed: {error}"),
                                )),
                            }
                        } else {
                            self.as_mut().set_status_text(QString::from(format!(
                                "Profile not found: {profile}"
                            )));
                        }
                    }
                    if command_request {
                        self.as_mut()
                            .set_status_text(QString::from(if status_values.is_empty() {
                                "No durable profiles".to_owned()
                            } else {
                                format!("Profiles: {status_values}")
                            }));
                    }
                }
                Err(error) => {
                    self.as_mut().set_profile_list_error(QString::from(&error));
                    self.as_mut().set_status_text(QString::from(format!(
                        "Profile list unavailable: {error}"
                    )));
                }
            }
        }
        let profile_preview_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .profile_preview_worker
                .as_mut()
                .and_then(ProfilePreviewWorker::poll)
        };
        if let Some(result) = profile_preview_result {
            self.as_mut().set_profile_preview_pending(false);
            match result {
                Ok(preview) => {
                    self.as_mut().set_profile_preview_error(QString::default());
                    self.as_mut()
                        .set_profile_preview_text(QString::from(preview));
                }
                Err(error) => {
                    self.as_mut()
                        .set_profile_preview_error(QString::from(&error));
                    self.as_mut().set_profile_preview_text(QString::default());
                    self.as_mut().set_status_text(QString::from(format!(
                        "Profile preview unavailable: {error}"
                    )));
                }
            }
        }
        let profile_delete_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .profile_delete_worker
                .as_mut()
                .and_then(ProfileDeleteWorker::poll)
        };
        if let Some(result) = profile_delete_result {
            match result {
                Ok(result) => {
                    let message = match result {
                        ProfileMutationResult::Created => "Profile created",
                        ProfileMutationResult::Renamed => "Profile display name updated",
                        ProfileMutationResult::Deleted(ref outcome) if outcome.cleanup_pending => {
                            "Profile removed. Remaining profile files will be cleaned up when Ferric next starts. Cookies and website storage stay on disk."
                        }
                        ProfileMutationResult::Deleted(_) => {
                            "Profile metadata deleted. Cookies and website storage stay on disk."
                        }
                    };
                    self.as_mut().publish_management_feedback(
                        FeedbackLane::Profile,
                        FeedbackState::Succeeded,
                        message,
                        "",
                    );
                    self.as_mut().set_status_text(QString::from(message));
                    self.as_mut().refresh_profiles_after_mutation();
                }
                Err(error) => {
                    self.as_mut().publish_management_feedback(FeedbackLane::Profile, FeedbackState::Failed, "The profile change couldn't be completed. Review the details and try again.", &error);
                    self.as_mut().set_status_text(QString::from(format!(
                        "Profile operation failed: {error}"
                    )));
                }
            }
        }
        self.as_mut().poll_config_edit();
        self.as_mut().poll_editor_write();
        let write_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .configuration_io
                .config_write_worker
                .as_mut()
                .and_then(ConfigWriteWorker::poll)
        };
        if let Some(result) = write_result {
            match result {
                Ok(result) => self.as_mut().set_status_text(QString::from(format!(
                    "Configuration written to {} ({} bytes)",
                    result.path.display(),
                    result.bytes
                ))),
                Err(error) => self.as_mut().set_status_text(QString::from(format!(
                    "Configuration write failed: {error}"
                ))),
            }
        }
        let print_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .print_worker
                .as_mut()
                .and_then(PrintWorker::poll)
        };
        if let Some(result) = print_result {
            let (pending, private_temporary) = {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                (
                    this.pending_print.take(),
                    std::mem::take(&mut this.pending_print_private),
                )
            };
            match result {
                Ok(path) if pending.as_ref() == Some(&path) => {
                    cleanup_print_artifact(&path, private_temporary);
                    self.as_mut()
                        .set_status_text(QString::from("Print job submitted"));
                }
                Ok(path) => {
                    cleanup_print_artifact(&path, private_temporary);
                    self.as_mut()
                        .set_status_text(QString::from("This print request is no longer active. Check the print queue before trying again."));
                }
                Err(error) => {
                    if let Some(path) = pending {
                        cleanup_print_artifact(&path, private_temporary);
                    }
                    self.as_mut()
                        .set_status_text(QString::from(format!("Print failed: {error}")));
                }
            }
        }
        let theme_changed = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .configuration_io
                .theme_watch
                .changed()
        };
        if theme_changed {
            let config = self.as_ref().rust().config.clone();
            if self.as_mut().update_theme_palette(&config) {
                self.as_mut()
                    .set_status_text(QString::from("Theme palette reloaded"));
            }
        }
        let now = Instant::now();
        let mut scripts_changed = false;
        let should_reload = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.configuration_io.config_watch.changed() {
                // Script edits observed during a pending configuration reload
                // still require a fresh catalog. Consuming their signatures
                // without scheduling discovery can strand the initial catalog.
                scripts_changed = true;
                if this.configuration_io.config_watch.pending_since().is_none() {
                    this.configuration_io.config_watch.mark_pending(now);
                }
            }
            this.configuration_io
                .config_watch
                .pending_since()
                .is_some_and(|since| now.duration_since(since) >= Duration::from_millis(150))
        };
        if scripts_changed {
            // Keep the validated immutable snapshot until the application
            // publishes the refresh result. A filesystem notification after an
            // install must not temporarily erase its registered actions.
            self.as_mut().refresh_userscript_inventory();
        }
        let path_text = self.as_ref().rust().config_path.to_string();
        if path_text.is_empty() {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .configuration_io
                .config_watch
                .clear_pending();
            return;
        }
        if self.as_ref().rust().processes.pending_config_edit.is_some() {
            return;
        }
        let path = PathBuf::from(path_text);
        let reload_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .configuration_io
                .config_reload_worker
                .as_mut()
                .and_then(ConfigReloadWorker::poll)
        };
        if let Some(result) = reload_result {
            match result {
                Ok(result) if result.operation == ConfigReadOperation::Check => {
                    let source_count = result.loaded.sources.len();
                    if let Err(error) = self.as_mut().queue_config_check_validation(
                        result.loaded.config,
                        result.profiles_config,
                        source_count,
                    ) {
                        self.as_mut().set_status_text(QString::from(format!(
                            "Configuration selector check failed: {error}"
                        )));
                    }
                }
                Ok(result) => {
                    let announce = result.operation == ConfigReadOperation::Reload;
                    let new_base = match serde_json::to_value(&result.loaded.config) {
                        Ok(value) => value,
                        Err(error) => {
                            self.report_config_reload_failure(&error.to_string(), announce);
                            return;
                        }
                    };
                    let profile_overrides = result.profile_overrides;
                    let candidate = match self.as_ref().runtime_config_candidate_from(
                        &new_base,
                        &profile_overrides,
                        &self.as_ref().rust().runtime_overrides,
                        &self.as_ref().rust().temporary_overrides,
                    ) {
                        Ok(candidate) => candidate,
                        Err(error) => {
                            self.report_config_reload_failure(&error, announce);
                            return;
                        }
                    };
                    let base_json = match serde_json::to_string(&new_base) {
                        Ok(value) => value,
                        Err(error) => {
                            self.report_config_reload_failure(&error.to_string(), announce);
                            return;
                        }
                    };
                    let layers = ConfigurationLayers {
                        base: result.loaded.config.clone(),
                        profile: profile_overrides.clone(),
                        runtime: self.as_ref().rust().runtime_overrides.clone(),
                        command_line: self.as_ref().rust().cli_overrides.clone(),
                        temporary: self.as_ref().rust().temporary_overrides.clone(),
                    };
                    let validation_items = selector_validation_items(&layers);
                    let validation_fingerprint =
                        serde_json::to_string(&validation_items).unwrap_or_default();
                    let mut sources = result.loaded.sources;
                    if let Some(state) = self.as_ref().rust().state.as_ref() {
                        sources.extend(state.userscript_watch_sources());
                    }
                    let profiles_path = result.profiles_path;
                    let publication = super::ConfigReloadPublication {
                        candidate,
                        layers,
                        fingerprint: validation_fingerprint.clone(),
                        path: path.clone(),
                        base_json,
                        profile_overrides,
                        sources,
                        profiles_path,
                        contexts: result.contexts,
                        announce,
                    };
                    if !validation_items.is_empty()
                        && validation_fingerprint
                            != self
                                .as_ref()
                                .rust()
                                .configuration_io
                                .active_selector_fingerprint
                    {
                        self.as_mut().begin_selector_validation(
                            validation_fingerprint,
                            validation_items,
                            PendingSelectorAction::Reload {
                                publication: Box::new(publication),
                            },
                        );
                        return;
                    }
                    if let Err(error) = self
                        .as_mut()
                        .queue_context_configuration_reload(publication)
                    {
                        self.report_config_reload_failure(&error, announce);
                        return;
                    }
                }
                Err(error) if error.operation == Some(ConfigReadOperation::Check) => {
                    // An explicit check deliberately returns its diagnostic.
                    // Background reloads retain detail behind local disclosure.
                    self.as_mut().set_status_text(QString::from(format!(
                        "Configuration check failed: {}",
                        error.detail
                    )));
                }
                Err(error) => self.as_mut().report_config_reload_failure(
                    &error.detail,
                    error.operation == Some(ConfigReadOperation::Reload),
                ),
            }
        }
        if should_reload
            && self
                .as_ref()
                .rust()
                .configuration_io
                .pending_context_reload
                .is_none()
            && self
                .as_ref()
                .rust()
                .configuration_io
                .pending_selector_validation
                .is_none()
        {
            let announce = self
                .as_ref()
                .rust()
                .configuration_io
                .config_reload_requested_by_user;
            let profile_name = self.as_ref().rust().profile_name.clone();
            let request = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .configuration_io
                .config_reload_worker
                .as_mut()
                .and_then(|worker| {
                    (!worker.is_pending()).then(|| {
                        if announce {
                            worker.request(path, profile_name)
                        } else {
                            worker.request_watched(path, profile_name)
                        }
                    })
                });
            if request.is_some() {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.configuration_io.config_watch.clear_pending();
                this.configuration_io.config_reload_requested_by_user = false;
            }
            if let Some(Err(error)) = request {
                self.report_config_reload_failure(&error, announce);
            }
        }
    }

    pub(super) fn set_runtime_setting(
        mut self: Pin<&mut Self>,
        key: &QString,
        literal: &QString,
        temporary: bool,
    ) -> bool {
        if !self.as_mut().setting_request_ready() {
            return false;
        }
        let mut arguments = Vec::with_capacity(if temporary { 2 } else { 1 });
        if temporary {
            arguments.push("--temp".into());
        }
        arguments.push(format!("{key}={literal}"));
        let command = ParsedCommand {
            name: "set".into(),
            arguments,
        };
        let result = self.as_mut().execute_runtime_config_command(&command);
        self.finish_runtime_setting_request(result, key, temporary, false)
    }

    pub(super) fn unset_runtime_setting(
        mut self: Pin<&mut Self>,
        key: &QString,
        temporary: bool,
    ) -> bool {
        if !self.as_mut().setting_request_ready() {
            return false;
        }
        let arguments = if temporary {
            vec!["--temp".into(), key.to_string()]
        } else {
            vec![key.to_string()]
        };
        let command = ParsedCommand {
            name: "unset".into(),
            arguments,
        };
        let result = self.as_mut().execute_runtime_config_command(&command);
        self.finish_runtime_setting_request(result, key, temporary, true)
    }

    fn setting_request_ready(mut self: Pin<&mut Self>) -> bool {
        if self.as_ref().rust().setting_action_state.to_string() == "pending" {
            self.as_mut().set_runtime_setting_error(QString::from(
                "Another settings change is still pending. Wait for it to finish.",
            ));
            return false;
        }
        true
    }

    fn finish_runtime_setting_request(
        mut self: Pin<&mut Self>,
        result: Result<Value, String>,
        key: &QString,
        temporary: bool,
        reset: bool,
    ) -> bool {
        let scope = if temporary {
            "this window and session"
        } else {
            "all regular profiles in this setup"
        };
        let verb = if reset {
            "Resetting"
        } else if temporary {
            "Applying"
        } else {
            "Saving"
        };
        self.as_mut().publish_management_feedback(
            FeedbackLane::Setting,
            FeedbackState::Pending,
            &format!("{verb} {key} for {scope}…"),
            "",
        );
        let feedback = super::browser_ui_state::SettingFeedback {
            id: self.as_ref().rust().setting_action_id.clone(),
            key: key.to_string(),
            success: format!(
                "{} {key} for {scope}",
                if reset {
                    "Reset"
                } else if temporary {
                    "Applied"
                } else {
                    "Saved"
                }
            ),
            failure: format!(
                "Couldn't {} {key} for {scope}",
                if reset {
                    "reset"
                } else if temporary {
                    "apply"
                } else {
                    "save"
                }
            ),
        };
        match result {
            Ok(_) => {
                self.as_mut().set_runtime_setting_error(QString::default());
                self.as_mut().continue_setting_feedback(Some(feedback));
                true
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(error.clone()));
                self.as_mut()
                    .finish_setting_feedback(Some(feedback), Err(&error));
                false
            }
        }
    }

    pub(super) fn continue_setting_feedback(
        mut self: Pin<&mut Self>,
        feedback: Option<super::browser_ui_state::SettingFeedback>,
    ) {
        let Some(feedback) = feedback else { return };
        let synchronous = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if let Some(pending) = this
                .configuration_io
                .pending_configuration_publication
                .as_mut()
            {
                pending.setting_feedback = Some(feedback);
                None
            } else if let Some(pending) = this.configuration_io.pending_selector_validation.as_mut()
            {
                pending.setting_feedback = Some(feedback);
                None
            } else {
                Some(feedback)
            }
        };
        self.finish_setting_feedback(synchronous, Ok(()));
    }

    pub(super) fn finish_setting_feedback(
        mut self: Pin<&mut Self>,
        feedback: Option<super::browser_ui_state::SettingFeedback>,
        result: Result<(), &str>,
    ) {
        let Some(feedback) = feedback else { return };
        if self.as_ref().rust().setting_action_id != feedback.id
            || self.as_ref().rust().setting_action_state.to_string() != "pending"
        {
            return;
        }
        match result {
            Ok(()) => {
                let apply_time = self.as_ref().rust().state.as_ref().and_then(|state| {
                    state
                        .configuration_snapshot()
                        .pending_changes
                        .into_iter()
                        .find(|change| change.key == feedback.key)
                        .map(|change| change.apply_time)
                });
                let uncertain =
                    self.as_ref().rust().runtime_config_persistence == "durability-uncertain";
                let detail = if uncertain {
                    self.as_ref().rust().status_text.to_string()
                } else {
                    String::new()
                };
                let mut message = if uncertain {
                    "Saved, but a system crash could lose the change.".to_owned()
                } else {
                    feedback.success
                };
                match apply_time {
                    Some("reload") => message.push_str("; use :config-reload to apply"),
                    Some("startup" | "restart") => message.push_str("; restart Ferric to apply"),
                    _ => {}
                }
                self.publish_management_feedback(
                    FeedbackLane::Setting,
                    FeedbackState::Succeeded,
                    &message,
                    &detail,
                );
            }
            Err(error) => {
                self.as_mut()
                    .set_runtime_setting_error(QString::from(error));
                self.publish_management_feedback(
                    FeedbackLane::Setting,
                    FeedbackState::Failed,
                    &feedback.failure,
                    error,
                );
            }
        }
    }

    pub(super) fn clear_site_experiment_presentation(mut self: Pin<&mut Self>) {
        self.as_mut().set_site_experiment_active(false);
        self.as_mut().set_site_experiment_id(QString::default());
        self.as_mut().set_site_experiment_kind(QString::default());
        self.as_mut().set_site_experiment_url(QString::default());
        self.as_mut().set_site_experiment_remaining_seconds(0);
    }

    pub(super) fn publish_site_experiment_presentation(
        mut self: Pin<&mut Self>,
        experiment: &SiteExperiment,
        remaining_seconds: u64,
    ) {
        self.as_mut().set_site_experiment_active(true);
        self.as_mut()
            .set_site_experiment_id(QString::from(&experiment.id));
        self.as_mut()
            .set_site_experiment_kind(QString::from(&experiment.kind));
        self.as_mut()
            .set_site_experiment_url(QString::from(&experiment.url));
        self.as_mut()
            .set_site_experiment_remaining_seconds(i64::try_from(remaining_seconds).unwrap_or(0));
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_runtime_binding_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let (mode_name, keychain, binding_command) = match command.name.as_str() {
            "bind" => {
                let (mode_name, keychain, command_args) = match command.arguments.as_slice() {
                    [keychain, command_args @ ..] if !command_args.is_empty() => {
                        ("normal", keychain.as_str(), command_args)
                    }
                    [flag, mode, keychain, command_args @ ..]
                        if flag == "--mode" && !command_args.is_empty() =>
                    {
                        (mode.as_str(), keychain.as_str(), command_args)
                    }
                    _ => return Err("bind accepts [--mode MODE] KEYCHAIN COMMAND".into()),
                };
                let binding_command = command_args.join(" ");
                (mode_name, keychain, Some(binding_command))
            }
            "unbind" => {
                let (mode_name, keychain) = match command.arguments.as_slice() {
                    [keychain] => ("normal", keychain.as_str()),
                    [flag, mode, keychain] if flag == "--mode" => {
                        (mode.as_str(), keychain.as_str())
                    }
                    _ => return Err("unbind accepts [--mode MODE] KEYCHAIN".into()),
                };
                (mode_name, keychain, None)
            }
            _ => return Err("not a runtime binding command".into()),
        };
        let mode = parse_ipc_mode(mode_name)?;
        if keychain.is_empty() || keychain.len() > 128 || keychain.chars().any(char::is_control) {
            return Err("binding keychain is empty, too long, or unsafe".into());
        }
        let parsed_binding = if let Some(binding_command) = binding_command.as_deref() {
            if binding_command.is_empty()
                || binding_command.len() > 256
                || binding_command.chars().any(char::is_control)
            {
                return Err("binding command is empty, too long, or unsafe".into());
            }
            let mut parsed = parse_chain(binding_command, ParseInput::Interactive)
                .map_err(|error| format!("binding command is invalid: {error}"))?;
            if parsed.len() != 1 {
                return Err("binding command must contain exactly one command".into());
            }
            let parsed = parsed.remove(0);
            self.as_ref()
                .rust()
                .registry
                .validate(&parsed, mode)
                .map_err(|error| format!("binding command is invalid: {error}"))?;
            Some(parsed)
        } else {
            None
        };

        let mut persistent = self.as_ref().rust().runtime_overrides.clone();
        persistent
            .set_binding(
                mode_name,
                keychain,
                binding_command.as_deref().unwrap_or_default(),
            )
            .map_err(|error| error.to_string())?;
        let temporary_layer = self.as_ref().rust().temporary_overrides.clone();
        let candidate = self
            .as_ref()
            .runtime_config_candidate(&persistent, &temporary_layer)?;
        if configured_bindings(&self.as_ref().rust().registry, &candidate).is_none() {
            return Err("effective binding configuration is invalid".into());
        }
        let persistent_storage = self.as_ref().rust().profile_persistence.is_durable()
            && self.as_ref().rust().storage_roots.is_some();
        let persist_path = if persistent_storage {
            Some(
                self.as_ref()
                    .rust()
                    .storage_roots
                    .as_ref()
                    .map(|roots| roots.state.join("runtime-overrides.toml"))
                    .ok_or_else(|| "runtime storage is unavailable".to_owned())?,
            )
        } else {
            None
        };
        let layers = ConfigurationLayers {
            base: self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .ok_or("base configuration is unavailable")?
                .configuration_layers()
                .base
                .clone(),
            profile: self.as_ref().rust().profile_overrides.clone(),
            runtime: persistent.clone(),
            command_line: self.as_ref().rust().cli_overrides.clone(),
            temporary: temporary_layer,
        };
        let status = if binding_command.is_some() {
            "Binding saved".to_owned()
        } else {
            "Binding removed".to_owned()
        };
        let Some(result) = self.as_mut().validate_or_queue_runtime_config(
            candidate,
            layers,
            status,
            persistent.clone(),
            persist_path,
        )?
        else {
            return Ok(serde_json::json!({
                "status": "pending-validation",
                "mode": mode_name,
                "keychain": keychain,
                "command": binding_command,
                "unbound": parsed_binding.is_none(),
                "scope": if persistent_storage { "persistent" } else { "memory-only" }
            }));
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .runtime_overrides = persistent;
        Ok(serde_json::json!({
            "status": "accepted",
            "persistence": self.as_ref().rust().runtime_config_persistence,
            "mode": mode_name,
            "keychain": keychain,
            "command": binding_command,
            "unbound": parsed_binding.is_none(),
            "scope": if persistent_storage { "persistent" } else { "memory-only" },
            "effective": result.get("bindings").is_some()
        }))
    }
}
