//! Shared command-envelope schema used by every IPC producer and consumer.

use serde_json::{Map, Value};

/// Returns the accepted JSON argument fields for a command.
#[must_use]
#[allow(clippy::match_same_arms)]
pub fn command_argument_names(command: &str) -> &'static [&'static str] {
    match command {
        "open" => &["input", "target", "clean_link", "external"],
        "open-current" => &["target"],
        "back" | "forward" | "tab-next" | "tab-prev" => &["count"],
        "tab-open" => &["input", "background"],
        "fullscreen" => &["state"],
        "reload" => &["bypass_cache"],
        "search" => &["query", "backward", "case"],
        "window-new" => &["profile", "private"],
        "tab-give" => &["window_id"],
        "set" => &["key", "value", "temporary", "pattern"],
        "unset" => &["key", "temporary", "pattern"],
        "get" => &["key", "url", "explain"],
        "help" => &["topic"],
        "config-export" | "config-write-defaults" | "print-pdf" | "save-page" => &["path"],
        "config-edit" | "config-reload" | "config-check" | "theme-reload" => &[],
        "bind" => &["mode", "keychain", "command"],
        "unbind" => &["mode", "keychain"],
        "binding-list" => &["mode"],
        "binding-explain" => &["keychain", "mode"],
        "learning-mode" => &["state"],
        "bookmark-add" => &["title"],
        "bookmark-edit" => &["id", "title"],
        "bookmark-delete" | "bookmark-open" | "history-open" | "download-open"
        | "download-show" | "download-cancel" | "download-pause" | "download-resume"
        | "download-retry" | "site-doctor-undo" => &["id"],
        "tab-select" => &["selector"],
        "tab-focus" | "tab-suspend" | "tab-discard" | "tab-resume" => &["id"],
        "tab-close" => &["id", "count"],
        "tab-mute" | "tab-pin" => &["id", "state"],
        "tab-move" => &["id", "direction", "context"],
        "zoom" => &["factor"],
        "search-next" => &["direction", "count"],
        "scroll" => &["direction", "count"],
        "scroll-page" => &["direction", "half", "count"],
        "scroll-to" => &["edge"],
        "window-focus" => &["id"],
        "window-move" => &["id", "workspace"],
        "window-close" => &[],
        "command-help" => &["id"],
        "command-execute" => &["id", "arguments"],
        "selection-search" => &["engine"],
        "quickmark-add" | "quickmark-edit" => &["name", "url"],
        "journey" => &["current", "search", "expand"],
        "switcher" => &["scope", "query"],
        "journey-reopen" => &["node", "target"],
        "profile-open" => &["name", "input"],
        "quickmark-delete" | "quickmark-open" | "session-save" | "session-delete"
        | "profile-delete" | "context-enter" | "context-save" => &["name"],
        "session-load" => &["name", "append"],
        "profile-create" => &["name", "ephemeral"],
        "history-clear" => &["since", "origin", "confirmed"],
        "url-clean" | "url-explain" => &["url"],
        "hint" => &["kind", "target", "rapid", "script", "first", "index"],
        "mode-enter" => &["mode"],
        "caret-move" => &["direction", "count"],
        "caret-select" => &["state"],
        "download" => &["input"],
        "permissions" => &["origin"],
        "site-status" => &["tab"],
        "permission-reset" => &["origin", "permission"],
        "site-doctor" => &["experiment"],
        "site-data-clear" => &["origin", "confirmed"],
        "blocking-toggle" => &["site"],
        "spawn" => &["argv", "userscript"],
        "grid-refine" => &["cell"],
        "grid-click" => &["button"],
        "script-run" => &["name"],
        "jseval" => &["world", "script"],
        "devtools" => &["detach"],
        "send" => &["target", "input", "url", "selection", "send_subject"],
        "repeat" => &["count"],
        "cancel" => &["operation_id"],
        "macro-record" | "macro-play" => &["register"],
        "yank" => &["source", "input", "clean", "primary"],
        "paste-open" => &["target", "primary"],
        "action" => &["subject", "verb", "input", "url"],
        "action-list" => &["subject"],
        "context-create" => &["name", "label", "profile", "workspace"],
        "context-delete" => &["name", "confirmed"],
        "context-route" => &[
            "action",
            "pattern",
            "context",
            "id",
            "priority",
            "behavior",
            "entry_points",
        ],
        _ => &[],
    }
}

/// Rejects fields outside the protocol schema for `command`.
///
/// # Errors
///
/// Returns an error when the argument object contains a field not declared by
/// the command's compatibility schema.
pub fn validate_command_argument_fields(
    command: &str,
    arguments: &Map<String, Value>,
) -> Result<(), String> {
    if arguments
        .keys()
        .any(|key| !command_argument_names(command).contains(&key.as_str()))
    {
        return Err("command arguments contain an unknown field".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_fields_are_closed() {
        assert!(command_argument_names("open").contains(&"input"));
        assert!(command_argument_names("context-route").contains(&"entry_points"));
        assert!(command_argument_names("not-a-command").is_empty());
        assert!(validate_command_argument_fields("open", &Map::new()).is_ok());
        assert!(
            validate_command_argument_fields(
                "open",
                &Map::from_iter([("unknown".into(), Value::Null)])
            )
            .is_err()
        );
    }
}
