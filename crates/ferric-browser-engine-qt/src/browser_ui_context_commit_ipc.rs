//! Addressed IPC admission for the same owner-handled context capture path as
//! interactive commands. An immediate IPC acceptance is not a durable receipt.

use super::{CxxQtType, DispatchTarget, IpcRoute, ParsedCommand, Pin, Value, qobject};
use ferric_browser_core::{VersioningCommand, VersioningScope};

impl qobject::BrowserUi {
    pub(super) fn queue_ipc_context_commit(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &IpcRoute,
    ) -> Option<Result<Value, String>> {
        let VersioningCommand::Commit {
            message,
            scope: VersioningScope::Context,
            pages: false,
            editable: false,
        } = VersioningCommand::from_parsed(command, 1).ok()?
        else {
            return None;
        };
        Some((|| {
            let mode = self.as_ref().rust().core_mode;
            self.as_ref()
                .rust()
                .registry
                .validate(command, mode)
                .map_err(|error| error.to_string())?;
            self.as_ref().validate_ipc_route(route)?;
            let binding = self.as_ref();
            let rust = binding.rust();
            let state = rust.state.as_ref().ok_or("core state unavailable")?;
            let target = match route.selector {
                DispatchTarget::Active => state.active_window(),
                DispatchTarget::LastFocused => {
                    state.last_focused_window().or(state.active_window())
                }
                DispatchTarget::Window(window) => Some(window),
                DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
            };
            if target != rust.window
                || route.context.as_deref().is_some_and(|name| {
                    rust.window
                        .and_then(|window| state.windows().get(&window))
                        .and_then(|window| window.context.as_deref())
                        != Some(name)
                })
            {
                return Err("context commit target is not this exact window owner".into());
            }
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.pending_context_commit_message = Some(message);
            this.pending_engine_action = Some("context-commit-request".into());
            Ok(serde_json::json!({
                "status": "accepted",
                "pending": "context-capture"
            }))
        })())
    }
}
