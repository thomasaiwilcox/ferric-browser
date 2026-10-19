use super::{
    BindingOutcome, CommandInvocation, CommandSource, CxxQtType, DispatchTarget, Mode, ParseInput,
    ParsedCommand, Pin, QString, RuntimeDispatch, binding_uses_full_command_executor, elapsed_ms,
    engine_action_name, modal_command_prefill, parse_chain, parse_hint_options, qobject,
    valid_macro_register,
};
use crate::hint_policy::hint_requests_pointer;
use crate::{FailureKind, OperationUpdate};

impl qobject::BrowserUi {
    pub(super) fn execute_command(self: Pin<&mut Self>, input: &QString) -> bool {
        let commands = match parse_chain(&input.to_string(), ParseInput::Interactive) {
            Ok(commands) => commands,
            Err(error) => {
                self.set_status_text(QString::from(error.to_string()));
                return false;
            }
        };
        if commands.len() == 1 && hint_requests_pointer(&commands[0]) {
            let options = match parse_hint_options(&commands[0]) {
                Ok(options) => options,
                Err(error) => {
                    self.set_status_text(QString::from(error));
                    return false;
                }
            };
            if let Err(error) = self
                .as_ref()
                .rust()
                .registry
                .validate(&commands[0], self.as_ref().rust().core_mode)
            {
                self.set_status_text(QString::from(error.to_string()));
                return false;
            }
            return self.set_local_pointer_hint_options(&options, CommandSource::Ui);
        }
        self.execute_parsed_commands(commands)
    }

    pub(super) fn queue_pending_engine_action(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if let Some(action) = this.pending_engine_action.take() {
            this.pending_engine_actions
                .push_back(super::engine_requests::NativeRequest::Presentation(action));
        }
        if let Some(target) = this.navigation.pending_journey_traversal.take() {
            this.navigation.pending_journey_traversals.push_back(target);
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
                if hint_requests_pointer(&parsed) {
                    if count != 1 {
                        self.set_status_text(QString::from(
                            "Pointer click does not accept a key count",
                        ));
                        return;
                    }
                    let options = match parse_hint_options(&parsed) {
                        Ok(options) => options,
                        Err(error) => {
                            self.set_status_text(QString::from(error));
                            return;
                        }
                    };
                    if let Err(error) = self
                        .as_ref()
                        .rust()
                        .registry
                        .validate(&parsed, self.as_ref().rust().core_mode)
                    {
                        self.set_status_text(QString::from(error.to_string()));
                        return;
                    }
                    let _ = self
                        .as_mut()
                        .set_local_pointer_hint_options(&options, CommandSource::Keyboard);
                    return;
                }
                let registry = self.as_ref().rust().registry.clone();
                if parsed.name == "scroll-target" && count > 1 {
                    self.set_status_text(QString::from("scroll-target does not accept a count"));
                    return;
                }
                if parsed.name == "hint" && parsed.arguments.is_empty() {
                    self.as_mut().set_hint_mode_kind(true);
                    return;
                }
                if parsed.name == "hint" && count > 1 {
                    if parsed
                        .arguments
                        .iter()
                        .any(|argument| argument == "--first")
                    {
                        parsed
                            .arguments
                            .extend(["--index".into(), count.min(5_000).to_string()]);
                    } else {
                        self.set_status_text(QString::from(
                            "A hint count is only valid with --first",
                        ));
                        return;
                    }
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
                if let Some(result) = self
                    .as_mut()
                    .handle_spatial_command(&parsed.name, &parsed.arguments)
                {
                    if result {
                        self.as_mut().set_binding_overlay(QString::default());
                    }
                    return;
                }
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

    pub(super) fn clipboard_dispatch_target(&self, command: &ParsedCommand) -> DispatchTarget {
        if command.name == "edit" {
            self.rust()
                .tab
                .map_or(DispatchTarget::Active, DispatchTarget::Tab)
        } else {
            DispatchTarget::Active
        }
    }

    pub(super) fn clipboard_edit_target_current(
        self: Pin<&mut Self>,
        tab: &QString,
        generation: &QString,
        document: &QString,
        popup_token: &QString,
    ) -> bool {
        let binding = self.as_ref();
        let this = binding.rust();
        let token = popup_token.to_string();
        let target = if token.is_empty() {
            super::TabId::from_display(&tab.to_string())
                .and_then(|tab| this.state.as_ref()?.capture_target(tab))
        } else {
            this.navigation
                .popup_journey_targets
                .iter()
                .find(|(candidate, _, _, claimed)| candidate == &token && *claimed)
                .map(|(_, target, _, _)| *target)
                .filter(|target| {
                    this.state
                        .as_ref()
                        .and_then(|state| state.capture_target(target.tab))
                        == Some(*target)
                })
        };
        target.is_some_and(|target| {
            target.tab.to_string() == tab.to_string()
                && target.generation.to_string() == generation.to_string()
                && target.document.to_string() == document.to_string()
        })
    }

    pub(super) fn popup_clipboard_edit(
        mut self: Pin<&mut Self>,
        token: &QString,
        operation: &QString,
    ) -> super::QVariant {
        let operation = operation.to_string();
        if ferric_browser_core::ClipboardEdit::parse(&operation).is_none() {
            self.set_status_text(QString::from("Unsupported native edit operation"));
            return super::QVariant::default();
        }
        let token = token.to_string();
        let target = {
            let binding = self.as_ref();
            let this = binding.rust();
            this.navigation
                .popup_journey_targets
                .iter()
                .find(|(candidate, _, _, claimed)| candidate == &token && *claimed)
                .map(|(_, target, _, _)| *target)
                .filter(|target| {
                    this.state
                        .as_ref()
                        .and_then(|state| state.capture_target(target.tab))
                        == Some(*target)
                })
        };
        let Some(target) = target else {
            self.set_status_text(QString::from(
                "The popup page changed. Focus the field and try again.",
            ));
            return super::QVariant::default();
        };
        let navigation = self.as_ref().navigation_context();
        let command = ParsedCommand {
            name: "edit".into(),
            arguments: vec![operation],
        };
        let mut invocation = CommandInvocation::new(command);
        invocation.context.source = CommandSource::Ui;
        match self.as_mut().dispatch_runtime(RuntimeDispatch {
            invocation,
            navigation,
            target: DispatchTarget::Tab(target.tab),
        }) {
            Ok(effects) => effects
                .iter()
                .find_map(|effect| match effect {
                    super::Effect::Engine(effect @ super::EngineEffect::ClipboardEdit { .. }) => {
                        Some(super::engine_requests::project(effect))
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            Err(error) => {
                self.set_status_text(QString::from(error));
                super::QVariant::default()
            }
        }
    }

    pub(super) fn take_engine_action(mut self: Pin<&mut Self>) -> QString {
        let (action, more_pending) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let action = match this.pending_engine_actions.front() {
                Some(super::engine_requests::NativeRequest::Presentation(_)) => {
                    match this.pending_engine_actions.pop_front() {
                        Some(super::engine_requests::NativeRequest::Presentation(action)) => {
                            Some(action)
                        }
                        _ => unreachable!("queue front was a presentation request"),
                    }
                }
                Some(
                    super::engine_requests::NativeRequest::Engine(_)
                    | super::engine_requests::NativeRequest::View(_),
                ) => None,
                None => this.pending_engine_action.take(),
            };
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
            "activated" => OperationUpdate::completed("activated"),
            "unknown" => OperationUpdate::completed("activation outcome unknown"),
            "stale" => {
                OperationUpdate::failed(FailureKind::StaleTarget, "activation target changed")
            }
            _ => return false,
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .set_operation(operation_id, status)
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
            OperationUpdate::completed("transferred")
        } else {
            OperationUpdate::failed(FailureKind::Engine, "live tab transfer failed")
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .set_operation(operation_id, status)
    }
}
