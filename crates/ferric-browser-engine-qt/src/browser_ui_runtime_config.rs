use super::{
    ConfigReadOperation, ConfigReloadWorker, ConfigWriteWorker, ConfigurationLayers, CxxQtType,
    Duration, HyprlandResponse, HyprlandWorker, Instant, NetworkPolicyWorker, ParseInput,
    ParsedCommand, PathBuf, Pin, PortalProbeWorker, PrintWorker, ProfileDeleteWorker,
    ProfileListWorker, ProfileMutationResult, ProfilePreviewWorker, QString,
    ReducedMotionProbeWorker, SiteExperiment, SystemFontScaleProbeWorker, UserscriptManagerResult,
    UserscriptManagerWorker, Value, cleanup_print_artifact, configured_bindings, desktop_portals,
    desktop_preferences, parse_chain, parse_ipc_mode, profile_from_list_values, qobject,
    safe_ipc_url, save_runtime_overrides_atomic,
};

impl qobject::BrowserUi {
    pub(super) fn poll_config(mut self: Pin<&mut Self>) {
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
                self.as_mut()
                    .finish_configure_profile(pending, result.map_err(|error| error.to_string()));
            } else {
                self.as_mut()
                    .set_status_text(QString::from("Profile bootstrap result was stale"));
            }
        }
        let network_policy_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
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
                .network_policy_reload_pending;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .network_policy_reload_pending = false;
            match result {
                Ok(policy) => self.as_mut().apply_network_policy_snapshot(policy),
                Err(error) => self.as_mut().set_status_text(QString::from(format!(
                    "Network policy unavailable: {error}"
                ))),
            }
            if reload_requested {
                self.as_mut()
                    .set_status_text(QString::from("Blocklist policy reloaded"));
            }
            self.as_mut().set_profile_bootstrap_pending(false);
        }
        let userscript_manager_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .userscript_manager_worker
                .as_mut()
                .and_then(UserscriptManagerWorker::poll)
        };
        if let Some(result) = userscript_manager_result {
            match result {
                UserscriptManagerResult::Inventory(result) => match result {
                    Ok(scripts) => {
                        self.as_mut().set_userscript_inventory_rows(&scripts);
                    }
                    Err(error) => {
                        self.as_mut().clear_userscript_inventory_rows();
                        self.as_mut().set_status_text(QString::from(format!(
                            "Userscript inventory unavailable: {error}"
                        )));
                    }
                },
                UserscriptManagerResult::Installed(result) => match result {
                    Ok(scripts) => {
                        self.as_mut().set_userscript_inventory_rows(&scripts);
                        self.as_mut()
                            .set_status_text(QString::from("Userscript installed"));
                        self.as_mut()
                            .set_userscript_install_state(QString::from("installed"));
                    }
                    Err(error) => {
                        self.as_mut().set_status_text(QString::from(format!(
                            "Userscript installation failed: {error}"
                        )));
                        self.as_mut()
                            .set_userscript_install_state(QString::from(format!("error:{error}")));
                    }
                },
                UserscriptManagerResult::Removed(result) => match result {
                    Ok(scripts) => {
                        self.as_mut().set_userscript_inventory_rows(&scripts);
                        self.as_mut()
                            .set_status_text(QString::from("Userscript removed"));
                        self.as_mut()
                            .set_userscript_install_state(QString::from("removed"));
                    }
                    Err(error) => {
                        self.as_mut().set_status_text(QString::from(format!(
                            "Userscript removal failed: {error}"
                        )));
                        self.as_mut()
                            .set_userscript_install_state(QString::from(format!("error:{error}")));
                    }
                },
                UserscriptManagerResult::SetEnabled { enabled, result } => match result {
                    Ok(scripts) => {
                        self.as_mut().set_userscript_inventory_rows(&scripts);
                        self.as_mut().set_status_text(QString::from(if enabled {
                            "Userscript enabled; it applies on the next navigation"
                        } else {
                            "Userscript disabled; it applies on the next navigation"
                        }));
                    }
                    Err(error) => self.as_mut().set_status_text(QString::from(format!(
                        "Userscript change rejected: {error}"
                    ))),
                },
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
                                .pending_engine_action = Some(format!(
                                "new-window\t{private}\t{profile_name}\tabout:blank"
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
                    self.as_mut().set_profile_values(QString::default());
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
                Ok(preview) => self
                    .as_mut()
                    .set_profile_preview_text(QString::from(preview)),
                Err(error) => {
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
                Ok(ProfileMutationResult::Deleted(outcome)) => {
                    let suffix = if outcome.cleanup_pending {
                        " (data cleanup pending; it will resume on next startup)"
                    } else {
                        ""
                    };
                    self.as_mut().set_status_text(QString::from(format!(
                        "Profile deleted: {}{suffix}. QtWebEngine storage was not deleted.",
                        outcome.profile_name
                    )));
                }
                Ok(ProfileMutationResult::Created) => self
                    .as_mut()
                    .set_status_text(QString::from("Profile created")),
                Ok(ProfileMutationResult::Renamed) => self
                    .as_mut()
                    .set_status_text(QString::from("Profile label renamed")),
                Err(error) => self
                    .as_mut()
                    .set_status_text(QString::from(format!("Profile operation failed: {error}"))),
            }
        }
        self.as_mut().poll_config_edit();
        self.as_mut().poll_editor_write();
        let write_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
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
                        .set_status_text(QString::from("Print result became stale"));
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
            rust.as_mut().get_mut().theme_watch.changed()
        };
        if theme_changed {
            let config = self.as_ref().rust().config.clone();
            if self.as_mut().update_theme_palette(&config) {
                self.as_mut()
                    .set_status_text(QString::from("Theme palette reloaded"));
            }
        }
        let path_text = self.as_ref().rust().config_path.to_string();
        if path_text.is_empty() {
            return;
        }
        if self.as_ref().rust().pending_config_edit.is_some() {
            return;
        }
        let path = PathBuf::from(path_text);
        let now = Instant::now();
        let should_reload = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.config_watch.changed() && this.config_watch.pending_since().is_none() {
                this.config_watch.mark_pending(now);
            }
            let ready = this
                .config_watch
                .pending_since()
                .is_some_and(|since| now.duration_since(since) >= Duration::from_millis(150));
            if ready {
                this.config_watch.clear_pending();
            }
            ready
        };
        let reload_result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .config_reload_worker
                .as_mut()
                .and_then(ConfigReloadWorker::poll)
        };
        if let Some(result) = reload_result {
            match result {
                Ok(result) if result.operation == ConfigReadOperation::Check => {
                    let source_count = result.loaded.sources.len();
                    self.as_mut().set_status_text(QString::from(format!(
                        "Configuration valid ({} source file{})",
                        source_count,
                        if source_count == 1 { "" } else { "s" }
                    )));
                }
                Ok(result) => {
                    let new_base = match serde_json::to_value(&result.loaded.config) {
                        Ok(value) => value,
                        Err(error) => {
                            self.set_status_text(QString::from(format!(
                                "Configuration reload rejected: {error}"
                            )));
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
                            self.set_status_text(QString::from(format!(
                                "Configuration reload rejected: {error}"
                            )));
                            return;
                        }
                    };
                    let base_json = match serde_json::to_string(&new_base) {
                        Ok(value) => value,
                        Err(error) => {
                            self.set_status_text(QString::from(format!(
                                "Configuration reload rejected: {error}"
                            )));
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
                    if let Err(error) = self.as_mut().commit_runtime_config(
                        candidate,
                        layers,
                        "Configuration reloaded",
                    ) {
                        self.set_status_text(QString::from(format!(
                            "Configuration reload rejected: {error}"
                        )));
                        return;
                    }
                    {
                        let mut rust = self.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        this.base_config = new_base;
                        this.profile_overrides = profile_overrides.clone();
                        let mut sources = result.loaded.sources;
                        if let Some(profiles_path) = result.profiles_path {
                            sources.push(profiles_path);
                        }
                        this.config_watch.set_sources(&path, &sources);
                    }
                    self.as_mut().set_config_base_json(QString::from(base_json));
                    self.as_mut().set_profile_overrides_json(QString::from(
                        serde_json::to_string(&profile_overrides).unwrap_or_else(|_| "{}".into()),
                    ));
                }
                Err(error) => self.as_mut().set_status_text(QString::from(format!(
                    "Configuration reload rejected: {error}"
                ))),
            }
        }
        if should_reload {
            let profile_name = self.as_ref().rust().profile_name.clone();
            let request = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .config_reload_worker
                .as_mut()
                .map(|worker| worker.request(path, profile_name));
            if matches!(request.as_ref(), Some(Ok(()))) {
                self.as_mut()
                    .set_status_text(QString::from("Configuration reload queued"));
            }
            if let Some(Err(error)) = request {
                self.set_status_text(QString::from(format!(
                    "Configuration reload unavailable: {error}"
                )));
            }
        }
    }

    pub(super) fn set_runtime_setting(
        mut self: Pin<&mut Self>,
        key: &QString,
        literal: &QString,
        temporary: bool,
    ) -> bool {
        let mut arguments = Vec::with_capacity(if temporary { 2 } else { 1 });
        if temporary {
            arguments.push("--temp".into());
        }
        arguments.push(format!("{key}={literal}"));
        let command = ParsedCommand {
            name: "set".into(),
            arguments,
        };
        match self.as_mut().execute_runtime_config_command(&command) {
            Ok(_) => {
                self.as_mut().set_runtime_setting_error(QString::default());
                true
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(error.clone()));
                self.as_mut()
                    .set_runtime_setting_error(QString::from(error));
                false
            }
        }
    }

    pub(super) fn unset_runtime_setting(
        mut self: Pin<&mut Self>,
        key: &QString,
        temporary: bool,
    ) -> bool {
        let arguments = if temporary {
            vec!["--temp".into(), key.to_string()]
        } else {
            vec![key.to_string()]
        };
        let command = ParsedCommand {
            name: "unset".into(),
            arguments,
        };
        match self.as_mut().execute_runtime_config_command(&command) {
            Ok(_) => {
                self.as_mut().set_runtime_setting_error(QString::default());
                true
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(error.clone()));
                self.as_mut()
                    .set_runtime_setting_error(QString::from(error));
                false
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
        if persistent_storage {
            let path = self
                .as_ref()
                .rust()
                .storage_roots
                .as_ref()
                .map(|roots| roots.state.join("runtime-overrides.toml"))
                .ok_or_else(|| "runtime storage is unavailable".to_owned())?;
            save_runtime_overrides_atomic(&path, &persistent)
                .map_err(|error| format!("could not save runtime overrides: {error}"))?;
        }
        let layers = ConfigurationLayers {
            base: serde_json::from_value(self.as_ref().rust().base_config.clone())
                .map_err(|error| format!("base configuration is invalid: {error}"))?,
            profile: self.as_ref().rust().profile_overrides.clone(),
            runtime: persistent.clone(),
            command_line: self.as_ref().rust().cli_overrides.clone(),
            temporary: temporary_layer,
        };
        let result = self.as_mut().commit_runtime_config(
            candidate,
            layers,
            if binding_command.is_some() {
                "Binding saved"
            } else {
                "Binding removed"
            },
        )?;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .runtime_overrides = persistent;
        Ok(serde_json::json!({
            "status": "accepted",
            "mode": mode_name,
            "keychain": keychain,
            "command": binding_command,
            "unbound": parsed_binding.is_none(),
            "scope": if persistent_storage { "persistent" } else { "memory-only" },
            "effective": result.get("bindings").is_some()
        }))
    }
}
