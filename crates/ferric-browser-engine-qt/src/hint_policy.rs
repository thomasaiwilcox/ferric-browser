//! Command-side hint policy shared by interactive and IPC entry points.
//!
//! The Qt adapter consumes the parsed result but does not decide which hint
//! targets are valid or which rapid actions retain Hint mode.

use crate::input_validation::is_bounded_untrusted_text;
use ferric_browser_core::{HintKind, ParsedCommand};

/// Fully validated options for a hint command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HintOptions {
    pub(super) family: String,
    pub(super) rapid: bool,
    pub(super) target: String,
    pub(super) script: Option<String>,
    pub(super) first: bool,
    pub(super) index: usize,
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

/// Returns whether selecting this candidate should consume its captured URL.
///
/// Images and media expose `currentSrc` for explicit URL actions such as
/// opening a tab, yanking, or downloading. Their ordinary current action must
/// still click the DOM element so an enclosing link or page handler retains
/// its native behavior.
pub(super) fn hint_uses_url_action(
    kind: HintKind,
    rapid: bool,
    target: &str,
    has_pending_action: bool,
) -> bool {
    kind == HintKind::Link
        || matches!(kind, HintKind::Image | HintKind::Media)
            && (rapid || target != "current" || has_pending_action)
}

/// Parses the legacy command arguments into named, validated hint options.
pub(super) fn parse_hint_options(command: &ParsedCommand) -> Result<HintOptions, String> {
    let mut family = "all".to_owned();
    let mut family_seen = false;
    let mut rapid = false;
    let mut target = "current".to_owned();
    let mut script = None;
    let mut first = false;
    let mut requested_index = None;
    let mut index = 0;
    while index < command.arguments.len() {
        match command.arguments[index].as_str() {
            value @ ("links" | "all" | "inputs" | "buttons" | "images" | "media"
            | "scrollables")
                if !family_seen =>
            {
                value.clone_into(&mut family);
                family_seen = true;
            }
            "--rapid" if !rapid => rapid = true,
            "--first" if !first => first = true,
            "--index" if requested_index.is_none() => {
                index += 1;
                requested_index = Some(
                    command
                        .arguments
                        .get(index)
                        .and_then(|value| value.parse::<usize>().ok())
                        .filter(|value| (1..=5_000).contains(value))
                        .ok_or_else(|| "hint index must be 1..=5000".to_owned())?,
                );
            }
            "--target" => {
                index += 1;
                target = command
                    .arguments
                    .get(index)
                    .filter(|value| is_hint_target(value))
                    .cloned()
                    .ok_or_else(|| {
                        "hint target must be current, tab, tab-bg, window, yank, clean-yank, download, userscript, ephemeral, choose, or external:NAME"
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
            "links" | "all" | "inputs" | "buttons" | "images" | "media" | "scrollables" => {
                return Err("hint family was specified more than once".into());
            }
            _ => {
                return Err(
                    "hint expects [--target TARGET] [--rapid] [--script NAME] [--first] [--index N] [links|all|inputs|buttons|images|media|scrollables]".into(),
                );
            }
        }
        index += 1;
    }
    if rapid && target == "current" {
        target = "yank".into();
    }
    if rapid && first {
        return Err("hint --rapid cannot be combined with --first".into());
    }
    if requested_index.is_some() && !first {
        return Err("hint --index requires --first".into());
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
        family,
        rapid,
        target,
        script,
        first,
        index: requested_index.unwrap_or(1),
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
            | "choose"
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
                family: "links".into(),
                rapid: false,
                target: "userscript".into(),
                script: Some("reader".into()),
                first: false,
                index: 1,
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

    #[test]
    fn current_images_and_media_activate_the_element_instead_of_the_source_url() {
        assert!(hint_uses_url_action(
            HintKind::Link,
            false,
            "current",
            false
        ));
        assert!(!hint_uses_url_action(
            HintKind::Image,
            false,
            "current",
            false
        ));
        assert!(!hint_uses_url_action(
            HintKind::Media,
            false,
            "current",
            false
        ));
        assert!(hint_uses_url_action(HintKind::Image, false, "tab", false));
        assert!(hint_uses_url_action(
            HintKind::Media,
            false,
            "current",
            true
        ));
        assert!(hint_uses_url_action(HintKind::Image, true, "tab-bg", false));
        assert!(!hint_uses_url_action(HintKind::Button, false, "tab", false));
    }

    #[test]
    fn parses_specialized_first_selection_and_rejects_conflicts() {
        let options = parse_hint_options(&ParsedCommand {
            name: "hint".into(),
            arguments: vec![
                "--target".into(),
                "choose".into(),
                "--first".into(),
                "--index".into(),
                "3".into(),
                "inputs".into(),
            ],
        })
        .expect("indexed specialized hint");
        assert_eq!(options.family, "inputs");
        assert_eq!(options.target, "choose");
        assert!(options.first);
        assert_eq!(options.index, 3);

        for arguments in [
            vec!["--index".into(), "2".into()],
            vec!["--first".into(), "--rapid".into()],
            vec!["--first".into(), "--index".into(), "5001".into()],
        ] {
            assert!(
                parse_hint_options(&ParsedCommand {
                    name: "hint".into(),
                    arguments,
                })
                .is_err()
            );
        }
    }
}
