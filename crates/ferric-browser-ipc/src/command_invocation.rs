//! Public assembly of stable IPC envelopes and canonical runtime commands.

use crate::{
    CommandEnvelope, command_argument_decoder::decode_command_arguments, decode_command_envelope,
};
use ferric_browser_core::ParsedCommand;
use serde_json::Value;
use std::fmt;

/// A validated IPC envelope paired with the canonical runtime command.
#[derive(Clone, PartialEq)]
pub struct CommandInvocation {
    pub envelope: CommandEnvelope,
    pub command: ParsedCommand,
}

impl fmt::Debug for CommandInvocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandInvocation")
            .field("envelope", &self.envelope)
            .field("command", &"[redacted]")
            .field("argument_count", &self.command.arguments.len())
            .finish()
    }
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
    use ferric_browser_core::{
        CommandRegistry, ParseInput, ParsedCommand, VersioningCommand, parse_chain,
    };
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
    fn invocation_debug_redacts_url_arguments() {
        let sentinel_url = "https://private.example.test/path?token=invocation-sentinel";
        let invocation = decode_command_invocation(&serde_json::json!({
            "command": "open",
            "arguments": {"input": sentinel_url}
        }))
        .expect("valid invocation");

        let rendered = format!("{invocation:?}");
        assert!(!rendered.contains(sentinel_url));
        assert!(!rendered.contains("invocation-sentinel"));
        assert!(rendered.contains("argument_count: 1"));
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
    fn rejected_ipc_command_values_are_not_reflected_in_errors() {
        const SENTINEL: &str = "private-ipc-option-sentinel";
        let cases: &[(&str, &[&str])] = &[
            ("open", &["--private-ipc-option-sentinel"]),
            ("tab-open", &["--private-ipc-option-sentinel"]),
            ("search", &["query", "--private-ipc-option-sentinel"]),
            ("search-next", &["--private-ipc-option-sentinel"]),
            ("scroll-page", &["down", "--private-ipc-option-sentinel"]),
            ("hint", &[SENTINEL]),
            (
                "context-create",
                &[
                    "work",
                    "--profile",
                    "default",
                    "--private-ipc-option-sentinel",
                    "private-value",
                ],
            ),
            (
                "context-create",
                &[
                    "work",
                    "--profile",
                    "default",
                    "--private-ipc-option-sentinel",
                ],
            ),
            (
                "context-route",
                &[
                    "add",
                    "https://example.test",
                    "work",
                    "--entry-point",
                    SENTINEL,
                ],
            ),
            (
                "context-route",
                &[
                    "add",
                    "https://example.test",
                    "work",
                    "--private-ipc-option-sentinel",
                    "private-value",
                ],
            ),
            (
                "context-route",
                &[
                    "add",
                    "https://example.test",
                    "work",
                    "--private-ipc-option-sentinel",
                ],
            ),
            ("window-new", &["--private-ipc-option-sentinel"]),
            ("history-clear", &["--private-ipc-option-sentinel"]),
            ("history-clear", &["private-history-argument-sentinel"]),
            ("get", &["--private-ipc-option-sentinel"]),
            (
                "command-execute",
                &["stable-command", "{\"private-ipc-json-sentinel\":"],
            ),
        ];

        for (name, arguments) in cases {
            let parsed = ParsedCommand {
                name: (*name).to_owned(),
                arguments: arguments
                    .iter()
                    .map(|argument| (*argument).to_owned())
                    .collect(),
            };
            let error = encode_command(&parsed).expect_err("invalid command must be rejected");
            assert!(!error.contains(SENTINEL), "{name}: {error}");
            assert!(
                !error.contains("private-history-argument-sentinel"),
                "{name}: {error}"
            );
            assert!(
                !error.contains("private-ipc-json-sentinel"),
                "{name}: {error}"
            );
        }

        let decode_error = decode_command_invocation(&json!({
            "command": "send",
            "arguments": {"target": "clipboard", "send_subject": SENTINEL}
        }))
        .expect_err("invalid structured send subject must be rejected");
        assert!(!decode_error.contains(SENTINEL), "{decode_error}");

        let unknown_name = ParsedCommand {
            name: SENTINEL.to_owned(),
            arguments: vec!["private-value".into()],
        };
        let unknown_error = encode_command(&unknown_name).expect_err("unknown command is rejected");
        assert!(!unknown_error.contains(SENTINEL), "{unknown_error}");
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
    fn versioning_ui_cli_and_structured_ipc_share_typed_requests() {
        let operation = ferric_browser_core::OperationId::fresh();
        for input in [
            "commit 'Before trial' --scope=window --pages",
            "commit -- --pages",
            "commit-list --limit=25 --offset=100",
            "stash 'Research set' --scope=window",
            "branch trial --clean",
            "diff main trial --kind=cookies --offset=200",
            "restore main --mode=archive --into=current --policies",
            "merge trial --cookies --site=example.test",
            "merge-resolve plan item branch",
            "merge-apply plan",
            &format!("versioning-cancel {operation}"),
        ] {
            let ui = parse_chain(input, ParseInput::Interactive)
                .unwrap()
                .remove(0);
            let cli = parse_chain(input, ParseInput::Cli).unwrap().remove(0);
            let expected = VersioningCommand::from_parsed(&ui, 1).unwrap();
            assert_eq!(
                VersioningCommand::from_parsed(&cli, 1).unwrap(),
                expected,
                "{input}"
            );
            let envelope = encode_command(&cli).unwrap();
            let ipc = decode_command_invocation(&envelope).unwrap();
            assert_eq!(
                VersioningCommand::from_parsed(&ipc.command, 1).unwrap(),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn structured_versioning_ipc_rejects_unknown_and_incompatible_options() {
        for request in [
            json!({"command":"commit","arguments":{"message":"note","unknown":true}}),
            json!({"command":"commit","arguments":{"message":"note","editable":true}}),
            json!({"command":"commit-list","arguments":{"limit":0}}),
            json!({"command":"branch","arguments":{"name":"trial","scope":"context"}}),
            json!({"command":"merge","arguments":{"branch":"trial","cookies":false}}),
            json!({"command":"merge-apply","arguments":{"plan":"plan","cookies":true}}),
        ] {
            assert!(decode_command_invocation(&request).is_err(), "{request}");
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
