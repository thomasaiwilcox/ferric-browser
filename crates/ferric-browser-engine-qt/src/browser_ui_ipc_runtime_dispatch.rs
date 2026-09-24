use super::{
    CxxQtType, Effect, EngineEffect, IpcRoute, ParsedCommand, Pin, RuntimeDispatch, Value,
    ipc_command_invocation, qobject,
};

impl qobject::BrowserUi {
    pub(super) fn execute_ipc_command(
        self: Pin<&mut Self>,
        command: ParsedCommand,
        route: &IpcRoute,
    ) -> Result<Value, String> {
        self.execute_ipc_command_with_operation(command, route, None)
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
