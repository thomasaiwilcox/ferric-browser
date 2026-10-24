use super::{
    CommandRegistry, CxxQtType, IpcRoute, ParsedCommand, Pin, PublicError, QString, Value,
    interactive_open_command, is_context_command, is_profile_command, is_session_command,
    parse_action_invocation, qobject,
};
use ferric_browser_ipc::ErrorCode;

pub(super) enum CommandDispatchControl {
    Unhandled,
    Continue,
    Failed,
}

impl qobject::BrowserUi {
    pub(super) fn execute_management_surface_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, PublicError> {
        let (action, label) = match command.name.as_str() {
            "settings" => ("show-settings", "settings"),
            "profiles" => ("show-profiles", "profiles"),
            "sessions" => ("show-sessions", "saved sessions"),
            _ => {
                return Err(PublicError::new(
                    ErrorCode::Unsupported,
                    "This browser panel is unavailable.",
                    "unknown management surface",
                ));
            }
        };
        if !command.arguments.is_empty() {
            return Err(PublicError::new(
                ErrorCode::InvalidArgument,
                format!(
                    "{} does not accept arguments. Use :{} to open it.",
                    command.name, command.name
                ),
                "management surface received unexpected arguments",
            ));
        }
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| {
                PublicError::new(
                    ErrorCode::InvalidArgument,
                    "This panel command can't run in the current mode.",
                    error.to_string(),
                )
            })?;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(action.into());
        self.as_mut()
            .set_status_text(QString::from(format!("Opening {label}")));
        Ok(serde_json::json!({"status": "accepted", "surface": command.name}))
    }

    pub(super) fn dispatch_support_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &IpcRoute,
    ) -> Option<Result<Value, PublicError>> {
        if !matches!(
            command.name.as_str(),
            "help" | "version" | "diagnostics" | "settings" | "profiles" | "sessions" | "get"
        ) {
            return None;
        }
        if route.context.is_some() {
            return Some(Err(PublicError::new(
                ErrorCode::InvalidArgument,
                "This command can't be combined with a context selector.",
                "support command received context routing",
            )));
        }
        if matches!(command.name.as_str(), "settings" | "profiles" | "sessions")
            && route.open_target != super::IpcOpenTarget::Tab
        {
            return Some(Err(PublicError::new(
                ErrorCode::InvalidArgument,
                "Browser panels open in an existing window.",
                "panel received an open target",
            )));
        }
        if let Err(error) = self.as_ref().validate_ipc_route_public(route) {
            return Some(Err(error));
        }
        if matches!(command.name.as_str(), "settings" | "profiles" | "sessions") {
            match self.as_ref().ipc_route_window(route) {
                Ok(window) if Some(window) == self.as_ref().rust().window => {}
                Ok(_) => {
                    return Some(Err(PublicError::new(
                        ErrorCode::Unsupported,
                        "Open this panel in the primary browser window.",
                        "target window cannot present this panel",
                    )));
                }
                Err(error) => return Some(Err(error)),
            }
        }
        Some(match command.name.as_str() {
            "get" => self.as_mut().execute_config_get_command(command),
            "help" => self
                .as_mut()
                .execute_help_command(command)
                .map_err(PublicError::engine),
            "version" => self
                .as_mut()
                .execute_version_command(command)
                .map_err(PublicError::engine),
            "diagnostics" => self
                .as_mut()
                .execute_diagnostics_command(command)
                .map_err(PublicError::engine),
            _ => self.as_mut().execute_management_surface_command(command),
        })
    }

    pub(super) fn dispatch_workflow_command(
        mut self: Pin<&mut Self>,
        command: &mut ParsedCommand,
        registry: &CommandRegistry,
    ) -> CommandDispatchControl {
        if !self.as_mut().admit_macro_command() {
            return CommandDispatchControl::Failed;
        }
        if let Some(result) = self
            .as_mut()
            .handle_spatial_command(&command.name, &command.arguments)
        {
            return if result {
                CommandDispatchControl::Continue
            } else {
                CommandDispatchControl::Failed
            };
        }
        if command.name == "open-current" {
            let target = match command.arguments.as_slice() {
                [] => "open",
                [flag, target] if flag == "--target" && target == "tab" => "tab-open",
                _ => {
                    self.set_status_text(QString::from("open-current accepts only --target tab"));
                    return CommandDispatchControl::Failed;
                }
            };
            let current_url = self.as_ref().rust().current_url.to_string();
            if current_url.is_empty() {
                self.set_status_text(QString::from("open-current requires a committed URL"));
                return CommandDispatchControl::Failed;
            }
            *command = ParsedCommand {
                name: target.into(),
                arguments: vec![current_url],
            };
        }
        if command.name == "open" {
            match interactive_open_command(command) {
                Ok(Some((parsed, route))) => {
                    if let Err(error) = self.as_mut().execute_ipc_command(parsed, &route) {
                        self.set_status_text(QString::from(error));
                        return CommandDispatchControl::Failed;
                    }
                    return CommandDispatchControl::Continue;
                }
                Ok(None) => {}
                Err(error) => {
                    self.set_status_text(QString::from(error));
                    return CommandDispatchControl::Failed;
                }
            }
        }
        if matches!(command.name.as_str(), "settings" | "profiles" | "sessions") {
            return match self.as_mut().execute_management_surface_command(command) {
                Ok(_) => CommandDispatchControl::Continue,
                Err(error) => {
                    self.set_status_text(QString::from(error.to_string()));
                    CommandDispatchControl::Failed
                }
            };
        }
        if matches!(command.name.as_str(), "quit" | "window-close") {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
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
            return CommandDispatchControl::Continue;
        }
        if command.name == "window-new" {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            if let Err(error) = self.as_mut().execute_window_new_command(command) {
                self.set_status_text(QString::from(error));
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if command.name == "fullscreen" {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            let state = match command.arguments.as_slice() {
                [] => "toggle",
                [state] if matches!(state.as_str(), "on" | "off" | "toggle") => state.as_str(),
                _ => {
                    self.set_status_text(QString::from(
                        "fullscreen accepts optional state on, off, or toggle",
                    ));
                    return CommandDispatchControl::Failed;
                }
            };
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(format!("fullscreen\t{state}"));
            self.as_mut()
                .set_status_text(QString::from(format!("Fullscreen {state} requested")));
            return CommandDispatchControl::Continue;
        }
        if command.name == "repeat" {
            if !self.as_mut().execute_repeat_command(command) {
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if matches!(
            command.name.as_str(),
            "macro-record" | "macro-stop" | "macro-play"
        ) {
            if !self.as_mut().execute_macro_command(command) {
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if command.name == "cancel" {
            if !self.as_mut().execute_cancel_command(command) {
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if command.name == "config-export" {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            if let Err(error) = self.as_mut().execute_config_export_command(command) {
                self.set_status_text(QString::from(error));
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if matches!(command.name.as_str(), "set" | "unset" | "bind" | "unbind") {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            let result = if matches!(command.name.as_str(), "bind" | "unbind") {
                self.as_mut().execute_runtime_binding_command(command)
            } else {
                self.as_mut().execute_runtime_config_command(command)
            };
            if let Err(error) = result {
                self.set_status_text(QString::from(error));
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if command.name == "action-list" {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            let subject = match command.arguments.as_slice() {
                [] => None,
                [subject] => Some(subject.as_str()),
                _ => {
                    self.set_status_text(QString::from("action-list accepts at most one subject"));
                    return CommandDispatchControl::Failed;
                }
            };
            if let Err(error) = self.as_mut().show_action_list(subject) {
                self.set_status_text(QString::from(error));
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if command.name == "switcher" {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            if let Err(error) = self.as_mut().execute_switcher_command(command) {
                self.set_status_text(QString::from(error));
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if command.name == "action" {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            match parse_action_invocation(command) {
                Ok((mapped, _)) => {
                    *command = mapped;
                }
                Err(error) => match self.as_mut().execute_userscript_action_command(command) {
                    Ok(Some(_)) => {
                        return CommandDispatchControl::Continue;
                    }
                    Ok(None) => {
                        self.set_status_text(QString::from(error));
                        return CommandDispatchControl::Failed;
                    }
                    Err(userscript_error) => {
                        self.set_status_text(QString::from(userscript_error.user_message()));
                        return CommandDispatchControl::Failed;
                    }
                },
            }
        }
        if command.name == "send" {
            if let Err(error) = registry.validate(command, self.as_ref().rust().core_mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            if let Err(error) = self.as_mut().execute_send_command(command) {
                self.set_status_text(QString::from(error));
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if is_session_command(&command.name) {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            if !self.as_mut().handle_session_command(command) {
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if is_profile_command(&command.name) {
            if let Err(error) = self.as_mut().execute_profile_command(command) {
                self.set_status_text(QString::from(error));
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        if is_context_command(&command.name) {
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            if let Err(error) = self.as_mut().execute_context_command(command) {
                self.set_status_text(QString::from(error));
                return CommandDispatchControl::Failed;
            }
            return CommandDispatchControl::Continue;
        }
        CommandDispatchControl::Unhandled
    }
}
