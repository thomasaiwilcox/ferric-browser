//! IPC command-envelope decoding.
//!
//! Route selection is protocol policy, but it is independent from Qt and from
//! command payload compatibility decoding. Keeping it here makes the bounded
//! envelope contract directly testable without the `QObject` bridge.

use ferric_browser_core::{CommandSource, DispatchTarget, WindowId, validate_open_target};
use ferric_browser_ipc::CommandEnvelope;
use serde_json::Value;

use super::{IpcOpenTarget, IpcRoute};

pub(super) fn typed_ipc_route(envelope: &CommandEnvelope) -> Result<IpcRoute, String> {
    let command = envelope.command.as_str();
    let arguments = Some(&envelope.arguments);
    let window = envelope.context.window.clone();
    let profile = envelope.context.profile.clone();
    let context_name = envelope.context.context.clone();
    let source = envelope
        .context
        .source
        .as_deref()
        .map(|value| {
            CommandSource::parse(value)
                .ok_or_else(|| format!("command context source is unsupported: {value}"))
        })
        .transpose()?
        .unwrap_or(CommandSource::Ipc);
    let external_open = arguments
        .and_then(|arguments| arguments.get("external"))
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| "command external must be a boolean".to_owned())
        })
        .transpose()?
        .unwrap_or(false);
    if external_open && command != "open" {
        return Err("command external is only valid for open".into());
    }
    let selector = match window.as_deref() {
        None | Some("active") => DispatchTarget::Active,
        Some("last-focused") => DispatchTarget::LastFocused,
        Some(window) => WindowId::from_display(window)
            .map(DispatchTarget::Window)
            .ok_or_else(|| {
                "window must be active, last-focused, or a valid window ID".to_owned()
            })?,
    };
    let open_target = if command == "open" {
        match arguments
            .and_then(|arguments| arguments.get("target"))
            .and_then(Value::as_str)
            .unwrap_or("tab")
        {
            "tab" => IpcOpenTarget::Tab,
            "tab-bg" => IpcOpenTarget::BackgroundTab,
            "window" => IpcOpenTarget::Window,
            "private-window" => IpcOpenTarget::PrivateWindow,
            _ => return Err("open target must be tab, tab-bg, window, or private-window".into()),
        }
    } else if command == "tab-open" {
        if profile.is_some() || context_name.is_some() {
            return Err("tab-open does not accept profile or context routing".into());
        }
        if arguments
            .and_then(|arguments| arguments.get("background"))
            .is_some_and(|value| value.as_bool() == Some(true))
        {
            IpcOpenTarget::BackgroundTab
        } else {
            IpcOpenTarget::Tab
        }
    } else {
        IpcOpenTarget::Tab
    };
    if command == "open" {
        validate_open_target(
            (open_target == IpcOpenTarget::BackgroundTab).then_some("tab-bg"),
            arguments
                .and_then(|arguments| arguments.get("clean_link"))
                .is_some_and(|value| value.as_bool() == Some(true)),
        )
        .map_err(str::to_owned)?;
    }
    Ok(IpcRoute {
        selector,
        open_target,
        profile,
        context: context_name,
        external_open,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_unknown_context_fields_and_invalid_target_combinations() {
        let object = json!({
            "command": "tab-open",
            "arguments": {},
            "context": {"profile": "default"}
        });
        let envelope = ferric_browser_ipc::decode_command_envelope(&object).expect("envelope");
        assert!(typed_ipc_route(&envelope).is_err());

        let object = json!({"command": "open", "arguments": {"target": "invalid"}});
        let envelope = ferric_browser_ipc::decode_command_envelope(&object).expect("envelope");
        assert!(typed_ipc_route(&envelope).is_err());
    }
}
