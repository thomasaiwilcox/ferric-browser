//! Projection of installed userscript actions into the typed action catalog.
//!
//! Filesystem discovery and availability policy live here so the Qt bridge only
//! publishes already-validated presentation records.

use crate::userscript;
use ferric_browser_application::BrowserApplication;
use ferric_browser_core::{ActionRegistry, ExistenceState, TabId, Target, WindowId};
use serde_json::Value;
use std::path::Path;

#[derive(Debug)]
pub(super) struct ActionPresentation {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) available: bool,
}

pub(super) fn action_value(action: &userscript::RegisteredAction, available: bool) -> Value {
    let arguments = action
        .required_fields
        .iter()
        .map(|field| {
            serde_json::json!({
                "name": field,
                "kind": if field == "url" || field == "hint_url" { "url" } else { "text" },
                "required": true
            })
        })
        .collect::<Vec<_>>();
    let sources = if is_hint_only(action) {
        serde_json::json!(["hint"])
    } else {
        serde_json::json!(["ui", "ipc", "hint"])
    };
    let completion_provider = if action
        .required_fields
        .iter()
        .any(|field| field == "url" || field == "hint_url")
    {
        "url"
    } else {
        "text"
    };
    let availability_predicate = match action.subject.as_str() {
        "url" => "current-document",
        "link" => "captured-link",
        "selection" => "live-selection",
        "tab" => "live-tab",
        "window" => "live-window",
        "context" => "profile-context",
        "download" => "download-state",
        "history-entry" => "stored-history",
        "bookmark" => "stored-bookmark",
        "quickmark" => "stored-quickmark",
        "session" => "stored-session",
        "command" => "command-registry",
        _ => "unknown",
    };
    serde_json::json!({
        "id": action.id,
        "subject": action.subject,
        "verb": action.verb,
        "label": action.label,
        "description": format!("Run the installed {} userscript action.", action.script),
        "command": "userscript",
        "effect": "sensitive",
        "sensitive": true,
        "script": action.script,
        "sources": sources,
        "confirmation": "when-changed",
        "required_capabilities": ["installed-userscript"],
        "completion_provider": completion_provider,
        "availability_predicate": availability_predicate,
        "availability": {
            "state": if available { "available" } else { "unavailable" },
            "reason": if available { "ready" } else { "userscript-unavailable" },
            "predicate": availability_predicate,
            "requires_subject_revalidation": true
        },
        "examples": [format!("action {} {}", action.subject, action.verb)],
        "arguments": arguments
    })
}

pub(super) fn action_values(
    root: Option<&Path>,
    subject: Option<&str>,
    core_state_available: bool,
    durable_storage_available: bool,
    private_profile: bool,
) -> Result<Vec<Value>, String> {
    let Some(root) = root else {
        return Ok(Vec::new());
    };
    let builtins = ActionRegistry::default_v1();
    userscript::registered_actions(root).map(|actions| {
        actions
            .into_iter()
            .filter(|action| {
                subject.is_none_or(|subject| action.subject == subject)
                    && builtins.resolve(&action.id).is_none()
            })
            .map(|action| {
                let available = is_available(
                    &action,
                    core_state_available,
                    durable_storage_available,
                    private_profile,
                );
                action_value(&action, available)
            })
            .collect()
    })
}

pub(super) fn action_presentations(
    root: Option<&Path>,
    subject: &str,
    core_state_available: bool,
    durable_storage_available: bool,
    private_profile: bool,
) -> Result<Vec<ActionPresentation>, String> {
    let Some(root) = root else {
        return Ok(Vec::new());
    };
    let builtins = ActionRegistry::default_v1();
    userscript::registered_actions(root).map(|actions| {
        actions
            .into_iter()
            .filter(|action| action.subject == subject && builtins.resolve(&action.id).is_none())
            .map(|action| ActionPresentation {
                available: is_available(
                    &action,
                    core_state_available,
                    durable_storage_available,
                    private_profile,
                ),
                id: action.id,
                label: action.label,
            })
            .collect()
    })
}

pub(super) fn is_available(
    action: &userscript::RegisteredAction,
    core_state_available: bool,
    durable_storage_available: bool,
    private_profile: bool,
) -> bool {
    let requires_storage = matches!(
        action.subject.as_str(),
        "download" | "history-entry" | "bookmark" | "quickmark" | "session"
    );
    core_state_available
        && (!requires_storage || durable_storage_available)
        && (!private_profile || action.allow_private)
}

pub(super) fn is_hint_only(action: &userscript::RegisteredAction) -> bool {
    action.subject == "link"
        && action
            .required_fields
            .iter()
            .any(|field| field == "hint_url")
        && !action.required_fields.iter().any(|field| field == "url")
}

pub(super) fn subject_argument_name(subject: &str) -> Option<&'static str> {
    match subject {
        "link" => Some("url"),
        "selection" => Some("selection"),
        "download" => Some("download_id"),
        "history-entry" => Some("history_id"),
        "bookmark" => Some("bookmark_id"),
        "quickmark" => Some("quickmark_name"),
        "session" => Some("session_name"),
        "command" => Some("command_id"),
        "url" => None,
        "tab" => Some("tab_id"),
        "window" => Some("window_id"),
        "context" => Some("context_name"),
        _ => None,
    }
}

pub(super) fn subject_target(
    state: &BrowserApplication,
    current: Target,
    subject: &str,
    value: &str,
) -> Result<Target, String> {
    if !matches!(subject, "tab" | "window" | "context") || value.is_empty() {
        return Ok(current);
    }
    let tab = match subject {
        "tab" => {
            let tab = TabId::from_display(value)
                .ok_or_else(|| "userscript tab action target is invalid".to_owned())?;
            state
                .tabs()
                .get(&tab)
                .filter(|tab_state| tab_state.existence == ExistenceState::Live)
                .map(|_| tab)
                .ok_or_else(|| "userscript tab action target is stale".to_owned())?
        }
        "window" => {
            let window = WindowId::from_display(value)
                .ok_or_else(|| "userscript window action target is invalid".to_owned())?;
            let window = state
                .windows()
                .get(&window)
                .ok_or_else(|| "userscript window action target is stale".to_owned())?;
            window
                .active_tab
                .filter(|tab| {
                    state.tabs().get(tab).is_some_and(|tab_state| {
                        tab_state.existence == ExistenceState::Live && tab_state.window == window.id
                    })
                })
                .ok_or_else(|| "userscript window has no live active tab".to_owned())?
        }
        "context" => state
            .windows()
            .values()
            .filter(|window| window.context.as_deref() == Some(value))
            .filter_map(|window| window.active_tab)
            .find(|tab| {
                state
                    .tabs()
                    .get(tab)
                    .is_some_and(|tab_state| tab_state.existence == ExistenceState::Live)
            })
            .ok_or_else(|| "userscript context action target is stale".to_owned())?,
        _ => unreachable!("subject was checked above"),
    };
    state
        .capture_target(tab)
        .ok_or_else(|| "userscript action target is stale".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{action_value, is_available, subject_argument_name};
    use crate::userscript::RegisteredAction;

    fn action(subject: &str, fields: &[&str], allow_private: bool) -> RegisteredAction {
        RegisteredAction {
            id: "userscript.example.run".into(),
            subject: subject.into(),
            verb: "run".into(),
            label: "Run example".into(),
            script: "example".into(),
            required_fields: fields.iter().map(|field| (*field).into()).collect(),
            allow_private,
        }
    }

    #[test]
    fn projection_keeps_the_userscript_boundary_typed() {
        let value = action_value(&action("link", &["hint_url"], false), true);
        assert_eq!(value["sources"], serde_json::json!(["hint"]));
        assert_eq!(value["arguments"][0]["kind"], "url");
        assert_eq!(value["availability"]["state"], "available");
    }

    #[test]
    fn availability_enforces_storage_and_private_boundaries() {
        assert!(!is_available(
            &action("bookmark", &[], true),
            true,
            false,
            false
        ));
        assert!(!is_available(&action("link", &[], false), true, true, true));
        assert!(is_available(&action("link", &[], true), true, true, true));
    }

    #[test]
    fn subjects_map_to_their_stable_argument_names() {
        assert_eq!(subject_argument_name("link"), Some("url"));
        assert_eq!(subject_argument_name("bookmark"), Some("bookmark_id"));
        assert_eq!(subject_argument_name("url"), None);
        assert_eq!(subject_argument_name("unregistered"), None);
    }
}
