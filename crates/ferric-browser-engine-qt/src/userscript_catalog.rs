//! Projection of installed userscript actions into the typed action catalog.
//!
//! Application-owned discovery supplies validated catalog records. This adapter
//! combines them with native capability availability for presentation.

use crate::userscript;
use ferric_browser_application::BrowserApplication;
use ferric_browser_core::{
    ActionRegistry, ExistenceState, PrivacyKind, TabId, Target, ValidatedUrl, WindowId,
};
use serde_json::Value;

#[derive(Debug)]
pub(super) struct ActionPresentation {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) available: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Availability {
    pub(super) core_state_available: bool,
    pub(super) durable_storage_available: bool,
    pub(super) download_session_available: bool,
    pub(super) privacy: Option<PrivacyKind>,
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
    actions: Result<Vec<userscript::RegisteredAction>, String>,
    subject: Option<&str>,
    availability: Availability,
) -> Result<Vec<Value>, String> {
    let builtins = ActionRegistry::default_v1();
    actions.map(|actions| {
        actions
            .into_iter()
            .filter(|action| {
                subject.is_none_or(|subject| action.subject == subject)
                    && builtins.resolve(&action.id).is_none()
            })
            .map(|action| {
                let available = is_available(&action, availability);
                action_value(&action, available)
            })
            .collect()
    })
}

pub(super) fn action_presentations(
    actions: Result<Vec<userscript::RegisteredAction>, String>,
    subject: &str,
    availability: Availability,
) -> Result<Vec<ActionPresentation>, String> {
    let builtins = ActionRegistry::default_v1();
    actions.map(|actions| {
        actions
            .into_iter()
            .filter(|action| action.subject == subject && builtins.resolve(&action.id).is_none())
            .map(|action| ActionPresentation {
                available: is_available(&action, availability),
                id: action.id,
                label: action.label,
            })
            .collect()
    })
}

pub(super) fn is_available(
    action: &userscript::RegisteredAction,
    availability: Availability,
) -> bool {
    let requires_storage = matches!(
        action.subject.as_str(),
        "history-entry" | "bookmark" | "quickmark" | "session"
    );
    availability.core_state_available
        && (!requires_storage || availability.durable_storage_available)
        && (action.subject != "download" || availability.download_session_available)
        && (!availability.privacy.is_some_and(PrivacyKind::is_transient) || action.allow_private)
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

pub(super) fn link_context_url(value: &str) -> Result<String, String> {
    let url = ValidatedUrl::parse(value.to_owned())
        .map_err(|error| format!("userscript action link URL is invalid: {error}"))?;
    let url = crate::url_safety::safe_ipc_url(url.as_str());
    if !ferric_browser_storage::is_safe_history_url(&url) {
        return Err("userscript action link URL is not eligible".into());
    }
    Ok(url)
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

impl crate::BrowserUiRust {
    pub(super) fn cached_userscript_actions(
        &self,
    ) -> Result<Vec<userscript::RegisteredAction>, String> {
        self.state.as_ref().map_or_else(
            || Ok(Vec::new()),
            |state| Ok(state.userscript_catalog()?.actions().to_vec()),
        )
    }

    pub(super) fn cached_userscript(
        &self,
        name: &str,
    ) -> Result<userscript::LoadedManifest, String> {
        self.state
            .as_ref()
            .ok_or_else(|| "userscript owner is unavailable".to_owned())?
            .userscript_catalog()?
            .manifest(name)
            .cloned()
            .ok_or_else(|| format!("userscript {name} is not installed"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Availability, PrivacyKind, action_value, is_available, subject_argument_name};
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
    fn link_context_sanitizes_before_applying_the_safe_url_policy() {
        assert_eq!(
            super::link_context_url(
                "https://user:password@example.test/post?keep=1&token=secret#comments"
            )
            .unwrap(),
            "https://example.test/post?keep=1"
        );
        assert_eq!(
            super::link_context_url("https://example.test/post#comments").unwrap(),
            "https://example.test/post"
        );
        for invalid in [
            "javascript:alert(1)",
            "data:text/plain,private",
            "https://example.test/\n",
        ] {
            assert!(super::link_context_url(invalid).is_err());
        }
    }

    #[test]
    fn availability_enforces_storage_session_and_private_boundaries() {
        let session = Availability {
            core_state_available: true,
            durable_storage_available: false,
            download_session_available: true,
            privacy: Some(PrivacyKind::Private),
        };
        assert!(is_available(
            &action("download", &["download_id"], true),
            session
        ));
        assert!(!is_available(
            &action("download", &["download_id"], false),
            session
        ));
        assert!(!is_available(
            &action("bookmark", &["bookmark_id"], true),
            session
        ));
        assert!(!is_available(
            &action("download", &["download_id"], true),
            Availability {
                download_session_available: false,
                ..session
            }
        ));
        assert!(!is_available(
            &action("link", &[], true),
            Availability {
                core_state_available: false,
                ..session
            }
        ));
    }

    #[test]
    fn subjects_map_to_their_stable_argument_names() {
        assert_eq!(subject_argument_name("link"), Some("url"));
        assert_eq!(subject_argument_name("bookmark"), Some("bookmark_id"));
        assert_eq!(subject_argument_name("url"), None);
        assert_eq!(subject_argument_name("unregistered"), None);
    }
}
