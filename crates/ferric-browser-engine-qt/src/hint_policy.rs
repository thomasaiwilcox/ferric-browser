//! Command-side hint policy shared by interactive and IPC entry points.
//!
//! The Qt adapter consumes the parsed result but does not decide which hint
//! targets are valid or which rapid actions retain Hint mode.

use crate::input_validation::is_bounded_untrusted_text;
use ferric_browser_core::{
    CommandSource, HintKind, HintSessionId, HintTarget, ParsedCommand, WindowId,
};
use std::collections::BTreeSet;

const MAX_LOCAL_HINT_ACTIONS_PER_SESSION: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LocalInteractionOwner {
    pub(super) window: WindowId,
    pub(super) session: HintSessionId,
    pub(super) target: HintTarget,
    pub(super) configuration_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LocalInteractionOperation {
    HintPaletteSelection,
    #[allow(dead_code)] // Reserved for the scoped Search palette contract.
    SearchPaletteSelection,
}

/// Local-only authority. It intentionally has no serialization or Clone impl.
pub(super) struct LocalInteractionPermit {
    owner: LocalInteractionOwner,
    operation: LocalInteractionOperation,
    request_id: u64,
}

#[derive(Default)]
pub(super) struct LocalInteractionAuthority {
    next_request_id: u64,
    consumed_request_ids: BTreeSet<u64>,
}

impl LocalInteractionAuthority {
    pub(super) fn mint(
        &mut self,
        source: CommandSource,
        owner: LocalInteractionOwner,
        operation: LocalInteractionOperation,
    ) -> Option<LocalInteractionPermit> {
        if source != CommandSource::Ui {
            return None;
        }
        let request_id = self.next_request_id.checked_add(1)?;
        self.next_request_id = request_id;
        Some(LocalInteractionPermit {
            owner,
            operation,
            request_id,
        })
    }

    pub(super) fn consume(
        &mut self,
        permit: LocalInteractionPermit,
        source: CommandSource,
        owner: LocalInteractionOwner,
        operation: LocalInteractionOperation,
    ) -> bool {
        if self.consumed_request_ids.len() >= MAX_LOCAL_HINT_ACTIONS_PER_SESSION
            || !self.consumed_request_ids.insert(permit.request_id)
        {
            return false;
        }
        source == CommandSource::Ui && permit.owner == owner && permit.operation == operation
    }

    pub(super) fn clear_session_requests(&mut self) {
        self.consumed_request_ids.clear();
    }
}

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

    fn local_owner() -> LocalInteractionOwner {
        LocalInteractionOwner {
            window: WindowId::from_display("windowid-1").expect("window ID"),
            session: HintSessionId::from_display("hintsessionid-2").expect("Hint session ID"),
            target: HintTarget {
                tab: ferric_browser_core::TabId::from_display("tabid-3").expect("tab ID"),
                generation: 4,
                document: ferric_browser_core::DocumentId::from_display("documentid-4")
                    .expect("document ID"),
            },
            configuration_revision: 9,
        }
    }

    fn another_local_owner() -> LocalInteractionOwner {
        LocalInteractionOwner {
            window: WindowId::from_display("windowid-11").expect("window ID"),
            session: HintSessionId::from_display("hintsessionid-12").expect("Hint session ID"),
            target: HintTarget {
                tab: ferric_browser_core::TabId::from_display("tabid-13").expect("tab ID"),
                generation: 4,
                document: ferric_browser_core::DocumentId::from_display("documentid-14")
                    .expect("document ID"),
            },
            configuration_revision: 9,
        }
    }

    #[test]
    fn local_hint_palette_permit_is_ui_only_owner_bound_and_one_shot() {
        let owner = local_owner();
        let operation = LocalInteractionOperation::HintPaletteSelection;
        let mut authority = LocalInteractionAuthority::default();
        for source in [
            CommandSource::Keyboard,
            CommandSource::Cli,
            CommandSource::Ipc,
            CommandSource::Switcher,
            CommandSource::Userscript,
            CommandSource::Macro,
        ] {
            assert!(authority.mint(source, owner, operation).is_none());
        }

        let other_owner = another_local_owner();
        for changed_owner in [
            LocalInteractionOwner {
                window: other_owner.window,
                ..owner
            },
            LocalInteractionOwner {
                session: other_owner.session,
                ..owner
            },
            LocalInteractionOwner {
                target: other_owner.target,
                ..owner
            },
            LocalInteractionOwner {
                configuration_revision: owner.configuration_revision + 1,
                ..owner
            },
        ] {
            let permit = authority
                .mint(CommandSource::Ui, owner, operation)
                .expect("direct local UI can mint a palette permit");
            assert!(!authority.consume(permit, CommandSource::Ui, changed_owner, operation));
        }

        let permit = authority
            .mint(CommandSource::Ui, owner, operation)
            .expect("direct local UI can mint a palette permit");
        let replay = LocalInteractionPermit {
            owner: permit.owner,
            operation: permit.operation,
            request_id: permit.request_id,
        };
        assert!(!authority.consume(permit, CommandSource::Cli, owner, operation));
        assert!(!authority.consume(replay, CommandSource::Ui, owner, operation));

        let permit = authority
            .mint(CommandSource::Ui, owner, operation)
            .expect("direct local UI can mint a fresh palette permit");
        let replay = LocalInteractionPermit {
            owner: permit.owner,
            operation: permit.operation,
            request_id: permit.request_id,
        };
        assert!(authority.consume(permit, CommandSource::Ui, owner, operation));
        assert!(!authority.consume(replay, CommandSource::Ui, owner, operation));

        let permit = authority
            .mint(
                CommandSource::Ui,
                owner,
                LocalInteractionOperation::SearchPaletteSelection,
            )
            .expect("the local authority supports a distinct Search operation");
        assert!(!authority.consume(
            permit,
            CommandSource::Ui,
            owner,
            LocalInteractionOperation::HintPaletteSelection,
        ));
    }

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

        for arguments in [
            vec!["--target".into(), "pointer".into(), "links".into()],
            vec!["--target".into(), "pointer".into(), "--rapid".into()],
            vec!["--target".into(), "pointer".into(), "--first".into()],
            vec![
                "--target".into(),
                "pointer".into(),
                "--first".into(),
                "--index".into(),
                "2".into(),
            ],
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
