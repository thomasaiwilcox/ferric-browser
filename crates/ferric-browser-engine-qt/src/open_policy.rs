//! Parsing policy for interactive `open` commands.
//!
//! This module distinguishes a command that needs route-aware IPC handling from
//! ordinary command execution without depending on Qt or IPC transport types.

use crate::input_validation::is_bounded_untrusted_text;
use ferric_browser_core::{ParsedCommand, validate_open_target};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct InteractiveOpen {
    pub(super) input: String,
    pub(super) target: Option<String>,
    pub(super) profile: Option<String>,
    pub(super) context: Option<String>,
    pub(super) clean_link: bool,
}

pub(super) fn clean_input(command: &ParsedCommand) -> Result<Option<String>, String> {
    if command.name != "open"
        || command.arguments.first().map(String::as_str) != Some("--clean-link")
    {
        return Ok(None);
    }
    let start = if command.arguments.get(1).map(String::as_str) == Some("--") {
        2
    } else {
        1
    };
    let input = command.arguments[start..].join(" ");
    (!input.is_empty())
        .then_some(input)
        .ok_or_else(|| "open --clean-link requires an input".to_owned())
        .map(Some)
}

pub(super) fn parse_interactive(
    command: &ParsedCommand,
) -> Result<Option<InteractiveOpen>, String> {
    if command.name != "open"
        || !command.arguments.iter().any(|argument| {
            matches!(
                argument.as_str(),
                "--" | "--target" | "--profile" | "--context" | "--clean-link" | "--ephemeral"
            )
        })
    {
        return Ok(None);
    }
    let mut target = None;
    let mut profile = None;
    let mut context = None;
    let mut clean_link = false;
    let mut input = Vec::new();
    let mut options_ended = false;
    let mut index = 0;
    while index < command.arguments.len() {
        let argument = &command.arguments[index];
        if options_ended {
            input.push(argument.clone());
        } else {
            match argument.as_str() {
                "--" => options_ended = true,
                "--target" | "--profile" | "--context" => {
                    index += 1;
                    let value = command
                        .arguments
                        .get(index)
                        .filter(|value| is_bounded_untrusted_text(value))
                        .cloned()
                        .ok_or_else(|| format!("{argument} requires a value"))?;
                    let destination = match argument.as_str() {
                        "--target" => &mut target,
                        "--profile" => &mut profile,
                        "--context" => &mut context,
                        _ => unreachable!("matched option has a destination"),
                    };
                    if destination.replace(value).is_some() {
                        return Err(format!("{argument} may be specified only once"));
                    }
                }
                "--clean-link" => {
                    if clean_link {
                        return Err("--clean-link may be specified only once".into());
                    }
                    clean_link = true;
                }
                "--ephemeral" => {
                    return Err(
                        "open --ephemeral requires a fresh GUI launch; use the CLI entry point"
                            .into(),
                    );
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown open option: {value}"));
                }
                value => {
                    options_ended = true;
                    input.push(value.to_owned());
                }
            }
        }
        index += 1;
    }
    if input.is_empty() {
        return Err("open requires an input".into());
    }
    let target = target
        .map(|target| -> Result<String, String> {
            match target.as_str() {
                "current" => Ok("tab".to_owned()),
                "tab" | "tab-bg" | "window" | "private-window" => Ok(target),
                _ => Err(
                    "open target must be current, tab, tab-bg, window, or private-window".into(),
                ),
            }
        })
        .transpose()?;
    validate_open_target(target.as_deref(), clean_link).map_err(str::to_owned)?;
    Ok(Some(InteractiveOpen {
        input: input.join(" "),
        target,
        profile,
        context,
        clean_link,
    }))
}

#[cfg(test)]
mod tests {
    use super::{InteractiveOpen, clean_input, parse_interactive};
    use ferric_browser_core::ParsedCommand;

    #[test]
    fn interactive_open_normalizes_current_and_keeps_route_context() {
        let parsed = parse_interactive(&ParsedCommand {
            name: "open".into(),
            arguments: vec![
                "--target".into(),
                "current".into(),
                "--profile".into(),
                "work".into(),
                "example.test".into(),
            ],
        })
        .expect("valid interactive open");
        assert_eq!(
            parsed,
            Some(InteractiveOpen {
                input: "example.test".into(),
                target: Some("tab".into()),
                profile: Some("work".into()),
                context: None,
                clean_link: false,
            })
        );
    }

    #[test]
    fn clean_input_accepts_an_option_terminator() {
        assert_eq!(
            clean_input(&ParsedCommand {
                name: "open".into(),
                arguments: vec![
                    "--clean-link".into(),
                    "--".into(),
                    "https://example.test".into()
                ],
            }),
            Ok(Some("https://example.test".into()))
        );
    }
}
