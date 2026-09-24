use super::{
    CxxQtType, DispatchTarget, Event, IpcOpenTarget, IpcRoute, ParsedCommand, Pin, QString, Value,
    binding_command_parameters, current_target, ipc_mode_name, is_context_command,
    is_library_command, is_link_clean_command, is_reopen_in_window_command, is_scroll_command,
    is_search_next_command, is_tab_clone_command, is_tab_detach_command, is_tab_give_command,
    is_tab_undo_command, is_yank_command, is_zoom_command, learning_mode_request,
    normalize_active_tab_command, parse_action_invocation, parse_hint_options, qobject,
    resolve_input, selection_yank_command,
};

impl qobject::BrowserUi {
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
            return self
                .as_mut()
                .dispatch_tab_open_ipc_command(command, route, operation_id);
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
        self.as_mut()
            .dispatch_fallback_ipc_command(command, route, operation_id)
    }
}
