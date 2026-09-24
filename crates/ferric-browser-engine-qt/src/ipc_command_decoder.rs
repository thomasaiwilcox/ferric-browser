//! Qt-local route resolution for shared typed IPC command invocations.

use crate::{
    ipc_contract::{IpcCommandError, IpcRoute},
    ipc_route::typed_ipc_route,
    open_policy::parse_interactive as parse_interactive_open,
};
use ferric_browser_core::ParsedCommand;
use ferric_browser_ipc::decode_command_invocation;
use serde_json::Value;

pub(super) fn interactive_open_command(
    command: &ParsedCommand,
) -> Result<Option<(ParsedCommand, IpcRoute)>, String> {
    let Some(open) = parse_interactive_open(command)? else {
        return Ok(None);
    };
    let mut params = serde_json::json!({
        "command": "open",
        "arguments": {"input": open.input, "clean_link": open.clean_link}
    });
    if let Some(target) = open.target {
        params["arguments"]["target"] = Value::String(target);
    }
    let mut context = serde_json::Map::new();
    if let Some(profile) = open.profile {
        context.insert("profile".into(), Value::String(profile));
    }
    if let Some(name) = open.context {
        context.insert("context".into(), Value::String(name));
    }
    if !context.is_empty() {
        params["context"] = Value::Object(context);
    }
    typed_ipc_command(&params)
        .map(Some)
        .map_err(|error| error.to_string())
}

pub(super) fn typed_ipc_command(
    params: &Value,
) -> Result<(ParsedCommand, IpcRoute), IpcCommandError> {
    let invocation = decode_command_invocation(params).map_err(IpcCommandError::from)?;
    let route = typed_ipc_route(&invocation.envelope).map_err(IpcCommandError::from)?;
    Ok((invocation.command, route))
}
