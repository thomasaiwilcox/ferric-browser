//! Typed argument mapping for built-in action commands.
//!
//! The action registry identifies an operation; this module converts its small,
//! positional action form into the canonical command arguments without Qt state.

use crate::input_validation::is_bounded_untrusted_text;
use ferric_browser_core::{ActionRegistry, ParsedCommand};

pub(super) fn map_basic(
    command: &str,
    subject: &str,
    verb: &str,
    arguments: &[String],
) -> Result<Option<Vec<String>>, String> {
    let mapped = match command {
        "open" => {
            let (target, input) = match arguments {
                [input] if is_bounded_untrusted_text(input) => (None, input),
                [flag, target, input]
                    if flag == "--target"
                        && matches!(
                            target.as_str(),
                            "current" | "tab" | "tab-bg" | "window" | "private-window"
                        ) =>
                {
                    if !is_bounded_untrusted_text(input) {
                        return Err(
                            "action open input is empty, oversized, or contains control characters"
                                .into(),
                        );
                    }
                    (Some(target), input)
                }
                _ => {
                    return Err(format!(
                        "action {subject} {verb} expects [--target TARGET] URL"
                    ));
                }
            };
            let mut mapped = Vec::with_capacity(target.map_or(1, |_| 3));
            if let Some(target) = target {
                mapped.extend(["--target".into(), target.clone()]);
            }
            mapped.push(input.clone());
            mapped
        }
        "download" => match arguments {
            [url] if is_bounded_untrusted_text(url) => vec![url.clone()],
            _ => return Err("action link download requires one URL".into()),
        },
        "tab-open" => match arguments {
            [input] if is_bounded_untrusted_text(input) => vec![input.clone()],
            [flag, input] if flag == "--background" && is_bounded_untrusted_text(input) => {
                vec![flag.clone(), input.clone()]
            }
            _ => return Err("action tab open expects [--background] INPUT".into()),
        },
        "url-clean" | "url-explain" => match arguments {
            [] => Vec::new(),
            [url] if is_bounded_untrusted_text(url) => vec![url.clone()],
            _ => return Err(format!("action {subject} {verb} accepts at most one URL")),
        },
        "tab-select" => match arguments {
            [selector] if is_bounded_untrusted_text(selector) => vec![selector.clone()],
            _ => return Err("action tab select requires one tab ID or displayed index".into()),
        },
        "tab-give" => match arguments {
            [window_id] if is_bounded_untrusted_text(window_id) => vec![window_id.clone()],
            _ => return Err("action tab give requires one target window ID".into()),
        },
        "tab-focus" | "tab-close" | "tab-suspend" | "tab-discard" | "tab-resume" => match arguments
        {
            [id] if is_bounded_untrusted_text(id) => vec![id.clone()],
            _ => return Err(format!("action tab {verb} requires one stable tab ID")),
        },
        "tab-mute" | "tab-pin" => match arguments {
            [id] if is_bounded_untrusted_text(id) => vec![id.clone()],
            [id, state]
                if is_bounded_untrusted_text(id)
                    && matches!(state.as_str(), "on" | "off" | "toggle") =>
            {
                vec![id.clone(), state.clone()]
            }
            _ => return Err(format!("action tab {verb} expects TAB_ID [on|off|toggle]")),
        },
        "window-new" => match arguments {
            [] => Vec::new(),
            [flag] if flag == "--private" => vec![flag.clone()],
            [flag, profile]
                if flag == "--profile"
                    && is_bounded_untrusted_text(profile)
                    && !profile.starts_with("--") =>
            {
                vec![flag.clone(), profile.clone()]
            }
            [profile_flag, profile, private_flag]
                if profile_flag == "--profile"
                    && is_bounded_untrusted_text(profile)
                    && !profile.starts_with("--")
                    && private_flag == "--private" =>
            {
                vec![profile_flag.clone(), profile.clone(), private_flag.clone()]
            }
            [private_flag, profile_flag, profile]
                if private_flag == "--private"
                    && profile_flag == "--profile"
                    && is_bounded_untrusted_text(profile)
                    && !profile.starts_with("--") =>
            {
                vec![private_flag.clone(), profile_flag.clone(), profile.clone()]
            }
            _ => return Err("action window new expects [--private] [--profile NAME]".into()),
        },
        "tab-move" => match arguments {
            [id, direction]
                if is_bounded_untrusted_text(id)
                    && matches!(direction.as_str(), "left" | "right") =>
            {
                vec![id.clone(), direction.clone()]
            }
            [id, flag, context]
                if is_bounded_untrusted_text(id)
                    && flag == "--context"
                    && is_bounded_untrusted_text(context) =>
            {
                vec![id.clone(), flag.clone(), context.clone()]
            }
            _ => return Err("action tab move requires TAB_ID and left or right".into()),
        },
        "window-close" | "reopen-in-window" | "tab-detach" | "tab-clone"
            if arguments.is_empty() =>
        {
            Vec::new()
        }
        "window-close" => return Err("action window close takes no arguments".into()),
        "reopen-in-window" | "tab-detach" => {
            return Err(format!("action {subject} {verb} takes no arguments"));
        }
        "tab-clone" => return Err("action tab clone takes no arguments".into()),
        "back" | "forward" => match arguments {
            [] => Vec::new(),
            [count]
                if count
                    .parse::<u32>()
                    .is_ok_and(|count| (1..=100).contains(&count)) =>
            {
                vec!["--count".into(), count.clone()]
            }
            _ => return Err(format!("action tab {verb} accepts an optional count")),
        },
        "reload" => match arguments {
            [] => Vec::new(),
            [flag] if flag == "--bypass-cache" => vec![flag.clone()],
            _ => return Err("action tab reload accepts --bypass-cache".into()),
        },
        "stop" if arguments.is_empty() => Vec::new(),
        "stop" => return Err("action tab stop takes no arguments".into()),
        "tab-next" | "tab-prev" => match arguments {
            [] => Vec::new(),
            [count]
                if count
                    .parse::<u32>()
                    .is_ok_and(|count| (1..=9_999).contains(&count)) =>
            {
                vec!["--count".into(), count.clone()]
            }
            _ => return Err(format!("action tab {verb} accepts an optional count")),
        },
        "fullscreen" => match arguments {
            [] => Vec::new(),
            [state] if matches!(state.as_str(), "on" | "off" | "toggle") => vec![state.clone()],
            _ => return Err("action window fullscreen accepts on, off, or toggle".into()),
        },
        "zoom" => match arguments {
            [factor] if is_bounded_untrusted_text(factor) => vec![factor.clone()],
            _ => return Err("action tab zoom requires one factor".into()),
        },
        "search-next" => match arguments {
            [] => Vec::new(),
            [direction] if matches!(direction.as_str(), "forward" | "backward") => {
                if direction == "backward" {
                    vec!["--backward".into()]
                } else {
                    Vec::new()
                }
            }
            [direction, count]
                if matches!(direction.as_str(), "forward" | "backward")
                    && count
                        .parse::<u32>()
                        .is_ok_and(|count| (1..=100).contains(&count)) =>
            {
                let mut args = Vec::new();
                if direction == "backward" {
                    args.push("--backward".into());
                }
                args.extend(["--count".into(), count.clone()]);
                args
            }
            _ => return Err("action tab search-next expects [forward|backward] [count]".into()),
        },
        "scroll" => match arguments {
            [direction] if matches!(direction.as_str(), "up" | "down" | "left" | "right") => {
                vec![direction.clone()]
            }
            [direction, count]
                if matches!(direction.as_str(), "up" | "down" | "left" | "right")
                    && count
                        .parse::<u32>()
                        .is_ok_and(|count| (1..=9_999).contains(&count)) =>
            {
                vec![direction.clone(), "--count".into(), count.clone()]
            }
            _ => return Err("action tab scroll expects DIRECTION [COUNT]".into()),
        },
        "scroll-page" => match arguments {
            [direction] if matches!(direction.as_str(), "up" | "down") => vec![direction.clone()],
            [direction, flag]
                if matches!(direction.as_str(), "up" | "down") && flag == "--half" =>
            {
                vec![direction.clone(), flag.clone()]
            }
            [direction, flag, count]
                if matches!(direction.as_str(), "up" | "down")
                    && flag == "--half"
                    && count
                        .parse::<u32>()
                        .is_ok_and(|count| (1..=9_999).contains(&count)) =>
            {
                vec![
                    direction.clone(),
                    flag.clone(),
                    "--count".into(),
                    count.clone(),
                ]
            }
            [direction, flag, count]
                if matches!(direction.as_str(), "up" | "down")
                    && flag == "--count"
                    && count
                        .parse::<u32>()
                        .is_ok_and(|count| (1..=9_999).contains(&count)) =>
            {
                vec![direction.clone(), flag.clone(), count.clone()]
            }
            [direction, half_flag, count_flag, count]
                if matches!(direction.as_str(), "up" | "down")
                    && half_flag == "--half"
                    && count_flag == "--count"
                    && count
                        .parse::<u32>()
                        .is_ok_and(|count| (1..=9_999).contains(&count)) =>
            {
                vec![
                    direction.clone(),
                    half_flag.clone(),
                    count_flag.clone(),
                    count.clone(),
                ]
            }
            _ => return Err("action tab scroll-page expects DIRECTION [--half|--count N]".into()),
        },
        "scroll-to" => match arguments {
            [edge] if matches!(edge.as_str(), "top" | "bottom") => vec![edge.clone()],
            _ => return Err("action tab scroll-to expects top or bottom".into()),
        },
        "window-focus" => match arguments {
            [id] if is_bounded_untrusted_text(id) => vec![id.clone()],
            _ => return Err("action window focus requires one stable window ID".into()),
        },
        "window-move" => match arguments {
            [id, workspace]
                if is_bounded_untrusted_text(id) && is_bounded_untrusted_text(workspace) =>
            {
                vec![id.clone(), workspace.clone()]
            }
            _ => return Err("action window move requires WINDOW_ID and WORKSPACE".into()),
        },
        "bookmark-add" => match arguments {
            [] => Vec::new(),
            [flag, title] if flag == "--title" && is_bounded_untrusted_text(title) => {
                vec![flag.clone(), title.clone()]
            }
            _ => return Err("action bookmark add accepts optional --title TEXT".into()),
        },
        "quickmark-add" => match arguments {
            [name] if is_bounded_untrusted_text(name) => vec![name.clone()],
            [name, url] if is_bounded_untrusted_text(name) && is_bounded_untrusted_text(url) => {
                vec![name.clone(), url.clone()]
            }
            _ => return Err("action quickmark add expects NAME [URL]".into()),
        },
        "history-open" | "bookmark-open" | "bookmark-delete" => match arguments {
            [id] if is_bounded_untrusted_text(id) => vec![id.clone()],
            _ => {
                return Err(format!(
                    "action {subject} {verb} requires one stable entry ID"
                ));
            }
        },
        "quickmark-open" | "quickmark-delete" => match arguments {
            [name] if is_bounded_untrusted_text(name) => vec![name.clone()],
            _ => {
                return Err(format!(
                    "action {subject} {verb} requires one quickmark name"
                ));
            }
        },
        "bookmark-edit" => match arguments {
            [id, flag, title]
                if is_bounded_untrusted_text(id)
                    && flag == "--title"
                    && is_bounded_untrusted_text(title) =>
            {
                vec![id.clone(), flag.clone(), title.clone()]
            }
            _ => return Err("action bookmark edit expects ID --title TEXT".into()),
        },
        "quickmark-edit" => match arguments {
            [name, url] if is_bounded_untrusted_text(name) && is_bounded_untrusted_text(url) => {
                vec![name.clone(), url.clone()]
            }
            _ => return Err("action quickmark edit expects NAME URL".into()),
        },
        "bookmark-list" | "quickmark-list" | "session-list" => {
            if arguments.is_empty() {
                Vec::new()
            } else {
                return Err(format!("action {subject} {verb} takes no arguments"));
            }
        }
        "session-save" | "session-delete" => match arguments {
            [name] if is_bounded_untrusted_text(name) => vec![name.clone()],
            _ => return Err(format!("action {subject} {verb} requires one session name")),
        },
        "session-load" => match arguments {
            [name] if is_bounded_untrusted_text(name) => vec![name.clone()],
            [flag, name] if flag == "--append" && is_bounded_untrusted_text(name) => {
                vec![flag.clone(), name.clone()]
            }
            _ => return Err("action session load expects [--append] NAME".into()),
        },
        "command-help" | "command-execute" => match arguments {
            [id] if is_bounded_untrusted_text(id) => vec![id.clone()],
            _ => return Err(format!("action command {verb} requires one command ID")),
        },
        "selection-search" => match arguments {
            [] => Vec::new(),
            [flag, engine] if flag == "--engine" && is_bounded_untrusted_text(engine) => {
                vec![engine.clone()]
            }
            _ => return Err("action selection search accepts optional --engine NAME".into()),
        },
        "context-enter" => match arguments {
            [name] if is_bounded_untrusted_text(name) => vec![name.clone()],
            _ => return Err("action context enter requires one context name".into()),
        },
        "context-save" => match arguments {
            [] => Vec::new(),
            [name] if is_bounded_untrusted_text(name) => vec![name.clone()],
            _ => return Err("action context save accepts at most one context name".into()),
        },
        "tab-undo" if arguments.is_empty() => Vec::new(),
        "tab-undo" => return Err("action tab undo takes no arguments".into()),
        "download-open" | "download-show" | "download-cancel" | "download-pause"
        | "download-resume" | "download-retry" => match arguments {
            [id] if is_bounded_untrusted_text(id) => vec![id.clone()],
            _ => return Err(format!("action download {verb} requires one download ID")),
        },
        "history-clear" => {
            let mut mapped = Vec::new();
            let mut index = 0;
            while index < arguments.len() {
                match arguments[index].as_str() {
                    "--confirm" => mapped.push("--confirm".into()),
                    "--since" | "--origin" => {
                        let value = arguments.get(index + 1).ok_or_else(|| {
                            format!(
                                "action history-entry clear requires a value after {}",
                                arguments[index]
                            )
                        })?;
                        if !is_bounded_untrusted_text(value) {
                            return Err(format!(
                                "action history-entry clear requires bounded data after {}",
                                arguments[index]
                            ));
                        }
                        mapped.extend([arguments[index].clone(), value.clone()]);
                        index += 1;
                    }
                    value => {
                        return Err(format!(
                            "action history-entry clear received unsupported argument {value}"
                        ));
                    }
                }
                index += 1;
            }
            mapped
        }
        "yank" => match subject {
            "url" if arguments.is_empty() && verb == "copy" => vec!["url".into()],
            "url" if arguments.is_empty() => vec!["url".into(), "--clean".into()],
            "url" => return Err(format!("action {subject} {verb} takes no arguments")),
            "link" => match arguments {
                [url] if is_bounded_untrusted_text(url) && verb == "copy" => {
                    vec!["url".into(), url.clone()]
                }
                [url] if is_bounded_untrusted_text(url) => {
                    vec!["url".into(), url.clone(), "--clean".into()]
                }
                _ => return Err(format!("action {subject} {verb} requires one URL")),
            },
            "selection" if verb == "copy" && arguments.is_empty() => vec!["selection".into()],
            "selection" => return Err("action selection copy takes no arguments".into()),
            "history-entry" => return Err("history entry does not support URL copying".into()),
            other => return Err(format!("{other} does not support URL copying")),
        },
        "send" => match (subject, arguments) {
            ("link", [flag, target]) if flag == "--to" && is_bounded_untrusted_text(target) => {
                vec![target.clone()]
            }
            ("link", [flag, target, url])
                if flag == "--to"
                    && is_bounded_untrusted_text(target)
                    && is_bounded_untrusted_text(url) =>
            {
                vec![target.clone(), url.clone()]
            }
            ("url", [flag, target]) if flag == "--to" && is_bounded_untrusted_text(target) => {
                vec![target.clone(), "--url".into()]
            }
            ("url", [flag, target, url])
                if flag == "--to"
                    && is_bounded_untrusted_text(target)
                    && is_bounded_untrusted_text(url) =>
            {
                vec![target.clone(), "--url".into(), url.clone()]
            }
            ("tab", [flag, target]) if flag == "--to" && is_bounded_untrusted_text(target) => {
                vec![target.clone(), "--tab".into()]
            }
            ("selection", [flag, target])
                if flag == "--to" && is_bounded_untrusted_text(target) =>
            {
                vec![target.clone(), "--selection".into()]
            }
            _ => return Err(format!("action {subject} send requires --to TARGET")),
        },
        _ => return Ok(None),
    };
    Ok(Some(mapped))
}

pub(super) fn parse_action_invocation(
    command: &ParsedCommand,
) -> Result<(ParsedCommand, String), String> {
    let [subject, verb, arguments @ ..] = command.arguments.as_slice() else {
        return Err("action requires SUBJECT VERB and typed arguments".into());
    };
    let registry = ActionRegistry::default_v1();
    let definition = registry
        .resolve_subject_verb(subject, verb)
        .ok_or_else(|| format!("unknown action subject/verb: {subject} {verb}"))?;
    let command_arguments = map_basic(definition.command.as_str(), subject, verb, arguments)?
        .ok_or_else(|| "action executor is not implemented".to_owned())?;
    Ok((
        ParsedCommand {
            name: definition.command.clone(),
            arguments: command_arguments,
        },
        definition.id.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::map_basic;

    #[test]
    fn maps_basic_actions_without_opening_the_qt_boundary() {
        assert_eq!(
            map_basic(
                "open",
                "url",
                "open",
                &["--target".into(), "tab-bg".into(), "example.test".into()]
            ),
            Ok(Some(vec![
                "--target".into(),
                "tab-bg".into(),
                "example.test".into()
            ]))
        );
        assert_eq!(
            map_basic("tab-mute", "tab", "mute", &["tab-1".into(), "off".into()]),
            Ok(Some(vec!["tab-1".into(), "off".into()]))
        );
        assert_eq!(
            map_basic("bookmark-add", "bookmark", "add", &[]),
            Ok(Some(Vec::new()))
        );
    }

    #[test]
    fn preserves_canonical_forms_and_subject_specific_failures() {
        assert_eq!(
            map_basic(
                "scroll-page",
                "tab",
                "scroll-page",
                &["down".into(), "--half".into(), "3".into()]
            ),
            Ok(Some(vec![
                "down".into(),
                "--half".into(),
                "--count".into(),
                "3".into()
            ]))
        );
        assert_eq!(
            map_basic("yank", "history-entry", "copy", &[]),
            Err("history entry does not support URL copying".into())
        );
        assert_eq!(
            map_basic(
                "send",
                "url",
                "send",
                &["--to".into(), "mpv".into(), "https://example.test".into()]
            ),
            Ok(Some(vec![
                "mpv".into(),
                "--url".into(),
                "https://example.test".into()
            ]))
        );
    }
}
