use super::{
    ActionRegistry, CxxQtType, MAX_MACRO_COMMANDS, MAX_MACRO_DEPTH, ParsedCommand, Pin, QString,
    Value, action_list_value, configured_action_target_values, configured_action_targets,
    discard_config_edit, discard_editor_request, finish_pending_selection_operation,
    is_repeatable_command, macro_status_text, qobject, userscript_action_values,
    valid_macro_register,
};
use crate::{IpcRoute, OperationUpdate, RepeatableCommand};

impl qobject::BrowserUi {
    pub(super) fn show_action_list(
        mut self: Pin<&mut Self>,
        subject: Option<&str>,
    ) -> Result<Value, String> {
        let mut actions = action_list_value(subject)?;
        if subject.is_none() || matches!(subject, Some("url" | "link" | "selection" | "tab")) {
            let targets = configured_action_targets(&self.as_ref().rust().config)?;
            if let Some(values) = actions.as_array_mut() {
                values.extend(
                    targets
                        .iter()
                        .filter(|(_, target)| {
                            subject.is_none_or(|subject| {
                                target
                                    .subject_types
                                    .iter()
                                    .any(|declared| declared == subject)
                            })
                        })
                        .flat_map(|(name, target)| {
                            configured_action_target_values(
                                name,
                                target,
                                self.as_ref().active_profile_is_transient(),
                            )
                            .into_iter()
                            .filter(|action| {
                                subject.is_none_or(|subject| {
                                    action.get("subject").and_then(Value::as_str) == Some(subject)
                                })
                            })
                        }),
                );
            }
        }
        if let Some(values) = actions.as_array_mut() {
            values.extend(userscript_action_values(
                self.as_ref().rust().cached_userscript_actions(),
                subject,
                self.as_ref().userscript_availability(),
            )?);
        }
        if let Some(values) = actions.as_array_mut() {
            let registry = ActionRegistry::default_v1();
            for action in values {
                let Some(id) = action.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(definition) = registry.resolve(id) else {
                    continue;
                };
                action["availability"] = self.as_ref().action_availability(definition);
            }
        }
        let lines = actions
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|action| {
                Some(format!(
                    "{}\t{}\t{}\t{}",
                    action.get("id")?.as_str()?,
                    action.get("label")?.as_str()?,
                    action.get("description")?.as_str()?,
                    action
                        .get("examples")?
                        .as_array()?
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" · ")
                ))
            })
            .collect::<Vec<_>>();
        self.as_mut().set_library_kind(QString::from("actions"));
        self.as_mut()
            .set_library_values(QString::from(lines.join("\n")));
        self.set_status_text(QString::from(format!(
            "{} action(s) available",
            lines.len()
        )));
        Ok(actions)
    }

    pub(super) fn note_repeatable_command(self: Pin<&mut Self>, command: &ParsedCommand) {
        self.note_repeatable_invocation(command, None, 1);
    }

    pub(super) fn note_repeatable_invocation(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: Option<&IpcRoute>,
        count: u32,
    ) {
        if !is_repeatable_command(&command.name) {
            return;
        }
        let mut recording_limit_reached = false;
        let recorded = RepeatableCommand {
            command: command.clone(),
            count,
            route: route.cloned(),
        };
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.last_repeatable = Some(recorded.clone());
            if let Some((_, commands)) = this.recording_macro.as_mut() {
                if commands.len() >= MAX_MACRO_COMMANDS {
                    this.recording_macro = None;
                    recording_limit_reached = true;
                } else {
                    commands.push(recorded);
                }
            }
        }
        if recording_limit_reached {
            self.as_mut().set_status_text(QString::from(
                "Macro recording stopped at the 1000-command limit",
            ));
        }
        self.as_mut().clear_site_experiment_presentation();
        self.as_mut().sync_macro_status_text();
    }

    pub(super) fn sync_macro_status_text(mut self: Pin<&mut Self>) {
        let status = macro_status_text(self.as_ref().rust());
        self.as_mut().set_macro_status_text(QString::from(status));
    }

    pub(super) fn execute_repeat_command(self: Pin<&mut Self>, command: &ParsedCommand) -> bool {
        let count = match command.arguments.as_slice() {
            [] => 1,
            [flag, value] if flag == "--count" => value
                .parse::<usize>()
                .ok()
                .filter(|count| (1..=100).contains(count))
                .unwrap_or(0),
            _ => 0,
        };
        if count == 0 {
            self.set_status_text(QString::from("repeat accepts optional --count N (1..100)"));
            return false;
        }
        let Some(last) = self.as_ref().rust().last_repeatable.clone() else {
            self.set_status_text(QString::from("No eligible command to repeat"));
            return false;
        };
        self.execute_recorded_commands(std::iter::repeat_n(last, count).collect())
    }

    fn execute_recorded_commands(
        mut self: Pin<&mut Self>,
        commands: Vec<RepeatableCommand>,
    ) -> bool {
        let chained = commands.len() > 1;
        for (index, recorded) in commands.into_iter().enumerate() {
            if index > 0 {
                self.as_mut().queue_pending_engine_action();
            }
            if let Some(route) = recorded.route {
                if !self.as_mut().admit_macro_command() {
                    return false;
                }
                if let Err(error) = self.as_mut().execute_ipc_command(recorded.command, &route) {
                    self.set_status_text(QString::from(error));
                    return false;
                }
            } else if !self
                .as_mut()
                .execute_counted_parsed_commands(vec![recorded.command], recorded.count)
            {
                return false;
            }
        }
        if chained {
            self.as_mut().queue_pending_engine_action();
        }
        true
    }

    pub(super) fn admit_macro_command(mut self: Pin<&mut Self>) -> bool {
        if self.as_ref().rust().macro_depth == 0 {
            return true;
        }
        let expanded = self.as_ref().rust().macro_expanded_commands;
        if expanded >= MAX_MACRO_COMMANDS {
            self.set_status_text(QString::from(
                "Macro expansion stopped at the 1000-command limit",
            ));
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .macro_expanded_commands = expanded + 1;
        true
    }

    pub(super) fn execute_macro_command(mut self: Pin<&mut Self>, command: &ParsedCommand) -> bool {
        match command.name.as_str() {
            "macro-record" => {
                let Some(register) = command.arguments.first() else {
                    self.set_status_text(QString::from("macro-record requires REGISTER"));
                    return false;
                };
                if command.arguments.len() != 1 || !valid_macro_register(register) {
                    self.set_status_text(QString::from(
                        "macro register must be one ASCII letter or digit",
                    ));
                    return false;
                }
                let mut rust = self.as_mut().rust_mut();
                rust.as_mut().get_mut().recording_macro = Some((register.clone(), Vec::new()));
                self.as_mut().sync_macro_status_text();
                self.set_status_text(QString::from(format!("Recording macro {register}")));
                true
            }
            "macro-stop" => {
                if !command.arguments.is_empty() {
                    self.set_status_text(QString::from("macro-stop does not accept arguments"));
                    return false;
                }
                let recording = self
                    .as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .recording_macro
                    .take();
                let Some((register, commands)) = recording else {
                    self.set_status_text(QString::from("No macro is recording"));
                    return false;
                };
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .macro_registers
                    .insert(register.clone(), commands);
                self.as_mut().sync_macro_status_text();
                self.set_status_text(QString::from(format!("Recorded macro {register}")));
                true
            }
            "macro-play" => {
                let Some(register) = command.arguments.first() else {
                    self.set_status_text(QString::from("macro-play requires REGISTER"));
                    return false;
                };
                if command.arguments.len() != 1 || !valid_macro_register(register) {
                    self.set_status_text(QString::from(
                        "macro register must be one ASCII letter or digit",
                    ));
                    return false;
                }
                let Some(commands) = self.as_ref().rust().macro_registers.get(register).cloned()
                else {
                    self.set_status_text(QString::from(format!(
                        "Macro {register} is empty or missing"
                    )));
                    return false;
                };
                if commands.is_empty() {
                    self.set_status_text(QString::from(format!("Macro {register} is empty")));
                    return false;
                }
                if self.as_ref().rust().macro_depth >= MAX_MACRO_DEPTH {
                    self.set_status_text(QString::from("macro nesting limit reached"));
                    return false;
                }
                let outermost = self.as_ref().rust().macro_depth == 0;
                if outermost {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .macro_expanded_commands = 0;
                }
                self.as_mut().rust_mut().as_mut().get_mut().macro_depth += 1;
                let result = self.as_mut().execute_recorded_commands(commands);
                self.as_mut().rust_mut().as_mut().get_mut().macro_depth -= 1;
                if outermost {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .macro_expanded_commands = 0;
                }
                self.as_mut().sync_macro_status_text();
                result
            }
            _ => false,
        }
    }

    pub(super) fn execute_cancel_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> bool {
        if command.arguments.len() > 1 {
            self.set_status_text(QString::from("cancel accepts at most one operation ID"));
            return false;
        }
        if let Some(id) = command.arguments.first() {
            return match self.as_mut().ipc_operation_cancel(&serde_json::json!({
                "operation_id": id
            })) {
                Ok(_) => {
                    self.set_status_text(QString::from("Operation cancelled"));
                    true
                }
                Err(error) => {
                    self.set_status_text(QString::from(error.to_string()));
                    false
                }
            };
        }
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        finish_pending_selection_operation(this, OperationUpdate::cancelled());
        this.processes.pending_caret = None;
        if let Some(pending) = this.processes.pending_editor.take() {
            discard_editor_request(pending);
        }
        if let Some(pending) = this.processes.pending_config_edit.take() {
            discard_config_edit(pending);
        }
        super::browser_ui_download_requests::finish_pending_download_operation(
            this,
            OperationUpdate::cancelled(),
        );
        self.set_status_text(QString::from("Pending browser work cancelled"));
        true
    }
}
