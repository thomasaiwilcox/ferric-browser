use super::{
    CxxQtType, Effect, EngineEffect, IpcRoute, ParsedCommand, Pin, PublicError, RuntimeDispatch,
    Value, ipc_command_invocation, parse_action_invocation, qobject,
};
use ferric_browser_ipc::ErrorCode;

impl qobject::BrowserUi {
    pub(super) fn execute_ipc_command_with_operation(
        self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<Value, PublicError> {
        self.execute_ipc_command_in_turn(command, route, operation_id, None)
    }

    pub(super) fn execute_ipc_command_in_turn(
        self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
    ) -> Result<Value, PublicError> {
        let read = preparation
            .filter(|turn| turn.is_current(self.as_ref().get_ref()))
            .and_then(|_| {
                super::browser_ui_configuration_read::ConfigurationReadTurn::capture(
                    self.as_ref().get_ref(),
                )
            });
        self.execute_ipc_command_with_read(command, route, operation_id, preparation, read.as_ref())
    }

    pub(super) fn execute_ipc_command_with_read(
        mut self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
        read: Option<&super::browser_ui_configuration_read::ConfigurationReadTurn>,
    ) -> Result<Value, PublicError> {
        let expanded = self
            .as_ref()
            .rust()
            .registry
            .expand_command(command)
            .map_err(|error| {
                PublicError::new(
                    ErrorCode::InvalidArgument,
                    "The command couldn't be expanded. Check its definition.",
                    error.to_string(),
                )
            })?;
        super::browser_ui_hint_pointer::reject_external_hint_interaction_chain(&expanded).map_err(
            |error| {
                PublicError::new(
                    ErrorCode::Denied,
                    "Use hints from the browser window.",
                    error,
                )
            },
        )?;
        if expanded.len() != 1 {
            return self.execute_routed_ipc_chain_with_read(
                expanded
                    .into_iter()
                    .map(|command| (command, route.clone()))
                    .collect(),
                operation_id,
                &mut None,
                preparation,
                read,
            );
        }
        let command = expanded.into_iter().next().expect("one expanded command");
        if command.name == "get" || super::browser_ui_site_policy::uses_read_turn(&command) {
            if preparation.is_some_and(|turn| !turn.is_current(self.as_ref().get_ref())) {
                return Err("The command owner changed during settings preparation".into());
            }
            if operation_id.is_some_and(|id| {
                self.as_ref()
                    .rust()
                    .operation(id)
                    .is_none_or(|record| record.state.is_terminal())
            }) {
                return Err(crate::PublicError::new(
                    ferric_browser_ipc::ErrorCode::Cancelled,
                    "The command was cancelled.",
                    "read operation is no longer live",
                ));
            }

            let rebound = preparation
                .filter(|turn| turn.is_current(self.as_ref().get_ref()))
                .and_then(|_| {
                    super::browser_ui_configuration_read::ConfigurationReadTurn::capture(
                        self.as_ref().get_ref(),
                    )
                });
            let read = rebound.as_ref().or(read);
            self.as_ref().validate_configuration_get_route(route)?;
            if let Some(read) = read {
                return self
                    .as_mut()
                    .execute_configuration_read_command(&command, read);
            }
            return self.queue_configuration_read(
                command,
                super::browser_ui_page_windows::PageContinuation::Ipc {
                    commands: Vec::new(),
                    route: route.clone(),
                },
                operation_id,
                &mut None,
            );
        }

        if !self.as_ref().in_page_window_mode()
            && super::browser_ui_page_workflows::needs_read_expansion(std::iter::once(&command))
            && let Some(commands) = super::browser_ui_page_workflows::expanded_read_workflow(
                self.as_ref().rust(),
                vec![crate::RepeatableCommand {
                    command: command.clone(),
                    count: 1,
                    route: Some(route.clone()),
                }],
            )
        {
            return self.execute_page_recorded_commands_with_read(
                commands,
                operation_id,
                &mut None,
                preparation,
                read,
            );
        }
        if preparation.is_none()
            && !(super::browser_ui_site_policy::is_draft_management(&command) && read.is_some())
            && super::browser_ui_configuration_preparation::needs_preparation(
                &command,
                &self.as_ref().rust().registry,
            )
        {
            self.as_ref().validate_ipc_route_public(route)?;
            return self.queue_configuration_preparation(
                super::browser_ui_configuration_preparation::DeferredConfigurationAction::Commands(
                    super::browser_ui_page_windows::PageContinuation::Ipc {
                        commands: vec![(command, route.clone())], route: route.clone(),
                    }), operation_id, &mut None,
            ).map_err(Into::into);
        }
        if self.as_ref().in_page_window_mode()
            && (matches!(
                command.name.as_str(),
                "repeat" | "macro-play" | "command-execute"
            ) || (command.name == "action" && crate::parse_action_invocation(&command).is_ok()))
        {
            let wrapper = command.clone();
            let mut result = self.execute_page_workflow_with_read(
                vec![crate::RepeatableCommand {
                    command,
                    count: 1,
                    route: Some(route.clone()),
                }],
                operation_id,
                &mut None,
                preparation,
                read,
            )?;
            if wrapper.name == "action" {
                let (_, id) =
                    crate::parse_action_invocation(&wrapper).map_err(PublicError::engine)?;
                // A registered subject/verb may have taken precedence.
                if result.get("action_id").is_none() {
                    result["action_id"] = Value::String(id);
                }
            } else if wrapper.name == "command-execute" {
                let mut outer = serde_json::json!({"status":"accepted", "action":"execute",
                    "command_id":wrapper.arguments[0], "result":result});
                for field in ["operation_id", "pending"] {
                    if let Some(value) = outer["result"].get(field).cloned() {
                        outer[field] = value;
                    }
                }
                result = outer;
            }
            return Ok(result);
        }
        if let Some(result) = self.as_mut().dispatch_support_command(&command, route) {
            return result;
        }
        if matches!(
            command.name.as_str(),
            "download"
                | "download-cancel"
                | "download-pause"
                | "download-resume"
                | "download-retry"
        ) {
            if route.context.is_some() {
                return Err(PublicError::new(
                    ErrorCode::InvalidArgument,
                    "A download command can't be combined with a context selector.",
                    "context routing is not valid for downloads",
                ));
            }
            self.as_ref().validate_ipc_route_public(route)?;
            return if command.name == "download" {
                self.execute_download_command(&command)
            } else {
                self.execute_download_control_command(&command)
            };
        }
        if command.name == "action" {
            return self.execute_action_ipc_command(
                &command,
                route,
                operation_id,
                preparation,
                read,
            );
        }
        if command.name == "context-delete" {
            if route.context.is_some() || route.profile.is_some() {
                return Err(PublicError::new(
                    ErrorCode::InvalidArgument,
                    "Delete a context in its current profile without a context or profile selector.",
                    "context routing is not valid for context lifecycle commands",
                ));
            }
            self.as_ref().validate_ipc_route_public(route)?;
            return self.execute_context_delete_command(&command);
        }
        if self.as_ref().in_page_window_mode()
            && super::browser_ui_page_creation::handles(&command, Some(route))
        {
            return self.execute_page_window_ipc_creation(
                &command,
                route,
                Vec::new(),
                operation_id,
                &mut None,
                preparation,
            );
        }
        if self.as_ref().in_page_window_mode() && command.name == "tab-close" {
            return self.execute_page_window_ipc_close(
                &command,
                route,
                Vec::new(),
                operation_id,
                &mut None,
                preparation,
            );
        }
        if super::browser_ui_site_policy::is_draft_management(&command) && preparation.is_none() {
            if route.context.is_some() {
                return Err("context routing is not valid for Site Policies drafts".into());
            }
            self.as_ref().validate_ipc_route_public(route)?;
            let read = read.ok_or_else(|| {
                PublicError::engine("Site Policies edit requires a cold owner-bound turn")
            })?;
            return self
                .execute_site_policy_management_read(&command, read)
                .map_err(PublicError::engine);
        }
        let result = self.as_mut().execute_adapter_ipc_command(
            command.clone(),
            route,
            operation_id,
            preparation,
        )?;
        // A preview or routing prompt has not executed the navigation. Clean
        // opens dispatch their accepted, cleaned command separately, either
        // immediately or after confirmation; recording here would duplicate it.
        if result.get("pending").and_then(Value::as_str) != Some("context-route")
            && result.get("requires_confirmation").and_then(Value::as_bool) != Some(true)
            && !(self.as_ref().in_page_window_mode()
                && (super::browser_ui_page_windows::handles(&command)
                    || super::browser_ui_page_mutations::handles(&command)))
            && !(command.name == "open"
                && command.arguments.first().map(String::as_str) == Some("--clean-link"))
        {
            self.as_mut()
                .note_repeatable_invocation(&command, Some(route), 1);
        }
        Ok(result)
    }

    fn execute_action_ipc_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
        read: Option<&super::browser_ui_configuration_read::ConfigurationReadTurn>,
    ) -> Result<Value, PublicError> {
        self.as_ref().validate_action_ipc_route(route)?;
        if let Some((mut result, action_id)) =
            self.as_mut().execute_userscript_action_command(command)?
        {
            result["action_id"] = Value::String(action_id);
            return Ok(result);
        }
        let (mapped, action_id) = parse_action_invocation(command).map_err(|error| {
            PublicError::new(
                ErrorCode::InvalidArgument,
                "Check this action and its arguments with :action-list.",
                error,
            )
        })?;
        let mut result = self.as_mut().execute_ipc_command_with_read(
            mapped,
            route,
            operation_id,
            preparation,
            read,
        )?;
        result["action_id"] = Value::String(action_id);
        Ok(result)
    }

    pub(super) fn execute_ipc_command(
        self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
    ) -> Result<Value, String> {
        self.execute_ipc_command_with_operation(command, route, None)
            .map_err(|error| error.to_string())
    }

    pub(super) fn dispatch_fallback_ipc_command(
        mut self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<Value, PublicError> {
        let dispatch = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let state = rust
                .state
                .as_ref()
                .ok_or_else(|| "core state unavailable".to_owned())?;
            let selector = if command.name == "edit" && route.source == super::CommandSource::Ui {
                rust.tab.map_or(route.selector, super::DispatchTarget::Tab)
            } else {
                route.selector
            };
            RuntimeDispatch {
                invocation: ipc_command_invocation(
                    state,
                    command,
                    selector,
                    route.source,
                    operation_id,
                ),
                navigation: binding.navigation_context_for_source(route.source),
                target: selector,
            }
        };
        let effects = self.as_mut().dispatch_runtime_public(dispatch)?;
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

    pub(super) fn dispatch_new_tab_open(
        self: Pin<&mut Self>,
        input: &str,
        route: &IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        self.as_ref()
            .validate_ipc_route_public(route)
            .map_err(|error| error.to_string())?;
        let mut arguments = Vec::new();
        if route.open_target == super::IpcOpenTarget::BackgroundTab {
            arguments.push("--background".into());
        }
        arguments.extend(["--".into(), input.to_owned()]);
        self.dispatch_tab_open_ipc_command(
            ParsedCommand {
                name: "tab-open".into(),
                arguments,
            },
            route,
            operation_id,
        )
    }

    pub(super) fn dispatch_planned_new_tab_open(
        self: Pin<&mut Self>,
        input: &str,
        route: &IpcRoute,
        foreground: bool,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        self.as_ref()
            .validate_ipc_route_public(route)
            .map_err(|error| error.to_string())?;
        self.dispatch_tab_open_ipc_command(
            planned_tab_open_command(input, foreground),
            route,
            operation_id,
        )
    }

    pub(super) fn dispatch_tab_open_ipc_command(
        mut self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        if self.as_ref().in_page_window_mode() && route.context.is_none() && route.profile.is_none()
        {
            return self
                .execute_page_window_ipc_creation(
                    &command,
                    route,
                    Vec::new(),
                    operation_id,
                    &mut None,
                    None,
                )
                .map_err(|error| error.to_string());
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
        let source_tab = dispatch.invocation.context.tab;
        let journey_parent = source_tab.and_then(|tab| {
            self.as_ref()
                .rust()
                .state
                .as_ref()?
                .journey()
                .current_node(tab)
        });
        let effects = self.as_mut().dispatch_runtime(dispatch)?;
        if let Some(parent) = journey_parent {
            self.as_mut().mark_journey_parent(&effects, parent);
        }
        let tab_id = effects.iter().find_map(|effect| match effect {
            Effect::Engine(EngineEffect::Navigate { target, .. }) => Some(target.tab.to_string()),
            _ => None,
        });
        self.as_mut().sync_tab_order_from_core();
        self.as_mut().set_pending_engine_action(&effects);
        Ok(serde_json::json!({
            "status": "accepted",
            "tab_id": tab_id
        }))
    }
}

pub(super) fn planned_tab_open_command(input: &str, foreground: bool) -> ParsedCommand {
    let mut arguments = Vec::new();
    if !foreground {
        arguments.push("--background".into());
    }
    arguments.extend(["--".into(), input.to_owned()]);
    ParsedCommand {
        name: "tab-open".into(),
        arguments,
    }
}

#[cfg(test)]
#[path = "tests/page_presentation.rs"]
mod page_presentation_tests;
