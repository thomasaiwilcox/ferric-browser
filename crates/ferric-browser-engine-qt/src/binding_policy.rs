//! Binding and command-presentation policy.
//!
//! These functions interpret typed command/configuration values without
//! accessing Qt objects, browser storage, or mutable application state.

use crate::input_validation::is_bounded_untrusted_text;
use ferric_browser_core::{
    BindingDefinition, BindingResolver, BindingTrie, CommandRegistry, Mode, ParsedCommand,
};
use serde_json::Value;

pub(super) fn parse_ipc_mode(mode: &str) -> Result<Mode, String> {
    match mode {
        "normal" => Ok(Mode::Normal),
        "insert" => Ok(Mode::Insert),
        "insert-locked" => Ok(Mode::InsertLocked),
        "command" => Ok(Mode::Command),
        "search" => Ok(Mode::Search),
        "hint" => Ok(Mode::Hint),
        "grid" => Ok(Mode::Grid),
        "caret" => Ok(Mode::Caret),
        "pass-through" => Ok(Mode::PassThrough),
        "pass-through-once" => Ok(Mode::PassThroughOnce),
        _ => Err(format!("unknown binding mode: {mode}")),
    }
}

pub(super) fn ipc_mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "normal",
        Mode::Insert => "insert",
        Mode::InsertLocked => "insert-locked",
        Mode::Command => "command",
        Mode::Search => "search",
        Mode::Hint => "hint",
        Mode::Grid => "grid",
        Mode::Caret => "caret",
        Mode::PassThrough => "pass-through",
        Mode::PassThroughOnce => "pass-through-once",
    }
}

pub(super) fn configured_mode_name(mode: &str) -> Option<Mode> {
    match mode {
        "grid" => Some(Mode::Grid),
        _ => parse_ipc_mode(mode).ok(),
    }
}

pub(super) fn binding_command_parameters(
    command: &ParsedCommand,
) -> Result<(Option<String>, Option<String>), String> {
    let mut keychain = None;
    let mut mode = None;
    let mut index = 0;
    while index < command.arguments.len() {
        match command.arguments[index].as_str() {
            "--mode" => {
                if mode.is_some() {
                    return Err(format!("{} accepts --mode at most once", command.name));
                }
                let value = command
                    .arguments
                    .get(index + 1)
                    .ok_or_else(|| format!("{} --mode requires a value", command.name))?;
                parse_ipc_mode(value)?;
                mode = Some(value.clone());
                index += 2;
            }
            value if command.name == "binding-explain" && keychain.is_none() => {
                if !is_bounded_untrusted_text(value) {
                    return Err("binding-explain requires a bounded keychain".into());
                }
                keychain = Some(value.to_owned());
                index += 1;
            }
            _ => return Err(format!("{} received an unexpected argument", command.name)),
        }
    }
    if command.name == "binding-explain" && keychain.is_none() {
        return Err("binding-explain requires a keychain".into());
    }
    Ok((keychain, mode))
}

pub(super) fn config_get_command_parameters(command: &ParsedCommand) -> Result<Value, String> {
    let mut key = None;
    let mut url = None;
    let mut explain = false;
    let mut index = 0;
    while index < command.arguments.len() {
        match command.arguments[index].as_str() {
            "--url" => {
                if url.is_some() {
                    return Err("get accepts --url at most once".into());
                }
                let value = command
                    .arguments
                    .get(index + 1)
                    .filter(|value| is_bounded_untrusted_text(value))
                    .cloned()
                    .ok_or_else(|| "get --url requires a bounded URL".to_owned())?;
                url = Some(value);
                index += 1;
            }
            "--explain" => {
                if explain {
                    return Err("get accepts --explain at most once".into());
                }
                explain = true;
            }
            value if !value.starts_with('-') && key.is_none() => {
                if !is_bounded_untrusted_text(value) {
                    return Err("get requires a bounded configuration key".into());
                }
                key = Some(value.to_owned());
            }
            value => return Err(format!("unknown get option or extra argument: {value}")),
        }
        index += 1;
    }
    let key = key.ok_or_else(|| "get requires a configuration key".to_owned())?;
    Ok(serde_json::json!({
        "key": key,
        "url": url,
        "explain": explain
    }))
}

pub(super) fn learning_mode_request(
    command: &ParsedCommand,
    current: bool,
) -> Result<Option<bool>, String> {
    match command.arguments.as_slice() {
        [] => Ok(None),
        [state] => match state.as_str() {
            "on" => Ok(Some(true)),
            "off" => Ok(Some(false)),
            "toggle" => Ok(Some(!current)),
            _ => Err("learning-mode state must be on, off, or toggle".into()),
        },
        _ => Err("learning-mode accepts at most one state".into()),
    }
}

pub(super) fn modal_command_prefill(command: &ParsedCommand, current_url: &str) -> Option<String> {
    match (command.name.as_str(), command.arguments.as_slice()) {
        ("open", []) => Some("open ".to_owned()),
        ("open-current", []) if !current_url.is_empty() => Some(format!("open {current_url}")),
        ("open-current", [flag, target])
            if flag == "--target" && target == "tab" && !current_url.is_empty() =>
        {
            Some(format!("tab-open {current_url}"))
        }
        ("tab-open", []) => Some("tab-open ".to_owned()),
        ("quickmark-add", []) => Some("quickmark-add ".to_owned()),
        ("quickmark-open", []) => Some("quickmark-open ".to_owned()),
        ("bookmark-open", []) => Some("bookmark-open ".to_owned()),
        _ => None,
    }
}

pub(super) fn binding_uses_full_command_executor(command: &ParsedCommand) -> bool {
    !command.arguments.is_empty()
        || matches!(
            command.name.as_str(),
            "tab-new"
                | "start-page"
                | "start-page-view"
                | "start-page-cycle"
                | "favourite-add"
                | "repeat"
                | "cancel"
                | "macro-record"
                | "macro-stop"
                | "macro-play"
                | "binding-list"
                | "bookmark-add"
                | "bookmark-list"
                | "devtools"
                | "fullscreen"
                | "history"
                | "hint"
                | "print"
                | "switcher"
                | "tab-clone"
                | "tab-close"
                | "tab-move"
                | "tab-mute"
                | "tab-pin"
                | "tab-undo"
                | "view-source"
                | "window-close"
                | "window-new"
        )
}

pub(super) fn normalize_active_tab_command(
    command: &mut ParsedCommand,
    active_tab_id: Option<String>,
) -> Result<(), &'static str> {
    let replacement = match command.name.as_str() {
        "tab-pin" | "tab-mute" => match command.arguments.as_slice() {
            [] => Some(vec!["toggle".into()]),
            [state] if matches!(state.as_str(), "on" | "off" | "toggle") => {
                Some(vec![state.clone()])
            }
            _ => None,
        },
        "tab-move" => match command.arguments.as_slice() {
            [direction] if matches!(direction.as_str(), "left" | "right") => {
                Some(vec![direction.clone()])
            }
            _ => None,
        },
        _ => None,
    };
    let Some(mut arguments) = replacement else {
        return Ok(());
    };
    let active_tab_id = active_tab_id.ok_or("No active tab")?;
    arguments.insert(0, active_tab_id);
    command.arguments = arguments;
    Ok(())
}

const DEFAULT_SWITCHER_MAX_RESULTS: u32 = 100;

pub(super) fn switcher_max_results(config: &Value) -> u32 {
    config
        .get("switcher")
        .and_then(Value::as_object)
        .and_then(|switcher| switcher.get("max_results"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(DEFAULT_SWITCHER_MAX_RESULTS)
        .clamp(10, 1000)
}

pub(super) fn configured_undo_limit(config: &Value) -> usize {
    config
        .get("tabs")
        .and_then(Value::as_object)
        .and_then(|tabs| tabs.get("undo_limit"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(100)
        .min(100)
}

pub(super) fn configured_bindings(
    registry: &CommandRegistry,
    config: &Value,
) -> Option<BindingResolver> {
    let mut definitions = BindingTrie::default_v1(registry.clone())
        .ok()?
        .definitions()
        .into_iter()
        .map(|definition| ((definition.mode, definition.canonical_keys()), definition))
        .collect::<std::collections::BTreeMap<_, _>>();
    let modes = config.get("bindings")?.as_object()?;
    for (mode_name, entries) in modes {
        let Some(mode) = configured_mode_name(mode_name) else {
            continue;
        };
        let Some(entries) = entries.as_object() else {
            continue;
        };
        for (keychain, command) in entries {
            let Some(command) = command.as_str() else {
                continue;
            };
            let keys = ferric_browser_core::binding_key_tokens(keychain)?;
            if matches!(command, "unbound" | "none") {
                definitions.remove(&(mode, keys));
            } else {
                definitions.insert(
                    (mode, keys.clone()),
                    BindingDefinition {
                        mode,
                        keys,
                        command: command.into(),
                    },
                );
            }
        }
    }
    BindingTrie::new(registry.clone(), definitions.into_values())
        .ok()
        .map(|trie| BindingResolver::new(trie, Mode::Normal))
}

pub(super) fn is_session_command(name: &str) -> bool {
    matches!(
        name,
        "session-save" | "session-load" | "session-list" | "session-delete"
    )
}

pub(super) fn is_profile_command(name: &str) -> bool {
    matches!(
        name,
        "profile-list" | "profile-open" | "profile-create" | "profile-delete"
    )
}

pub(super) fn is_context_command(name: &str) -> bool {
    matches!(
        name,
        "context-list"
            | "context-create"
            | "context-delete"
            | "context-enter"
            | "context-save"
            | "context-route"
    )
}

pub(super) fn is_library_command(name: &str) -> bool {
    matches!(
        name,
        "bookmark-add"
            | "bookmark-delete"
            | "bookmark-edit"
            | "bookmark-open"
            | "bookmark-list"
            | "quickmark-add"
            | "quickmark-delete"
            | "quickmark-edit"
            | "quickmark-open"
            | "quickmark-list"
            | "history"
            | "history-open"
            | "journey"
            | "journey-reopen"
            | "history-clear"
    )
}

#[cfg(test)]
mod alias_tests {
    use super::*;
    use ferric_browser_core::BindingOutcome;

    #[test]
    fn an_alias_override_replaces_the_default_plus_binding() {
        for authored in ["Plus", "+"] {
            let mut config = serde_json::json!({"bindings":{"normal":{}}});
            config["bindings"]["normal"][authored] = Value::String("zoom out".into());
            let mut resolver =
                configured_bindings(&CommandRegistry::default_v1(), &config).unwrap();
            for delivered in ["+", "Plus"] {
                assert!(resolver.has_initial_key(delivered));
                assert_eq!(
                    resolver.feed(delivered, 0),
                    BindingOutcome::Execute {
                        command: "zoom out".into(),
                        count: 1,
                    }
                );
            }
        }
    }

    #[test]
    fn an_alias_unbind_masks_the_default_plus_binding() {
        for authored in ["Plus", "+"] {
            for marker in ["unbound", "none"] {
                let mut config = serde_json::json!({"bindings":{"normal":{}}});
                config["bindings"]["normal"][authored] = Value::String(marker.into());
                let mut resolver =
                    configured_bindings(&CommandRegistry::default_v1(), &config).unwrap();
                for delivered in ["+", "Plus"] {
                    assert!(!resolver.has_initial_key(delivered));
                    assert_eq!(resolver.feed(delivered, 0), BindingOutcome::Consumed);
                }
                assert!(resolver.has_initial_key("j"));
            }
        }
    }

    #[test]
    fn large_authored_binding_tables_preserve_defaults_and_resolve_aliases() {
        let registry = CommandRegistry::default_v1();
        let defaults = configured_bindings(&registry, &serde_json::json!({"bindings":{}}))
            .unwrap()
            .definitions();
        let mut config = serde_json::json!({"bindings":{"normal":{}}});
        for index in 0..4_000 {
            config["bindings"]["normal"][format!("g{index:04}+")] =
                Value::String("zoom out".into());
        }
        let mut resolver = configured_bindings(&registry, &config).unwrap();
        let definitions = resolver.definitions();
        assert_eq!(definitions.len(), defaults.len() + 4_000);
        for binding in &defaults {
            assert!(definitions.contains(binding), "default {binding:?}");
        }
        for index in [0, 1_999, 3_999] {
            for key in format!("g{index:04}").chars() {
                assert!(matches!(
                    resolver.feed(&key.to_string(), 0),
                    BindingOutcome::Pending { .. }
                ));
            }
            assert_eq!(
                resolver.feed("Plus", 0),
                BindingOutcome::Execute {
                    command: "zoom out".into(),
                    count: 1
                }
            );
        }
    }
}
