//! Typed IPC command context construction.
//!
//! This module projects the already-validated IPC route onto the immutable core
//! state. It deliberately contains no Qt or `WebEngine` integration, so command
//! dispatch keeps one auditable protocol-to-runtime boundary.

use crate::{
    command_options::command_count,
    ipc_contract::{IpcOpenTarget, IpcRoute},
};
use ferric_browser_application::BrowserApplication;
use ferric_browser_core::{CommandInvocation, CommandSource, DispatchTarget, ParsedCommand};
use serde_json::{Value, json};

pub(super) fn command_context(
    application: Option<&BrowserApplication>,
    route: &IpcRoute,
    operation_id: &str,
    count: u32,
) -> Value {
    let captured = application.map(|application| {
        application.capture_command_context(
            route.selector,
            route.source,
            count,
            Some(operation_id.to_owned()),
        )
    });
    let window_id = captured.as_ref().and_then(|context| context.window);
    let tab_id = captured.as_ref().and_then(|context| context.tab);
    let profile_id = captured.as_ref().and_then(|context| context.profile);
    let profile = profile_id.and_then(|profile| application?.profiles().get(&profile));
    json!({
        "source": captured.as_ref().map_or(route.source.as_str(), |context| context.source.as_str()),
        "operation_id": captured.as_ref().and_then(|context| context.operation_id.as_deref()).unwrap_or(operation_id),
        "count": captured.as_ref().map_or(count, |context| context.count),
        "window_id": window_id.map(|id| id.to_string()),
        "tab_id": tab_id.map(|id| id.to_string()),
        "profile_id": profile_id.map(|id| id.to_string()),
        "profile": profile.map(|profile| profile.label.clone()),
        "privacy": profile.map_or("unknown", |profile| profile.privacy.name()),
        "context": captured.as_ref().and_then(|context| context.context.clone()),
        "requested_profile": route.profile,
        "requested_context": route.context,
        "target": match route.open_target {
            IpcOpenTarget::Tab => "tab",
            IpcOpenTarget::BackgroundTab => "tab-bg",
            IpcOpenTarget::Window => "window",
            IpcOpenTarget::PrivateWindow => "private-window",
        }
    })
}

pub(super) fn command_invocation(
    application: &BrowserApplication,
    command: ParsedCommand,
    selector: DispatchTarget,
    source: CommandSource,
    operation_id: Option<&str>,
) -> CommandInvocation {
    let count = command_count(&command);
    CommandInvocation::with_count(command, count).with_context(application.capture_command_context(
        selector,
        source,
        count,
        operation_id.map(ToOwned::to_owned),
    ))
}
