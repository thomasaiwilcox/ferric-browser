//! IPC command-envelope decoding.
//!
//! Route selection is protocol policy, but it is independent from Qt and from
//! command payload compatibility decoding. Keeping it here makes the bounded
//! envelope contract directly testable without the `QObject` bridge.

use ferric_browser_core::{CommandSource, DispatchTarget, WindowId, validate_open_target};
use serde_json::{Map, Value};

use super::{IpcOpenTarget, IpcRoute, is_bounded_untrusted_text};

pub(super) fn typed_ipc_route(
    object: &Map<String, Value>,
    arguments: Option<&Map<String, Value>>,
    command: &str,
) -> Result<IpcRoute, String> {
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "command" | "arguments" | "context"))
    {
        return Err("command request contains an unknown field".into());
    }
    let context = object
        .get("context")
        .map(|value| {
            value
                .as_object()
                .ok_or_else(|| "command context must be an object".to_owned())
        })
        .transpose()?;
    if context.is_some_and(|context| {
        context
            .keys()
            .any(|key| !matches!(key.as_str(), "window" | "profile" | "context" | "source"))
    }) {
        return Err("command context contains an unknown field".into());
    }
    let string_context = |key: &str| -> Result<Option<String>, String> {
        context
            .and_then(|context| context.get(key))
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| is_bounded_untrusted_text(value))
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| format!("command context {key} must be a nonempty string"))
            })
            .transpose()
    };
    let window = string_context("window")?;
    let profile = string_context("profile")?;
    let context_name = string_context("context")?;
    let source = context
        .and_then(|context| context.get("source"))
        .map(|value| {
            let value = value
                .as_str()
                .filter(|value| is_bounded_untrusted_text(value))
                .ok_or_else(|| "command context source must be a nonempty string".to_owned())?;
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
        assert!(
            typed_ipc_route(
                object.as_object().expect("object"),
                object.get("arguments").and_then(Value::as_object),
                "tab-open"
            )
            .is_err()
        );

        let object = json!({"command": "open", "arguments": {"target": "invalid"}});
        assert!(
            typed_ipc_route(
                object.as_object().expect("object"),
                object.get("arguments").and_then(Value::as_object),
                "open"
            )
            .is_err()
        );
    }
}
