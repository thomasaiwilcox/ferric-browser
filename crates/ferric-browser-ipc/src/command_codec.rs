//! Canonical encoding of parsed browser commands into IPC envelopes.

use ferric_browser_core::{ParsedCommand, canonical_origin};
use serde_json::Value;

#[path = "command_codec_automation.rs"]
mod command_codec_automation;
#[path = "command_codec_configuration.rs"]
mod command_codec_configuration;
#[path = "command_codec_content.rs"]
mod command_codec_content;
#[path = "command_codec_contexts.rs"]
mod command_codec_contexts;
#[path = "command_codec_library.rs"]
mod command_codec_library;
#[path = "command_codec_navigation.rs"]
mod command_codec_navigation;
#[path = "command_codec_tabs_windows.rs"]
mod command_codec_tabs_windows;

const MAX_JOURNEY_QUERY_BYTES: usize = 256;

pub(super) fn validate_command_text(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().any(char::is_control) {
        Err(format!(
            "{label} must be nonempty and contain no control characters"
        ))
    } else {
        Ok(())
    }
}

/// Encodes a parsed command into the stable typed IPC envelope.
///
/// Each command family owns its argument grammar; this façade owns only the
/// shared envelope projection.
///
/// # Errors
///
/// Returns an error when the command arguments do not satisfy the public
/// command schema.
pub fn encode_command(command: &ParsedCommand) -> Result<Value, String> {
    let mut encoded = None;
    for encode in [
        command_codec_navigation::encode,
        command_codec_configuration::encode,
        command_codec_library::encode,
        command_codec_tabs_windows::encode,
        command_codec_contexts::encode,
        command_codec_content::encode,
        command_codec_automation::encode,
    ] {
        if let Some(value) = encode(command)? {
            encoded = Some(value);
            break;
        }
    }
    let encoded = encoded.map_or_else(
        || {
            if command.arguments.is_empty() {
                Ok(EncodedCommand::new(serde_json::json!({})))
            } else {
                Err(format!("{} does not accept arguments", command.name))
            }
        },
        Ok,
    )?;
    let argument_object = encoded
        .arguments
        .as_object()
        .cloned()
        .ok_or_else(|| "command arguments must encode as an object".to_owned())?;
    crate::CommandEnvelope::new(command.name.clone(), argument_object, encoded.context)
        .map(crate::CommandEnvelope::into_value)
}

#[derive(Debug)]
pub(super) struct EncodedCommand {
    arguments: Value,
    context: crate::CommandContext,
}

impl EncodedCommand {
    pub(super) fn new(arguments: Value) -> Self {
        Self {
            arguments,
            context: crate::CommandContext::default(),
        }
    }

    pub(super) fn with_context(arguments: Value, context: crate::CommandContext) -> Self {
        Self { arguments, context }
    }
}

pub(super) fn normalize_cli_history_origin(value: &str) -> Result<String, String> {
    let authority = value
        .split_once("://")
        .map(|(_, authority)| authority)
        .filter(|authority| !authority.is_empty() && !authority.contains(['/', '?', '#', '@']))
        .ok_or_else(|| "history-clear --origin requires an exact HTTP(S) origin".to_owned())?;
    if authority.contains('\\') {
        return Err("history-clear --origin requires an exact HTTP(S) origin".into());
    }
    canonical_origin(value)
        .ok_or_else(|| "history-clear --origin requires an exact HTTP(S) origin".to_owned())
}
