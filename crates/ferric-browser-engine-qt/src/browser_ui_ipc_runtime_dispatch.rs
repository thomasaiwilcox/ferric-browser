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
        self.execute_adapter_ipc_command(command, route, operation_id)
            .map_err(PublicError::engine)
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
    ) -> Result<Value, String> {
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
        let effects = self.as_mut().dispatch_runtime(dispatch)?;
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

    pub(super) fn dispatch_tab_open_ipc_command(
        mut self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        self.as_mut().note_repeatable_command(&command);
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
        let effects = self.as_mut().dispatch_runtime(dispatch)?;
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
