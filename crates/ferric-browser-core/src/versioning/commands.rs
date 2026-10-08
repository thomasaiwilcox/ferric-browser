//! Closed, Qt-independent command grammar over tokens from `parse_chain`.
//! This module never lexes input, resolves names, or performs an effect.

use std::{collections::BTreeMap, fmt};

use crate::ParsedCommand;

use super::limits::{DEFAULT_LIST_ITEMS, MAX_LIST_ITEMS};
use super::{BranchName, CapabilityKey, OperationId};

const MAX_MESSAGE_BYTES: usize = 1_024;
const MAX_REFERENCE_BYTES: usize = 128;

/// Static lower-bound requirements for discovery. Option-sensitive gates
/// (for example snapshot versus `--clean`) are checked at admission later.
#[must_use]
pub fn command_capabilities(name: &str) -> &'static [&'static str] {
    match name {
        "commit" | "commit-list" | "commit-info" | "commit-delete" | "diff" | "restore" => {
            &["structural"]
        }
        "stash" | "stash-list" | "stash-info" | "stash-resume" | "stash-drop" => {
            &["live_retention"]
        }
        "branch" | "branch-list" | "branch-info" | "branch-delete" | "switch" => &["clean_branch"],
        "merge" | "merge-resolve" => &["cookie_identity", "cookie_snapshot"],
        "merge-apply" => &["cookie_identity", "cookie_snapshot", "cookie_merge_apply"],
        _ => &[],
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersioningScope {
    Tab,
    Window,
    Context,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffKind {
    Browser,
    Page,
    Cookies,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreMode {
    Live,
    Archive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreInto {
    NewWindow,
    Current,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionChoice {
    Main,
    Branch,
    Skip,
}

/// A syntactically bounded reference. Namespace/ambiguity resolution belongs
/// to the admitted owner, never to the parser or current focus.
#[derive(Clone, Eq, PartialEq)]
pub struct VersioningRef(String);

impl VersioningRef {
    /// # Errors
    /// Rejects empty, overlong or control-bearing reference tokens.
    pub fn new(value: &str) -> Result<Self, VersioningCommandError> {
        if value.trim().is_empty()
            || value.len() > MAX_REFERENCE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(VersioningCommandError::InvalidValue);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for VersioningRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VersioningRef([redacted])")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum VersioningCommand {
    Commit {
        message: String,
        scope: VersioningScope,
        pages: bool,
        editable: bool,
    },
    CommitList {
        limit: usize,
        offset: usize,
    },
    CommitInfo {
        reference: VersioningRef,
    },
    CommitDelete {
        reference: VersioningRef,
    },
    Stash {
        name: Option<BranchName>,
        scope: VersioningScope,
    },
    StashList,
    StashInfo {
        reference: VersioningRef,
    },
    StashResume {
        reference: VersioningRef,
    },
    StashDrop {
        reference: VersioningRef,
    },
    Branch {
        name: BranchName,
        scope: VersioningScope,
        clean: bool,
    },
    BranchList,
    BranchInfo {
        reference: VersioningRef,
    },
    BranchDelete {
        reference: VersioningRef,
    },
    Switch {
        reference: VersioningRef,
    },
    Diff {
        left: VersioningRef,
        right: VersioningRef,
        kind: DiffKind,
        offset: usize,
    },
    Restore {
        reference: VersioningRef,
        mode: RestoreMode,
        into: RestoreInto,
        policies: bool,
    },
    Merge {
        branch: VersioningRef,
        site: Option<String>,
    },
    MergeResolve {
        plan: VersioningRef,
        item: VersioningRef,
        choice: ResolutionChoice,
    },
    MergeApply {
        plan: VersioningRef,
    },
    Status,
    Cancel {
        operation: OperationId,
    },
}

impl fmt::Debug for VersioningCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Commit { .. } => "Commit",
            Self::CommitList { .. } => "CommitList",
            Self::CommitInfo { .. } => "CommitInfo",
            Self::CommitDelete { .. } => "CommitDelete",
            Self::Stash { .. } => "Stash",
            Self::StashList => "StashList",
            Self::StashInfo { .. } => "StashInfo",
            Self::StashResume { .. } => "StashResume",
            Self::StashDrop { .. } => "StashDrop",
            Self::Branch { .. } => "Branch",
            Self::BranchList => "BranchList",
            Self::BranchInfo { .. } => "BranchInfo",
            Self::BranchDelete { .. } => "BranchDelete",
            Self::Switch { .. } => "Switch",
            Self::Diff { .. } => "Diff",
            Self::Restore { .. } => "Restore",
            Self::Merge { .. } => "Merge",
            Self::MergeResolve { .. } => "MergeResolve",
            Self::MergeApply { .. } => "MergeApply",
            Self::Status => "Status",
            Self::Cancel { .. } => "Cancel",
        };
        f.write_str(kind)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersioningCommandError {
    UnknownCommand,
    MissingArgument,
    UnexpectedArgument,
    UnknownFlag,
    DuplicateFlag,
    InvalidValue,
    IncompatibleOptions,
    UnsupportedOption,
    CountNotSupported,
}

impl fmt::Display for VersioningCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownCommand => "unknown versioning command",
            Self::MissingArgument => "missing versioning command argument",
            Self::UnexpectedArgument => "unexpected versioning command argument",
            Self::UnknownFlag => "unknown versioning command option",
            Self::DuplicateFlag => "duplicate versioning command option",
            Self::InvalidValue => "invalid versioning command value",
            Self::IncompatibleOptions => "incompatible versioning command options",
            Self::UnsupportedOption => "versioning option is not available",
            Self::CountNotSupported => "versioning commands do not accept a count",
        })
    }
}

impl std::error::Error for VersioningCommandError {}

struct Options {
    positional: Vec<String>,
    flags: BTreeMap<String, Option<String>>,
}

impl Options {
    fn parse(
        args: &[String],
        values: &[&str],
        switches: &[&str],
    ) -> Result<Self, VersioningCommandError> {
        let mut parsed = Self {
            positional: Vec::new(),
            flags: BTreeMap::new(),
        };
        let mut literal = false;
        for token in args {
            if token == "--" && !literal {
                literal = true;
                continue;
            }
            if !literal && token.starts_with("--") {
                let (name, value) = token[2..]
                    .split_once('=')
                    .map_or((&token[2..], None), |(name, value)| (name, Some(value)));
                if !values.contains(&name) && !switches.contains(&name) {
                    return Err(VersioningCommandError::UnknownFlag);
                }
                if values.contains(&name) != value.is_some() || value.is_some_and(str::is_empty) {
                    return Err(VersioningCommandError::InvalidValue);
                }
                if parsed
                    .flags
                    .insert(name.to_owned(), value.map(ToOwned::to_owned))
                    .is_some()
                {
                    return Err(VersioningCommandError::DuplicateFlag);
                }
            } else {
                parsed.positional.push(token.clone());
            }
        }
        Ok(parsed)
    }

    fn exact(&self, count: usize) -> Result<(), VersioningCommandError> {
        match self.positional.len().cmp(&count) {
            std::cmp::Ordering::Less => Err(VersioningCommandError::MissingArgument),
            std::cmp::Ordering::Greater => Err(VersioningCommandError::UnexpectedArgument),
            std::cmp::Ordering::Equal => Ok(()),
        }
    }

    fn optional_one(&self) -> Result<(), VersioningCommandError> {
        if self.positional.len() > 1 {
            Err(VersioningCommandError::UnexpectedArgument)
        } else {
            Ok(())
        }
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.flags.get(name).and_then(Option::as_deref)
    }
    fn has(&self, name: &str) -> bool {
        self.flags.contains_key(name)
    }
    fn reference(&self, index: usize) -> Result<VersioningRef, VersioningCommandError> {
        VersioningRef::new(&self.positional[index])
    }
}

fn scope(
    value: Option<&str>,
    default: VersioningScope,
    allow_context: bool,
) -> Result<VersioningScope, VersioningCommandError> {
    match value {
        None => Ok(default),
        Some("tab") => Ok(VersioningScope::Tab),
        Some("window") => Ok(VersioningScope::Window),
        Some("context") if allow_context => Ok(VersioningScope::Context),
        Some("context") => Err(VersioningCommandError::UnsupportedOption),
        Some(_) => Err(VersioningCommandError::InvalidValue),
    }
}

fn name(value: &str) -> Result<BranchName, VersioningCommandError> {
    BranchName::new(value).map_err(|_| VersioningCommandError::InvalidValue)
}

impl VersioningCommand {
    /// Option-sensitive primitive requirements. A matching primitive fact is
    /// still not production authorisation; the application also checks the
    /// qualified production gate and captured privacy/owner policy.
    #[must_use]
    pub fn required_capabilities(&self) -> &'static [CapabilityKey] {
        use CapabilityKey::{
            ArchiveCapture, ArchiveOffline, CleanBranch, CookieIdentity, CookieMergeApply,
            CookieSeedVerify, CookieSnapshot, LiveRetention, Structural,
        };
        match self {
            Self::Commit { pages: true, .. } => &[Structural, ArchiveCapture],
            Self::Commit { .. }
            | Self::CommitList { .. }
            | Self::CommitInfo { .. }
            | Self::CommitDelete { .. }
            | Self::Diff { .. }
            | Self::Restore {
                mode: RestoreMode::Live,
                ..
            } => &[Structural],
            Self::Stash { .. }
            | Self::StashList
            | Self::StashInfo { .. }
            | Self::StashResume { .. }
            | Self::StashDrop { .. } => &[LiveRetention],
            Self::Branch { clean: false, .. } => &[
                CleanBranch,
                CookieIdentity,
                CookieSnapshot,
                CookieSeedVerify,
            ],
            Self::Branch { clean: true, .. }
            | Self::BranchList
            | Self::BranchInfo { .. }
            | Self::BranchDelete { .. }
            | Self::Switch { .. } => &[CleanBranch],
            Self::Merge { .. } | Self::MergeResolve { .. } => &[CookieIdentity, CookieSnapshot],
            Self::Restore {
                mode: RestoreMode::Archive,
                ..
            } => &[Structural, ArchiveOffline],
            Self::MergeApply { .. } => &[CookieIdentity, CookieSnapshot, CookieMergeApply],
            Self::Status | Self::Cancel { .. } => &[],
        }
    }

    /// Converts one already-lexed command into a closed typed request. A
    /// non-unit invocation count is never silently applied to a capture.
    ///
    /// # Errors
    /// Rejects unknown/duplicate options, missing or trailing arguments,
    /// incompatible combinations and unavailable context capture.
    #[allow(clippy::too_many_lines)]
    pub fn from_parsed(
        command: &ParsedCommand,
        count: u32,
    ) -> Result<Self, VersioningCommandError> {
        if count != 1 {
            return Err(VersioningCommandError::CountNotSupported);
        }
        let (values, switches): (&[&str], &[&str]) = match command.name.as_str() {
            "commit" => (&["scope"], &["pages", "editable"]),
            "commit-list" => (&["limit", "offset"], &[]),
            "stash" | "branch" => (
                &["scope"],
                if command.name == "branch" {
                    &["clean"]
                } else {
                    &[]
                },
            ),
            "diff" => (&["kind", "offset"], &[]),
            "restore" => (&["mode", "into"], &["policies"]),
            "merge" => (&["site"], &["cookies"]),
            "commit-info" | "commit-delete" | "stash-list" | "stash-info" | "stash-resume"
            | "stash-drop" | "branch-list" | "branch-info" | "branch-delete" | "switch"
            | "merge-resolve" | "merge-apply" | "versioning-status" | "versioning-cancel" => {
                (&[], &[])
            }
            _ => return Err(VersioningCommandError::UnknownCommand),
        };
        let options = Options::parse(&command.arguments, values, switches)?;
        match command.name.as_str() {
            "commit" => {
                options.exact(1)?;
                let message = &options.positional[0];
                if message.trim().is_empty() || message.len() > MAX_MESSAGE_BYTES {
                    return Err(VersioningCommandError::InvalidValue);
                }
                let pages = options.has("pages");
                let editable = options.has("editable");
                if editable && !pages {
                    return Err(VersioningCommandError::IncompatibleOptions);
                }
                Ok(Self::Commit {
                    message: message.clone(),
                    scope: scope(options.value("scope"), VersioningScope::Window, true)?,
                    pages,
                    editable,
                })
            }
            "commit-list" => {
                options.exact(0)?;
                let limit = options
                    .value("limit")
                    .map_or(Ok(DEFAULT_LIST_ITEMS), |value| {
                        value
                            .parse::<usize>()
                            .ok()
                            .filter(|n| (1..=MAX_LIST_ITEMS).contains(n))
                            .ok_or(VersioningCommandError::InvalidValue)
                    })?;
                let offset = options.value("offset").map_or(Ok(0), |value| {
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|n| *n <= 100_000)
                        .ok_or(VersioningCommandError::InvalidValue)
                })?;
                Ok(Self::CommitList { limit, offset })
            }
            "commit-info" => {
                options.exact(1)?;
                Ok(Self::CommitInfo {
                    reference: options.reference(0)?,
                })
            }
            "commit-delete" => {
                options.exact(1)?;
                Ok(Self::CommitDelete {
                    reference: options.reference(0)?,
                })
            }
            "stash" => {
                options.optional_one()?;
                let name = options
                    .positional
                    .first()
                    .map(|value| name(value))
                    .transpose()?;
                Ok(Self::Stash {
                    name,
                    scope: scope(options.value("scope"), VersioningScope::Tab, false)?,
                })
            }
            "stash-list" => {
                options.exact(0)?;
                Ok(Self::StashList)
            }
            "stash-info" => {
                options.exact(1)?;
                Ok(Self::StashInfo {
                    reference: options.reference(0)?,
                })
            }
            "stash-resume" => {
                options.exact(1)?;
                Ok(Self::StashResume {
                    reference: options.reference(0)?,
                })
            }
            "stash-drop" => {
                options.exact(1)?;
                Ok(Self::StashDrop {
                    reference: options.reference(0)?,
                })
            }
            "branch" => {
                options.exact(1)?;
                Ok(Self::Branch {
                    name: name(&options.positional[0])?,
                    scope: scope(options.value("scope"), VersioningScope::Tab, false)?,
                    clean: options.has("clean"),
                })
            }
            "branch-list" => {
                options.exact(0)?;
                Ok(Self::BranchList)
            }
            "branch-info" => {
                options.exact(1)?;
                Ok(Self::BranchInfo {
                    reference: options.reference(0)?,
                })
            }
            "branch-delete" => {
                options.exact(1)?;
                Ok(Self::BranchDelete {
                    reference: options.reference(0)?,
                })
            }
            "switch" => {
                options.exact(1)?;
                Ok(Self::Switch {
                    reference: options.reference(0)?,
                })
            }
            "diff" => {
                options.exact(2)?;
                let kind = match options.value("kind") {
                    None | Some("browser") => DiffKind::Browser,
                    Some("page") => DiffKind::Page,
                    Some("cookies") => DiffKind::Cookies,
                    Some(_) => return Err(VersioningCommandError::InvalidValue),
                };
                let offset = options.value("offset").map_or(Ok(0), |value| {
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|n| *n <= 100_000)
                        .ok_or(VersioningCommandError::InvalidValue)
                })?;
                Ok(Self::Diff {
                    left: options.reference(0)?,
                    right: options.reference(1)?,
                    kind,
                    offset,
                })
            }
            "restore" => {
                options.exact(1)?;
                let mode = match options.value("mode") {
                    None | Some("live") => RestoreMode::Live,
                    Some("archive") => RestoreMode::Archive,
                    Some(_) => return Err(VersioningCommandError::InvalidValue),
                };
                let into = match options.value("into") {
                    None | Some("new-window") => RestoreInto::NewWindow,
                    Some("current") => RestoreInto::Current,
                    Some(_) => return Err(VersioningCommandError::InvalidValue),
                };
                Ok(Self::Restore {
                    reference: options.reference(0)?,
                    mode,
                    into,
                    policies: options.has("policies"),
                })
            }
            "merge" => {
                options.exact(1)?;
                if !options.has("cookies") {
                    return Err(VersioningCommandError::MissingArgument);
                }
                let site = options
                    .value("site")
                    .map(|site| {
                        if site.contains(['/', '\\', '@', '?', '#'])
                            || (site.contains(':')
                                && !(site.starts_with('[') && site.ends_with(']')))
                        {
                            return Err(VersioningCommandError::InvalidValue);
                        }
                        let origin = crate::canonical_origin(&format!("https://{site}"))
                            .ok_or(VersioningCommandError::InvalidValue)?;
                        let canonical = origin
                            .strip_prefix("https://")
                            .ok_or(VersioningCommandError::InvalidValue)?
                            .to_owned();
                        if canonical.len() > 253 {
                            Err(VersioningCommandError::InvalidValue)
                        } else {
                            Ok(canonical)
                        }
                    })
                    .transpose()?;
                Ok(Self::Merge {
                    branch: options.reference(0)?,
                    site,
                })
            }
            "merge-resolve" => {
                options.exact(3)?;
                let choice = match options.positional[2].as_str() {
                    "main" => ResolutionChoice::Main,
                    "branch" => ResolutionChoice::Branch,
                    "skip" => ResolutionChoice::Skip,
                    _ => return Err(VersioningCommandError::InvalidValue),
                };
                Ok(Self::MergeResolve {
                    plan: options.reference(0)?,
                    item: options.reference(1)?,
                    choice,
                })
            }
            "merge-apply" => {
                options.exact(1)?;
                Ok(Self::MergeApply {
                    plan: options.reference(0)?,
                })
            }
            "versioning-status" => {
                options.exact(0)?;
                Ok(Self::Status)
            }
            "versioning-cancel" => {
                options.exact(1)?;
                let operation = options.positional[0]
                    .parse()
                    .map_err(|_| VersioningCommandError::InvalidValue)?;
                Ok(Self::Cancel { operation })
            }
            _ => Err(VersioningCommandError::UnknownCommand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParseInput, parse_chain};

    fn parse(input: &str, source: ParseInput) -> Result<VersioningCommand, VersioningCommandError> {
        let parsed = parse_chain(input, source).unwrap();
        assert_eq!(parsed.len(), 1);
        VersioningCommand::from_parsed(&parsed[0], 1)
    }

    #[test]
    fn lexical_sources_share_one_typed_contract_without_shell_expansion() {
        for source in [ParseInput::Interactive, ParseInput::Cli, ParseInput::Ipc] {
            let command = parse(
                "commit 'a $(touch /tmp/nope) * literal' --scope=window --pages",
                source,
            )
            .unwrap();
            assert!(
                matches!(command, VersioningCommand::Commit { message, scope: VersioningScope::Window, pages: true, editable: false }
                if message == "a $(touch /tmp/nope) * literal")
            );
            let branch = parse("branch 'Trial line' --clean", source).unwrap();
            assert!(
                matches!(branch, VersioningCommand::Branch { name, clean: true, .. } if name.as_str() == "Trial line")
            );
        }
    }

    #[test]
    fn closed_flags_and_counts_reject_ambiguous_requests() {
        for (input, error) in [
            ("commit note --unknown", VersioningCommandError::UnknownFlag),
            (
                "commit note --pages --pages",
                VersioningCommandError::DuplicateFlag,
            ),
            (
                "commit note --editable",
                VersioningCommandError::IncompatibleOptions,
            ),
            (
                "commit note extra",
                VersioningCommandError::UnexpectedArgument,
            ),
            (
                "commit-list --limit=0",
                VersioningCommandError::InvalidValue,
            ),
            (
                "commit-list --limit=1001",
                VersioningCommandError::InvalidValue,
            ),
            (
                "commit-list --offset=100001",
                VersioningCommandError::InvalidValue,
            ),
            (
                "diff main child --offset=100001",
                VersioningCommandError::InvalidValue,
            ),
            ("merge child", VersioningCommandError::MissingArgument),
            (
                "merge child --cookies --site=https://example.test",
                VersioningCommandError::InvalidValue,
            ),
            (
                "diff main child --kind=other",
                VersioningCommandError::InvalidValue,
            ),
        ] {
            assert_eq!(parse(input, ParseInput::Cli), Err(error), "{input}");
        }
        let parsed = parse_chain("commit note", ParseInput::Cli).unwrap();
        assert_eq!(
            VersioningCommand::from_parsed(&parsed[0], 2),
            Err(VersioningCommandError::CountNotSupported)
        );
    }

    #[test]
    fn list_and_diff_offsets_are_bounded_and_default_to_zero() {
        assert!(matches!(
            parse("commit-list --limit=100 --offset=200", ParseInput::Cli),
            Ok(VersioningCommand::CommitList {
                limit: 100,
                offset: 200
            })
        ));
        assert!(matches!(
            parse("diff main child --kind=page --offset=300", ParseInput::Cli),
            Ok(VersioningCommand::Diff {
                kind: DiffKind::Page,
                offset: 300,
                ..
            })
        ));
        assert!(matches!(
            parse("diff main child", ParseInput::Cli),
            Ok(VersioningCommand::Diff { offset: 0, .. })
        ));
        let cookies = parse("diff main child --kind=cookies", ParseInput::Cli).unwrap();
        assert_eq!(
            cookies.required_capabilities(),
            &[CapabilityKey::Structural]
        );
    }

    #[test]
    fn option_sensitive_capabilities_do_not_downgrade_snapshot_branching() {
        let clean = parse("branch trial --clean", ParseInput::Cli).unwrap();
        let snapshot = parse("branch trial", ParseInput::Cli).unwrap();
        assert_eq!(clean.required_capabilities(), &[CapabilityKey::CleanBranch]);
        assert_eq!(
            snapshot.required_capabilities(),
            &[
                CapabilityKey::CleanBranch,
                CapabilityKey::CookieIdentity,
                CapabilityKey::CookieSnapshot,
                CapabilityKey::CookieSeedVerify,
            ]
        );
        let structural = parse("commit note", ParseInput::Cli).unwrap();
        let pages = parse("commit note --pages", ParseInput::Cli).unwrap();
        assert_eq!(
            structural.required_capabilities(),
            &[CapabilityKey::Structural]
        );
        assert_eq!(
            pages.required_capabilities(),
            &[CapabilityKey::Structural, CapabilityKey::ArchiveCapture]
        );
    }

    #[test]
    fn site_filter_is_a_canonical_host_not_an_origin_or_path() {
        let command = parse("merge trial --cookies --site=例え.テスト", ParseInput::Cli).unwrap();
        assert!(
            matches!(command, VersioningCommand::Merge { site: Some(site), .. } if site.starts_with("xn--"))
        );
        for site in [
            "example..test",
            "example.test:443",
            "example.test/path",
            "user@example.test",
        ] {
            let input = format!("merge trial --cookies --site={site}");
            assert_eq!(
                parse(&input, ParseInput::Cli),
                Err(VersioningCommandError::InvalidValue)
            );
        }
    }

    #[test]
    fn all_initial_command_families_parse_with_defaults() {
        let operation = OperationId::fresh();
        for input in [
            "commit note",
            "commit-list",
            "commit-info main",
            "commit-delete main",
            "stash",
            "stash-list",
            "stash-info lease",
            "stash-resume lease",
            "stash-drop lease",
            "branch child --clean",
            "branch-list",
            "branch-info child",
            "branch-delete child",
            "switch main",
            "diff main child",
            "restore main",
            "merge child --cookies",
            "merge-resolve plan item skip",
            "merge-apply plan",
            "versioning-status",
            &format!("versioning-cancel {operation}"),
        ] {
            assert!(parse(input, ParseInput::Cli).is_ok(), "{input}");
        }
        assert_eq!(
            parse("commit -- --pages", ParseInput::Cli),
            Ok(VersioningCommand::Commit {
                message: "--pages".into(),
                scope: VersioningScope::Window,
                pages: false,
                editable: false,
            })
        );
    }

    #[test]
    fn context_scope_is_enabled_only_for_structural_commit() {
        assert_eq!(
            parse("commit note --scope=context", ParseInput::Cli),
            Ok(VersioningCommand::Commit {
                message: "note".into(),
                scope: VersioningScope::Context,
                pages: false,
                editable: false,
            })
        );
        assert_eq!(
            parse("stash --scope=context", ParseInput::Cli),
            Err(VersioningCommandError::UnsupportedOption)
        );
        assert_eq!(
            parse("branch child --scope=context", ParseInput::Cli),
            Err(VersioningCommandError::UnsupportedOption)
        );
    }
}
