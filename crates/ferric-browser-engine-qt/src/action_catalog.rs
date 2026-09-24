//! Read-only action catalog projections.
//!
//! These functions turn authoritative registry/configuration data into the
//! typed presentation payloads consumed by the Qt models. They deliberately
//! do not inspect a `QObject` or execute an action.

use crate::input_validation::is_bounded_untrusted_text;
use ferric_browser_config::ActionTargetConfig;
use ferric_browser_core::{ActionRegistry, ActionSubject};
use serde_json::Value;

/// The bounded data required to render one configured external action.
///
/// This is deliberately smaller than the JSON discovery representation used
/// by IPC: QML only needs a stable action ID, a label, and whether it can be
/// offered in the current profile.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ExternalActionPresentation {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) available: bool,
}

/// Projects configured external actions for one context-menu subject.
///
/// Configuration decoding stays at this boundary, and the resulting typed
/// columns can cross into QML without asking presentation code to parse an
/// action-policy JSON document.
pub(super) fn external_action_presentations(
    config: &Value,
    subject: &str,
    private_profile: bool,
) -> Result<Vec<ExternalActionPresentation>, String> {
    let subject_label = match subject {
        "selection" => "selection",
        "tab" => "tab",
        "url" => "URL",
        "link" => "link",
        _ => return Err(format!("unknown external action subject: {subject}")),
    };

    Ok(configured_action_targets(config)?
        .into_iter()
        .filter(|(_, target)| {
            target
                .subject_types
                .iter()
                .any(|declared| declared == subject)
        })
        .map(|(name, target)| ExternalActionPresentation {
            id: format!("external.{name}.{subject}.send"),
            label: format!("Send {subject_label} to {name}"),
            available: !private_profile || target.allow_private,
        })
        .collect())
}

/// Serializes the built-in action registry for discovery and completion.
pub(super) fn action_list_value(subject: Option<&str>) -> Result<Value, String> {
    let registry = ActionRegistry::default_v1();
    if let Some(subject) = subject
        && !registry
            .definitions()
            .iter()
            .any(|definition| definition.subject.as_str() == subject)
    {
        return Err(format!("unknown action subject: {subject}"));
    }
    Ok(Value::Array(
        registry
            .definitions()
            .iter()
            .filter(|definition| {
                subject.is_none_or(|subject| definition.subject.as_str() == subject)
            })
            .map(|definition| {
                serde_json::json!({
                    "id": definition.id,
                    "subject": definition.subject.as_str(),
                    "verb": definition.verb,
                    "label": definition.label,
                    "description": definition.description,
                    "command": definition.command,
                    "arguments": definition.arguments.iter().map(|argument| serde_json::json!({
                        "name": argument.name,
                        "kind": format!("{:?}", argument.kind).to_ascii_lowercase(),
                        "required": argument.required
                    })).collect::<Vec<_>>(),
                    "sources": definition.sources.iter().map(|source| source.as_str()).collect::<Vec<_>>(),
                    "effect": format!("{:?}", definition.effect).to_ascii_lowercase(),
                    "confirmation": definition.confirmation.as_str(),
                    "sensitive": definition.sensitive,
                    "required_capabilities": definition.required_capabilities(),
                    "completion_provider": definition.completion_provider(),
                    "availability_predicate": definition.availability_predicate(),
                    "availability": {
                        "state": "subject-dependent",
                        "predicate": definition.availability_predicate(),
                        "requires_subject_revalidation": true
                    },
                    "examples": definition.examples
                })
            })
            .collect(),
    ))
}

/// Counts configured security deny rules without converting malformed input
/// into a browser policy decision.
pub(super) fn security_deny_rule_count(config_json: &str) -> usize {
    serde_json::from_str::<Value>(config_json)
        .ok()
        .and_then(|config| config.get("blocking").cloned())
        .and_then(|blocking| blocking.get("security_deny_hosts").cloned())
        .filter(Value::is_array)
        .and_then(|rules| rules.as_array().map(Vec::len))
        .unwrap_or(0)
}

/// Decodes configured external action targets at the configuration boundary.
pub(super) fn configured_action_targets(
    config: &Value,
) -> Result<Vec<(String, ActionTargetConfig)>, String> {
    let Some(values) = config.get("action_targets").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    values
        .iter()
        .map(|(name, value)| {
            serde_json::from_value::<ActionTargetConfig>(value.clone())
                .map(|target| (name.clone(), target))
                .map_err(|error| format!("configured action target {name} is invalid: {error}"))
        })
        .collect()
}

pub(super) fn configured_action_target_supports_subject(
    config: &Value,
    subject: ActionSubject,
) -> Result<bool, String> {
    let subject = subject.as_str();
    Ok(configured_action_targets(config)?
        .iter()
        .any(|(_, target)| target.subject_types.iter().any(|value| value == subject)))
}

pub(super) fn configured_action_target_value(
    name: &str,
    target: &ActionTargetConfig,
    subject: &str,
    private_profile: bool,
) -> Value {
    let selection = subject == "selection";
    let tab = subject == "tab";
    let url_like = matches!(subject, "url" | "link");
    let subject_label = match subject {
        "selection" => "selection",
        "tab" => "tab",
        "url" => "URL",
        _ => "link",
    };
    let (availability_state, availability_reason) = if private_profile && !target.allow_private {
        ("unavailable", "private-profile")
    } else {
        ("available", "ready")
    };
    let completion_provider = if url_like { "url" } else { "text" };
    let availability_predicate = match subject {
        "url" => "current-document",
        "link" => "captured-link",
        "selection" => "live-selection",
        "tab" => "live-tab",
        _ => "unknown",
    };
    serde_json::json!({
        "id": format!("external.{name}.{subject}.send"),
        "subject": subject,
        "verb": "send",
        "label": format!("Send {subject_label} to {name}"),
        "description": if selection {
            "Send the current visible selection to a configured external target."
        } else if tab {
            "Send the active tab URL and title to a configured external target."
        } else if subject == "url" {
            "Send the current or explicitly supplied URL to a configured external target."
        } else {
            "Send a validated link to a configured external target."
        },
        "command": "send",
        "effect": "sensitive",
        "confirmation": "never",
        "sensitive": true,
        "required_capabilities": ["configured-action-target"],
        "target": name,
        "subjects": target.subject_types,
        "detach": target.detach,
        "allow_private": target.allow_private,
        "completion_provider": completion_provider,
        "availability_predicate": availability_predicate,
        "availability": {
            "state": availability_state,
            "reason": availability_reason,
            "predicate": availability_predicate,
            "requires_subject_revalidation": true
        },
        "examples": [format!("action {subject} send --to {name}")],
        "arguments": if selection || tab {
            serde_json::json!([{"name": "target", "kind": "text", "required": true}])
        } else if url_like {
            serde_json::json!([
                {"name": "target", "kind": "text", "required": true},
                {"name": "url", "kind": "url", "required": subject == "link"}
            ])
        } else {
            serde_json::json!([{"name": "target", "kind": "text", "required": true}])
        }
    })
}

pub(super) fn configured_action_target_values(
    name: &str,
    target: &ActionTargetConfig,
    private_profile: bool,
) -> Vec<Value> {
    ["url", "link", "selection", "tab"]
        .into_iter()
        .filter(|subject| {
            target
                .subject_types
                .iter()
                .any(|declared| declared == subject)
        })
        .map(|subject| configured_action_target_value(name, target, subject, private_profile))
        .collect()
}

pub(super) fn configured_switcher_action_values(
    config: &Value,
    private_profile: bool,
) -> Result<Vec<Value>, String> {
    let mut values = Vec::new();
    for (name, target) in configured_action_targets(config)? {
        values.extend(
            configured_action_target_values(&name, &target, private_profile)
                .into_iter()
                .filter(|value| {
                    matches!(
                        value.get("subject").and_then(Value::as_str),
                        Some("url" | "tab")
                    ) && value
                        .get("availability")
                        .and_then(|availability| availability.get("state"))
                        .and_then(Value::as_str)
                        == Some("available")
                }),
        );
    }
    Ok(values)
}

/// Parses an externally configured action ID without accepting an arbitrary
/// process name or action subject.
pub(super) fn parse_external_action_id(action_id: &str) -> Option<(String, String)> {
    let [prefix, name, subject, verb] = action_id.split('.').collect::<Vec<_>>().try_into().ok()?;
    if prefix != "external"
        || verb != "send"
        || !is_bounded_untrusted_text(name)
        || !name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        || !matches!(subject, "url" | "link" | "selection" | "tab")
    {
        return None;
    }
    Some((name.to_owned(), subject.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_filters_known_subjects_and_counts_only_valid_rules() {
        let actions = action_list_value(Some("bookmark")).expect("known subject");
        assert!(actions.as_array().is_some_and(|values| {
            !values.is_empty() && values.iter().all(|value| value["subject"] == "bookmark")
        }));
        assert!(action_list_value(Some("not-a-subject")).is_err());
        assert_eq!(
            security_deny_rule_count(
                r#"{"blocking":{"security_deny_hosts":["example.test","invalid.test"]}}"#
            ),
            2
        );
        assert_eq!(security_deny_rule_count("not json"), 0);
        let config = serde_json::json!({
            "action_targets": {
                "mpv": {"subject_types": ["url", "link"], "executable": "mpv"}
            }
        });
        assert!(
            configured_action_target_supports_subject(&config, ActionSubject::Url)
                .expect("valid action target")
        );
        assert_eq!(
            parse_external_action_id("external.mpv.link.send"),
            Some(("mpv".into(), "link".into()))
        );
        assert!(parse_external_action_id("external.MPV.link.send").is_none());

        let external = external_action_presentations(&config, "link", false)
            .expect("known external action subject");
        assert_eq!(external.len(), 1);
        assert_eq!(external[0].id, "external.mpv.link.send");
        assert_eq!(external[0].label, "Send link to mpv");
        assert!(external[0].available);
        let private = external_action_presentations(&config, "link", true)
            .expect("known external action subject");
        assert!(!private[0].available);
        assert!(external_action_presentations(&config, "invalid", false).is_err());
    }
}
