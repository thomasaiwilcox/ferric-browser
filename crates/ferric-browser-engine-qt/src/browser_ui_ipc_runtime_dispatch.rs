use super::{
    CxxQtType, Effect, EngineEffect, IpcRoute, ParsedCommand, Pin, PublicError, RuntimeDispatch,
    Value, ipc_command_invocation, parse_action_invocation, qobject,
};
use ferric_browser_ipc::ErrorCode;

impl qobject::BrowserUi {
    pub(super) fn execute_ipc_command_with_operation(
        mut self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
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
            let mut result = serde_json::json!({"status": "accepted"});
            let mut operations = Vec::new();
            for command in expanded {
                // Each asynchronous command owns its own admission. Reusing
                // the envelope ID would let the first completion terminate
                // unrelated work or replace its cancellation authority.
                result = self
                    .as_mut()
                    .execute_ipc_command_with_operation(command, route, None)?;
                if let Some(id) = result.get("operation_id").and_then(Value::as_str) {
                    operations.push(id.to_owned());
                }
            }
            result["operation_ids"] = serde_json::json!(operations);
            return Ok(result);
        }
        let command = expanded.into_iter().next().expect("one expanded command");
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
            return self.execute_action_ipc_command(&command, route, operation_id);
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
        let result =
            self.as_mut()
                .execute_adapter_ipc_command(command.clone(), route, operation_id)?;
        // A preview or routing prompt has not executed the navigation. Clean
        // opens dispatch their accepted, cleaned command separately, either
        // immediately or after confirmation; recording here would duplicate it.
        if result.get("pending").and_then(Value::as_str) != Some("context-route")
            && result.get("requires_confirmation").and_then(Value::as_bool) != Some(true)
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
        let mut result =
            self.as_mut()
                .execute_ipc_command_with_operation(mapped, route, operation_id)?;
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

    pub(super) fn dispatch_tab_open_ipc_command(
        mut self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
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
