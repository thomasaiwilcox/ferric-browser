//! Public assembly of stable IPC envelopes and canonical runtime commands.

use crate::{
    CommandEnvelope, command_argument_decoder::decode_command_arguments, decode_command_envelope,
};
use ferric_browser_core::ParsedCommand;
use serde_json::Value;

/// A validated IPC envelope paired with the canonical runtime command.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandInvocation {
    pub envelope: CommandEnvelope,
    pub command: ParsedCommand,
}

/// Decodes one stable IPC envelope into a canonical command invocation.
///
/// # Errors
///
/// Returns an error when the envelope or command arguments violate the shared
/// command contract.
pub fn decode_command_invocation(params: &Value) -> Result<CommandInvocation, String> {
    let envelope = decode_command_envelope(params)?;
    let command = decode_command_arguments(&envelope)?;
    Ok(CommandInvocation { envelope, command })
}

#[cfg(test)]
mod tests {
    use super::decode_command_invocation;
    use crate::encode_command;
    use ferric_browser_core::{CommandRegistry, ParseInput, parse_chain};
    use serde_json::json;

    #[test]
    fn invocation_preserves_command_and_routing_context() {
        let invocation = decode_command_invocation(&json!({
            "command": "open",
            "arguments": {"input": "https://example.test", "clean_link": true},
            "context": {"profile": "work", "source": "cli"}
        }))
        .expect("valid command invocation");

        assert_eq!(invocation.command.name, "open");
        assert_eq!(
            invocation.command.arguments,
            ["--clean-link", "https://example.test"]
        );
        assert_eq!(invocation.envelope.context.profile.as_deref(), Some("work"));
        assert_eq!(invocation.envelope.context.source.as_deref(), Some("cli"));
    }

    #[test]
    fn invocation_rejects_closed_argument_contracts() {
        assert!(
            decode_command_invocation(&json!({
                "command": "hint",
                "arguments": {"kind": "scripts"}
            }))
            .is_err()
        );
        assert!(
            decode_command_invocation(&json!({
                "command": "open",
                "arguments": {"input": "https://example.test", "clean_link": "yes"}
            }))
            .is_err()
        );
    }

    #[test]
    fn every_registered_command_example_round_trips_through_the_public_envelope() {
        let registry = CommandRegistry::default_v1();
        for definition in registry.definitions() {
            for example in &definition.examples {
                let parsed = parse_chain(example, ParseInput::Cli)
                    .unwrap_or_else(|error| panic!("could not parse example {example:?}: {error}"));
                assert_eq!(
                    parsed.len(),
                    1,
                    "example {example:?} must contain one command"
                );
                let envelope = encode_command(&parsed[0]).unwrap_or_else(|error| {
                    panic!("could not encode example {example:?}: {error}")
                });
                let invocation = decode_command_invocation(&envelope).unwrap_or_else(|error| {
                    panic!("could not decode example {example:?}: {error}")
                });
                assert_eq!(
                    invocation.command.name, parsed[0].name,
                    "example {example:?} changed command family while crossing the IPC boundary"
                );
                assert_eq!(
                    invocation.envelope.into_value(),
                    envelope,
                    "example {example:?} changed its public JSON shape"
                );
            }
        }
    }

    #[test]
    fn open_options_are_projected_to_routing_without_becoming_url_text() {
        // Characterization: this used to encode `--target tab` as part of the
        // URL input, so a supported documented command navigated to nonsense.
        let command = parse_chain(
            "open --target tab --profile work --context project https://example.test",
            ParseInput::Cli,
        )
        .expect("valid command")
        .pop()
        .expect("one command");
        let envelope = encode_command(&command).expect("encode open options");
        assert_eq!(envelope["arguments"]["input"], "https://example.test");
        assert_eq!(envelope["arguments"]["target"], "tab");
        assert_eq!(envelope["context"]["profile"], "work");
        assert_eq!(envelope["context"]["context"], "project");

        let invocation = decode_command_invocation(&envelope).expect("decode open options");
        assert_eq!(invocation.command.arguments, ["https://example.test"]);
    }
}
