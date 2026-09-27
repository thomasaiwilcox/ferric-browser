//! Command argument schemas for the JSON-RPC compatibility envelope.
//!
//! This is the single local authority for which fields each legacy command
//! accepts. Payload conversion stays in the decoder; this module deliberately
//! does not know about Qt objects or command execution.

use crate::hint_policy::external_hint_target;
use serde_json::{Map, Value};

/// Returns the complete set of accepted JSON argument names for `command`.
///
/// Unknown commands intentionally accept no fields. The command decoder keeps
/// its existing compatibility behavior for an unknown command name while the
/// field boundary remains closed.
#[cfg(test)]
pub(super) fn command_argument_names(command: &str) -> &'static [&'static str] {
    ferric_browser_ipc::command_argument_names(command)
}

/// Validates argument fields whose type does not depend on command execution.
///
/// This closes the JSON envelope before the compatibility decoder converts
/// fields to the legacy command argument sequence. Command-specific semantic
/// checks (such as hint-target compatibility) stay with that conversion.
pub(super) fn validate_command_argument_envelope(
    command: &str,
    arguments: Option<&Map<String, Value>>,
) -> Result<(), String> {
    let Some(arguments) = arguments else {
        return Ok(());
    };
    ferric_browser_ipc::validate_command_argument_fields(command, arguments)?;

    for (command_name, argument_name) in [
        ("context-delete", "confirmed"),
        ("session-load", "append"),
        ("profile-create", "ephemeral"),
        ("set", "temporary"),
        ("unset", "temporary"),
        ("get", "explain"),
        ("paste-open", "primary"),
        ("blocking-toggle", "site"),
        ("site-data-clear", "confirmed"),
        ("history-clear", "confirmed"),
        ("journey", "current"),
        ("hint", "rapid"),
    ] {
        if command == command_name
            && arguments
                .get(argument_name)
                .is_some_and(|value| value.as_bool().is_none())
        {
            return Err(format!(
                "command argument {argument_name} must be a boolean"
            ));
        }
    }
    if command == "command-execute"
        && arguments
            .get("arguments")
            .is_some_and(|value| !value.is_object())
    {
        return Err("command argument arguments must be an object".into());
    }
    if command == "paste-open"
        && arguments
            .get("target")
            .and_then(Value::as_str)
            .is_some_and(|target| !matches!(target, "current" | "tab"))
    {
        return Err("command argument target must be current or tab".into());
    }
    if command == "history-clear"
        && arguments
            .get("since")
            .and_then(Value::as_i64)
            .is_none_or(|timestamp| timestamp < 0)
        && arguments.contains_key("since")
    {
        return Err("command argument since must be a nonnegative Unix timestamp".into());
    }
    if command == "open"
        && arguments
            .get("clean_link")
            .is_some_and(|value| value.as_bool().is_none())
    {
        return Err("command argument clean_link must be a boolean".into());
    }
    if command == "scroll-target"
        && !arguments
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(|action| matches!(action, "select" | "auto" | "document" | "status"))
    {
        return Err("command argument action must be select, auto, document, or status".into());
    }
    validate_command_argument_semantics(command, arguments)
}

/// Validates relationships between otherwise well-shaped command arguments.
///
/// Keeping these checks beside the field schema prevents JSON-RPC callers from
/// observing a different hint, yank, or caret policy than CLI callers.
fn validate_command_argument_semantics(
    command: &str,
    arguments: &Map<String, Value>,
) -> Result<(), String> {
    if command == "hint"
        && arguments
            .get("target")
            .and_then(Value::as_str)
            .is_some_and(|target| {
                !matches!(
                    target,
                    "current"
                        | "tab"
                        | "tab-bg"
                        | "window"
                        | "yank"
                        | "clean-yank"
                        | "download"
                        | "userscript"
                        | "ephemeral"
                ) && external_hint_target(target).is_none()
            })
    {
        return Err(
            "command argument target must be current, tab, tab-bg, window, yank, clean-yank, download, userscript, ephemeral, or external:NAME"
                .into(),
        );
    }
    if command == "hint"
        && arguments
            .get("target")
            .and_then(Value::as_str)
            .is_some_and(|target| matches!(target, "tab" | "window" | "ephemeral"))
        && arguments.get("rapid").and_then(Value::as_bool) == Some(true)
    {
        return Err(
            if arguments.get("target").and_then(Value::as_str) == Some("ephemeral") {
                "hint target ephemeral does not support rapid=true"
            } else {
                "hint target tab or window does not support rapid=true"
            }
            .into(),
        );
    }
    if command == "hint"
        && arguments
            .get("target")
            .and_then(Value::as_str)
            .is_some_and(|target| external_hint_target(target).is_some())
        && arguments.get("rapid").and_then(Value::as_bool) == Some(true)
    {
        return Err("external hint targets do not support rapid=true".into());
    }
    if command == "hint"
        && arguments
            .get("target")
            .and_then(Value::as_str)
            .is_some_and(|target| target == "userscript")
        && arguments.get("script").and_then(Value::as_str).is_none()
    {
        return Err("hint target userscript requires script".into());
    }
    if command == "hint"
        && arguments
            .get("target")
            .and_then(Value::as_str)
            .is_none_or(|target| target != "userscript")
        && arguments.contains_key("script")
    {
        return Err("hint script is only valid with target userscript".into());
    }
    if command == "hint"
        && arguments
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| !matches!(kind, "links" | "all"))
    {
        return Err("command argument kind must be links or all".into());
    }
    if command == "yank" {
        for key in ["clean", "primary"] {
            if arguments
                .get(key)
                .is_some_and(|value| value.as_bool().is_none())
            {
                return Err(format!("command argument {key} must be a boolean"));
            }
        }
        if arguments
            .get("source")
            .and_then(Value::as_str)
            .is_some_and(|source| !matches!(source, "url" | "title" | "selection"))
        {
            return Err("command argument source must be url, title, or selection".into());
        }
        if arguments
            .get("source")
            .and_then(Value::as_str)
            .is_some_and(|source| source == "selection" || source == "title")
            && arguments.contains_key("input")
        {
            return Err("title and selection copying do not accept an input URL".into());
        }
        if arguments
            .get("source")
            .and_then(Value::as_str)
            .is_some_and(|source| source == "selection" || source == "title")
            && arguments
                .get("clean")
                .and_then(Value::as_bool)
                .is_some_and(|clean| clean)
        {
            return Err("title and selection copying do not accept --clean".into());
        }
    }
    if command == "mode-enter"
        && arguments
            .get("mode")
            .and_then(Value::as_str)
            .is_some_and(|mode| {
                !matches!(
                    mode,
                    "normal" | "insert" | "caret" | "passthrough" | "pass-through"
                )
            })
    {
        return Err("command argument mode is invalid".into());
    }
    if command == "caret-move"
        && arguments
            .get("direction")
            .and_then(Value::as_str)
            .is_some_and(|direction| {
                !matches!(
                    direction,
                    "left"
                        | "right"
                        | "up"
                        | "down"
                        | "word-next"
                        | "word-prev"
                        | "line-start"
                        | "line-end"
                )
            })
    {
        return Err("command argument direction is invalid".into());
    }
    if command == "caret-move"
        && arguments
            .get("count")
            .is_some_and(|count| count.as_u64().is_none())
    {
        return Err("command argument count must be an integer".into());
    }
    if command == "caret-select"
        && arguments
            .get("state")
            .and_then(Value::as_str)
            .is_some_and(|state| !matches!(state, "on" | "off" | "toggle"))
    {
        return Err("command argument state is invalid".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_exact_known_command_fields_and_closes_unknown_commands() {
        assert_eq!(
            command_argument_names("open"),
            ["input", "target", "clean_link", "external"]
        );
        assert_eq!(command_argument_names("context-route").len(), 7);
        assert_eq!(command_argument_names("scroll-target"), ["action"]);
        assert!(command_argument_names("not-a-command").is_empty());
    }

    #[test]
    fn rejects_unknown_and_mistyped_common_envelope_fields() {
        assert!(
            validate_command_argument_envelope(
                "open",
                serde_json::json!({"input": "https://example.test", "unexpected": true})
                    .as_object()
            )
            .is_err()
        );
        assert!(
            validate_command_argument_envelope(
                "history-clear",
                serde_json::json!({"confirmed": "yes"}).as_object()
            )
            .is_err()
        );
        assert!(
            validate_command_argument_envelope(
                "open",
                serde_json::json!({"input": "https://example.test", "clean_link": "yes"})
                    .as_object()
            )
            .is_err()
        );
        for arguments in [
            serde_json::json!({}),
            serde_json::json!({"action": null}),
            serde_json::json!({"action": "element"}),
            serde_json::json!({"action": "auto", "unexpected": true}),
        ] {
            assert!(
                validate_command_argument_envelope("scroll-target", arguments.as_object()).is_err()
            );
        }
        for action in ["select", "auto", "document", "status"] {
            let arguments = serde_json::json!({"action": action});
            assert!(
                validate_command_argument_envelope("scroll-target", arguments.as_object()).is_ok()
            );
        }
    }

    #[test]
    fn rejects_invalid_cross_field_hint_and_yank_requests() {
        assert!(
            validate_command_argument_envelope(
                "hint",
                serde_json::json!({"target": "userscript"}).as_object()
            )
            .is_err()
        );
        assert!(
            validate_command_argument_envelope(
                "hint",
                serde_json::json!({"target": "external:reader", "rapid": true}).as_object()
            )
            .is_err()
        );
        assert!(
            validate_command_argument_envelope(
                "yank",
                serde_json::json!({"source": "title", "clean": true}).as_object()
            )
            .is_err()
        );
    }
}
