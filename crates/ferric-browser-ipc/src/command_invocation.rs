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
    fn editing_and_passthrough_variants_round_trip_through_the_public_envelope() {
        for mode in [
            "insert",
            "insert-locked",
            "passthrough",
            "pass-through",
            "passthrough-once",
            "pass-through-once",
        ] {
            let command = ferric_browser_core::ParsedCommand {
                name: "mode-enter".into(),
                arguments: vec![mode.into()],
            };
            let envelope = encode_command(&command).expect("encode input mode");
            assert_eq!(envelope["arguments"], json!({"mode": mode}));
            let invocation = decode_command_invocation(&envelope).expect("decode input mode");
            assert_eq!(invocation.command, command);
        }
    }

    #[test]
    fn locked_insert_binding_commands_round_trip_through_the_public_envelope() {
        for text in [
            "bind --mode insert-locked Escape mode-enter normal",
            "unbind --mode insert-locked Escape",
            "binding-list --mode insert-locked",
            "binding-explain i --mode insert-locked",
        ] {
            let commands = parse_chain(text, ParseInput::Cli).expect("parse binding command");
            let envelope = encode_command(&commands[0]).expect("encode locked binding command");
            assert_eq!(envelope["arguments"]["mode"], "insert-locked");
            let invocation =
                decode_command_invocation(&envelope).expect("decode locked binding command");
            assert_eq!(
                encode_command(&invocation.command).expect("re-encode locked binding command"),
                envelope
            );
        }
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
    fn region_surface_hint_families_round_trip_and_reject_other_actions() {
        for family in ["regions", "surfaces"] {
            let command = ferric_browser_core::ParsedCommand {
                name: "hint".into(),
                arguments: vec![family.into()],
            };
            let envelope = encode_command(&command).expect("encode explicit facet family");
            assert_eq!(envelope["arguments"], json!({"kind": family}));
            let invocation = decode_command_invocation(&envelope).expect("decode facet family");
            assert_eq!(invocation.command, command);
        }

        for arguments in [
            json!({"kind":"regions", "target":"current"}),
            json!({"kind":"regions", "target":"tab"}),
            json!({"kind":"surfaces", "rapid":true}),
            json!({"kind":"surfaces", "first":true}),
            json!({"kind":"surfaces", "index":2, "first":true}),
            json!({"kind":"regions", "script":"reader"}),
        ] {
            let envelope = json!({"command":"hint", "arguments":arguments});
            assert!(decode_command_invocation(&envelope).is_err(), "{envelope}");
        }

        for arguments in [
            vec!["regions".into(), "--target".into(), "current".into()],
            vec!["surfaces".into(), "--rapid".into()],
            vec!["surfaces".into(), "--first".into()],
        ] {
            let command = ferric_browser_core::ParsedCommand {
                name: "hint".into(),
                arguments,
            };
            assert!(encode_command(&command).is_err());
        }
    }

    #[test]
    fn scroll_target_actions_round_trip_and_reject_open_ended_forms() {
        for action in ["select", "auto", "document", "status"] {
            let parsed = ferric_browser_core::ParsedCommand {
                name: "scroll-target".into(),
                arguments: vec![action.into()],
            };
            let envelope = encode_command(&parsed).expect("encode scroll target action");
            assert_eq!(envelope["arguments"], json!({"action": action}));
            assert_eq!(
                decode_command_invocation(&envelope)
                    .expect("decode scroll target action")
                    .command,
                parsed
            );
        }
        for envelope in [
            json!({"command":"scroll-target","arguments":{}}),
            json!({"command":"scroll-target","arguments":{"action":null}}),
            json!({"command":"scroll-target","arguments":{"action":1}}),
            json!({"command":"scroll-target","arguments":{"action":"element"}}),
            json!({"command":"scroll-target","arguments":{"action":"auto","extra":true}}),
            json!({"command":"scroll-target","arguments":{"action":"auto\n"}}),
        ] {
            assert!(decode_command_invocation(&envelope).is_err(), "{envelope}");
        }
        for arguments in [
            vec![],
            vec!["auto".into(), "status".into()],
            vec!["Auto".into()],
            vec!["element".into()],
        ] {
            assert!(
                encode_command(&ferric_browser_core::ParsedCommand {
                    name: "scroll-target".into(),
                    arguments,
                })
                .is_err()
            );
        }
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
                if definition.name == "target-search" && !parsed[0].arguments.is_empty() {
                    assert!(
                        encode_command(&parsed[0]).is_err(),
                        "scoped Target Search unexpectedly has a public IPC envelope: {example}"
                    );
                    continue;
                }
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

#[cfg(test)]
mod passthrough_tests {
    use crate::{decode_command_invocation, encode_command};
    use ferric_browser_core::{ParseInput, parse_chain};
    #[test]
    fn one_key_mode_is_accepted_by_binding_commands() {
        for text in [
            "bind --mode pass-through-once f mode-enter normal",
            "unbind --mode pass-through-once f",
            "binding-list --mode pass-through-once",
            "binding-explain f --mode pass-through-once",
        ] {
            let commands = parse_chain(text, ParseInput::Cli).unwrap();
            let envelope = encode_command(&commands[0]).unwrap();
            assert_eq!(envelope["arguments"]["mode"], "pass-through-once");
            let invocation = decode_command_invocation(&envelope).unwrap();
            assert_eq!(encode_command(&invocation.command).unwrap(), envelope);
        }
    }
}
