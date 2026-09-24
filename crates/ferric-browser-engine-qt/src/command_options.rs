//! Qt-independent option parsing for command execution.

use ferric_browser_core::{ParsedCommand, canonical_origin};

pub(super) fn history_clear(
    command: &ParsedCommand,
) -> Result<(Option<i64>, Option<String>, bool), String> {
    let mut since = None;
    let mut origin = None;
    let mut confirmed = false;
    let mut index = 0;
    while index < command.arguments.len() {
        match command.arguments[index].as_str() {
            "--confirm" if !confirmed => confirmed = true,
            "--since" if since.is_none() => {
                let value = command
                    .arguments
                    .get(index + 1)
                    .ok_or_else(|| "history-clear --since requires Unix seconds".to_owned())?;
                let timestamp = value
                    .parse::<i64>()
                    .ok()
                    .filter(|timestamp| *timestamp >= 0)
                    .ok_or_else(|| {
                        "history-clear --since requires a nonnegative Unix timestamp".to_owned()
                    })?;
                since = Some(timestamp);
                index += 1;
            }
            "--origin" if origin.is_none() => {
                let value = command.arguments.get(index + 1).ok_or_else(|| {
                    "history-clear --origin requires an exact HTTP(S) origin".to_owned()
                })?;
                origin = Some(normalize_history_clear_origin(value)?);
                index += 1;
            }
            value if value.starts_with("--") => {
                return Err(format!("unknown history-clear option: {value}"));
            }
            value => return Err(format!("unexpected history-clear argument: {value}")),
        }
        index += 1;
    }
    Ok((since, origin, confirmed))
}

pub(super) fn search_next(
    arguments: &[String],
    default_backward: bool,
) -> Result<(bool, u32), String> {
    let mut backward = default_backward;
    let mut backward_seen = false;
    let mut count = 1;
    let mut count_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--backward" if !backward_seen => {
                backward = true;
                backward_seen = true;
                index += 1;
            }
            "--backward" => return Err("search-next accepts --backward at most once".to_owned()),
            "--count" if !count_seen => {
                let value = arguments.get(index + 1).ok_or_else(|| {
                    "search-next --count requires a value from 1 to 100".to_owned()
                })?;
                count = value
                    .parse::<u32>()
                    .ok()
                    .filter(|value| (1..=100).contains(value))
                    .ok_or_else(|| "search-next count must be 1 to 100".to_owned())?;
                count_seen = true;
                index += 2;
            }
            "--count" => return Err("search-next accepts --count at most once".to_owned()),
            _ => {
                return Err(
                    "search-next accepts --backward and optional --count N flags".to_owned(),
                );
            }
        }
    }
    Ok((backward, count))
}

pub(super) fn scroll(command: &str, arguments: &[String]) -> Result<(bool, u32), String> {
    match command {
        "scroll" => match arguments {
            [] => Ok((false, 1)),
            [flag, value] if flag == "--count" => Ok((false, scroll_count(value)?)),
            _ => Err("scroll accepts only --count N once, after DIRECTION".to_owned()),
        },
        "scroll-page" => match arguments {
            [] => Ok((false, 1)),
            [flag] if flag == "--half" => Ok((true, 1)),
            [flag, value] if flag == "--count" => Ok((false, scroll_count(value)?)),
            [half, count, value] if half == "--half" && count == "--count" => {
                Ok((true, scroll_count(value)?))
            }
            _ => Err(
                "scroll-page accepts --half optionally followed by --count N, each once".to_owned(),
            ),
        },
        _ => Err("unsupported scroll command".to_owned()),
    }
}

#[must_use]
pub(super) fn is_tab_undo(name: &str) -> bool {
    name == "tab-undo"
}

#[must_use]
pub(super) fn is_tab_clone(name: &str) -> bool {
    name == "tab-clone"
}

#[must_use]
pub(super) fn is_reopen_in_window(name: &str) -> bool {
    name == "reopen-in-window"
}

#[must_use]
pub(super) fn is_tab_detach(name: &str) -> bool {
    name == "tab-detach"
}

#[must_use]
pub(super) fn is_tab_give(name: &str) -> bool {
    name == "tab-give"
}

#[must_use]
pub(super) fn is_zoom(name: &str) -> bool {
    name == "zoom"
}

#[must_use]
pub(super) fn is_search_next(name: &str) -> bool {
    name == "search-next"
}

#[must_use]
pub(super) fn is_scroll(name: &str) -> bool {
    matches!(name, "scroll" | "scroll-page" | "scroll-to")
}

#[must_use]
pub(super) fn is_link_clean(name: &str) -> bool {
    matches!(name, "url-clean" | "url-explain")
}

#[must_use]
pub(super) fn is_yank(name: &str) -> bool {
    name == "yank"
}

#[must_use]
pub(super) fn selection_yank_command() -> ParsedCommand {
    ParsedCommand {
        name: "yank".into(),
        arguments: vec!["selection".into()],
    }
}

#[must_use]
pub(super) fn command_count(command: &ParsedCommand) -> u32 {
    command
        .arguments
        .windows(2)
        .find_map(|arguments| {
            (arguments[0] == "--count")
                .then(|| arguments[1].parse::<u32>().ok())
                .flatten()
        })
        .filter(|count| (1..=9_999).contains(count))
        .unwrap_or(1)
}

pub(super) fn normalize_history_clear_origin(value: &str) -> Result<String, String> {
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

fn scroll_count(value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|value| (1..=9_999).contains(value))
        .ok_or_else(|| "scroll count must be 1 to 9999".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{history_clear, scroll, search_next};
    use ferric_browser_core::ParsedCommand;

    #[test]
    fn options_keep_counts_and_exact_origins_bounded() {
        assert_eq!(
            search_next(&["--backward".into(), "--count".into(), "2".into()], false),
            Ok((true, 2))
        );
        assert_eq!(
            scroll(
                "scroll-page",
                &["--half".into(), "--count".into(), "3".into()]
            ),
            Ok((true, 3))
        );
        assert_eq!(
            history_clear(&ParsedCommand {
                name: "history-clear".into(),
                arguments: vec![
                    "--origin".into(),
                    "https://example.test".into(),
                    "--confirm".into()
                ]
            }),
            Ok((None, Some("https://example.test".into()), true))
        );
    }
}
