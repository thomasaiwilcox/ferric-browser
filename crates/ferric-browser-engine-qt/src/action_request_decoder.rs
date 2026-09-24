//! Typed action IPC decoding.
//!
//! Action identity and parameter contracts come from the core registry. This
//! module translates a validated action request into the shared command
//! decoder without accessing a Qt object or application state.

use crate::{
    action_catalog::parse_external_action_id,
    input_validation::is_bounded_untrusted_text,
    ipc_command_decoder::typed_ipc_command,
    ipc_contract::{IpcCommandError, IpcRoute},
};
use ferric_browser_core::{
    ActionDefinition, ActionRegistry, ActionSubject, ArgumentKind, CommandRegistry, ParsedCommand,
    ValidatedUrl,
};
use serde_json::Value;

pub(super) fn typed_ipc_action(
    params: &Value,
) -> Result<(ParsedCommand, IpcRoute, String), IpcCommandError> {
    let object = params
        .as_object()
        .ok_or_else(|| "action.execute params must be an object".to_owned())?;
    let action = object
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| "action.execute requires a string action".to_owned())?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "action" | "arguments" | "context"))
    {
        return Err("action request contains an unknown field".into());
    }
    if let Some((target_name, subject)) = parse_external_action_id(action) {
        let mut arguments = object
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}))
            .as_object()
            .cloned()
            .ok_or_else(|| "external action arguments must be an object".to_owned())?;
        if arguments
            .keys()
            .any(|key| !matches!(key.as_str(), "target" | "url" | "input"))
        {
            return Err("external action contains an unknown argument".into());
        }
        if let Some(target) = arguments.remove("target")
            && target.as_str() != Some(target_name.as_str())
        {
            return Err("external action target does not match its action ID".into());
        }
        let url = match (arguments.remove("url"), arguments.remove("input")) {
            (Some(_), Some(_)) => {
                return Err("external action arguments cannot contain both url and input".into());
            }
            (Some(value), None) | (None, Some(value)) => Some(
                value
                    .as_str()
                    .filter(|value| is_bounded_untrusted_text(value))
                    .ok_or_else(|| "external action URL must be bounded text".to_owned())?
                    .to_owned(),
            ),
            (None, None) => None,
        };
        let mut command_arguments = serde_json::json!({
            "target": target_name,
            "send_subject": subject,
        });
        match subject.as_str() {
            "link" => {
                command_arguments["input"] = Value::String(
                    url.ok_or_else(|| "external link action requires a URL".to_owned())?,
                );
            }
            "url" => {
                if let Some(url) = url {
                    command_arguments["url"] = Value::String(url);
                }
            }
            "tab" | "selection" if url.is_some() => {
                return Err(format!("external {subject} action does not accept a URL").into());
            }
            "tab" | "selection" => {}
            _ => unreachable!("external action ID parser validated subject"),
        }
        let mut command = serde_json::json!({
            "command": "send",
            "arguments": command_arguments
        });
        if let Some(context) = object.get("context") {
            command["context"] = context.clone();
        }
        return typed_ipc_command(&command)
            .map(|(command, route)| (command, route, action.to_owned()));
    }
    let action_registry = ActionRegistry::default_v1();
    let (command_name, action_id) = if let Some(definition) = action_registry.resolve(action) {
        (definition.command.clone(), definition.id.clone())
    } else if CommandRegistry::default_v1().resolve(action).is_ok() {
        (action.to_owned(), format!("legacy.command.{action}"))
    } else {
        return Err(format!("unknown action: {action}").into());
    };
    let definition = action_registry.resolve(action);
    if let Some(definition) = definition {
        let arguments = object
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        validate_action_arguments(definition, &arguments)?;
    }
    let mut command = serde_json::json!({
        "command": command_name,
        "arguments": object.get("arguments").cloned().unwrap_or_else(|| serde_json::json!({}))
    });
    let link_action = action_registry
        .resolve(action)
        .is_some_and(|definition| definition.subject == ActionSubject::Link);
    if let Some(definition) = action_registry.resolve(action)
        && definition.command == "yank"
        && let Some(arguments) = command["arguments"].as_object_mut()
    {
        arguments
            .entry("source")
            .or_insert_with(|| Value::String(definition.subject.as_str().into()));
        if definition.verb == "clean-copy" {
            arguments.insert("clean".into(), Value::Bool(true));
        }
    }
    if link_action
        && let Some(arguments) = command["arguments"].as_object_mut()
        && let Some(url) = arguments.remove("url")
    {
        if arguments.contains_key("input") {
            return Err("link action arguments cannot contain both url and input".into());
        }
        arguments.insert("input".into(), url);
    }
    if let Some(definition) = action_registry.resolve(action)
        && definition.command == "send"
        && let Some(arguments) = command["arguments"].as_object_mut()
    {
        arguments.insert(
            "send_subject".into(),
            Value::String(definition.subject.as_str().into()),
        );
    }
    if let Some(context) = object.get("context") {
        command["context"] = context.clone();
    }
    typed_ipc_command(&command).map(|(command, route)| (command, route, action_id))
}

fn validate_action_arguments(
    definition: &ActionDefinition,
    arguments: &Value,
) -> Result<(), String> {
    let arguments = arguments
        .as_object()
        .ok_or_else(|| format!("action {} arguments must be an object", definition.id))?;
    for key in arguments.keys() {
        if !definition
            .arguments
            .iter()
            .any(|argument| argument.name == *key)
        {
            return Err(format!(
                "action {} does not accept argument {key}",
                definition.id
            ));
        }
    }
    for argument in &definition.arguments {
        if argument.required && !arguments.contains_key(&argument.name) {
            return Err(format!(
                "action {} requires argument {}",
                definition.id, argument.name
            ));
        }
        if let Some(value) = arguments.get(&argument.name) {
            let valid = match argument.kind {
                ArgumentKind::Text | ArgumentKind::Enum => {
                    value.as_str().is_some_and(is_bounded_untrusted_text)
                }
                ArgumentKind::Url => value.as_str().is_some_and(|value| {
                    is_bounded_untrusted_text(value) && ValidatedUrl::parse(value).is_ok()
                }),
                ArgumentKind::Boolean => value.is_boolean(),
                ArgumentKind::Integer => value.as_i64().is_some(),
                ArgumentKind::Object => value.is_object(),
            };
            if !valid {
                return Err(format!(
                    "action {} argument {} has the wrong type",
                    definition.id, argument.name
                ));
            }
        }
    }
    Ok(())
}
