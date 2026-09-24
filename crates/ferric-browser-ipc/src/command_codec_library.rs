//! command codec library command encoding.

// The family encoder shares the parent's schema helpers by design.
#[allow(clippy::wildcard_imports)]
use super::*;

#[allow(clippy::too_many_lines)]
pub(super) fn encode(command: &ParsedCommand) -> Result<Option<EncodedCommand>, String> {
    let arguments = match command.name.as_str() {
        "paste-open" => match command.arguments.as_slice() {
            [] => serde_json::json!({"primary": false}),
            [flag] if flag == "--primary" => serde_json::json!({"primary": true}),
            [flag, target]
                if flag == "--target" && matches!(target.as_str(), "current" | "tab") =>
            {
                serde_json::json!({"target": target, "primary": false})
            }
            [target_flag, target, primary]
                if target_flag == "--target"
                    && matches!(target.as_str(), "current" | "tab")
                    && primary == "--primary" =>
            {
                serde_json::json!({"target": target, "primary": true})
            }
            [primary, target_flag, target]
                if primary == "--primary"
                    && target_flag == "--target"
                    && matches!(target.as_str(), "current" | "tab") =>
            {
                serde_json::json!({"target": target, "primary": true})
            }
            _ => return Err("paste-open accepts [--target current|tab] [--primary]".into()),
        },
        "bookmark-add" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag, title] if flag == "--title" => serde_json::json!({"title": title}),
            _ => return Err("bookmark-add accepts optional --title TEXT".into()),
        },
        "bookmark-delete" => {
            if command.arguments.len() != 1 {
                return Err("bookmark-delete requires exactly one ID".into());
            }
            validate_command_text(&command.arguments[0], "bookmark ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "bookmark-list" | "quickmark-list" | "history" => {
            if !command.arguments.is_empty() {
                return Err(format!("{} does not accept arguments", command.name));
            }
            serde_json::json!({})
        }
        "switcher" => {
            let mut value = serde_json::json!({"scope": "all", "query": ""});
            let mut scope_set = false;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--scope" => {
                        if scope_set {
                            return Err("switcher accepts --scope at most once".into());
                        }
                        let scope = command
                            .arguments
                            .get(index + 1)
                            .ok_or_else(|| "switcher --scope requires a value".to_owned())?;
                        if !matches!(
                            scope.as_str(),
                            "all"
                                | "tabs"
                                | "windows"
                                | "contexts"
                                | "commands"
                                | "history"
                                | "marks"
                                | "sessions"
                                | "downloads"
                                | "closed"
                                | "actions"
                        ) {
                            return Err("switcher --scope must be a valid switcher scope".into());
                        }
                        value["scope"] = serde_json::Value::String(scope.clone());
                        scope_set = true;
                        index += 2;
                    }
                    query if !query.starts_with('-') && value["query"] == "" => {
                        value["query"] = serde_json::Value::String(query.to_owned());
                        index += 1;
                    }
                    _ => return Err("switcher accepts optional --scope SCOPE and QUERY".into()),
                }
            }
            value
        }
        "journey" => {
            let mut value = serde_json::json!({"current": false});
            let mut current_set = false;
            let mut search_set = false;
            let mut expand_set = false;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--current" if !current_set => {
                        value["current"] = serde_json::Value::Bool(true);
                        current_set = true;
                    }
                    "--current" => {
                        return Err("journey accepts --current at most once".into());
                    }
                    "--search" | "--expand" => {
                        let key = if command.arguments[index] == "--search" {
                            "search"
                        } else {
                            "expand"
                        };
                        let already_set = if key == "search" {
                            search_set
                        } else {
                            expand_set
                        };
                        if already_set {
                            return Err(format!("journey accepts {key} at most once"));
                        }
                        let argument = command.arguments.get(index + 1).ok_or_else(|| {
                            format!("journey {} requires a value", command.arguments[index])
                        })?;
                        if key == "search"
                            && (argument.is_empty()
                                || argument.len() > MAX_JOURNEY_QUERY_BYTES
                                || argument.chars().any(char::is_control))
                        {
                            return Err(
                                "journey search must be 1..256 bytes without control characters"
                                    .into(),
                            );
                        }
                        if key == "expand" && uuid::Uuid::parse_str(argument).is_err() {
                            return Err("journey --expand requires a node UUID".into());
                        }
                        value[key] = serde_json::Value::String(argument.clone());
                        if key == "search" {
                            search_set = true;
                        } else {
                            expand_set = true;
                        }
                        index += 1;
                    }
                    _ => {
                        return Err(
                            "journey accepts --current, --search TEXT, and --expand NODE_UUID"
                                .into(),
                        );
                    }
                }
                index += 1;
            }
            value
        }
        "journey-reopen" => {
            let node = command
                .arguments
                .first()
                .ok_or_else(|| "journey-reopen requires a node UUID".to_owned())?;
            if uuid::Uuid::parse_str(node).is_err() {
                return Err("journey-reopen requires a node UUID".into());
            }
            if command.arguments.len() > 3
                || (command.arguments.len() > 1 && command.arguments[1] != "--target")
                || (command.arguments.len() == 3
                    && !matches!(command.arguments[2].as_str(), "current" | "tab" | "window"))
            {
                return Err(
                    "journey-reopen requires NODE_UUID [--target current|tab|window]".into(),
                );
            }
            let mut value = serde_json::json!({"node": node});
            if let Some(target) = command.arguments.get(2) {
                value["target"] = serde_json::Value::String(target.clone());
            }
            value
        }
        "quickmark-add" => {
            if !(1..=2).contains(&command.arguments.len()) {
                return Err("quickmark-add requires NAME and optional URL".into());
            }
            validate_command_text(&command.arguments[0], "quickmark name")?;
            let mut value = serde_json::json!({"name": command.arguments[0]});
            if let Some(url) = command.arguments.get(1) {
                value["url"] = serde_json::Value::String(url.clone());
            }
            value
        }
        "quickmark-delete" => {
            if command.arguments.len() != 1 {
                return Err("quickmark-delete requires exactly one name".into());
            }
            validate_command_text(&command.arguments[0], "quickmark name")?;
            serde_json::json!({"name": command.arguments[0]})
        }
        "history-clear" => {
            let mut since = None;
            let mut origin = None;
            let mut confirmed = false;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--confirm" if !confirmed => confirmed = true,
                    "--since" if since.is_none() => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "history-clear --since requires Unix seconds".to_owned()
                        })?;
                        let timestamp = value
                            .parse::<i64>()
                            .ok()
                            .filter(|value| *value >= 0)
                            .ok_or_else(|| {
                                "history-clear --since requires a nonnegative Unix timestamp"
                                    .to_owned()
                            })?;
                        since = Some(timestamp);
                        index += 1;
                    }
                    "--origin" if origin.is_none() => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "history-clear --origin requires an exact HTTP(S) origin".to_owned()
                        })?;
                        origin = Some(normalize_cli_history_origin(value)?);
                        index += 1;
                    }
                    value if value.starts_with("--") => {
                        return Err(format!("unknown history-clear option: {value}"));
                    }
                    value => return Err(format!("unexpected history-clear argument: {value}")),
                }
                index += 1;
            }
            let mut value = serde_json::json!({"confirmed": confirmed});
            if let Some(since) = since {
                value["since"] = serde_json::Value::Number(since.into());
            }
            if let Some(origin) = origin {
                value["origin"] = serde_json::Value::String(origin);
            }
            value
        }
        "url-clean" | "url-explain" => {
            if command.arguments.len() > 1 {
                return Err(format!("{} accepts at most one URL", command.name));
            }
            command.arguments.first().map_or_else(
                || serde_json::json!({}),
                |url| serde_json::json!({"url": url}),
            )
        }
        "session-save" | "session-load" | "session-delete" | "profile-delete" => {
            if command.arguments.len() != 1 {
                return Err(format!("{} requires exactly one name", command.name));
            }
            validate_command_text(&command.arguments[0], "name")?;
            serde_json::json!({"name": command.arguments[0]})
        }
        "profile-open" => {
            if !(1..=2).contains(&command.arguments.len()) {
                return Err("profile-open requires NAME and optional URL or search input".into());
            }
            validate_command_text(&command.arguments[0], "profile-open name")?;
            let mut value = serde_json::json!({"name": command.arguments[0]});
            if let Some(input) = command.arguments.get(1) {
                value["input"] = serde_json::Value::String(input.clone());
            }
            value
        }
        "profile-create" => {
            if !(1..=2).contains(&command.arguments.len())
                || (command.arguments.len() == 2 && command.arguments[1] != "--ephemeral")
            {
                return Err("profile-create requires NAME with optional --ephemeral".into());
            }
            validate_command_text(&command.arguments[0], "profile-create name")?;
            serde_json::json!({
                "name": command.arguments[0],
                "ephemeral": command.arguments.get(1).is_some()
            })
        }
        "bookmark-edit" => {
            if command.arguments.len() != 3 || command.arguments[1] != "--title" {
                return Err("bookmark-edit requires ID --title TEXT".into());
            }
            validate_command_text(&command.arguments[0], "bookmark ID")?;
            serde_json::json!({"id": command.arguments[0], "title": command.arguments[2]})
        }
        "bookmark-open" | "history-open" => {
            if command.arguments.len() != 1 {
                return Err(format!("{} requires exactly one ID", command.name));
            }
            validate_command_text(&command.arguments[0], "entry ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "quickmark-edit" => {
            if command.arguments.len() != 2 {
                return Err("quickmark-edit requires NAME and URL".into());
            }
            validate_command_text(&command.arguments[0], "quickmark name")?;
            serde_json::json!({"name": command.arguments[0], "url": command.arguments[1]})
        }
        "quickmark-open" => {
            if command.arguments.len() != 1 {
                return Err("quickmark-open requires exactly one name".into());
            }
            validate_command_text(&command.arguments[0], "quickmark name")?;
            serde_json::json!({"name": command.arguments[0]})
        }
        "selection-search" => {
            if command.arguments.is_empty() {
                serde_json::json!({})
            } else if command.arguments.len() == 2 && command.arguments[0] == "--engine" {
                validate_command_text(&command.arguments[1], "selection-search engine")?;
                serde_json::json!({"engine": command.arguments[1]})
            } else {
                return Err("selection-search accepts optional --engine NAME".into());
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(EncodedCommand::new(arguments)))
}
