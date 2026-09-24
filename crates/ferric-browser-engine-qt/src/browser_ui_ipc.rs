//! IPC query, routing, command execution, and response projection for `BrowserUi`.

use super::{
    ActionRegistry, CxxQtType, DispatchTarget, Effect, EngineEffect, Event, IpcOpenTarget,
    IpcRoute, ParsedCommand, PendingLinkNavigation, Pin, PublicError, QString, Request, Response,
    RuntimeDispatch, Uuid, ValidatedUrl, Value, action_failure_category,
    binding_command_parameters, clean_link, clean_open_input, current_target, ipc_action_failure,
    ipc_action_failure_with_context, ipc_command_context, ipc_command_count,
    ipc_command_failure_with_context, ipc_command_invocation, ipc_failure, ipc_mode_name,
    is_context_command, is_library_command, is_link_clean_command, is_mutating_ipc_method,
    is_reopen_in_window_command, is_scroll_command, is_search_next_command, is_tab_clone_command,
    is_tab_detach_command, is_tab_give_command, is_tab_undo_command, is_yank_command,
    is_zoom_command, learning_mode_request, link_cleaning_policy, link_result_value,
    normalize_active_tab_command, parse_action_invocation, parse_external_action_id,
    parse_hint_options, parse_switcher_command, qobject, resolve_input, selection_yank_command,
    typed_ipc_action, typed_ipc_command, userscript, userscript_action_argument_name,
    userscript_action_is_hint_only,
};

impl qobject::BrowserUi {
    pub(super) fn validate_ipc_route(&self, route: &IpcRoute) -> Result<(), String> {
        let state = self
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| "core state unavailable".to_owned())?;
        let window = match route.selector {
            DispatchTarget::Active => state.active_window(),
            DispatchTarget::LastFocused => state.last_focused_window().or(state.active_window()),
            DispatchTarget::Window(window) => Some(window),
            DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
        }
        .and_then(|window| state.windows().get(&window).map(|_| window))
        .ok_or_else(|| "requested window is not available".to_owned())?;
        if let Some(context_name) = route.context.as_deref() {
            let context = self
                .rust()
                .contexts
                .as_ref()
                .and_then(|contexts| {
                    contexts
                        .contexts()
                        .iter()
                        .find(|context| context.name == context_name)
                })
                .ok_or_else(|| format!("context not found: {context_name}"))?;
            if route.open_target == IpcOpenTarget::PrivateWindow {
                return Err("private-window cannot use a durable context".into());
            }
            if !matches!(route.open_target, IpcOpenTarget::Window)
                && context.profile != self.rust().profile_name
            {
                return Err("context belongs to a different profile".into());
            }
            if route
                .profile
                .as_deref()
                .is_some_and(|profile| profile != context.profile)
            {
                return Err("profile and context selectors disagree".into());
            }
        }
        if let Some(profile) = route.profile.as_deref() {
            if !matches!(
                route.open_target,
                IpcOpenTarget::Window | IpcOpenTarget::PrivateWindow
            ) {
                let requested = state
                    .profiles()
                    .values()
                    .find(|candidate| candidate.label == profile)
                    .ok_or_else(|| format!("profile not found: {profile}"))?;
                if requested.id != state.windows()[&window].profile {
                    return Err("the requested profile is not an active GUI profile".into());
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_clean_open_ipc(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        let Some(input) = clean_open_input(command)? else {
            return Err("not a clean-link open command".into());
        };
        let navigation = resolve_input(
            &input,
            &self.as_ref().navigation_context_for_source(route.source),
        )
        .map_err(|error| error.to_string())?;
        let rules = link_cleaning_policy::active_rules(self.as_ref().rust().storage_roots.as_ref());
        let result =
            clean_link(navigation.url.as_str(), &rules).map_err(|error| error.to_string())?;
        if !result.changed {
            return self.execute_ipc_command_with_operation(
                ParsedCommand {
                    name: "open".into(),
                    arguments: vec![navigation.url.to_string()],
                },
                route,
                operation_id,
            );
        }
        let cleaned = ValidatedUrl::parse(result.cleaned.clone())
            .map_err(|error| format!("cleaned URL is invalid: {error}"))?;
        let mut value = link_result_value("open", &result, &rules);
        value["requires_confirmation"] = Value::Bool(true);
        let pending = match route.open_target {
            IpcOpenTarget::Window | IpcOpenTarget::PrivateWindow => {
                let private = route.open_target == IpcOpenTarget::PrivateWindow;
                if private && route.profile.is_some() {
                    return Err("private-window cannot select a durable profile".into());
                }
                let context_profile = route.context.as_deref().and_then(|context_name| {
                    self.as_ref()
                        .rust()
                        .contexts
                        .as_ref()?
                        .contexts()
                        .iter()
                        .find(|context| context.name == context_name)
                        .map(|context| context.profile.clone())
                });
                let profile = route
                    .profile
                    .clone()
                    .or(context_profile)
                    .unwrap_or_else(|| {
                        if private {
                            "private".into()
                        } else {
                            "secondary".into()
                        }
                    });
                let new_window_action = route.context.as_deref().map_or_else(
                    || format!("new-window\t{}\t{}\t{}", private, profile, cleaned.as_str()),
                    |context| {
                        format!(
                            "new-window\t{}\t{}\t{}\t{}",
                            private,
                            profile,
                            context,
                            cleaned.as_str()
                        )
                    },
                );
                PendingLinkNavigation {
                    target: None,
                    new_window_action: Some(new_window_action),
                    journey_parent: None,
                    url: cleaned,
                }
            }
            IpcOpenTarget::BackgroundTab => {
                let (window, original_window, original_tab, journey_parent) = {
                    let binding = self.as_ref();
                    let state = binding
                        .rust()
                        .state
                        .as_ref()
                        .ok_or_else(|| "core state unavailable".to_owned())?;
                    let window = match route.selector {
                        DispatchTarget::Active => state.active_window(),
                        DispatchTarget::LastFocused => {
                            state.last_focused_window().or(state.active_window())
                        }
                        DispatchTarget::Window(window) => Some(window),
                        DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
                    }
                    .ok_or_else(|| "requested window is not available".to_owned())?;
                    let source_tab = match route.selector {
                        DispatchTarget::Tab(tab) => Some(tab),
                        _ => state
                            .windows()
                            .get(&window)
                            .and_then(|window| window.active_tab),
                    };
                    let journey_parent =
                        source_tab.and_then(|tab| state.journey().current_node(tab));
                    (
                        window,
                        state.active_window(),
                        state.windows()[&window].active_tab,
                        journey_parent,
                    )
                };
                self.as_mut()
                    .reduce_event(Event::OpenTab { window })
                    .map_err(|error| error.clone())?;
                let tab = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.windows().get(&window))
                    .and_then(|window| window.active_tab)
                    .ok_or_else(|| "new background tab was not created".to_owned())?;
                if let Some(original_tab) = original_tab {
                    self.as_mut()
                        .reduce_event(Event::ActivateTab {
                            window,
                            tab: original_tab,
                        })
                        .map_err(|error| error.clone())?;
                } else if let Some(original_window) = original_window
                    && original_window != window
                {
                    self.as_mut()
                        .reduce_event(Event::FocusWindow {
                            window: original_window,
                        })
                        .map_err(|error| error.clone())?;
                }
                value["tab_id"] = Value::String(tab.to_string());
                let target = current_target(self.as_ref().rust().state.as_ref(), Some(tab))
                    .ok_or_else(|| "new background tab target is not live".to_owned())?;
                PendingLinkNavigation {
                    target: Some(target),
                    new_window_action: None,
                    journey_parent,
                    url: cleaned,
                }
            }
            IpcOpenTarget::Tab => {
                let binding = self.as_ref();
                let state = binding
                    .rust()
                    .state
                    .as_ref()
                    .ok_or_else(|| "core state unavailable".to_owned())?;
                let window = match route.selector {
                    DispatchTarget::Active => state.active_window(),
                    DispatchTarget::LastFocused => {
                        state.last_focused_window().or(state.active_window())
                    }
                    DispatchTarget::Window(window) => Some(window),
                    DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
                }
                .ok_or_else(|| "requested window is not available".to_owned())?;
                let target = state
                    .windows()
                    .get(&window)
                    .and_then(|window| window.active_tab)
                    .and_then(|tab| current_target(Some(state), Some(tab)))
                    .ok_or_else(|| "requested tab is not live".to_owned())?;
                PendingLinkNavigation {
                    target: Some(target),
                    new_window_action: None,
                    journey_parent: None,
                    url: cleaned,
                }
            }
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = None;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_link_navigation = Some(pending);
        self.as_mut().publish_link_preview("open", &result, true);
        self.as_mut().set_link_preview_visible(true);
        self.as_mut().sync_core_tabs();
        self.set_status_text(QString::from("Clean-link navigation awaits confirmation"));
        Ok(value)
    }

    pub(super) fn assign_ipc_context(
        mut self: Pin<&mut Self>,
        route: &IpcRoute,
    ) -> Result<(), String> {
        let Some(context_name) = route.context.as_deref() else {
            return Ok(());
        };
        let window = {
            let binding = self.as_ref();
            let state = binding
                .rust()
                .state
                .as_ref()
                .ok_or_else(|| "core state unavailable".to_owned())?;
            match route.selector {
                DispatchTarget::Active => state.active_window(),
                DispatchTarget::LastFocused => {
                    state.last_focused_window().or(state.active_window())
                }
                DispatchTarget::Window(window) => Some(window),
                DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
            }
            .ok_or_else(|| "requested window is not available".to_owned())?
        };
        self.as_mut()
            .reduce_event(Event::SetWindowContext {
                window,
                context: Some(context_name.to_owned()),
            })
            .map_err(|error| error.clone())?;
        self.as_mut().sync_core_tabs();
        Ok(())
    }

    pub(super) fn execute_switcher_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let (scope, query) = parse_switcher_command(&command.arguments)?;
        self.as_mut()
            .set_switcher_request_scope(QString::from(&scope));
        self.as_mut()
            .set_switcher_request_query(QString::from(&query));
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some("show-switcher".into());
        self.as_mut()
            .set_status_text(QString::from("Universal switcher requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "scope": scope,
            "query": query
        }))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_ipc_command(
        self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
    ) -> Result<Value, String> {
        self.execute_ipc_command_with_operation(command, route, None)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_ipc_command_with_operation(
        mut self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        let expanded = self
            .as_ref()
            .rust()
            .registry
            .expand_command(command)
            .map_err(|error| error.to_string())?;
        if expanded.len() != 1 {
            let mut result = None;
            for command in expanded {
                result = Some(self.as_mut().execute_ipc_command_with_operation(
                    command,
                    route,
                    operation_id,
                )?);
            }
            return Ok(result.unwrap_or_else(|| serde_json::json!({"status": "accepted"})));
        }
        let mut command = expanded.into_iter().next().expect("one expanded command");
        if command.name == "open-current" {
            if route.context.is_some() {
                return Err("context routing is not valid for open-current".into());
            }
            let target = match command.arguments.as_slice() {
                [] => "open",
                [flag, target] if flag == "--target" && target == "tab" => "tab-open",
                _ => return Err("open-current accepts only --target tab".into()),
            };
            let current_url = self.as_ref().rust().current_url.to_string();
            if current_url.is_empty() {
                return Err("open-current requires a committed URL".into());
            }
            return self.as_mut().execute_ipc_command_with_operation(
                ParsedCommand {
                    name: target.into(),
                    arguments: vec![current_url],
                },
                route,
                operation_id,
            );
        }
        if matches!(command.name.as_str(), "quit" | "window-close") {
            if route.context.is_some() || !command.arguments.is_empty() {
                return Err(format!(
                    "{} does not accept routing or arguments",
                    command.name
                ));
            }
            self.as_ref().validate_ipc_route(route)?;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(if command.name == "quit" {
                "quit-request".into()
            } else {
                "window-close-request".into()
            });
            self.as_mut()
                .set_status_text(QString::from(if command.name == "quit" {
                    "Shutdown confirmation requested"
                } else {
                    "Window close confirmation requested"
                }));
            return Ok(if command.name == "quit" {
                serde_json::json!({"status": "accepted", "shutdown": "pending"})
            } else {
                serde_json::json!({"status": "accepted", "close": "pending"})
            });
        }
        if matches!(
            command.name.as_str(),
            "config-edit"
                | "config-reload"
                | "config-check"
                | "config-write-defaults"
                | "theme-reload"
        ) {
            if route.context.is_some() {
                return Err(format!("context routing is not valid for {}", command.name));
            }
            self.as_ref().validate_ipc_route(route)?;
            return match command.name.as_str() {
                "config-edit" => self.execute_config_edit_command(&command),
                "config-reload" => self.execute_config_reload_command(&command),
                "config-check" => self.execute_config_check_command(&command),
                "config-write-defaults" => self.execute_config_write_defaults_command(&command),
                "theme-reload" => self.execute_theme_reload_command(&command),
                _ => unreachable!("configuration command was matched above"),
            };
        }
        if command.name == "get" {
            if route.context.is_some() {
                return Err("context routing is not valid for configuration queries".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_config_get_command(&command);
        }
        if matches!(command.name.as_str(), "help" | "version" | "diagnostics") {
            if route.context.is_some() {
                return Err(format!("context routing is not valid for {}", command.name));
            }
            self.as_ref().validate_ipc_route(route)?;
            return match command.name.as_str() {
                "help" => self.as_mut().execute_help_command(&command),
                "version" => self.as_mut().execute_version_command(&command),
                "diagnostics" => self.as_mut().execute_diagnostics_command(&command),
                _ => unreachable!("support command was matched above"),
            };
        }
        if command.name == "config-export" {
            if route.context.is_some() {
                return Err("context routing is not valid for configuration export".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_config_export_command(&command);
        }
        if command.name == "learning-mode" {
            if route.context.is_some() {
                return Err("context routing is not valid for learning mode".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            let current = self.as_ref().rust().learning_mode;
            let enabled = learning_mode_request(&command, current)?.unwrap_or(current);
            self.as_mut().set_learning_mode(enabled);
            self.as_mut().set_status_text(QString::from(if enabled {
                "Learning mode enabled"
            } else {
                "Learning mode disabled"
            }));
            return Ok(serde_json::json!({
                "status": "accepted",
                "learning_mode": enabled,
                "per_window": true
            }));
        }
        if matches!(command.name.as_str(), "binding-list" | "binding-explain") {
            if route.context.is_some() {
                return Err("context routing is not valid for binding queries".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            let (keychain, mode) = binding_command_parameters(&command)?;
            let mode_value = mode
                .as_deref()
                .map_or(Value::Null, |mode| Value::String(mode.to_owned()));
            let result = if let Some(keychain) = keychain.as_deref() {
                self.as_ref().ipc_bindings_explain(&serde_json::json!({
                    "keychain": keychain,
                    "mode": mode_value
                }))?
            } else {
                self.as_ref()
                    .ipc_bindings_query(&serde_json::json!({"mode": mode_value}))?
            };
            let search = keychain.or(mode);
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(search.map_or_else(
                || "show-binding-help".into(),
                |search| format!("show-binding-help\t{search}"),
            ));
            self.as_mut()
                .set_status_text(QString::from(if command.name == "binding-list" {
                    "Binding list requested"
                } else {
                    "Binding explanation requested"
                }));
            return Ok(result);
        }
        if matches!(command.name.as_str(), "set" | "unset" | "bind" | "unbind") {
            if route.context.is_some() {
                return Err("context routing is not valid for configuration commands".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return if matches!(command.name.as_str(), "bind" | "unbind") {
                self.execute_runtime_binding_command(&command)
            } else {
                self.execute_runtime_config_command(&command)
            };
        }
        if matches!(
            command.name.as_str(),
            "repeat" | "cancel" | "macro-record" | "macro-stop" | "macro-play"
        ) {
            if route.context.is_some() {
                return Err("context routing is not valid for input workflow commands".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            let accepted = self.as_mut().execute_parsed_commands(vec![command]);
            if accepted {
                return Ok(serde_json::json!({"status": "accepted"}));
            }
            return Err(self.as_ref().rust().status_text.to_string());
        }
        if command.name == "window-new" {
            if route.context.is_some() {
                return Err("context routing is not valid for window creation".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_window_new_command(&command);
        }
        if is_context_command(&command.name) {
            if route.context.is_some() || route.profile.is_some() {
                return Err("context routing is not valid for context lifecycle commands".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_context_command(&command);
        }
        if command.name == "session-load" {
            if route.context.is_some() || route.profile.is_some() {
                return Err("context or profile routing is not valid for session loading".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_session_load_command(&command);
        }
        if command.name == "blocking-status" {
            if route.context.is_some() {
                return Err("context routing is not valid for blocking status".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            if !command.arguments.is_empty() {
                return Err("blocking-status does not accept arguments".into());
            }
            let status = self.as_ref().blocking_status_message();
            self.as_mut().set_status_text(QString::from(status));
            return self.as_ref().ipc_blocking_status(&Value::Null);
        }
        if command.name == "site-status" {
            if route.context.is_some() {
                return Err("context routing is not valid for site status".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            let params = match command.arguments.as_slice() {
                [] => Value::Null,
                [flag, id] if flag == "--tab" => {
                    serde_json::json!({"tab": id})
                }
                _ => return Err("site-status accepts optional --tab TAB_ID".into()),
            };
            return self.as_ref().ipc_site_status(&params);
        }
        if command.name == "site-data-clear" {
            if route.context.is_some() {
                return Err("context routing is not valid for site-data clearing".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_site_data_clear_command(&command);
        }
        if command.name == "site-doctor" {
            if route.context.is_some() {
                return Err("context routing is not valid for Site Doctor".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            let [kind] = command.arguments.as_slice() else {
                return Err("site-doctor requires one experiment name".into());
            };
            let payload = self
                .as_mut()
                .begin_site_doctor_experiment(&QString::from(kind.as_str()))
                .to_string();
            if payload.is_empty() {
                return Err(self.as_ref().rust().status_text.to_string());
            }
            return serde_json::from_str(&payload).map_err(|error| error.to_string());
        }
        if command.name == "site-doctor-undo" {
            if route.context.is_some() {
                return Err("context routing is not valid for Site Doctor".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            let [experiment_id] = command.arguments.as_slice() else {
                return Err("site-doctor-undo requires one experiment ID".into());
            };
            if !self
                .as_mut()
                .finish_site_doctor_experiment(&QString::from(experiment_id.as_str()), false)
            {
                return Err(self.as_ref().rust().status_text.to_string());
            }
            return Ok(serde_json::json!({"status": "undone", "experiment_id": experiment_id}));
        }
        if command.name == "blocking-toggle" {
            if route.context.is_some() {
                return Err("context routing is not valid for blocking toggle".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            let site = match command.arguments.as_slice() {
                [] => false,
                [flag] if flag == "--site" => true,
                _ => return Err("blocking-toggle accepts optional --site".into()),
            };
            if site {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some("blocking-toggle-site".into());
                self.as_mut()
                    .set_status_text(QString::from("Site blocker toggle requested"));
                return Ok(serde_json::json!({"status": "accepted", "pending": true}));
            }
            let enabled = !self.as_ref().rust().blocking_enabled;
            self.as_mut().set_blocking_enabled(enabled);
            self.as_mut().set_status_text(QString::from(if enabled {
                "Network blocking enabled"
            } else {
                "Network blocking disabled"
            }));
            return Ok(serde_json::json!({"status": "accepted", "enabled": enabled}));
        }
        if command.name == "blocklist-update" {
            if route.context.is_some() {
                return Err("context routing is not valid for blocklist updates".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            if !command.arguments.is_empty() {
                return Err("blocklist-update does not accept arguments".into());
            }
            if self
                .as_ref()
                .rust()
                .profile_persistence
                .lacks_durable_storage()
            {
                return Err(
                    "blocklist updates require a normal profile with durable storage".into(),
                );
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some("blocklist-update".into());
            self.as_mut()
                .set_status_text(QString::from("Blocklist update requested"));
            return Ok(serde_json::json!({"status": "accepted", "pending": true}));
        }
        if command.name == "link-cleaning-update" {
            if route.context.is_some() {
                return Err("context routing is not valid for clean-link updates".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            if !command.arguments.is_empty() {
                return Err("link-cleaning-update does not accept arguments".into());
            }
            if self
                .as_ref()
                .rust()
                .profile_persistence
                .lacks_durable_storage()
            {
                return Err(
                    "clean-link updates require a normal profile with durable storage".into(),
                );
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some("link-cleaning-update".into());
            self.as_mut()
                .set_status_text(QString::from("Clean-link update requested"));
            return Ok(serde_json::json!({"status": "accepted", "pending": true}));
        }
        if is_tab_undo_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for tab-undo".into());
            }
            return self.execute_tab_undo_command(&command);
        }
        if is_tab_clone_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for tab-clone".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_tab_clone_command(&command);
        }
        if is_reopen_in_window_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for reopen-in-window".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_reopen_in_window_command(&command);
        }
        if is_tab_detach_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for tab-detach".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_tab_detach_command(&command, operation_id);
        }
        if is_tab_give_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for tab-give".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_tab_give_command(&command, operation_id);
        }
        if command.name == "search" {
            if route.context.is_some() {
                return Err("context routing is not valid for search".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_search_command(&command);
        }
        if is_zoom_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for zoom".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_zoom_command(&command);
        }
        if is_search_next_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for search-next".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_search_next_command(&command);
        }
        if is_scroll_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for page scrolling".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_scroll_command(&command);
        }
        if is_link_clean_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for URL cleaning".into());
            }
            return self.execute_link_clean_command(&command);
        }
        if is_yank_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for clipboard copying".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_yank_command(&command);
        }
        if command.name == "caret-yank" {
            if route.context.is_some() {
                return Err("context routing is not valid for clipboard copying".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.execute_yank_command(&selection_yank_command());
        }
        if command.name == "mode-enter" {
            if route.context.is_some() {
                return Err("context routing is not valid for mode changes".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            self.as_mut().execute_mode_enter(&command)?;
            return Ok(serde_json::json!({
                "status": "accepted",
                "mode": ipc_mode_name(self.as_ref().rust().core_mode)
            }));
        }
        if command.name == "caret-move" {
            if route.context.is_some() {
                return Err("context routing is not valid for caret movement".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            self.as_mut().execute_caret_move(&command)?;
            return Ok(serde_json::json!({"status": "accepted", "mode": "caret"}));
        }
        if command.name == "caret-select" {
            if route.context.is_some() {
                return Err("context routing is not valid for caret selection".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            self.as_mut().execute_caret_select(&command)?;
            return Ok(serde_json::json!({"status": "accepted", "mode": "caret"}));
        }
        if command.name == "selection-search" {
            if route.context.is_some() {
                return Err("context routing is not valid for selection search".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_selection_search(&command);
        }
        if command.name == "tab-close" {
            if route.context.is_some() {
                return Err("context routing is not valid for tab-close".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_tab_close_command(&command);
        }
        let active_tab_id = self
            .as_ref()
            .tab_for_index(self.as_ref().rust().active_tab_index)
            .map(|tab| tab.to_string());
        normalize_active_tab_command(&mut command, active_tab_id).map_err(str::to_owned)?;
        if matches!(
            command.name.as_str(),
            "tab-focus"
                | "tab-close"
                | "tab-suspend"
                | "tab-discard"
                | "tab-resume"
                | "tab-move"
                | "tab-mute"
                | "tab-pin"
        ) {
            if route.context.is_some() {
                return Err("context routing is not valid for tab actions".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_tab_action(&command, operation_id);
        }
        if command.name == "window-focus" {
            if route.context.is_some() {
                return Err("context routing is not valid for window actions".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_window_action(&command);
        }
        if command.name == "window-move" {
            if route.context.is_some() {
                return Err("context routing is not valid for window actions".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_window_move(&command);
        }
        if matches!(command.name.as_str(), "command-help" | "command-execute") {
            if route.context.is_some() {
                return Err("context routing is not valid for command actions".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_command_action(&command, Some(route));
        }
        if command.name == "edit-text" {
            if route.context.is_some() {
                return Err("context routing is not valid for external editing".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            self.as_mut().execute_editor_command(&command)?;
            return Ok(serde_json::json!({"status": "accepted", "pending": true}));
        }
        if command.name == "download" {
            if route.context.is_some() {
                return Err("context routing is not valid for downloads".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_download_command(&command);
        }
        if command.name == "print-pdf" {
            if route.context.is_some() {
                return Err("context routing is not valid for PDF generation".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_print_pdf_command(&command);
        }
        if command.name == "save-page" {
            if route.context.is_some() {
                return Err("context routing is not valid for page saving".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_save_page_command(&command);
        }
        if command.name == "view-source" {
            if route.context.is_some() {
                return Err("context routing is not valid for source viewing".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_view_source_command(&command);
        }
        if command.name == "devtools" {
            if route.context.is_some() {
                return Err("context routing is not valid for DevTools".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_devtools_command(&command);
        }
        if command.name == "print" {
            if route.context.is_some() {
                return Err("context routing is not valid for printing".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_print_command(&command);
        }
        if matches!(command.name.as_str(), "permissions" | "permission-reset") {
            if route.context.is_some() {
                return Err("context routing is not valid for permission commands".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_permission_command(&command);
        }
        if command.name == "downloads" {
            if route.context.is_some() || !command.arguments.is_empty() {
                return Err("downloads does not accept routing or arguments".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some("show-downloads".into());
            return Ok(serde_json::json!({"status": "accepted", "manager": "downloads"}));
        }
        if command.name == "switcher" {
            if route.context.is_some() {
                return Err("context routing is not valid for the universal switcher".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_switcher_command(&command);
        }
        if matches!(command.name.as_str(), "download-open" | "download-show") {
            if route.context.is_some() {
                return Err("context routing is not valid for download desktop actions".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_download_desktop_command(&command);
        }
        if matches!(
            command.name.as_str(),
            "download-cancel" | "download-pause" | "download-resume" | "download-retry"
        ) {
            if route.context.is_some() {
                return Err("context routing is not valid for download controls".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_download_control_command(&command);
        }
        if command.name == "spawn" {
            if route.context.is_some() {
                return Err("context routing is not valid for external processes".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_spawn_command(&command);
        }
        if command.name == "script-run" {
            if route.context.is_some() {
                return Err("context routing is not valid for userscripts".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_script_run_command(&command);
        }
        if command.name == "jseval" {
            if route.context.is_some() {
                return Err("context routing is not valid for JavaScript evaluation".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_jseval_command(&command);
        }
        if command.name == "send" {
            if route.context.is_some() {
                return Err("context routing is not valid for external action targets".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            return self.as_mut().execute_send_command(&command);
        }
        if command.name == "paste-open" {
            if route.context.is_some() {
                return Err("context routing is not valid for clipboard navigation".into());
            }
            let mut target = "current";
            let mut primary = false;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--primary" if !primary => primary = true,
                    "--primary" => return Err("paste-open accepts --primary at most once".into()),
                    "--target" if target == "current" => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "paste-open --target requires current or tab".to_owned()
                        })?;
                        if !matches!(value.as_str(), "current" | "tab") {
                            return Err("paste-open --target requires current or tab".into());
                        }
                        target = value.as_str();
                        index += 1;
                    }
                    "--target" => return Err("paste-open accepts --target at most once".into()),
                    _ => return Err("paste-open accepts [--target current|tab] [--primary]".into()),
                }
                index += 1;
            }
            self.as_ref().validate_ipc_route(route)?;
            if self
                .as_mut()
                .paste_open_channel(&QString::from(target), primary)
            {
                return Ok(serde_json::json!({"status": "accepted"}));
            }
            return Err(self.as_ref().rust().status_text.to_string());
        }
        if is_library_command(&command.name) {
            if route.context.is_some() {
                return Err("context routing is not valid for history or mark commands".into());
            }
            if matches!(command.name.as_str(), "journey" | "journey-reopen")
                && self.as_ref().active_profile_is_transient()
            {
                return Err("private and ephemeral journeys are native memory-only views".into());
            }
            return self.execute_library_command(&command);
        }
        if command.name == "hint" {
            if route.context.is_some() {
                return Err("context routing is not valid for hints".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            let options = parse_hint_options(&command)?;
            self.as_mut().set_hint_options(
                options.links_only,
                options.rapid,
                &options.target,
                options.script.as_deref(),
            );
            return Ok(serde_json::json!({"status": "accepted", "mode": "hint"}));
        }
        if command.name == "action-list" {
            if route.context.is_some() {
                return Err("context routing is not valid for action discovery".into());
            }
            let subject = match command.arguments.as_slice() {
                [] => None,
                [subject] => Some(subject.as_str()),
                _ => return Err("action-list accepts at most one subject".into()),
            };
            let actions = self.as_mut().show_action_list(subject)?;
            return Ok(serde_json::json!({"status": "accepted", "actions": actions}));
        }
        if command.name == "action" {
            if route.context.is_some() {
                return Err("context routing is not valid for actions".into());
            }
            if let Some((mut result, action_id)) =
                self.as_mut().execute_userscript_action_command(&command)?
            {
                result["action_id"] = Value::String(action_id);
                return Ok(result);
            }
            let (mapped, action_id) = parse_action_invocation(&command)?;
            let mut result =
                self.as_mut()
                    .execute_ipc_command_with_operation(mapped, route, operation_id)?;
            result["action_id"] = Value::String(action_id);
            return Ok(result);
        }
        if command.name == "tab-open" {
            if route.context.is_some() || route.profile.is_some() {
                return Err("tab-open does not accept context or profile routing".into());
            }
            self.as_ref().validate_ipc_route(route)?;
            self.as_mut().note_repeatable_command(&command);
            let dispatch = {
                let binding = self.as_ref();
                let rust = binding.rust();
                let state = rust
                    .state
                    .as_ref()
                    .ok_or_else(|| "core state unavailable".to_owned())?;
                RuntimeDispatch {
                    invocation: ipc_command_invocation(
                        state,
                        command,
                        route.selector,
                        route.source,
                        operation_id,
                    ),
                    navigation: binding.navigation_context_for_source(route.source),
                    target: route.selector,
                }
            };
            let effects = self.as_mut().dispatch_runtime(dispatch)?;
            let tab_id = effects.iter().find_map(|effect| match effect {
                Effect::Engine(EngineEffect::Navigate { target, .. }) => {
                    Some(target.tab.to_string())
                }
                _ => None,
            });
            self.as_mut().sync_tab_order_from_core();
            self.as_mut().set_pending_engine_action(&effects);
            return Ok(serde_json::json!({
                "status": "accepted",
                "tab_id": tab_id
            }));
        }
        self.as_mut().note_repeatable_command(&command);
        if route.context.is_some() && command.name != "open" {
            return Err("context routing is only valid for open".into());
        }
        self.as_ref().validate_ipc_route(route)?;
        // Validate profile/context affinity before mutating the window. A
        // rejected cross-profile route must not leave a durable context
        // assignment behind.
        if route.context.is_none() || route.open_target != IpcOpenTarget::Window {
            self.as_mut().assign_ipc_context(route)?;
        }
        if command.name == "open"
            && route.context.is_none()
            && command.arguments.first().is_some_and(|input| {
                self.as_mut().queue_context_route_for_input(
                    input,
                    if route.external_open {
                        "external-open"
                    } else {
                        "explicit-open"
                    },
                )
            })
        {
            self.as_mut()
                .set_status_text(QString::from("Context route confirmation required"));
            return Ok(serde_json::json!({
                "status": "accepted",
                "pending": "context-route"
            }));
        }
        if command.name == "open"
            && command.arguments.first().map(String::as_str) == Some("--clean-link")
        {
            return self.execute_clean_open_ipc(&command, route, operation_id);
        }
        match route.open_target {
            IpcOpenTarget::PrivateWindow | IpcOpenTarget::Window => {
                if command.name != "open" {
                    return Err("window targets are only valid for open".into());
                }
                let input = command
                    .arguments
                    .first()
                    .ok_or_else(|| "open requires an input".to_owned())?;
                let navigation = resolve_input(
                    input,
                    &self.as_ref().navigation_context_for_source(route.source),
                )
                .map_err(|error| error.to_string())?;
                let private = route.open_target == IpcOpenTarget::PrivateWindow;
                if private && route.profile.is_some() {
                    return Err("private-window cannot select a durable profile".into());
                }
                let context_profile = route.context.as_deref().and_then(|context_name| {
                    self.as_ref()
                        .rust()
                        .contexts
                        .as_ref()?
                        .contexts()
                        .iter()
                        .find(|context| context.name == context_name)
                        .map(|context| context.profile.clone())
                });
                let profile = route
                    .profile
                    .clone()
                    .or(context_profile)
                    .unwrap_or_else(|| {
                        if private {
                            "private".into()
                        } else {
                            "secondary".into()
                        }
                    });
                let pending_action = route.context.as_deref().map_or_else(
                    || {
                        format!(
                            "new-window\t{}\t{}\t{}",
                            private,
                            profile,
                            navigation.url.as_str()
                        )
                    },
                    |context| {
                        format!(
                            "new-window\t{}\t{}\t{}\t{}",
                            private,
                            profile,
                            context,
                            navigation.url.as_str()
                        )
                    },
                );
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(pending_action);
                return Ok(serde_json::json!({
                    "status": "accepted",
                    "target_window": "pending",
                    "context": route.context,
                    "profile": profile
                }));
            }
            IpcOpenTarget::BackgroundTab if command.name != "open" => {
                return Err("tab-bg is only valid for open".into());
            }
            IpcOpenTarget::Tab | IpcOpenTarget::BackgroundTab => {}
        }
        if route.open_target == IpcOpenTarget::BackgroundTab {
            let input = command
                .arguments
                .first()
                .ok_or_else(|| "open requires an input".to_owned())?;
            let navigation = resolve_input(
                input,
                &self.as_ref().navigation_context_for_source(route.source),
            )
            .map_err(|error| error.to_string())?;
            let (window, original_window, original_tab, journey_parent) = {
                let binding = self.as_ref();
                let state = binding
                    .rust()
                    .state
                    .as_ref()
                    .ok_or_else(|| "core state unavailable".to_owned())?;
                let window = match route.selector {
                    DispatchTarget::Active => state.active_window(),
                    DispatchTarget::LastFocused => {
                        state.last_focused_window().or(state.active_window())
                    }
                    DispatchTarget::Window(window) => Some(window),
                    DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
                }
                .ok_or_else(|| "requested window is not available".to_owned())?;
                let source_tab = match route.selector {
                    DispatchTarget::Tab(tab) => Some(tab),
                    _ => state
                        .windows()
                        .get(&window)
                        .and_then(|window| window.active_tab),
                };
                let journey_parent = source_tab.and_then(|tab| state.journey().current_node(tab));
                (
                    window,
                    state.active_window(),
                    state.windows()[&window].active_tab,
                    journey_parent,
                )
            };
            self.as_mut()
                .reduce_event(Event::OpenTab { window })
                .map_err(|error| error.clone())?;
            let tab = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| state.windows().get(&window))
                .and_then(|window| window.active_tab)
                .ok_or_else(|| "new background tab was not created".to_owned())?;
            if let Some(original_tab) = original_tab {
                self.as_mut()
                    .reduce_event(Event::ActivateTab {
                        window,
                        tab: original_tab,
                    })
                    .map_err(|error| error.clone())?;
            } else if let Some(original_window) = original_window
                && original_window != window
            {
                self.as_mut()
                    .reduce_event(Event::FocusWindow {
                        window: original_window,
                    })
                    .map_err(|error| error.clone())?;
            }
            let target = current_target(self.as_ref().rust().state.as_ref(), Some(tab))
                .ok_or_else(|| "new background tab target is not live".to_owned())?;
            let effects = self
                .as_mut()
                .reduce_event(Event::StartNavigation {
                    target,
                    url: navigation.url,
                })
                .map_err(|error| error.clone())?;
            if let Some(parent) = journey_parent {
                self.as_mut().mark_journey_parent(&effects, parent);
            }
            self.as_mut().set_pending_engine_action(&effects);
            self.as_mut().sync_core_tabs();
            return Ok(serde_json::json!({
                "status": "accepted",
                "tab_id": tab.to_string(),
                "target_window": window.to_string()
            }));
        }
        let dispatch = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let state = rust
                .state
                .as_ref()
                .ok_or_else(|| "core state unavailable".to_owned())?;
            RuntimeDispatch {
                invocation: ipc_command_invocation(
                    state,
                    command,
                    route.selector,
                    route.source,
                    operation_id,
                ),
                navigation: binding.navigation_context_for_source(route.source),
                target: route.selector,
            }
        };
        let effects = self.as_mut().dispatch_runtime(dispatch)?;
        let target = effects.iter().find_map(|effect| match effect {
            Effect::Engine(EngineEffect::Navigate { target, .. }) => Some(*target),
            _ => None,
        });
        self.as_mut().set_pending_engine_action(&effects);
        self.as_mut().sync_core_tabs();
        Ok(serde_json::json!({
            "status": "accepted",
            "tab_id": target.map(|target| target.tab.to_string())
        }))
    }

    pub(super) fn handle_userscript_action_request(
        mut self: Pin<&mut Self>,
        request: &Request,
        action_id: &str,
    ) -> Response {
        let Some(object) = request.params.as_object() else {
            return ipc_action_failure(
                &request.id,
                action_id,
                "unissued",
                "userscript action parameters must be an object",
            );
        };
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "action" | "arguments"))
        {
            return ipc_action_failure(
                &request.id,
                action_id,
                "unissued",
                "userscript action parameters contain an unknown field",
            );
        }
        let expected_argument = match self
            .as_ref()
            .rust()
            .userscript_roots
            .as_ref()
            .map(|roots| userscript::registered_actions(&roots.config))
        {
            None => {
                return ipc_action_failure(
                    &request.id,
                    action_id,
                    "unissued",
                    "userscripts are unavailable without configured storage",
                );
            }
            Some(Err(error)) => {
                return ipc_action_failure(&request.id, action_id, "unissued", error);
            }
            Some(Ok(actions)) => {
                let Some(action) = actions.iter().find(|action| action.id == action_id) else {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        "userscript action is not installed",
                    );
                };
                if userscript_action_is_hint_only(action) {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        "hint-only userscript action must be activated from a validated hint",
                    );
                }
                userscript_action_argument_name(&action.subject)
            }
        };
        let value = match object.get("arguments") {
            None => Ok(String::new()),
            Some(arguments) => {
                let Some(arguments) = arguments.as_object() else {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        "userscript action arguments must be an object",
                    );
                };
                if arguments.len() > 1
                    || expected_argument.is_none() && !arguments.is_empty()
                    || expected_argument.is_some_and(|name| !arguments.contains_key(name))
                {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        expected_argument.map_or_else(
                            || "userscript action does not accept subject arguments".to_owned(),
                            |name| format!("userscript action requires argument {name}"),
                        ),
                    );
                }
                if arguments
                    .keys()
                    .any(|key| Some(key.as_str()) != expected_argument)
                {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        format!(
                            "userscript action arguments must use {}",
                            expected_argument.unwrap_or("no subject field")
                        ),
                    );
                }
                let mut values = arguments.values();
                let value = values.next();
                if values.next().is_some() {
                    return ipc_action_failure(
                        &request.id,
                        action_id,
                        "unissued",
                        "userscript action accepts one subject value",
                    );
                }
                value
                    .map(|value| {
                        value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                            "userscript action subject value must be a string".to_owned()
                        })
                    })
                    .unwrap_or(Ok(String::new()))
            }
        };
        let value = match value {
            Ok(value) => value,
            Err(error) => {
                return ipc_action_failure(&request.id, action_id, "unissued", error);
            }
        };
        match self
            .as_mut()
            .execute_registered_userscript_action(action_id, &value)
        {
            Ok(mut result) => {
                let operation_id = result
                    .get("operation_id")
                    .and_then(Value::as_str)
                    .unwrap_or("untracked")
                    .to_owned();
                self.as_mut()
                    .record_action_audit(action_id, &operation_id, "accepted", None);
                result["action_id"] = Value::String(action_id.to_owned());
                Response::success(request.id.clone(), result)
            }
            Err(error) => {
                let error = PublicError::engine(error);
                let category = action_failure_category(error.code());
                self.as_mut().record_action_audit(
                    action_id,
                    "unissued",
                    "rejected",
                    Some(category),
                );
                ipc_action_failure(&request.id, action_id, "unissued", error)
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_ipc_request(mut self: Pin<&mut Self>, request: &Request) -> Response {
        if self.as_ref().rust().ipc_shutdown_gate && is_mutating_ipc_method(&request.method) {
            return ipc_failure(
                &request.id,
                "E_BUSY",
                "shutdown is in progress; mutating IPC is temporarily disabled",
            );
        }
        match request.method.as_str() {
            "command.execute" => {
                let (command, route) = match typed_ipc_command(&request.params) {
                    Ok(command) => command,
                    Err(error) => {
                        return Response::failure(
                            request.id.clone(),
                            error.into_public().into_protocol_error(),
                        );
                    }
                };
                let operation_id = format!("op-{}", Uuid::new_v4());
                let count = ipc_command_count(&command);
                let command_context = ipc_command_context(
                    self.as_ref().rust().state.as_ref(),
                    &route,
                    &operation_id,
                    count,
                );
                match self.as_mut().execute_ipc_command_with_operation(
                    command,
                    &route,
                    Some(&operation_id),
                ) {
                    Ok(mut result) => {
                        result["operation_id"] = Value::String(operation_id.clone());
                        result["command_context"] = command_context;
                        self.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .operation_states
                            .insert(operation_id, "accepted".into());
                        Response::success(request.id.clone(), result)
                    }
                    Err(error) => {
                        ipc_command_failure_with_context(&request.id, error, command_context)
                    }
                }
            }
            "action.execute" => {
                if let Some(action_id) = request.params.get("action").and_then(Value::as_str)
                    && action_id.starts_with("userscript.")
                {
                    return self
                        .as_mut()
                        .handle_userscript_action_request(request, action_id);
                }
                let audit_action_id = request
                    .params
                    .get("action")
                    .and_then(Value::as_str)
                    .and_then(|action| {
                        let registry = ActionRegistry::default_v1();
                        registry
                            .resolve(action)
                            .map(|definition| definition.id.clone())
                            .or_else(|| parse_external_action_id(action).map(|_| action.to_owned()))
                    })
                    .unwrap_or_else(|| "unknown".into());
                let (command, route, action_id) = match typed_ipc_action(&request.params) {
                    Ok(command) => command,
                    Err(error) => {
                        let error = error.into_public();
                        let category = action_failure_category(error.code());
                        self.as_mut().record_action_audit(
                            &audit_action_id,
                            "unissued",
                            "rejected",
                            Some(category),
                        );
                        return ipc_action_failure(
                            &request.id,
                            &audit_action_id,
                            "unissued",
                            error,
                        );
                    }
                };
                let operation_id = format!("op-{}", Uuid::new_v4());
                let count = ipc_command_count(&command);
                let command_context = ipc_command_context(
                    self.as_ref().rust().state.as_ref(),
                    &route,
                    &operation_id,
                    count,
                );
                if let Err(error) = self
                    .as_ref()
                    .validate_action_availability(&action_id, route.source)
                {
                    let error = PublicError::engine(error);
                    let category = action_failure_category(error.code());
                    self.as_mut().record_action_audit(
                        &action_id,
                        &operation_id,
                        "failed",
                        Some(category),
                    );
                    return ipc_action_failure_with_context(
                        &request.id,
                        &action_id,
                        &operation_id,
                        error,
                        command_context,
                    );
                }
                match self.as_mut().execute_ipc_command_with_operation(
                    command,
                    &route,
                    Some(&operation_id),
                ) {
                    Ok(mut result) => {
                        self.as_mut().record_action_audit(
                            &action_id,
                            &operation_id,
                            "accepted",
                            None,
                        );
                        result["action_id"] = Value::String(action_id);
                        result["operation_id"] = Value::String(operation_id.clone());
                        result["command_context"] = command_context;
                        self.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .operation_states
                            .insert(operation_id, "accepted".into());
                        Response::success(request.id.clone(), result)
                    }
                    Err(error) => {
                        let error = PublicError::engine(error);
                        let category = action_failure_category(error.code());
                        self.as_mut().record_action_audit(
                            &action_id,
                            &operation_id,
                            "failed",
                            Some(category),
                        );
                        self.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .operation_states
                            .insert(operation_id.clone(), format!("failed ({category})"));
                        ipc_action_failure_with_context(
                            &request.id,
                            &action_id,
                            &operation_id,
                            error,
                            command_context,
                        )
                    }
                }
            }
            "tabs.query" => match self.as_ref().ipc_tabs_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "windows.query" => match self.as_ref().ipc_windows_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "window.focus" => match self.as_mut().ipc_window_focus(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "profiles.query" => match self.as_ref().ipc_profiles_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "actions.query" => match self.as_ref().ipc_actions_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "bindings.query" => match self.as_ref().ipc_bindings_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "bindings.explain" => match self.as_ref().ipc_bindings_explain(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "downloads.query" => {
                if self.as_ref().rust().profile_persistence.is_durable()
                    && (self.as_ref().rust().storage_library.is_none()
                        || self.as_ref().rust().storage_library_dirty.get())
                {
                    self.as_mut().request_storage_library();
                }
                match self.as_ref().ipc_downloads_query(&request.params) {
                    Ok(result) => Response::success(request.id.clone(), result),
                    Err(error) => {
                        let (code, message) = error.into_public_parts();
                        ipc_failure(&request.id, code, message)
                    }
                }
            }
            "permissions.query" => {
                if self.as_ref().rust().profile_persistence.is_durable()
                    && (self.as_ref().rust().storage_library.is_none()
                        || self.as_ref().rust().storage_library_dirty.get())
                {
                    self.as_mut().request_storage_library();
                }
                match self.as_ref().ipc_permissions_query(&request.params) {
                    Ok(result) => Response::success(request.id.clone(), result),
                    Err(error) => {
                        let (code, message) = error.into_public_parts();
                        ipc_failure(&request.id, code, message)
                    }
                }
            }
            "contexts.query" => match self.as_ref().ipc_contexts_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "switcher.query" => {
                if self.as_ref().rust().profile_persistence.is_durable()
                    && (self.as_ref().rust().storage_library.is_none()
                        || self.as_ref().rust().storage_library_dirty.get())
                {
                    self.as_mut().request_storage_library();
                }
                self.as_mut().request_session_names();
                match self.as_ref().ipc_switcher_query(&request.params) {
                    Ok(result) => Response::success(request.id.clone(), result),
                    Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
                }
            }
            "switcher.activate" => match self.as_mut().ipc_switcher_activate(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "site.status" => {
                if self.as_ref().rust().profile_persistence.is_durable()
                    && (self.as_ref().rust().storage_library.is_none()
                        || self.as_ref().rust().storage_library_dirty.get())
                {
                    self.as_mut().request_storage_library();
                }
                match self.as_ref().ipc_site_status(&request.params) {
                    Ok(result) => Response::success(request.id.clone(), result),
                    Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
                }
            }
            "blocking.status" => match self.as_ref().ipc_blocking_status(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "operations.query" => match self.as_ref().ipc_operations_query(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "operations.cancel" => match self.as_mut().ipc_operation_cancel(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_NOT_FOUND", error),
            },
            "diagnostics.get" => match self.as_ref().ipc_diagnostics(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            "config.get" => match self.as_ref().ipc_config_get(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => {
                    let (code, message) = error.into_public_parts();
                    ipc_failure(&request.id, code, message)
                }
            },
            "events.subscribe" => match self.as_ref().ipc_events_subscribe(&request.params) {
                Ok(result) => Response::success(request.id.clone(), result),
                Err(error) => ipc_failure(&request.id, "E_INVALID_PARAMS", error),
            },
            _ => ipc_failure(
                &request.id,
                "E_UNSUPPORTED",
                format!("unsupported IPC method: {}", request.method),
            ),
        }
    }
}
