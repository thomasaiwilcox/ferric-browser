use crate::browser_ui_command_workflows::CommandDispatchControl;
use ferric_browser_core::CommonCommandExecutor;

use super::{
    CommandInvocation, CxxQtType, ParsedCommand, Pin, QString, RuntimeDispatch,
    binding_command_parameters, is_library_command, is_zoom_command, learning_mode_request,
    normalize_active_tab_command, qobject, selection_yank_command,
};

impl qobject::BrowserUi {
    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_parsed_commands(
        mut self: Pin<&mut Self>,
        commands: Vec<ParsedCommand>,
    ) -> bool {
        let registry = self.as_ref().rust().registry.clone();
        let commands = match registry.expand_chain(commands) {
            Ok(commands) => commands,
            Err(error) => {
                self.set_status_text(QString::from(error.to_string()));
                return false;
            }
        };
        if self.as_mut().reject_generic_pointer_hint_chain(&commands) {
            return false;
        }
        let command_count = commands.len();
        for (command_index, mut command) in commands.into_iter().enumerate() {
            if command_index > 0 {
                self.as_mut().queue_pending_engine_action();
            }
            match self
                .as_mut()
                .dispatch_workflow_command(&mut command, &registry)
            {
                CommandDispatchControl::Continue => continue,
                CommandDispatchControl::Failed => return false,
                CommandDispatchControl::Unhandled => {}
            }
            if let Some(executor) = registry
                .resolve(&command.name)
                .ok()
                .and_then(ferric_browser_core::CommandDefinition::common_executor)
            {
                // Generic spawn commands retain registry mode restrictions.
                // Native action entry points have their own eligibility rules.
                if executor == CommonCommandExecutor::Spawn
                    && let Err(error) = registry.validate(&command, self.as_ref().rust().core_mode)
                {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self
                    .as_mut()
                    .execute_common_command(executor, &command, None)
                {
                    // These handlers historically exposed their local detail in
                    // the owning window. IPC still receives only the safe text.
                    let message = if error.code() == ferric_browser_ipc::ErrorCode::Engine {
                        error.diagnostic_context()
                    } else {
                        error.user_message()
                    };
                    self.set_status_text(QString::from(message));
                    return false;
                }
                if executor == CommonCommandExecutor::ScrollTarget
                    && command.arguments == ["select"]
                {
                    // Interactive selection owns the next input. As before,
                    // discard the remaining chain instead of running it early.
                    return true;
                }
                continue;
            }
            if let Some(result) = self.as_mut().dispatch_start_page_command(&command) {
                if !result {
                    return false;
                }
                continue;
            }
            if command.name == "blocking-status" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if !command.arguments.is_empty() {
                    self.set_status_text(QString::from(
                        "blocking-status does not accept arguments",
                    ));
                    return false;
                }
                let status = self.as_ref().blocking_status_message();
                self.as_mut().set_status_text(QString::from(status));
                continue;
            }
            if command.name == "site-status" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some("show-site-ledger".into());
                self.as_mut()
                    .set_status_text(QString::from("Site Ledger requested"));
                continue;
            }
            if command.name == "site-data-clear" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let (origin, confirmed) = match command.arguments.as_slice() {
                    [origin] => (origin.as_str(), false),
                    [origin, flag] if flag == "--confirm" => (origin.as_str(), true),
                    _ => {
                        self.set_status_text(QString::from(
                            "site-data-clear requires ORIGIN and optional --confirm",
                        ));
                        return false;
                    }
                };
                if confirmed {
                    if !self.as_mut().site_data_clear(&QString::from(origin), true) {
                        return false;
                    }
                } else {
                    let plan = self
                        .as_mut()
                        .site_data_clear_plan(&QString::from(origin))
                        .to_string();
                    self.as_mut()
                        .set_status_text(QString::from(format!("Site-data scope preview: {plan}")));
                }
                continue;
            }
            if command.name == "site-doctor" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let [kind] = command.arguments.as_slice() else {
                    self.set_status_text(QString::from("site-doctor requires one experiment name"));
                    return false;
                };
                if self
                    .as_mut()
                    .begin_site_doctor_experiment(&QString::from(kind.as_str()))
                    .is_empty()
                {
                    return false;
                }
                continue;
            }
            if command.name == "site-doctor-undo" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let [experiment_id] = command.arguments.as_slice() else {
                    self.set_status_text(QString::from(
                        "site-doctor-undo requires one experiment ID",
                    ));
                    return false;
                };
                if !self
                    .as_mut()
                    .finish_site_doctor_experiment(&QString::from(experiment_id.as_str()), false)
                {
                    return false;
                }
                continue;
            }
            if command.name == "blocking-toggle" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let site = match command.arguments.as_slice() {
                    [] => false,
                    [flag] if flag == "--site" => true,
                    _ => {
                        self.set_status_text(QString::from(
                            "blocking-toggle accepts optional --site",
                        ));
                        return false;
                    }
                };
                if site {
                    if !self.as_mut().toggle_blocking_site() {
                        return false;
                    }
                } else {
                    let enabled = !self.as_ref().rust().blocking_enabled;
                    self.as_mut().set_blocking_enabled(enabled);
                    self.as_mut().set_status_text(QString::from(if enabled {
                        "Network blocking enabled"
                    } else {
                        "Network blocking disabled"
                    }));
                }
                continue;
            }
            if command.name == "blocklist-update" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if !command.arguments.is_empty() {
                    self.set_status_text(QString::from(
                        "blocklist-update does not accept arguments",
                    ));
                    return false;
                }
                if self
                    .as_ref()
                    .rust()
                    .profile_persistence
                    .lacks_durable_storage()
                {
                    self.set_status_text(QString::from(
                        "Blocklist updates require a normal profile with durable storage",
                    ));
                    return false;
                }
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some("blocklist-update".into());
                self.as_mut()
                    .set_status_text(QString::from("Blocklist update requested"));
                continue;
            }
            if command.name == "link-cleaning-update" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if !command.arguments.is_empty() {
                    self.set_status_text(QString::from(
                        "link-cleaning-update does not accept arguments",
                    ));
                    return false;
                }
                if self
                    .as_ref()
                    .rust()
                    .profile_persistence
                    .lacks_durable_storage()
                {
                    self.set_status_text(QString::from(
                        "Clean-link updates require a normal profile with durable storage",
                    ));
                    return false;
                }
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some("link-cleaning-update".into());
                self.as_mut()
                    .set_status_text(QString::from("Clean-link update requested"));
                continue;
            }
            if command.name == "open"
                && command.arguments.first().map(String::as_str) == Some("--clean-link")
            {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_clean_open_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "open"
                && command.arguments.first().map(String::as_str) != Some("--clean-link")
                && command.arguments.first().is_some_and(|input| {
                    self.as_mut()
                        .queue_context_route_for_input(input, "explicit-open")
                })
            {
                self.as_mut()
                    .set_status_text(QString::from("Context route confirmation required"));
                continue;
            }
            if command.name == "paste-open" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let mut target = "current";
                let mut primary = false;
                let mut index = 0;
                while index < command.arguments.len() {
                    match command.arguments[index].as_str() {
                        "--primary" if !primary => primary = true,
                        "--primary" => {
                            self.set_status_text(QString::from(
                                "paste-open accepts --primary at most once",
                            ));
                            return false;
                        }
                        "--target" if target == "current" => {
                            let Some(value) = command.arguments.get(index + 1) else {
                                self.set_status_text(QString::from(
                                    "paste-open --target requires current or tab",
                                ));
                                return false;
                            };
                            if !matches!(value.as_str(), "current" | "tab") {
                                self.set_status_text(QString::from(
                                    "paste-open --target requires current or tab",
                                ));
                                return false;
                            }
                            target = value.as_str();
                            index += 1;
                        }
                        "--target" => {
                            self.set_status_text(QString::from(
                                "paste-open accepts --target at most once",
                            ));
                            return false;
                        }
                        _ => {
                            self.set_status_text(QString::from(
                                "paste-open accepts [--target current|tab] [--primary]",
                            ));
                            return false;
                        }
                    }
                    index += 1;
                }
                if self
                    .as_mut()
                    .paste_open_channel(&QString::from(target), primary)
                {
                    continue;
                }
                return false;
            }
            if command.name == "hint" {
                if !self.as_mut().dispatch_hint_command(&command, &registry) {
                    return false;
                }
                continue;
            }
            if command.name == "target-search" {
                if !self
                    .as_mut()
                    .dispatch_target_search_command(&command, &registry)
                {
                    return false;
                }
                continue;
            }
            if command.name == "get" {
                if let Err(error) = self.as_mut().execute_config_get_command(&command) {
                    self.set_status_text(QString::from(error.user_message()));
                    return false;
                }
                continue;
            }
            if matches!(command.name.as_str(), "help" | "version" | "diagnostics") {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let result = match command.name.as_str() {
                    "help" => self.as_mut().execute_help_command(&command),
                    "version" => self.as_mut().execute_version_command(&command),
                    "diagnostics" => self.as_mut().execute_diagnostics_command(&command),
                    _ => unreachable!("support command was matched above"),
                };
                if let Err(error) = result {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if matches!(
                command.name.as_str(),
                "config-edit"
                    | "config-reload"
                    | "config-check"
                    | "config-write-defaults"
                    | "theme-reload"
            ) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let result = match command.name.as_str() {
                    "config-edit" => self.as_mut().execute_config_edit_command(&command),
                    "config-reload" => self.as_mut().execute_config_reload_command(&command),
                    "config-check" => self.as_mut().execute_config_check_command(&command),
                    "config-write-defaults" => self
                        .as_mut()
                        .execute_config_write_defaults_command(&command),
                    "theme-reload" => self.as_mut().execute_theme_reload_command(&command),
                    _ => unreachable!("configuration command was matched above"),
                };
                if let Err(error) = result {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "learning-mode" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let current = self.as_ref().rust().learning_mode;
                let enabled = match learning_mode_request(&command, current) {
                    Ok(requested) => requested.unwrap_or(current),
                    Err(error) => {
                        self.set_status_text(QString::from(error));
                        return false;
                    }
                };
                self.as_mut().set_learning_mode(enabled);
                self.as_mut().set_status_text(QString::from(if enabled {
                    "Learning mode enabled"
                } else {
                    "Learning mode disabled"
                }));
                continue;
            }
            if matches!(command.name.as_str(), "binding-list" | "binding-explain") {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let (keychain, mode) = match binding_command_parameters(&command) {
                    Ok(parameters) => parameters,
                    Err(error) => {
                        self.set_status_text(QString::from(error));
                        return false;
                    }
                };
                let explanation = if command.name == "binding-explain" {
                    self.as_ref()
                        .binding_explanation_feedback(keychain.as_deref(), mode.as_deref())
                } else {
                    None
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
                let status = explanation.unwrap_or_else(|| {
                    super::browser_ui_query_bindings::binding_command_status(&command.name).into()
                });
                self.as_mut().set_status_text(QString::from(status));
                continue;
            }
            if is_library_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_library_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                self.as_mut()
                    .set_status_text(QString::from("Library command executed"));
                continue;
            }

            if is_zoom_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_zoom_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }

            if command.name == "caret-yank" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self
                    .as_mut()
                    .execute_yank_command(&selection_yank_command())
                {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "mode-enter" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_mode_enter(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "caret-move" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_caret_move(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "caret-select" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_caret_select(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "selection-search" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_selection_search(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "tab-close" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_tab_close_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            let active_tab_id = self
                .as_ref()
                .tab_for_index(self.as_ref().rust().active_tab_index)
                .map(|tab| tab.to_string());
            if let Err(error) = normalize_active_tab_command(&mut command, active_tab_id) {
                self.set_status_text(QString::from(error));
                return false;
            }
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
                if command.name == "tab-move"
                    && command
                        .arguments
                        .iter()
                        .any(|argument| argument == "--context")
                {
                    let (index, context) = match command.arguments.as_slice() {
                        [flag, context] if flag == "--context" => {
                            (self.as_ref().rust().active_tab_index, context.clone())
                        }
                        [index, flag, context] if flag == "--context" => {
                            let parsed = index
                                .parse::<usize>()
                                .ok()
                                .filter(|index| *index > 0)
                                .and_then(|index| index.checked_sub(1))
                                .and_then(|index| i32::try_from(index).ok());
                            let Some(parsed) = parsed else {
                                self.set_status_text(QString::from(
                                    "tab-move index must be a positive displayed index",
                                ));
                                return false;
                            };
                            (parsed, context.clone())
                        }
                        _ => {
                            self.set_status_text(QString::from(
                                "tab-move accepts [INDEX] --context NAME",
                            ));
                            return false;
                        }
                    };
                    let Some(tab) = self.as_ref().tab_for_index(index) else {
                        self.set_status_text(QString::from(
                            "tab-move index is outside this window",
                        ));
                        return false;
                    };
                    command = ParsedCommand {
                        name: command.name.clone(),
                        arguments: vec![tab.to_string(), "--context".into(), context],
                    };
                }
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_tab_action(&command, None) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "window-focus" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_window_action(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "window-move" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_window_move(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if matches!(command.name.as_str(), "command-help" | "command-execute") {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_command_action(&command, None) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "edit-text" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_editor_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "download" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_download_command(&command) {
                    self.set_status_text(QString::from(error.user_message()));
                    return false;
                }
                continue;
            }

            if matches!(command.name.as_str(), "permissions" | "permission-reset") {
                if let Err(error) = self.as_mut().execute_permission_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "downloads" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some("show-downloads".into());
                self.as_mut().set_status_text(QString::from("Downloads"));
                continue;
            }

            if matches!(
                command.name.as_str(),
                "download-cancel" | "download-pause" | "download-resume" | "download-retry"
            ) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_download_control_command(&command) {
                    self.set_status_text(QString::from(error.user_message()));
                    return false;
                }
                continue;
            }

            self.as_mut().note_repeatable_command(&command);
            let effects = {
                if self.as_ref().rust().state.is_none() {
                    self.set_status_text(QString::from("Core state unavailable"));
                    return false;
                }
                let navigation = self.as_ref().navigation_context();
                let target = self.as_ref().clipboard_dispatch_target(&command);
                self.as_mut().dispatch_runtime(RuntimeDispatch {
                    invocation: CommandInvocation::new(command),
                    navigation,
                    target,
                })
            };
            let effects = match effects {
                Ok(effects) => effects,
                Err(error) => {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
            };
            self.as_mut().sync_tab_order_from_core();
            self.as_mut().set_pending_engine_action(&effects);
        }
        if command_count > 1 {
            self.as_mut().queue_pending_engine_action();
        }
        if self.as_ref().rust().status_text.to_string() == "Command mode" {
            self.set_status_text(QString::from("Command executed"));
        }
        true
    }
}
