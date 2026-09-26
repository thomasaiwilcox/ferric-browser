use super::{
    CommandRegistry, CxxQtType, MAX_MACRO_COMMANDS, ParsedCommand, Pin, QString,
    interactive_open_command, is_context_command, is_profile_command, is_session_command,
    parse_action_invocation, qobject,
};

pub(super) enum CommandDispatchControl {
    Unhandled,
    Continue,
    Failed,
}

impl qobject::BrowserUi {
    pub(super) fn dispatch_workflow_command(
        mut self: Pin<&mut Self>,
        command: &mut ParsedCommand,
        registry: &CommandRegistry,
    ) -> CommandDispatchControl {
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
        if self.as_ref().rust().macro_depth > 0 {
            let expanded = self.as_ref().rust().macro_expanded_commands;
            if expanded >= MAX_MACRO_COMMANDS {
                self.set_status_text(QString::from(
                    "Macro expansion stopped at the 1000-command limit",
                ));
                return CommandDispatchControl::Failed;
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .macro_expanded_commands = expanded + 1;
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
                Ok((mapped, action_id)) => {
                    *command = mapped;
                    self.as_mut()
                        .set_status_text(QString::from(format!("Executing {action_id}")));
                }
                Err(error) => match self.as_mut().execute_userscript_action_command(command) {
                    Ok(Some((_, action_id))) => {
                        self.as_mut().set_status_text(QString::from(format!(
                            "Executing userscript action {action_id}"
                        )));
                        return CommandDispatchControl::Continue;
                    }
                    Ok(None) => {
                        self.set_status_text(QString::from(error));
                        return CommandDispatchControl::Failed;
                    }
                    Err(userscript_error) => {
                        self.set_status_text(QString::from(userscript_error));
                        return CommandDispatchControl::Failed;
                    }
                },
            }
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
            let mode = self.as_ref().rust().core_mode;
            if let Err(error) = registry.validate(command, mode) {
                self.set_status_text(QString::from(error.to_string()));
                return CommandDispatchControl::Failed;
            }
            if !self.as_mut().handle_profile_command(command) {
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
            self.as_mut()
                .set_status_text(QString::from("Context command executed"));
            return CommandDispatchControl::Continue;
        }
        CommandDispatchControl::Unhandled
    }
}
