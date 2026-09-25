use super::{
    BindingOutcome, CommandInvocation, CxxQtType, DispatchTarget, Mode, ParseInput, ParsedCommand,
    Pin, QString, RuntimeDispatch, binding_uses_full_command_executor, elapsed_ms,
    engine_action_name, modal_command_prefill, operation_is_terminal, operation_status_kind,
    parse_chain, publish_ipc_event, qobject, valid_macro_register,
};

impl qobject::BrowserUi {
    pub(super) fn queue_pending_engine_action(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if let Some(action) = this.pending_engine_action.take() {
            this.pending_engine_actions.push_back(action);
        }
        if let Some(target) = this.pending_journey_traversal.take() {
            this.pending_journey_traversals.push_back(target);
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn apply_binding_outcome(mut self: Pin<&mut Self>, outcome: BindingOutcome) {
        match outcome {
            BindingOutcome::Execute { command, count } => {
                self.as_mut().set_binding_overlay(QString::default());
                let mut parsed_commands = match parse_chain(&command, ParseInput::Interactive) {
                    Ok(commands) => commands,
                    Err(error) => {
                        self.set_status_text(QString::from(format!("Binding rejected: {error}")));
                        return;
                    }
                };
                if parsed_commands.len() != 1 {
                    self.set_status_text(QString::from(
                        "Binding rejected: command must contain exactly one command",
                    ));
                    return;
                }
                let mut parsed = parsed_commands.remove(0);
                let registry = self.as_ref().rust().registry.clone();
                if parsed.name == "hint" && parsed.arguments.is_empty() {
                    self.as_mut().set_hint_mode_kind(true);
                    return;
                }
                if parsed.name == "repeat" && count > 1 {
                    parsed.arguments = vec!["--count".into(), count.to_string()];
                }
                if matches!(parsed.name.as_str(), "scroll" | "scroll-page")
                    && count > 1
                    && !parsed
                        .arguments
                        .iter()
                        .any(|argument| argument == "--count")
                {
                    parsed
                        .arguments
                        .extend(["--count".into(), count.to_string()]);
                }
                let expanded = match registry.expand_command(parsed) {
                    Ok(commands) => commands,
                    Err(error) => {
                        self.set_status_text(QString::from(format!("Binding rejected: {error}")));
                        return;
                    }
                };
                if expanded.len() != 1 {
                    let _ = self.as_mut().execute_parsed_commands(expanded);
                    return;
                }
                let parsed = expanded.into_iter().next().expect("one expanded command");
                let current_url = self.as_ref().rust().current_url.to_string();
                if let Some(prefill) = modal_command_prefill(&parsed, &current_url) {
                    self.as_mut().enter_command();
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_action = Some(format!("command-prefill\t{prefill}"));
                    return;
                }
                if binding_uses_full_command_executor(&parsed) {
                    let _ = self.as_mut().execute_parsed_commands(vec![parsed]);
                    return;
                }
                self.as_mut().note_repeatable_command(&parsed);
                let effects = {
                    if self.as_ref().rust().state.is_none() {
                        self.set_status_text(QString::from("Core state unavailable"));
                        return;
                    }
                    let navigation = self.as_ref().navigation_context();
                    self.as_mut().dispatch_runtime(RuntimeDispatch {
                        invocation: CommandInvocation::with_count(parsed, count),
                        navigation,
                        target: DispatchTarget::Active,
                    })
                };
                let effects = match effects {
                    Ok(effects) => effects,
                    Err(error) => {
                        self.set_status_text(QString::from(error.to_string()));
                        return;
                    }
                };
                if let Some(action) = effects.iter().find_map(engine_action_name) {
                    let mut rust = self.as_mut().rust_mut();
                    rust.as_mut().get_mut().pending_engine_action = Some(action.to_owned());
                }
                self.as_mut().sync_core_tabs();
                let learning_mode = self.as_ref().rust().learning_mode;
                self.set_status_text(QString::from(if learning_mode {
                    format!("Binding executed: {command}")
                } else {
                    "Binding executed".into()
                }));
            }
            BindingOutcome::Pending {
                prefix,
                continuations,
                ..
            } => {
                let prefix_text = if prefix.is_empty() {
                    "count".to_owned()
                } else {
                    prefix.join(" ")
                };
                let continuation_text = if continuations.is_empty() {
                    "exact command".to_owned()
                } else {
                    continuations.join(" ")
                };
                self.as_mut().set_binding_overlay(QString::from(format!(
                    "{prefix_text} · next: {continuation_text} · Esc cancels"
                )));
                self.set_status_text(QString::from(format!(
                    "{} · {}",
                    prefix.join(" "),
                    continuations.join(" ")
                )));
            }
            BindingOutcome::Rejected { reason } => {
                self.as_mut().set_binding_overlay(QString::default());
                self.set_status_text(QString::from(reason));
            }
            BindingOutcome::Consumed => self.set_binding_overlay(QString::default()),
        }
    }

    pub(super) fn handle_key(mut self: Pin<&mut Self>, key: &QString) -> bool {
        let now_ms = elapsed_ms(self.as_ref().rust().binding_clock);
        let key = key.to_string();
        if let Some(result) = self.as_mut().handle_macro_key(&key, now_ms) {
            return result;
        }
        let outcome = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(bindings) = this.bindings.as_mut() else {
                return false;
            };
            bindings.feed(&key, now_ms)
        };
        self.as_mut().apply_binding_outcome(outcome);
        true
    }

    pub(super) fn handle_macro_key(
        mut self: Pin<&mut Self>,
        key: &str,
        now_ms: u64,
    ) -> Option<bool> {
        let active_prefix = self.as_ref().rust().macro_key_prefix.clone();
        if let Some(prefix) = active_prefix {
            let expired = self
                .as_ref()
                .rust()
                .macro_key_started_ms
                .is_some_and(|started| {
                    now_ms >= started.saturating_add(ferric_browser_core::DEFAULT_CHORD_TIMEOUT_MS)
                });
            if expired {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.macro_key_prefix = None;
                this.macro_key_started_ms = None;
                self.as_mut().set_binding_overlay(QString::default());
                self.set_status_text(QString::from("Macro register prefix timed out"));
                return Some(true);
            }

            {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.macro_key_prefix = None;
                this.macro_key_started_ms = None;
            }
            self.as_mut().set_binding_overlay(QString::default());
            if key.len() != 1 || !key.is_ascii() || !valid_macro_register(key) {
                self.set_status_text(QString::from(
                    "Macro register must be one ASCII letter or digit",
                ));
                return Some(true);
            }

            let command = if prefix == "q" {
                let recording = self
                    .as_ref()
                    .rust()
                    .recording_macro
                    .as_ref()
                    .map(|(register, _)| register.clone());
                if recording.as_deref() == Some(key) {
                    ParsedCommand {
                        name: "macro-stop".into(),
                        arguments: Vec::new(),
                    }
                } else if recording.is_some() {
                    self.set_status_text(QString::from(
                        "Stop the active macro before recording another register",
                    ));
                    return Some(true);
                } else {
                    ParsedCommand {
                        name: "macro-record".into(),
                        arguments: vec![key.into()],
                    }
                }
            } else {
                ParsedCommand {
                    name: "macro-play".into(),
                    arguments: vec![key.into()],
                }
            };
            return Some(self.as_mut().execute_parsed_commands(vec![command]));
        }

        if self.as_ref().rust().core_mode != Mode::Normal || !matches!(key, "q" | "@") {
            return None;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if let Some(bindings) = this.bindings.as_mut() {
                bindings.reset();
            }
            this.macro_key_prefix = Some(key.into());
            this.macro_key_started_ms = Some(now_ms);
        }
        self.as_mut().set_binding_overlay(QString::from(format!(
            "{key} · next: register a-z/0-9 · Esc cancels"
        )));
        self.set_status_text(QString::from(if key == "q" {
            "Macro recording: waiting for register"
        } else {
            "Macro replay: waiting for register"
        }));
        Some(true)
    }

    pub(super) fn tick_bindings(mut self: Pin<&mut Self>) {
        let now_ms = elapsed_ms(self.as_ref().rust().binding_clock);
        let macro_prefix_expired =
            self.as_ref()
                .rust()
                .macro_key_started_ms
                .is_some_and(|started| {
                    now_ms >= started.saturating_add(ferric_browser_core::DEFAULT_CHORD_TIMEOUT_MS)
                });
        if macro_prefix_expired {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.macro_key_prefix = None;
            this.macro_key_started_ms = None;
            self.as_mut().set_binding_overlay(QString::default());
            self.set_status_text(QString::from("Macro register prefix timed out"));
            return;
        }
        let outcome = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .bindings
                .as_mut()
                .and_then(|bindings| bindings.tick(now_ms))
        };
        if let Some(outcome) = outcome {
            self.as_mut().apply_binding_outcome(outcome);
        }
    }

    pub(super) fn take_engine_action(mut self: Pin<&mut Self>) -> QString {
        let (action, more_pending) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let action = this
                .pending_engine_actions
                .pop_front()
                .or_else(|| this.pending_engine_action.take());
            let more_pending =
                !this.pending_engine_actions.is_empty() || this.pending_engine_action.is_some();
            (action, more_pending)
        };
        if more_pending {
            self.as_mut().runtime_work_available();
        }
        action.map_or_else(QString::default, QString::from)
    }

    pub(super) fn complete_window_focus(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
        outcome: &QString,
    ) -> bool {
        let operation_id = operation_id.to_string();
        if operation_id.is_empty() || operation_id.len() > 128 {
            return false;
        }
        let outcome = outcome.to_string();
        let status = match outcome.as_str() {
            "activated" => "completed: activated",
            "unknown" => "completed: unknown",
            "stale" => "failed (stale activation target)",
            _ => return false,
        };
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if !this.operation_states.contains_key(&operation_id)
            || this
                .operation_states
                .get(&operation_id)
                .is_some_and(|current| operation_is_terminal(current))
        {
            return false;
        }
        this.operation_states
            .insert(operation_id.clone(), status.into());
        publish_ipc_event(
            &mut this.ipc_sequence,
            "operation.completed",
            serde_json::json!({
                "operation_id": operation_id,
                "status": operation_status_kind(status),
                "activation": outcome
            }),
        );
        true
    }

    pub(super) fn complete_transfer_operation(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
        succeeded: bool,
    ) -> bool {
        let operation_id = operation_id.to_string();
        if operation_id.is_empty() || operation_id.len() > 128 {
            return false;
        }
        let status = if succeeded {
            "completed: transferred"
        } else {
            "failed (live tab transfer failed)"
        };
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if !this.operation_states.contains_key(&operation_id)
            || this
                .operation_states
                .get(&operation_id)
                .is_some_and(|current| operation_is_terminal(current))
        {
            return false;
        }
        this.operation_states
            .insert(operation_id.clone(), status.into());
        publish_ipc_event(
            &mut this.ipc_sequence,
            "operation.completed",
            serde_json::json!({
                "operation_id": operation_id,
                "status": operation_status_kind(status),
                "transfer": succeeded
            }),
        );
        true
    }
}
