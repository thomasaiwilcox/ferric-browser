use crate::browser_ui_command_workflows::CommandDispatchControl;

use super::{
    CommandInvocation, CxxQtType, DispatchTarget, ParseInput, ParsedCommand, Pin, QString,
    RuntimeDispatch, binding_command_parameters, is_library_command, is_link_clean_command,
    is_reopen_in_window_command, is_scroll_command, is_search_next_command, is_tab_clone_command,
    is_tab_detach_command, is_tab_give_command, is_tab_undo_command, is_yank_command,
    is_zoom_command, learning_mode_request, normalize_active_tab_command, parse_chain,
    parse_hint_options, qobject, selection_yank_command,
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
        for mut command in commands {
            match self
                .as_mut()
                .dispatch_workflow_command(&mut command, &registry)
            {
                CommandDispatchControl::Continue => continue,
                CommandDispatchControl::Failed => return false,
                CommandDispatchControl::Unhandled => {}
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
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_action = Some("blocking-toggle-site".into());
                    self.as_mut()
                        .set_status_text(QString::from("Site blocker toggle requested"));
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
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                let options = match parse_hint_options(&command) {
                    Ok(options) => options,
                    Err(error) => {
                        self.set_status_text(QString::from(error));
                        return false;
                    }
                };
                self.as_mut().set_hint_options(
                    options.links_only,
                    options.rapid,
                    &options.target,
                    options.script.as_deref(),
                );
                continue;
            }
            if command.name == "get" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_config_get_command(&command) {
                    self.set_status_text(QString::from(error));
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
            if is_tab_undo_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_tab_undo_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if is_tab_clone_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_tab_clone_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if is_reopen_in_window_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_reopen_in_window_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if is_tab_detach_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_tab_detach_command(&command, None) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if is_tab_give_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_tab_give_command(&command, None) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "search" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_search_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
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
            if is_search_next_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_search_next_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if is_scroll_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_scroll_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if is_link_clean_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_link_clean_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if is_yank_command(&command.name) {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_yank_command(&command) {
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
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "print-pdf" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_print_pdf_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "save-page" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_save_page_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "view-source" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_view_source_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "devtools" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_devtools_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "print" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_print_command(&command) {
                    self.set_status_text(QString::from(error));
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
                self.as_mut()
                    .set_status_text(QString::from("Downloads manager requested"));
                continue;
            }
            if matches!(command.name.as_str(), "download-open" | "download-show") {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_download_desktop_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
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
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "spawn" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_spawn_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "script-run" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_script_run_command(&command) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
                continue;
            }
            if command.name == "jseval" {
                let mode = self.as_ref().rust().core_mode;
                if let Err(error) = registry.validate(&command, mode) {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
                if let Err(error) = self.as_mut().execute_jseval_command(&command) {
                    self.set_status_text(QString::from(error));
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
                self.as_mut().dispatch_runtime(RuntimeDispatch {
                    invocation: CommandInvocation::new(command),
                    navigation,
                    target: DispatchTarget::Active,
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
        self.set_status_text(QString::from("Command executed"));
        true
    }

    pub(super) fn execute_command(self: Pin<&mut Self>, input: &QString) -> bool {
        let commands = match parse_chain(&input.to_string(), ParseInput::Interactive) {
            Ok(commands) => commands,
            Err(error) => {
                self.set_status_text(QString::from(error.to_string()));
                return false;
            }
        };
        self.execute_parsed_commands(commands)
    }
}
