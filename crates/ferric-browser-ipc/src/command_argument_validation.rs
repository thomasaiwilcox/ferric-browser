//! Shared scalar parsing and closed-field validation for IPC commands.

use crate::validate_command_argument_fields;
use serde_json::Value;

pub(crate) const MAX_COMMAND_TEXT_BYTES: usize = 16 * 1024;
const MAX_JSEVAL_SCRIPT_BYTES: usize = 64 * 1024;

pub(crate) fn is_bounded_untrusted_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_COMMAND_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

pub(crate) fn is_bounded_journey_query(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

pub(crate) fn validate_jseval_script(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_JSEVAL_SCRIPT_BYTES {
        return Err("jseval script must be nonempty and at most 64 KiB".into());
    }
    if value.chars().any(|character| {
        character == '\0' || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    }) {
        return Err("jseval script contains a disallowed control character".into());
    }
    Ok(())
}

pub(crate) fn normalize_history_clear_origin(value: &str) -> Result<String, String> {
    let authority = value
        .split_once("://")
        .map(|(_, authority)| authority)
        .filter(|authority| !authority.is_empty() && !authority.contains(['/', '?', '#', '@']))
        .ok_or_else(|| "history-clear --origin requires an exact HTTP(S) origin".to_owned())?;
    if authority.contains('\\') {
        return Err("history-clear --origin requires an exact HTTP(S) origin".into());
    }
    ferric_browser_core::canonical_origin(value)
        .ok_or_else(|| "history-clear --origin requires an exact HTTP(S) origin".to_owned())
}

pub(crate) fn valid_ipc_mode(mode: &str) -> bool {
    matches!(
        mode,
        "normal" | "insert" | "command" | "search" | "hint" | "caret" | "pass-through"
    )
}

pub(crate) fn valid_switcher_scope(scope: &str) -> bool {
    matches!(
        scope,
        "all"
            | "tabs"
            | "windows"
            | "contexts"
            | "commands"
            | "history"
            | "bookmarks"
            | "quickmarks"
            | "sessions"
            | "downloads"
            | "closed"
            | "actions"
    )
}

pub(crate) fn external_hint_target(value: &str) -> bool {
    value.strip_prefix("external:").is_some_and(|name| {
        is_bounded_untrusted_text(name)
            && name.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
    })
}

pub(crate) fn validate_command_arguments(
    command: &str,
    arguments: Option<&serde_json::Map<String, Value>>,
) -> Result<(), String> {
    let Some(arguments) = arguments else {
        return Ok(());
    };
    validate_command_argument_fields(command, arguments)?;
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
        ("hint", "first"),
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
    if command == "open"
        && arguments
            .get("clean_link")
            .is_some_and(|value| value.as_bool().is_none())
    {
        return Err("command argument clean_link must be a boolean".into());
    }
    if command == "hint" {
        validate_hint_arguments(arguments)?;
    }
    if command == "scroll-target"
        && !arguments
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(|action| matches!(action, "select" | "auto" | "document" | "status"))
    {
        return Err("command argument action must be select, auto, document, or status".into());
    }
    Ok(())
}

fn validate_hint_arguments(arguments: &serde_json::Map<String, Value>) -> Result<(), String> {
    let target = arguments.get("target").and_then(Value::as_str);
    if arguments
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| {
            !matches!(
                kind,
                "links" | "all" | "inputs" | "buttons" | "images" | "media" | "scrollables"
            )
        })
    {
        return Err("command argument kind must be a supported hint family".into());
    }
    if target.is_some_and(|target| {
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
                | "choose"
        ) && !external_hint_target(target)
    }) {
        return Err("command argument target must be a supported hint target".into());
    }
    if arguments.get("rapid").and_then(Value::as_bool) == Some(true)
        && target.is_some_and(|target| {
            !matches!(
                target,
                "current" | "tab-bg" | "yank" | "clean-yank" | "download" | "userscript"
            ) || external_hint_target(target)
        })
    {
        return Err("command argument target does not support rapid=true".into());
    }
    if target == Some("userscript") && arguments.get("script").and_then(Value::as_str).is_none() {
        return Err("hint target userscript requires script".into());
    }
    if target != Some("userscript") && arguments.contains_key("script") {
        return Err("hint script is only valid with target userscript".into());
    }
    let first = arguments.get("first").and_then(Value::as_bool) == Some(true);
    let rapid = arguments.get("rapid").and_then(Value::as_bool) == Some(true);
    if first && rapid {
        return Err("hint first and rapid modes cannot be combined".into());
    }
    if let Some(index) = arguments.get("index") {
        if !first {
            return Err("hint index requires first=true".into());
        }
        if !index
            .as_u64()
            .is_some_and(|index| (1..=5_000).contains(&index))
        {
            return Err("hint index must be 1..=5000".into());
        }
    }
    Ok(())
}
