//! Structured IPC projection of the core versioning command grammar.

use ferric_browser_core::{
    DiffKind, ParsedCommand, ResolutionChoice, RestoreInto, RestoreMode, VersioningCommand,
    VersioningScope,
};
use serde_json::{Map, Value, json};

use super::EncodedCommand;

pub(crate) fn is_versioning_command(name: &str) -> bool {
    matches!(
        name,
        "commit"
            | "commit-list"
            | "commit-info"
            | "commit-delete"
            | "stash"
            | "stash-list"
            | "stash-info"
            | "stash-resume"
            | "stash-drop"
            | "branch"
            | "branch-list"
            | "branch-info"
            | "branch-delete"
            | "switch"
            | "diff"
            | "restore"
            | "merge"
            | "merge-resolve"
            | "merge-apply"
            | "versioning-status"
            | "versioning-cancel"
    )
}

fn scope_name(scope: VersioningScope) -> &'static str {
    match scope {
        VersioningScope::Tab => "tab",
        VersioningScope::Window => "window",
        VersioningScope::Context => "context",
    }
}

pub(super) fn encode(command: &ParsedCommand) -> Result<Option<EncodedCommand>, String> {
    if !is_versioning_command(&command.name) {
        return Ok(None);
    }
    let parsed = VersioningCommand::from_parsed(command, 1).map_err(|error| error.to_string())?;
    let arguments = match parsed {
        VersioningCommand::Commit {
            message,
            scope,
            pages,
            editable,
        } => {
            json!({"message": message, "scope": scope_name(scope), "pages": pages, "editable": editable})
        }
        VersioningCommand::CommitList { limit, offset } => {
            json!({"limit": limit, "offset": offset})
        }
        VersioningCommand::CommitInfo { reference }
        | VersioningCommand::CommitDelete { reference }
        | VersioningCommand::BranchInfo { reference }
        | VersioningCommand::BranchDelete { reference }
        | VersioningCommand::Switch { reference } => json!({"ref": reference.as_str()}),
        VersioningCommand::Stash { name, scope } => {
            json!({"name": name.as_ref().map(ferric_browser_core::BranchName::as_str), "scope": scope_name(scope)})
        }
        VersioningCommand::StashList
        | VersioningCommand::BranchList
        | VersioningCommand::Status => json!({}),
        VersioningCommand::StashInfo { reference }
        | VersioningCommand::StashResume { reference }
        | VersioningCommand::StashDrop { reference } => json!({"id": reference.as_str()}),
        VersioningCommand::Branch { name, scope, clean } => {
            json!({"name": name.as_str(), "scope": scope_name(scope), "clean": clean})
        }
        VersioningCommand::Diff {
            left,
            right,
            kind,
            offset,
        } => {
            json!({"left": left.as_str(), "right": right.as_str(),
            "kind": match kind { DiffKind::Browser => "browser", DiffKind::Page => "page", DiffKind::Cookies => "cookies" },
            "offset": offset})
        }
        VersioningCommand::Restore {
            reference,
            mode,
            into,
            policies,
        } => json!({"ref": reference.as_str(),
            "mode": match mode { RestoreMode::Live => "live", RestoreMode::Archive => "archive" },
            "into": match into { RestoreInto::NewWindow => "new-window", RestoreInto::Current => "current" },
            "policies": policies}),
        VersioningCommand::Merge { branch, site } => {
            json!({"branch": branch.as_str(), "cookies": true, "site": site})
        }
        VersioningCommand::MergeResolve { plan, item, choice } => {
            json!({"plan": plan.as_str(), "item": item.as_str(),
            "choice": match choice { ResolutionChoice::Main => "main", ResolutionChoice::Branch => "branch", ResolutionChoice::Skip => "skip" }})
        }
        VersioningCommand::MergeApply { plan } => json!({"plan": plan.as_str()}),
        VersioningCommand::Cancel { operation } => json!({"operation": operation.to_string()}),
    };
    Ok(Some(EncodedCommand::new(arguments)))
}

fn text(
    arguments: &Map<String, Value>,
    key: &str,
    required: bool,
) -> Result<Option<String>, String> {
    match arguments.get(key) {
        None | Some(Value::Null) if !required => Ok(None),
        Some(Value::String(value))
            if !value.is_empty()
                && value.len() <= 1_024
                && !value.chars().any(char::is_control) =>
        {
            Ok(Some(value.clone()))
        }
        _ => Err(format!(
            "versioning argument {key} must be bounded nonempty text"
        )),
    }
}

fn flag(arguments: &Map<String, Value>, key: &str) -> Result<bool, String> {
    match arguments.get(key) {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(format!("versioning argument {key} must be a boolean")),
    }
}

fn push_value(
    tokens: &mut Vec<String>,
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<(), String> {
    if let Some(value) = text(arguments, key, false)? {
        tokens.push(format!("--{key}={value}"));
    }
    Ok(())
}

fn push_flag(
    tokens: &mut Vec<String>,
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<(), String> {
    if flag(arguments, key)? {
        tokens.push(format!("--{key}"));
    }
    Ok(())
}

fn required(
    tokens: &mut Vec<String>,
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<(), String> {
    let value =
        text(arguments, key, true)?.ok_or_else(|| format!("missing versioning argument {key}"))?;
    if value.starts_with("--") && !tokens.iter().any(|token| token == "--") {
        tokens.push("--".into());
    }
    tokens.push(value);
    Ok(())
}

/// Decodes structured fields directly into tokens; no string lexer or shell
/// interpretation is applied. Core validates the same typed grammar again.
pub(crate) fn decode(name: &str, arguments: &Map<String, Value>) -> Result<ParsedCommand, String> {
    if !is_versioning_command(name) {
        return Err("not a versioning command".into());
    }
    let mut tokens = Vec::new();
    match name {
        "commit" => {
            push_value(&mut tokens, arguments, "scope")?;
            push_flag(&mut tokens, arguments, "pages")?;
            push_flag(&mut tokens, arguments, "editable")?;
            required(&mut tokens, arguments, "message")?;
        }
        "commit-list" => {
            if let Some(value) = arguments.get("limit") {
                let limit = value
                    .as_u64()
                    .filter(|limit| (1..=1_000).contains(limit))
                    .ok_or_else(|| "versioning limit must be 1 to 1000".to_owned())?;
                tokens.push(format!("--limit={limit}"));
            }
            if let Some(value) = arguments.get("offset") {
                let offset = value
                    .as_u64()
                    .filter(|offset| *offset <= 100_000)
                    .ok_or_else(|| "versioning offset must be at most 100000".to_owned())?;
                tokens.push(format!("--offset={offset}"));
            }
        }
        "commit-info" | "commit-delete" | "branch-info" | "branch-delete" | "switch" => {
            required(&mut tokens, arguments, "ref")?;
        }
        "stash" => {
            push_value(&mut tokens, arguments, "scope")?;
            if let Some(name) = text(arguments, "name", false)? {
                if name.starts_with("--") {
                    tokens.push("--".into());
                }
                tokens.push(name);
            }
        }
        "stash-info" | "stash-resume" | "stash-drop" => required(&mut tokens, arguments, "id")?,
        "branch" => {
            push_value(&mut tokens, arguments, "scope")?;
            push_flag(&mut tokens, arguments, "clean")?;
            required(&mut tokens, arguments, "name")?;
        }
        "diff" => {
            push_value(&mut tokens, arguments, "kind")?;
            if let Some(value) = arguments.get("offset") {
                let offset = value
                    .as_u64()
                    .filter(|offset| *offset <= 100_000)
                    .ok_or_else(|| "versioning offset must be at most 100000".to_owned())?;
                tokens.push(format!("--offset={offset}"));
            }
            required(&mut tokens, arguments, "left")?;
            required(&mut tokens, arguments, "right")?;
        }
        "restore" => {
            push_value(&mut tokens, arguments, "mode")?;
            push_value(&mut tokens, arguments, "into")?;
            push_flag(&mut tokens, arguments, "policies")?;
            required(&mut tokens, arguments, "ref")?;
        }
        "merge" => {
            if !flag(arguments, "cookies")? {
                return Err("merge requires cookies=true".into());
            }
            tokens.push("--cookies".into());
            push_value(&mut tokens, arguments, "site")?;
            required(&mut tokens, arguments, "branch")?;
        }
        "merge-resolve" => {
            required(&mut tokens, arguments, "plan")?;
            required(&mut tokens, arguments, "item")?;
            required(&mut tokens, arguments, "choice")?;
        }
        "merge-apply" => required(&mut tokens, arguments, "plan")?,
        "versioning-cancel" => required(&mut tokens, arguments, "operation")?,
        "stash-list" | "branch-list" | "versioning-status" => {}
        _ => return Err("not a versioning command".into()),
    }
    let command = ParsedCommand {
        name: name.into(),
        arguments: tokens,
    };
    VersioningCommand::from_parsed(&command, 1).map_err(|error| error.to_string())?;
    Ok(command)
}
