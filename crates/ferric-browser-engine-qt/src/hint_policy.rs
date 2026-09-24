//! Command-side hint policy shared by interactive and IPC entry points.
//!
//! The Qt adapter consumes the parsed result but does not decide which hint
//! targets are valid or which rapid actions retain Hint mode.

use crate::input_validation::is_bounded_untrusted_text;
use ferric_browser_core::ParsedCommand;

/// Fully validated options for a hint command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HintOptions {
    pub(super) links_only: bool,
    pub(super) rapid: bool,
    pub(super) target: String,
    pub(super) script: Option<String>,
}

/// Returns the userscript name encoded by an external hint target.
///
/// CLI and IPC both use this small language, so no caller can accidentally
/// accept a broader userscript target than the other.
pub(super) fn external_hint_target(value: &str) -> Option<&str> {
    let name = value.strip_prefix("external:")?;
    (is_bounded_untrusted_text(name)
        && name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        }))
    .then_some(name)
}

pub(super) fn rapid_hint_keeps_mode(rapid: bool, action: Option<&str>) -> bool {
    rapid
        && matches!(
            action,
            Some("yank" | "clean-yank" | "tab-bg" | "userscript" | "download")
        )
}

/// Parses the legacy command arguments into named, validated hint options.
pub(super) fn parse_hint_options(command: &ParsedCommand) -> Result<HintOptions, String> {
    let mut links_only = false;
    let mut rapid = false;
    let mut target = "current".to_owned();
    let mut script = None;
    let mut index = 0;
    while index < command.arguments.len() {
        match command.arguments[index].as_str() {
            "links" if !links_only => links_only = true,
            "all" if !links_only => {}
            "--rapid" if !rapid => rapid = true,
            "--target" => {
                index += 1;
                target = command
                    .arguments
                    .get(index)
                    .filter(|value| is_hint_target(value))
                    .cloned()
                    .ok_or_else(|| {
                        "hint target must be current, tab, tab-bg, window, yank, clean-yank, download, userscript, ephemeral, or external:NAME"
                            .to_owned()
                    })?;
            }
            "--script" => {
                index += 1;
                let value = command
                    .arguments
                    .get(index)
                    .filter(|value| is_bounded_untrusted_text(value))
                    .cloned()
                    .ok_or_else(|| "hint script must be a nonempty safe name".to_owned())?;
                if script.replace(value).is_some() {
                    return Err("hint script was specified more than once".into());
                }
            }
            "links" | "all" => return Err("hint kind was specified more than once".into()),
            _ => {
                return Err(
                    "hint expects [--target TARGET] [--rapid] [--script NAME] [links|all]".into(),
                );
            }
        }
        index += 1;
    }
    if rapid && target == "current" {
        target = "yank".into();
    }
    if rapid && target == "ephemeral" {
        return Err("hint target ephemeral does not support --rapid".into());
    }
    if rapid && external_hint_target(&target).is_some() {
        return Err("external hint targets do not support --rapid".into());
    }
    if rapid
        && !matches!(
            target.as_str(),
            "tab-bg" | "yank" | "clean-yank" | "download" | "userscript"
        )
    {
        return Err(
            "rapid hint target must be tab-bg, yank, clean-yank, download, or userscript".into(),
        );
    }
    if target == "userscript" && script.is_none() {
        return Err("hint target userscript requires --script NAME".into());
    }
    if target != "userscript" && script.is_some() {
        return Err("--script is only valid with the userscript hint target".into());
    }
    Ok(HintOptions {
        links_only,
        rapid,
        target,
        script,
    })
}

fn is_hint_target(value: &str) -> bool {
    matches!(
        value,
        "current"
            | "tab"
            | "tab-bg"
            | "window"
            | "yank"
            | "clean-yank"
            | "download"
            | "userscript"
            | "ephemeral"
    ) || external_hint_target(value).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_options_and_rejects_invalid_combinations() {
        let options = parse_hint_options(&ParsedCommand {
            name: "hint".into(),
            arguments: vec![
                "--target".into(),
                "userscript".into(),
                "--script".into(),
                "reader".into(),
                "links".into(),
            ],
        })
        .expect("valid userscript hint command");
        assert_eq!(
            options,
            HintOptions {
                links_only: true,
                rapid: false,
                target: "userscript".into(),
                script: Some("reader".into()),
            }
        );
        assert!(
            parse_hint_options(&ParsedCommand {
                name: "hint".into(),
                arguments: vec!["--rapid".into(), "--target".into(), "window".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn external_hint_targets_have_a_small_safe_name_language() {
        assert_eq!(
            external_hint_target("external:open-in-reader"),
            Some("open-in-reader")
        );
        assert!(external_hint_target("external:").is_none());
        assert!(external_hint_target("external:Capital").is_none());
        assert!(external_hint_target("external:contains/slash").is_none());
    }
}
