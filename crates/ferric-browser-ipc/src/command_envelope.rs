//! Typed command envelope shared by command producers and consumers.

use serde_json::{Map, Value};
use std::fmt;

const MAX_COMMAND_NAME_BYTES: usize = 256;
const MAX_CONTEXT_TEXT_BYTES: usize = 16 * 1024;

/// Optional routing metadata carried alongside a browser command.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct CommandContext {
    pub window: Option<String>,
    pub profile: Option<String>,
    pub context: Option<String>,
    pub source: Option<String>,
}

impl fmt::Debug for CommandContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandContext")
            .field("window_present", &self.window.is_some())
            .field("profile_present", &self.profile.is_some())
            .field("context_present", &self.context.is_some())
            .field("source_present", &self.source.is_some())
            .finish()
    }
}

/// A validated command request at the JSON IPC boundary.
#[derive(Clone, PartialEq)]
pub struct CommandEnvelope {
    pub command: String,
    pub arguments: Map<String, Value>,
    pub context: CommandContext,
}

impl fmt::Debug for CommandEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandEnvelope")
            .field("command", &"[redacted]")
            .field("argument_count", &self.arguments.len())
            .field("context", &self.context)
            .finish()
    }
}

impl CommandEnvelope {
    /// Creates an envelope and validates it against the public field schema.
    ///
    /// # Errors
    ///
    /// Returns an error when the command, argument fields, or context values do
    /// not satisfy the shared IPC schema.
    pub fn new(
        command: String,
        arguments: Map<String, Value>,
        context: CommandContext,
    ) -> Result<Self, String> {
        validate_text(&command, "command name", MAX_COMMAND_NAME_BYTES)?;
        crate::validate_command_argument_fields(&command, &arguments)?;
        for (label, value) in [
            ("window", context.window.as_deref()),
            ("profile", context.profile.as_deref()),
            ("context", context.context.as_deref()),
            ("source", context.source.as_deref()),
        ] {
            if let Some(value) = value {
                validate_text(
                    value,
                    &format!("command context {label}"),
                    MAX_CONTEXT_TEXT_BYTES,
                )?;
            }
        }
        Ok(Self {
            command,
            arguments,
            context,
        })
    }

    /// Serializes the envelope using the stable public JSON shape.
    #[must_use]
    pub fn into_value(self) -> Value {
        let mut object = Map::from_iter([
            ("command".into(), Value::String(self.command)),
            ("arguments".into(), Value::Object(self.arguments)),
        ]);
        let mut context = Map::new();
        for (key, value) in [
            ("window", self.context.window),
            ("profile", self.context.profile),
            ("context", self.context.context),
            ("source", self.context.source),
        ] {
            if let Some(value) = value {
                context.insert(key.into(), Value::String(value));
            }
        }
        if !context.is_empty() {
            object.insert("context".into(), Value::Object(context));
        }
        Value::Object(object)
    }
}

/// Decodes and validates the stable command envelope.
///
/// # Errors
///
/// Returns an error when the value is not a command object or any envelope
/// field violates the shared IPC schema.
pub fn decode_command_envelope(value: &Value) -> Result<CommandEnvelope, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "command.execute params must be an object".to_owned())?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "command" | "arguments" | "context"))
    {
        return Err("command request contains an unknown field".into());
    }
    let command = object
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "command.execute requires a string command".to_owned())?
        .to_owned();
    let arguments = object.get("arguments").map_or_else(
        || Ok(Map::new()),
        |value| {
            value
                .as_object()
                .cloned()
                .ok_or_else(|| "command arguments must be an object".to_owned())
        },
    )?;
    let context = object
        .get("context")
        .map_or_else(|| Ok(CommandContext::default()), decode_context)?;
    CommandEnvelope::new(command, arguments, context)
}

fn decode_context(value: &Value) -> Result<CommandContext, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "command context must be an object".to_owned())?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "window" | "profile" | "context" | "source"))
    {
        return Err("command context contains an unknown field".into());
    }
    let text = |key: &str| {
        object
            .get(key)
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| format!("command context {key} must be a nonempty string"))
            })
            .transpose()
    };
    Ok(CommandContext {
        window: text("window")?,
        profile: text("profile")?,
        context: text("context")?,
        source: text("source")?,
    })
}

fn validate_text(value: &str, label: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        Err(format!(
            "{label} must be bounded nonempty text without control characters"
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_typed_command_envelope() {
        let value = serde_json::json!({
            "command": "open",
            "arguments": {"input": "https://example.test", "external": true},
            "context": {"window": "active", "source": "ipc"}
        });
        let envelope = decode_command_envelope(&value).expect("valid envelope");
        assert_eq!(envelope.command, "open");
        assert_eq!(envelope.context.window.as_deref(), Some("active"));
        assert_eq!(envelope.into_value(), value);
    }

    #[test]
    fn rejects_unknown_outer_and_context_fields() {
        assert!(
            decode_command_envelope(
                &serde_json::json!({"command": "back", "arguments": {}, "extra": true})
            )
            .is_err()
        );
        assert!(
            decode_command_envelope(&serde_json::json!({
                "command": "back",
                "arguments": {},
                "context": {"extra": true}
            }))
            .is_err()
        );
    }

    #[test]
    fn debug_redacts_command_values_and_routing_identity() {
        let sentinel = "private-command-sentinel";
        let envelope = CommandEnvelope::new(
            "open".into(),
            Map::from_iter([("input".into(), Value::String(sentinel.into()))]),
            CommandContext {
                profile: Some(sentinel.into()),
                ..CommandContext::default()
            },
        )
        .expect("valid command envelope");

        let rendered = format!("{envelope:?}");
        assert!(!rendered.contains(sentinel));
        assert!(rendered.contains("argument_count: 1"));
        assert!(rendered.contains("profile_present: true"));
        assert_eq!(
            envelope.into_value()["arguments"]["input"],
            Value::String(sentinel.into()),
            "debug redaction must not alter the serialized command"
        );
    }
}
