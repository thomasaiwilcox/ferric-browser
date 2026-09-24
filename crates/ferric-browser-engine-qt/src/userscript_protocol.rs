//! Bounded newline-delimited protocol used by local userscript processes.

use serde_json::{Value, json};

use crate::userscript::{MAX_PROTOCOL_BYTES, MAX_PROTOCOL_LINE_BYTES, Manifest};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserscriptAction {
    Open { url: String },
    Yank { url: String, clean: bool },
    Command { name: String, arguments: Value },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedOutput {
    pub message: String,
    pub action: Option<UserscriptAction>,
}

#[must_use]
pub fn protocol_input(operation_id: &str, context: &Value) -> Value {
    json!({
        "protocol_version": 1,
        "invocation_id": operation_id,
        "context": context,
    })
}

/// Parses and validates bounded newline-delimited userscript output.
///
/// # Errors
///
/// Returns an error for oversized, malformed, undeclared, duplicated, or
/// otherwise unsafe result messages/actions.
pub fn parse_output(bytes: &[u8], manifest: &Manifest) -> Result<ParsedOutput, String> {
    if bytes.len() > MAX_PROTOCOL_BYTES {
        return Err("userscript stdout exceeds 1 MiB".into());
    }
    if bytes.is_empty() {
        return Err("userscript produced no JSON result".into());
    }
    let mut messages = Vec::new();
    let mut action = None;
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_PROTOCOL_LINE_BYTES {
            return Err("userscript result line exceeds 64 KiB".into());
        }
        let value: Value = serde_json::from_slice(line)
            .map_err(|error| format!("invalid userscript JSON result: {error}"))?;
        let result_type = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| "userscript result requires a string type".to_owned())?;
        if !manifest
            .allowed_results
            .iter()
            .any(|allowed| allowed == result_type)
        {
            return Err(format!(
                "userscript result type is not allowed: {result_type}"
            ));
        }
        match result_type {
            "message" => {
                ensure_keys(&value, &["type", "text"])?;
                let message = value
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "userscript message result requires text".to_owned())?;
                if message.chars().any(char::is_control) {
                    return Err("userscript message contains a control character".into());
                }
                messages.push(message.chars().take(256).collect::<String>());
            }
            "open" => {
                ensure_action_is_unique(action.as_ref())?;
                ensure_keys(&value, &["type", "url"])?;
                let url = bounded_text(&value, "url", "userscript open result")?;
                action = Some(UserscriptAction::Open { url });
            }
            "yank" => {
                ensure_action_is_unique(action.as_ref())?;
                ensure_keys(&value, &["type", "url", "clean"])?;
                let url = bounded_text(&value, "url", "userscript yank result")?;
                let clean = value.get("clean").map_or(Ok(false), |value| {
                    value
                        .as_bool()
                        .ok_or_else(|| "userscript yank clean must be a boolean".to_owned())
                })?;
                action = Some(UserscriptAction::Yank { url, clean });
            }
            "command" => {
                ensure_action_is_unique(action.as_ref())?;
                ensure_keys(&value, &["type", "command", "arguments"])?;
                let name = bounded_text(&value, "command", "userscript command result")?;
                if !manifest
                    .allowed_commands
                    .iter()
                    .any(|allowed| allowed == &name)
                {
                    return Err(format!("userscript command is not allowed: {name}"));
                }
                let arguments = value
                    .get("arguments")
                    .cloned()
                    .ok_or_else(|| "userscript command result requires arguments".to_owned())?;
                if !arguments.is_object() {
                    return Err("userscript command result arguments must be an object".into());
                }
                action = Some(UserscriptAction::Command { name, arguments });
            }
            _ => {
                return Err(format!(
                    "userscript result type is not implemented: {result_type}"
                ));
            }
        }
    }
    if messages.is_empty() && action.is_none() {
        return Err("userscript produced no JSON result".into());
    }
    Ok(ParsedOutput {
        message: messages.join(" | "),
        action,
    })
}

fn ensure_action_is_unique(action: Option<&UserscriptAction>) -> Result<(), String> {
    if action.is_some() {
        Err("userscript produced more than one action result".into())
    } else {
        Ok(())
    }
}

fn ensure_keys(value: &Value, allowed: &[&str]) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "userscript result must be an object".to_owned())?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("userscript result contains an unknown field".into());
    }
    Ok(())
}

fn bounded_text(value: &Value, key: &str, label: &str) -> Result<String, String> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 8192)
        .ok_or_else(|| format!("{label} requires a nonempty value of at most 8192 bytes"))?;
    if text.chars().any(char::is_control) {
        return Err(format!("{label} contains a control character"));
    }
    Ok(text.to_owned())
}
