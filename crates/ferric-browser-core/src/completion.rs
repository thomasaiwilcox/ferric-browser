use crate::{CommandRegistry, CompletionProvider};

pub const DEFAULT_COMPLETION_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionCategory {
    Command,
    Value,
    Url,
    Tab,
    Setting,
    Bookmark,
    History,
}

impl CompletionCategory {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Value => "value",
            Self::Url => "url",
            Self::Tab => "tab",
            Self::Setting => "setting",
            Self::Bookmark => "bookmark",
            Self::History => "history",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionCandidate {
    pub insert_text: String,
    pub label: String,
    pub detail: String,
    pub category: CompletionCategory,
    pub recency: u64,
    pub frequency: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionResult {
    pub replacement_start: usize,
    pub replacement_end: usize,
    pub candidates: Vec<CompletionCandidate>,
}

/// Computes deterministic candidates for a command-line edit.
///
/// The returned replacement range is expressed in Unicode scalar positions,
/// not bytes. No filesystem, network, or profile lookup occurs here; callers
/// provide bounded catalog data explicitly. Ranking is exact match, prefix
/// match, token match, then recency/frequency and lexical tie-break.
#[must_use]
pub fn complete(
    input: &str,
    cursor: usize,
    registry: &CommandRegistry,
    catalog: &[CompletionCandidate],
    limit: usize,
) -> CompletionResult {
    let cursor = cursor.min(input.chars().count());
    let before_cursor = input.chars().take(cursor).collect::<String>();
    let (start, prefix) = current_token(&before_cursor);
    let command_prefix = input
        .chars()
        .take(start)
        .collect::<String>()
        .trim_start_matches(':')
        .trim()
        .to_owned();
    let command_context = command_prefix.is_empty();

    let candidate_limit = limit.min(DEFAULT_COMPLETION_LIMIT);
    let mut candidates = if command_context {
        command_candidates(registry, &prefix)
    } else {
        let command_name = command_prefix.split_whitespace().next().unwrap_or_default();
        let provider = registry
            .resolve(command_name)
            .map_or(CompletionProvider::None, |definition| definition.completion);
        enum_candidates(&command_prefix, &prefix)
            .unwrap_or_else(|| argument_candidates(provider, &prefix, catalog, candidate_limit))
    };
    if command_prefix.split_ascii_whitespace().next() == Some("scroll-target") {
        let order = ["select", "auto", "document", "status"];
        candidates.sort_by_key(|candidate| {
            order
                .iter()
                .position(|value| *value == candidate.insert_text)
                .unwrap_or(order.len())
        });
    } else {
        candidates.sort_by(|left, right| compare_candidates(left, right, &prefix));
    }
    candidates.truncate(candidate_limit);

    CompletionResult {
        replacement_start: start,
        replacement_end: cursor,
        candidates,
    }
}

fn enum_candidates(command_prefix: &str, prefix: &str) -> Option<Vec<CompletionCandidate>> {
    let tokens = command_prefix.split_ascii_whitespace().collect::<Vec<_>>();
    let command = tokens.first().copied().unwrap_or_default();
    let values: &[&str] =
        if command == "open" && tokens.last().is_some_and(|token| *token == "--target") {
            &["current", "tab", "tab-bg", "window", "private-window"]
        } else {
            match command {
                "fullscreen" | "caret-select" | "tab-mute" | "tab-pin" | "blocking-toggle" => {
                    &["on", "off", "toggle"]
                }
                "mode-enter" => &["normal", "insert", "caret", "passthrough"],
                "hint" => match tokens.last().copied() {
                    Some("--target") => &[
                        "current",
                        "tab",
                        "tab-bg",
                        "window",
                        "yank",
                        "clean-yank",
                        "download",
                        "userscript",
                        "ephemeral",
                        "choose",
                    ],
                    _ => &[
                        "links",
                        "all",
                        "inputs",
                        "buttons",
                        "images",
                        "media",
                        "scrollables",
                    ],
                },
                "scroll" => &["up", "down", "left", "right"],
                "scroll-page" => &["up", "down"],
                "scroll-to" => &["top", "bottom"],
                "scroll-target" => &["select", "auto", "document", "status"],
                "caret-move" => &[
                    "left",
                    "right",
                    "up",
                    "down",
                    "word-next",
                    "word-prev",
                    "line-start",
                    "line-end",
                ],
                "search-next" => &["forward", "backward"],
                "zoom" => &["in", "out", "reset"],
                "context-route" => &["add", "remove", "list"],
                "switcher" if tokens.last().copied() == Some("--scope") => &[
                    "all",
                    "tabs",
                    "windows",
                    "contexts",
                    "commands",
                    "history",
                    "marks",
                    "sessions",
                    "downloads",
                    "closed",
                    "actions",
                ],
                "yank" => &["url", "title", "selection"],
                _ => return None,
            }
        };
    Some(
        values
            .iter()
            .filter(|value| value.starts_with(prefix))
            .map(|value| CompletionCandidate {
                insert_text: (*value).into(),
                label: (*value).into(),
                detail: "command value".into(),
                category: CompletionCategory::Value,
                recency: 0,
                frequency: 0,
            })
            .collect(),
    )
}

fn current_token(input: &str) -> (usize, String) {
    let mut start = input.chars().count();
    for (position, character) in input.char_indices().rev() {
        if character.is_whitespace() {
            break;
        }
        start = input[..position].chars().count();
    }
    let prefix = input.chars().skip(start).collect();
    (start, prefix)
}

fn command_candidates(registry: &CommandRegistry, prefix: &str) -> Vec<CompletionCandidate> {
    let mut candidates = registry
        .definitions()
        .iter()
        .filter(|definition| {
            definition.name.starts_with(prefix)
                || definition
                    .aliases
                    .iter()
                    .any(|alias| alias.starts_with(prefix))
        })
        .map(|definition| CompletionCandidate {
            insert_text: definition.name.clone(),
            label: definition.name.clone(),
            detail: definition.description.clone(),
            category: CompletionCategory::Command,
            recency: 0,
            frequency: 0,
        })
        .collect::<Vec<_>>();
    candidates.extend(
        registry
            .alias_definitions()
            .iter()
            .filter(|alias| alias.name.starts_with(prefix))
            .map(|alias| CompletionCandidate {
                insert_text: alias.name.clone(),
                label: alias.name.clone(),
                detail: alias.expansion.first().map_or_else(
                    || "Command alias".into(),
                    |command| format!("Alias for {}", command.name),
                ),
                category: CompletionCategory::Command,
                recency: 0,
                frequency: 0,
            }),
    );
    candidates
}

fn argument_candidates(
    provider: CompletionProvider,
    prefix: &str,
    catalog: &[CompletionCandidate],
    limit: usize,
) -> Vec<CompletionCandidate> {
    let mut candidates = Vec::with_capacity(catalog.len());
    for candidate in catalog {
        let category_matches = match provider {
            CompletionProvider::None | CompletionProvider::Profiles => false,
            CompletionProvider::Commands => candidate.category == CompletionCategory::Command,
            CompletionProvider::Urls => matches!(
                candidate.category,
                CompletionCategory::Url
                    | CompletionCategory::Tab
                    | CompletionCategory::Bookmark
                    | CompletionCategory::History
            ),
            CompletionProvider::Tabs => candidate.category == CompletionCategory::Tab,
            CompletionProvider::Settings => candidate.category == CompletionCategory::Setting,
        };
        if category_matches
            && (candidate.insert_text.starts_with(prefix)
                || ascii_case_insensitive_contains(&candidate.label, prefix))
        {
            candidates.push((rank(candidate, prefix), candidate));
        }
    }
    let keep = limit.min(candidates.len());
    if candidates.len() > keep {
        candidates.select_nth_unstable_by(keep, compare_scored_candidates);
        candidates.truncate(keep);
    }
    let mut candidates = candidates
        .into_iter()
        .map(|(_, candidate)| candidate.clone())
        .collect::<Vec<_>>();
    if matches!(provider, CompletionProvider::Urls) && prefix.is_empty() {
        candidates.extend(
            ["about:blank", "https://", "http://", "file://"].map(|value| CompletionCandidate {
                insert_text: value.into(),
                label: value.into(),
                detail: "URL scheme".into(),
                category: CompletionCategory::Url,
                recency: 0,
                frequency: 0,
            }),
        );
    }
    candidates.sort_by(|left, right| compare_candidates(left, right, prefix));
    candidates.truncate(limit);
    candidates
}

fn compare_scored_candidates(
    left: &(u8, &CompletionCandidate),
    right: &(u8, &CompletionCandidate),
) -> std::cmp::Ordering {
    left.0
        .cmp(&right.0)
        .then_with(|| right.1.recency.cmp(&left.1.recency))
        .then_with(|| right.1.frequency.cmp(&left.1.frequency))
        .then_with(|| left.1.label.cmp(&right.1.label))
        .then_with(|| left.1.insert_text.cmp(&right.1.insert_text))
}

/// Match the same ASCII-folded label semantics as `to_ascii_lowercase`,
/// without allocating a lowercased copy for every catalog row.
fn ascii_case_insensitive_contains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    haystack.windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    })
}

fn compare_candidates(
    left: &CompletionCandidate,
    right: &CompletionCandidate,
    prefix: &str,
) -> std::cmp::Ordering {
    rank(left, prefix)
        .cmp(&rank(right, prefix))
        .then_with(|| right.recency.cmp(&left.recency))
        .then_with(|| right.frequency.cmp(&left.frequency))
        .then_with(|| left.label.cmp(&right.label))
        .then_with(|| left.insert_text.cmp(&right.insert_text))
}

fn rank(candidate: &CompletionCandidate, prefix: &str) -> u8 {
    if candidate.insert_text == prefix || candidate.label == prefix {
        0
    } else if candidate.insert_text.starts_with(prefix) || candidate.label.starts_with(prefix) {
        1
    } else if candidate
        .label
        .split_ascii_whitespace()
        .any(|token| token.starts_with(prefix))
    {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParseInput, parse_chain};

    #[test]
    fn ranks_command_prefixes_deterministically() {
        let registry = CommandRegistry::default_v1();
        let result = complete("re", 2, &registry, &[], 100);
        assert_eq!(result.replacement_start, 0);
        assert_eq!(result.replacement_end, 2);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.insert_text.as_str())
                .collect::<Vec<_>>(),
            vec!["reload", "reopen-in-window", "repeat"]
        );
    }

    #[test]
    fn uses_the_command_provider_for_arguments() {
        let registry = CommandRegistry::default_v1();
        let catalog = vec![CompletionCandidate {
            insert_text: "https://example.test".into(),
            label: "Example".into(),
            detail: "tab".into(),
            category: CompletionCategory::Tab,
            recency: 10,
            frequency: 2,
        }];
        let result = complete("open https://e", 14, &registry, &catalog, 100);
        assert_eq!(result.replacement_start, 5);
        assert_eq!(result.replacement_end, 14);
        assert_eq!(result.candidates[0].label, "Example");
    }

    #[test]
    fn bounded_catalog_selection_preserves_rank_order() {
        let registry = CommandRegistry::default_v1();
        let catalog = vec![
            CompletionCandidate {
                insert_text: "https://example.test/old".into(),
                label: "Example old".into(),
                detail: "history".into(),
                category: CompletionCategory::History,
                recency: 1,
                frequency: 1,
            },
            CompletionCandidate {
                insert_text: "https://example.test/new".into(),
                label: "Example new".into(),
                detail: "history".into(),
                category: CompletionCategory::History,
                recency: 10,
                frequency: 1,
            },
            CompletionCandidate {
                insert_text: "https://other.test".into(),
                label: "Other".into(),
                detail: "history".into(),
                category: CompletionCategory::History,
                recency: 100,
                frequency: 100,
            },
        ];
        let result = complete("open example", 12, &registry, &catalog, 1);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Example new"]
        );
    }

    #[test]
    fn url_provider_has_bounded_scheme_starters() {
        let registry = CommandRegistry::default_v1();
        let result = complete("open ", 5, &registry, &[], 100);
        assert_eq!(result.candidates.len(), 4);
        assert!(
            result
                .candidates
                .iter()
                .all(|candidate| candidate.category == CompletionCategory::Url)
        );
    }

    #[test]
    fn empty_url_query_includes_local_places_before_scheme_starters() {
        let registry = CommandRegistry::default_v1();
        let catalog = vec![CompletionCandidate {
            insert_text: "https://example.test/visited".into(),
            label: "Visited page".into(),
            detail: "https://example.test/visited".into(),
            category: CompletionCategory::History,
            recency: 42,
            frequency: 3,
        }];
        let result = complete("open ", 5, &registry, &catalog, 100);
        assert_eq!(result.candidates.len(), 5);
        assert_eq!(result.candidates[0].label, "Visited page");
        assert!(
            result
                .candidates
                .iter()
                .any(|item| item.insert_text == "https://")
        );
    }

    #[test]
    fn supplies_bounded_enum_values_without_parsing_execution() {
        let registry = CommandRegistry::default_v1();
        let result = complete("fullscreen t", 12, &registry, &[], 100);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.insert_text.as_str())
                .collect::<Vec<_>>(),
            vec!["toggle"]
        );
        assert_eq!(result.candidates[0].category, CompletionCategory::Value);

        let result = complete("open --target t", 15, &registry, &[], 100);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.insert_text.as_str())
                .collect::<Vec<_>>(),
            vec!["tab", "tab-bg"]
        );

        let result = complete("switcher --scope c", 18, &registry, &[], 100);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.insert_text.as_str())
                .collect::<Vec<_>>(),
            vec!["closed", "commands", "contexts"]
        );

        let result = complete("scroll-target ", 14, &registry, &[], 100);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.insert_text.as_str())
                .collect::<Vec<_>>(),
            vec!["select", "auto", "document", "status"]
        );
        let result = complete("scroll-target d", 15, &registry, &[], 100);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.insert_text.as_str())
                .collect::<Vec<_>>(),
            vec!["document"]
        );
    }

    #[test]
    fn command_parser_remains_the_execution_boundary() {
        let registry = CommandRegistry::default_v1();
        let result = complete(":open ex", 8, &registry, &[], 100);
        assert_eq!(result.replacement_start, 6);
        assert_eq!(
            parse_chain(":open ex", ParseInput::Interactive).unwrap()[0].name,
            "open"
        );
    }
}
