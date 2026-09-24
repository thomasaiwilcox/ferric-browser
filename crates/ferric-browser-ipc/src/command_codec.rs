//! Canonical encoding of parsed browser commands into IPC envelopes.

use ferric_browser_core::{ParsedCommand, canonical_origin};
use serde_json::Value;

const MAX_JOURNEY_QUERY_BYTES: usize = 256;

fn validate_command_text(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().any(char::is_control) {
        Err(format!(
            "{label} must be nonempty and contain no control characters"
        ))
    } else {
        Ok(())
    }
}

/// Encodes a parsed command into the stable typed IPC envelope.
///
/// # Errors
///
/// Returns an error when the command arguments do not satisfy the public
/// command schema.
#[allow(clippy::match_same_arms, clippy::too_many_lines)]
pub fn encode_command(command: &ParsedCommand) -> Result<Value, String> {
    let arguments = match command.name.as_str() {
        "open" => {
            if command.arguments.is_empty() {
                return Err("open requires an input".into());
            }
            if command.arguments.first().map(String::as_str) == Some("--clean-link") {
                if command.arguments.len() < 2 {
                    return Err("open --clean-link requires an input".into());
                }
                serde_json::json!({
                    "input": command.arguments[1..].join(" "),
                    "clean_link": true,
                    "external": true
                })
            } else {
                serde_json::json!({"input": command.arguments.join(" "), "external": true})
            }
        }
        "tab-open" => {
            let mut background = false;
            let mut input = Vec::new();
            let mut options_ended = false;
            for argument in &command.arguments {
                if options_ended {
                    input.push(argument.clone());
                } else {
                    match argument.as_str() {
                        "--background" if !background => background = true,
                        "--background" => {
                            return Err("tab-open accepts --background at most once".into());
                        }
                        "--" => {
                            options_ended = true;
                        }
                        value if value.starts_with("--") => {
                            return Err(format!("unknown tab-open option: {value}"));
                        }
                        _ => input.push(argument.clone()),
                    }
                }
            }
            if input.is_empty() {
                return Err("tab-open requires an input".into());
            }
            serde_json::json!({
                "input": input.join(" "),
                "background": background
            })
        }
        "open-current" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag, target] if flag == "--target" && target == "tab" => {
                serde_json::json!({"target": "tab"})
            }
            _ => return Err("open-current accepts only --target tab".into()),
        },
        "back" | "forward" | "tab-next" | "tab-prev" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag, value] if flag == "--count" => {
                let count = value
                    .parse::<u32>()
                    .ok()
                    .filter(|value| (1..=100).contains(value))
                    .ok_or_else(|| {
                        format!("{} --count must be a value from 1 to 100", command.name)
                    })?;
                serde_json::json!({"count": count})
            }
            _ => return Err(format!("{} accepts optional --count 1..100", command.name)),
        },
        "set" => {
            let mut temporary = false;
            let mut pattern = None;
            let mut setting = None;
            let mut arguments = command.arguments.iter();
            while let Some(argument) = arguments.next() {
                match argument.as_str() {
                    "--temp" if !temporary => temporary = true,
                    "--pattern" if pattern.is_none() => {
                        pattern = Some(
                            arguments
                                .next()
                                .ok_or_else(|| "set --pattern requires a value".to_owned())?,
                        );
                    }
                    value if setting.is_none() => setting = Some(value),
                    _ => return Err("set accepts [--temp] [--pattern PATTERN] KEY=VALUE".into()),
                }
            }
            let setting = setting.ok_or_else(|| "set requires KEY=VALUE".to_owned())?;
            let (key, value) = setting
                .split_once('=')
                .ok_or_else(|| "set requires KEY=VALUE".to_owned())?;
            let mut object = serde_json::Map::from_iter([
                ("key".into(), serde_json::Value::String(key.into())),
                ("value".into(), serde_json::Value::String(value.into())),
                ("temporary".into(), serde_json::Value::Bool(temporary)),
            ]);
            if let Some(pattern) = pattern {
                object.insert("pattern".into(), serde_json::Value::String(pattern.clone()));
            }
            serde_json::Value::Object(object)
        }
        "unset" => {
            let mut temporary = false;
            let mut pattern = None;
            let mut key = None;
            let mut arguments = command.arguments.iter();
            while let Some(argument) = arguments.next() {
                match argument.as_str() {
                    "--temp" if !temporary => temporary = true,
                    "--pattern" if pattern.is_none() => {
                        pattern = Some(
                            arguments
                                .next()
                                .ok_or_else(|| "unset --pattern requires a value".to_owned())?,
                        );
                    }
                    value if key.is_none() => key = Some(value),
                    _ => return Err("unset accepts [--temp] [--pattern PATTERN] KEY".into()),
                }
            }
            let key = key.ok_or_else(|| "unset requires KEY".to_owned())?;
            let mut object = serde_json::Map::from_iter([
                ("key".into(), serde_json::Value::String(key.into())),
                ("temporary".into(), serde_json::Value::Bool(temporary)),
            ]);
            if let Some(pattern) = pattern {
                object.insert("pattern".into(), serde_json::Value::String(pattern.clone()));
            }
            serde_json::Value::Object(object)
        }
        "get" => {
            let mut key = None;
            let mut url = None;
            let mut explain = false;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--url" => {
                        if url.is_some() {
                            return Err("get accepts --url at most once".into());
                        }
                        url = Some(
                            command
                                .arguments
                                .get(index + 1)
                                .ok_or_else(|| "get --url requires a value".to_owned())?
                                .clone(),
                        );
                        index += 1;
                    }
                    "--explain" if !explain => explain = true,
                    "--explain" => return Err("get accepts --explain at most once".into()),
                    value if !value.starts_with('-') && key.is_none() => {
                        key = Some(value.to_owned());
                    }
                    value => return Err(format!("unknown get option or extra argument: {value}")),
                }
                index += 1;
            }
            let key = key.ok_or_else(|| "get requires a configuration key".to_owned())?;
            validate_command_text(&key, "configuration key")?;
            if let Some(url) = url.as_deref() {
                validate_command_text(url, "configuration URL")?;
            }
            serde_json::json!({"key": key, "url": url, "explain": explain})
        }
        "help" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [topic] => {
                validate_command_text(topic, "help topic")?;
                serde_json::json!({"topic": topic})
            }
            _ => return Err("help accepts at most one command or setting topic".into()),
        },
        "version" | "diagnostics" => {
            if !command.arguments.is_empty() {
                return Err(format!("{} does not accept arguments", command.name));
            }
            serde_json::json!({})
        }
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
        "fullscreen" => {
            if command.arguments.len() > 1
                || command
                    .arguments
                    .first()
                    .is_some_and(|state| !matches!(state.as_str(), "on" | "off" | "toggle"))
            {
                return Err("fullscreen accepts optional state on, off, or toggle".into());
            }
            serde_json::json!({
                "state": command.arguments.first().map_or("toggle", String::as_str)
            })
        }
        "reload" => {
            if command.arguments.is_empty() {
                serde_json::json!({"bypass_cache": false})
            } else if command.arguments == ["--bypass-cache"] {
                serde_json::json!({"bypass_cache": true})
            } else {
                return Err("reload accepts only --bypass-cache".into());
            }
        }
        "tab-close" => {
            let mut id = None;
            let mut count = None;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--id" => {
                        if id.is_some() {
                            return Err("tab-close accepts --id at most once".into());
                        }
                        id = Some(
                            command
                                .arguments
                                .get(index + 1)
                                .ok_or_else(|| "tab-close --id requires a tab ID".to_owned())?
                                .clone(),
                        );
                        index += 2;
                    }
                    "--count" => {
                        if count.is_some() {
                            return Err("tab-close accepts --count at most once".into());
                        }
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "tab-close --count requires a value from 1 to 100".to_owned()
                        })?;
                        let parsed = value
                            .parse::<u32>()
                            .ok()
                            .filter(|value| (1..=100).contains(value));
                        count = Some(parsed.ok_or_else(|| {
                            "tab-close count must be a value from 1 to 100".to_owned()
                        })?);
                        index += 2;
                    }
                    value if !value.starts_with('-') && id.is_none() => {
                        id = Some(value.to_owned());
                        index += 1;
                    }
                    _ => return Err("tab-close accepts [--id ID] [--count N]".into()),
                }
            }
            if id.is_some() && count.is_some() {
                return Err("tab-close cannot combine --id and --count".into());
            }
            if let Some(id) = id.as_deref() {
                validate_command_text(id, "tab ID")?;
            }
            let mut object = serde_json::Map::new();
            if let Some(id) = id {
                object.insert("id".into(), serde_json::Value::String(id.clone()));
            }
            if let Some(count) = count {
                object.insert("count".into(), serde_json::json!(count));
            }
            serde_json::Value::Object(object)
        }
        "search" => {
            let mut backward = false;
            let mut case = "smart";
            let mut case_seen = false;
            let mut query = Vec::new();
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--backward" if !backward => backward = true,
                    "--case" if !case_seen => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "search --case requires smart, sensitive, or insensitive".to_owned()
                        })?;
                        if !matches!(value.as_str(), "smart" | "sensitive" | "insensitive") {
                            return Err(
                                "search --case requires smart, sensitive, or insensitive".into()
                            );
                        }
                        case = value;
                        case_seen = true;
                        index += 1;
                    }
                    "--" => {
                        query.extend(command.arguments[index + 1..].iter().cloned());
                        break;
                    }
                    value if value.starts_with("--") => {
                        return Err(format!("unknown search option: {value}"));
                    }
                    value => query.push(value.to_owned()),
                }
                index += 1;
            }
            if query.is_empty() {
                return Err("search requires query text".into());
            }
            serde_json::json!({
                "query": query.join(" "),
                "backward": backward,
                "case": case
            })
        }
        "window-new" => {
            let mut profile = None;
            let mut private = false;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--profile" if profile.is_none() => {
                        profile = Some(
                            command
                                .arguments
                                .get(index + 1)
                                .filter(|name| !name.starts_with("--") && !name.is_empty())
                                .ok_or_else(|| "window-new --profile requires a name".to_owned())?
                                .clone(),
                        );
                        index += 1;
                    }
                    "--private" if !private => private = true,
                    option => return Err(format!("unknown window-new option: {option}")),
                }
                index += 1;
            }
            let mut value = serde_json::json!({"private": private});
            if let Some(profile) = profile {
                validate_command_text(&profile, "window-new profile")?;
                value["profile"] = serde_json::Value::String(profile);
            }
            value
        }
        "tab-give" => {
            if command.arguments.len() != 1 {
                return Err("tab-give requires exactly one WINDOW_ID".into());
            }
            validate_command_text(&command.arguments[0], "window ID")?;
            serde_json::json!({"window_id": command.arguments[0]})
        }
        "tab-select" => {
            if command.arguments.len() != 1 {
                return Err("tab-select requires exactly one tab ID or displayed index".into());
            }
            validate_command_text(&command.arguments[0], "tab selector")?;
            serde_json::json!({"selector": command.arguments[0]})
        }
        "tab-focus" | "tab-suspend" | "tab-discard" | "tab-resume" => {
            if command.arguments.len() != 1 {
                return Err(format!("{} requires exactly one tab ID", command.name));
            }
            validate_command_text(&command.arguments[0], "tab ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "tab-pin" | "tab-mute" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [state] if matches!(state.as_str(), "on" | "off" | "toggle") => {
                serde_json::json!({"state": state})
            }
            [id] => {
                validate_command_text(id, "tab ID")?;
                serde_json::json!({"id": id})
            }
            [id, state] if matches!(state.as_str(), "on" | "off" | "toggle") => {
                validate_command_text(id, "tab ID")?;
                serde_json::json!({"id": id, "state": state})
            }
            _ => {
                return Err(format!("{} accepts [TAB_ID] [on|off|toggle]", command.name));
            }
        },
        "tab-move" => match command.arguments.as_slice() {
            [direction] if matches!(direction.as_str(), "left" | "right") => {
                serde_json::json!({"direction": direction})
            }
            [id, direction] if matches!(direction.as_str(), "left" | "right") => {
                validate_command_text(id, "tab ID")?;
                serde_json::json!({"id": id, "direction": direction})
            }
            [id, flag, context] if flag == "--context" => {
                validate_command_text(id, "tab ID or index")?;
                validate_command_text(context, "context")?;
                serde_json::json!({"id": id, "context": context})
            }
            [flag, context] if flag == "--context" => {
                validate_command_text(context, "context")?;
                serde_json::json!({"context": context})
            }
            _ => {
                return Err(
                    "tab-move requires [TAB_ID] left|right, or [INDEX] --context NAME".into(),
                );
            }
        },
        "window-focus" => {
            if command.arguments.len() != 1 {
                return Err("window-focus requires exactly one WINDOW_ID".into());
            }
            validate_command_text(&command.arguments[0], "window ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "window-move" => {
            if command.arguments.len() != 2 {
                return Err("window-move requires WINDOW_ID and WORKSPACE".into());
            }
            validate_command_text(&command.arguments[0], "window ID")?;
            validate_command_text(&command.arguments[1], "workspace")?;
            serde_json::json!({
                "id": command.arguments[0],
                "workspace": command.arguments[1]
            })
        }
        "zoom" => {
            if command.arguments.len() != 1 {
                return Err("zoom requires in, out, reset, or a factor".into());
            }
            let factor = command.arguments[0].as_str();
            if !matches!(factor, "in" | "out" | "reset")
                && factor
                    .parse::<f64>()
                    .ok()
                    .is_none_or(|value| !(0.25..=5.0).contains(&value))
            {
                return Err(
                    "zoom factor must be in, out, reset, or a value from 0.25 to 5.0".into(),
                );
            }
            serde_json::json!({"factor": command.arguments[0]})
        }
        "search-next" => {
            let mut direction = "forward";
            let mut count = None;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--backward" if direction == "forward" => direction = "backward",
                    "--count" if count.is_none() => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "search-next --count requires a value from 1 to 100".to_owned()
                        })?;
                        count = Some(
                            value
                                .parse::<u32>()
                                .ok()
                                .filter(|n| (1..=100).contains(n))
                                .ok_or_else(|| {
                                    "search-next --count must be a value from 1 to 100".to_owned()
                                })?,
                        );
                        index += 1;
                    }
                    option => return Err(format!("unknown search-next option: {option}")),
                }
                index += 1;
            }
            serde_json::json!({"direction": direction, "count": count.unwrap_or(1)})
        }
        "scroll" => {
            let direction = command
                .arguments
                .first()
                .ok_or_else(|| "scroll requires a direction".to_owned())?;
            if !matches!(direction.as_str(), "up" | "down" | "left" | "right") {
                return Err("scroll direction must be up, down, left, or right".into());
            }
            if command.arguments.len() == 1 {
                serde_json::json!({"direction": direction})
            } else if command.arguments.len() == 3 && command.arguments[1] == "--count" {
                let count = command.arguments[2]
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=100).contains(n))
                    .ok_or_else(|| "scroll --count must be a value from 1 to 100".to_owned())?;
                serde_json::json!({"direction": direction, "count": count})
            } else {
                return Err("scroll accepts DIRECTION [--count N]".into());
            }
        }
        "scroll-page" => {
            let direction = command
                .arguments
                .first()
                .ok_or_else(|| "scroll-page requires up or down".to_owned())?;
            if !matches!(direction.as_str(), "up" | "down") {
                return Err("scroll-page direction must be up or down".into());
            }
            let mut half = false;
            let mut count = None;
            let mut index = 1;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--half" if !half => half = true,
                    "--count" if count.is_none() => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "scroll-page --count requires a value from 1 to 100".to_owned()
                        })?;
                        count = Some(
                            value
                                .parse::<u32>()
                                .ok()
                                .filter(|n| (1..=100).contains(n))
                                .ok_or_else(|| {
                                    "scroll-page --count must be a value from 1 to 100".to_owned()
                                })?,
                        );
                        index += 1;
                    }
                    option => return Err(format!("unknown scroll-page option: {option}")),
                }
                index += 1;
            }
            let mut value = serde_json::json!({"direction": direction, "half": half});
            if let Some(count) = count {
                value["count"] = serde_json::json!(count);
            }
            value
        }
        "scroll-to" => {
            if command.arguments.len() != 1
                || !matches!(command.arguments[0].as_str(), "top" | "bottom")
            {
                return Err("scroll-to requires top or bottom".into());
            }
            serde_json::json!({"edge": command.arguments[0]})
        }
        "config-export" => {
            if command.arguments.len() != 1 {
                return Err("config-export requires exactly one path".into());
            }
            serde_json::json!({"path": command.arguments[0]})
        }
        "bind" => {
            let (mode, start) = if command.arguments.first().map(String::as_str) == Some("--mode") {
                let mode = command
                    .arguments
                    .get(1)
                    .ok_or_else(|| "bind --mode requires MODE".to_owned())?;
                if !matches!(
                    mode.as_str(),
                    "normal" | "insert" | "caret" | "passthrough" | "command"
                ) {
                    return Err(
                        "bind --mode must be normal, insert, caret, passthrough, or command".into(),
                    );
                }
                (Some(mode.clone()), 2)
            } else {
                (None, 0)
            };
            if command.arguments.len() < start + 2 {
                return Err("bind requires KEYCHAIN and COMMAND_TEXT".into());
            }
            let mut value = serde_json::json!({
                "keychain": command.arguments[start],
                "command": command.arguments[start + 1..].join(" ")
            });
            if let Some(mode) = mode {
                value["mode"] = serde_json::Value::String(mode);
            }
            value
        }
        "unbind" => {
            let (mode, keychain) =
                if command.arguments.first().map(String::as_str) == Some("--mode") {
                    let mode = command
                        .arguments
                        .get(1)
                        .ok_or_else(|| "unbind --mode requires MODE".to_owned())?;
                    if !matches!(
                        mode.as_str(),
                        "normal" | "insert" | "caret" | "passthrough" | "command"
                    ) {
                        return Err(
                            "unbind --mode must be normal, insert, caret, passthrough, or command"
                                .into(),
                        );
                    }
                    let keychain = command
                        .arguments
                        .get(2)
                        .ok_or_else(|| "unbind requires KEYCHAIN".to_owned())?;
                    (Some(mode.clone()), keychain.clone())
                } else if command.arguments.len() == 1 {
                    (None, command.arguments[0].clone())
                } else {
                    return Err("unbind accepts [--mode MODE] KEYCHAIN".into());
                };
            let mut value = serde_json::json!({"keychain": keychain});
            if let Some(mode) = mode {
                value["mode"] = serde_json::Value::String(mode);
            }
            value
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
        "context-list" => {
            if !command.arguments.is_empty() {
                return Err("context-list does not accept arguments".into());
            }
            serde_json::json!({})
        }
        "context-create" => {
            let name = command
                .arguments
                .first()
                .ok_or_else(|| "context-create requires NAME".to_owned())?;
            validate_command_text(name, "context-create name")?;
            let mut object = serde_json::Map::from_iter([(
                "name".into(),
                serde_json::Value::String(name.clone()),
            )]);
            let mut profile_set = false;
            let mut index = 1;
            while index < command.arguments.len() {
                let option = command.arguments[index].as_str();
                let value = command
                    .arguments
                    .get(index + 1)
                    .cloned()
                    .ok_or_else(|| format!("{option} requires a value"))?;
                match option {
                    "--label" | "--profile" | "--workspace" => {
                        validate_command_text(&value, &format!("context-create {option}"))?;
                        let key = option.trim_start_matches('-');
                        if object
                            .insert(key.into(), serde_json::Value::String(value))
                            .is_some()
                        {
                            return Err(format!("{option} was specified more than once"));
                        }
                        if option == "--profile" {
                            profile_set = true;
                        }
                    }
                    _ => return Err(format!("unknown context-create option: {option}")),
                }
                index += 2;
            }
            if !profile_set {
                return Err("context-create requires --profile NAME".into());
            }
            serde_json::Value::Object(object)
        }
        "context-delete" => {
            let name = command
                .arguments
                .first()
                .ok_or_else(|| "context-delete requires NAME".to_owned())?;
            validate_command_text(name, "context-delete name")?;
            if command.arguments.len() > 2
                || (command.arguments.len() == 2 && command.arguments[1] != "--confirm")
            {
                return Err("context-delete expects NAME [--confirm]".into());
            }
            serde_json::json!({
                "name": name,
                "confirmed": command.arguments.get(1).is_some_and(|value| value == "--confirm")
            })
        }
        "context-enter" => {
            if command.arguments.len() != 1 {
                return Err("context-enter requires exactly one name".into());
            }
            validate_command_text(&command.arguments[0], "context-enter name")?;
            serde_json::json!({"name": command.arguments[0]})
        }
        "context-save" => {
            if command.arguments.len() > 1 {
                return Err("context-save accepts at most one name".into());
            }
            if let Some(name) = command.arguments.first() {
                validate_command_text(name, "context-save name")?;
            }
            command.arguments.first().map_or_else(
                || serde_json::json!({}),
                |name| serde_json::json!({"name": name}),
            )
        }
        "context-route" => {
            let action = command
                .arguments
                .first()
                .ok_or_else(|| "context-route requires ADD, REMOVE, or LIST".to_owned())?;
            if !matches!(action.as_str(), "add" | "remove" | "list") {
                return Err("context-route action must be add, remove, or list".into());
            }
            match action.as_str() {
                "list" if command.arguments.len() == 1 => {
                    serde_json::json!({"action": "list"})
                }
                "add" if command.arguments.len() >= 3 => {
                    validate_command_text(&command.arguments[1], "context-route pattern")?;
                    validate_command_text(&command.arguments[2], "context-route context")?;
                    let mut object = serde_json::Map::from_iter([
                        ("action".into(), serde_json::Value::String("add".into())),
                        (
                            "pattern".into(),
                            serde_json::Value::String(command.arguments[1].clone()),
                        ),
                        (
                            "context".into(),
                            serde_json::Value::String(command.arguments[2].clone()),
                        ),
                    ]);
                    let mut entry_points = Vec::new();
                    let mut index = 3;
                    while index < command.arguments.len() {
                        let option = command.arguments[index].as_str();
                        let value = command
                            .arguments
                            .get(index + 1)
                            .ok_or_else(|| format!("{option} requires a value"))?;
                        match option {
                            "--priority" => {
                                if object.contains_key("priority") {
                                    return Err(
                                        "context-route --priority was specified more than once"
                                            .into(),
                                    );
                                }
                                let priority = value.parse::<i32>().map_err(|_| {
                                    "context-route priority must be a 32-bit integer".to_owned()
                                })?;
                                object.insert("priority".into(), serde_json::json!(priority));
                            }
                            "--behavior" => {
                                if object.contains_key("behavior") {
                                    return Err(
                                        "context-route --behavior was specified more than once"
                                            .into(),
                                    );
                                }
                                if !matches!(value.as_str(), "prompt" | "suggest") {
                                    return Err(
                                        "context-route behavior must be prompt or suggest".into()
                                    );
                                }
                                object.insert(
                                    "behavior".into(),
                                    serde_json::Value::String(value.clone()),
                                );
                            }
                            "--entry-point" => {
                                if !matches!(
                                    value.as_str(),
                                    "external-open" | "explicit-open" | "typed-initial-url"
                                ) {
                                    return Err(format!(
                                        "unsupported context route entry point: {value}"
                                    ));
                                }
                                if entry_points.iter().any(|point| point == value) {
                                    return Err(format!(
                                        "context route entry point specified more than once: {value}"
                                    ));
                                }
                                if entry_points.len() >= 3 {
                                    return Err(
                                        "context-route accepts at most three entry points".into()
                                    );
                                }
                                entry_points.push(value.clone());
                            }
                            _ => return Err(format!("unknown context-route option: {option}")),
                        }
                        index += 2;
                    }
                    if !entry_points.is_empty() {
                        object.insert(
                            "entry_points".into(),
                            serde_json::Value::Array(
                                entry_points
                                    .into_iter()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            ),
                        );
                    }
                    serde_json::Value::Object(object)
                }
                "remove" if command.arguments.len() == 2 => {
                    validate_command_text(&command.arguments[1], "context-route route ID")?;
                    serde_json::json!({
                        "action": "remove",
                        "id": command.arguments[1],
                    })
                }
                "list" => return Err("context-route list takes no arguments".into()),
                "add" => return Err("context-route add requires PATTERN and CONTEXT".into()),
                "remove" => return Err("context-route remove requires ROUTE_ID".into()),
                _ => unreachable!("context-route action was validated"),
            }
        }
        "hint" => {
            let mut object = serde_json::Map::new();
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "links" | "all" => {
                        if object.contains_key("kind") {
                            return Err("hint kind was specified more than once".into());
                        }
                        object.insert(
                            "kind".into(),
                            serde_json::Value::String(command.arguments[index].clone()),
                        );
                    }
                    "--rapid" => {
                        if object
                            .insert("rapid".into(), serde_json::Value::Bool(true))
                            .is_some()
                        {
                            return Err("hint --rapid was specified more than once".into());
                        }
                    }
                    "--target" => {
                        let target = command
                            .arguments
                            .get(index + 1)
                            .ok_or_else(|| "hint --target requires TARGET".to_owned())?;
                        if !matches!(
                            target.as_str(),
                            "current"
                                | "tab"
                                | "tab-bg"
                                | "window"
                                | "yank"
                                | "clean-yank"
                                | "download"
                                | "userscript"
                                | "ephemeral"
                        ) {
                            return Err(
                                "hint target must be current, tab, tab-bg, window, yank, clean-yank, download, userscript, or ephemeral"
                                    .into(),
                            );
                        }
                        if object
                            .insert("target".into(), serde_json::Value::String(target.clone()))
                            .is_some()
                        {
                            return Err("hint --target was specified more than once".into());
                        }
                        index += 1;
                    }
                    "--script" => {
                        let script = command
                            .arguments
                            .get(index + 1)
                            .filter(|value| {
                                !value.is_empty() && !value.chars().any(char::is_control)
                            })
                            .ok_or_else(|| "hint --script requires NAME".to_owned())?;
                        if object
                            .insert("script".into(), serde_json::Value::String(script.clone()))
                            .is_some()
                        {
                            return Err("hint --script was specified more than once".into());
                        }
                        index += 1;
                    }
                    option => return Err(format!("unknown hint option: {option}")),
                }
                index += 1;
            }
            if object
                .get("rapid")
                .is_some_and(|rapid| rapid == &serde_json::Value::Bool(true))
                && object
                    .get("target")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|target| {
                        !matches!(
                            target,
                            "current"
                                | "tab-bg"
                                | "yank"
                                | "clean-yank"
                                | "download"
                                | "userscript"
                        )
                    })
            {
                return Err(
                    "rapid hint target must be tab-bg, yank, clean-yank, download, or userscript"
                        .into(),
                );
            }
            if object
                .get("rapid")
                .is_some_and(|rapid| rapid == &serde_json::Value::Bool(true))
                && object
                    .get("target")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|target| target == "ephemeral")
            {
                return Err("hint target ephemeral does not support --rapid".into());
            }
            if object
                .get("target")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|target| target == "userscript")
                && !object.contains_key("script")
            {
                return Err("hint target userscript requires --script NAME".into());
            }
            if object
                .get("target")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|target| target != "userscript")
                && object.contains_key("script")
            {
                return Err("--script is only valid with target userscript".into());
            }
            serde_json::Value::Object(object)
        }
        "mode-enter" => {
            if command.arguments.len() != 1
                || !matches!(
                    command.arguments[0].as_str(),
                    "normal" | "insert" | "caret" | "passthrough" | "pass-through"
                )
            {
                return Err("mode-enter requires one of normal, insert, caret, passthrough".into());
            }
            serde_json::json!({"mode": command.arguments[0]})
        }
        "caret-move" => {
            if !(1..=3).contains(&command.arguments.len())
                || !matches!(
                    command.arguments.first().map(String::as_str),
                    Some(
                        "left"
                            | "right"
                            | "up"
                            | "down"
                            | "word-next"
                            | "word-prev"
                            | "line-start"
                            | "line-end"
                    )
                )
            {
                return Err("caret-move expects DIRECTION [--count N]".into());
            }
            let count = match command.arguments.as_slice() {
                [_] => 1,
                [_, flag, value] if flag == "--count" => value
                    .parse::<u32>()
                    .ok()
                    .filter(|count| (1..=9_999).contains(count))
                    .ok_or_else(|| "caret-move count must be 1..9999".to_owned())?,
                _ => return Err("caret-move expects DIRECTION [--count N]".into()),
            };
            serde_json::json!({
                "direction": command.arguments[0],
                "count": count
            })
        }
        "caret-select" => {
            if command.arguments.len() > 1
                || command
                    .arguments
                    .first()
                    .is_some_and(|state| !matches!(state.as_str(), "on" | "off" | "toggle"))
            {
                return Err("caret-select expects [on|off|toggle]".into());
            }
            command.arguments.first().map_or_else(
                || serde_json::json!({}),
                |state| serde_json::json!({"state": state}),
            )
        }
        "edit-text" => {
            if !command.arguments.is_empty() {
                return Err("edit-text does not accept arguments".into());
            }
            serde_json::json!({})
        }
        "download" => {
            if command.arguments.len() != 1 {
                return Err("download requires exactly one URL".into());
            }
            serde_json::json!({"input": command.arguments[0]})
        }
        "print-pdf" | "save-page" => {
            if command.arguments.len() != 1 {
                return Err(format!("{} requires exactly one path", command.name));
            }
            serde_json::json!({"path": command.arguments[0]})
        }
        "config-write-defaults" => {
            if command.arguments.len() != 1 {
                return Err("config-write-defaults requires exactly one path".into());
            }
            serde_json::json!({"path": command.arguments[0]})
        }
        "config-edit" | "config-reload" | "config-check" | "theme-reload" => {
            if !command.arguments.is_empty() {
                return Err(format!("{} does not accept arguments", command.name));
            }
            serde_json::json!({})
        }
        "permissions" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [origin] => serde_json::json!({"origin": origin}),
            _ => return Err("permissions accepts an optional exact origin".into()),
        },
        "permission-reset" => {
            if command.arguments.len() != 2 {
                return Err("permission-reset requires ORIGIN and PERMISSION".into());
            }
            serde_json::json!({
                "origin": command.arguments[0],
                "permission": command.arguments[1]
            })
        }
        "site-data-clear" => match command.arguments.as_slice() {
            [origin] => serde_json::json!({"origin": origin}),
            [origin, flag] if flag == "--confirm" => {
                serde_json::json!({"origin": origin, "confirmed": true})
            }
            _ => return Err("site-data-clear requires ORIGIN and optional --confirm".into()),
        },
        "site-status" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag, tab] if flag == "--tab" => {
                validate_command_text(tab, "tab ID")?;
                serde_json::json!({"tab": tab})
            }
            _ => return Err("site-status accepts optional --tab TAB_ID".into()),
        },
        "site-doctor" => {
            if command.arguments.len() != 1 {
                return Err("site-doctor requires one experiment name".into());
            }
            validate_command_text(&command.arguments[0], "experiment name")?;
            serde_json::json!({"experiment": command.arguments[0]})
        }
        "site-doctor-undo" => {
            if command.arguments.len() != 1 {
                return Err("site-doctor-undo requires one experiment ID".into());
            }
            validate_command_text(&command.arguments[0], "experiment ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "blocking-toggle" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag] if flag == "--site" => serde_json::json!({"site": true}),
            _ => return Err("blocking-toggle accepts optional --site".into()),
        },
        "quit" => {
            if !command.arguments.is_empty() {
                return Err("quit does not accept arguments".into());
            }
            serde_json::json!({})
        }
        "download-open" | "download-show" | "download-cancel" | "download-pause"
        | "download-resume" | "download-retry" => {
            if command.arguments.len() != 1 {
                return Err(format!("{} requires exactly one download ID", command.name));
            }
            validate_command_text(&command.arguments[0], "download ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "spawn" => {
            if command.arguments.first().map(String::as_str) == Some("--userscript") {
                let [_, name] = command.arguments.as_slice() else {
                    return Err("spawn --userscript requires a manifest name".into());
                };
                if name.is_empty() || name.chars().any(char::is_control) {
                    return Err("spawn userscript name is empty or invalid".into());
                }
                serde_json::json!({"userscript": name})
            } else {
                let argv = command
                    .arguments
                    .strip_prefix(&["--".to_owned()])
                    .ok_or_else(|| "spawn requires -- PROGRAM [ARG...]".to_owned())?;
                if argv.is_empty() || argv.len() > 256 {
                    return Err("spawn requires 1..256 argv values".into());
                }
                if argv
                    .iter()
                    .any(|value| value.is_empty() || value.chars().any(char::is_control))
                {
                    return Err("spawn argv values must be nonempty and contain no controls".into());
                }
                serde_json::json!({"argv": argv})
            }
        }
        "script-run" => {
            if command.arguments.len() != 1
                || command.arguments[0].is_empty()
                || command.arguments[0].chars().any(char::is_control)
            {
                return Err("script-run requires exactly one valid userscript name".into());
            }
            serde_json::json!({"name": command.arguments[0]})
        }
        "jseval" => {
            let mut world = "isolated";
            let mut world_seen = false;
            let mut script = None;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--world" => {
                        if world_seen || index + 1 >= command.arguments.len() {
                            return Err("jseval accepts --world isolated|page at most once".into());
                        }
                        world = command.arguments[index + 1].as_str();
                        world_seen = true;
                        index += 2;
                    }
                    "--" if script.is_none() => {
                        index += 1;
                        if index >= command.arguments.len() {
                            return Err("jseval requires SCRIPT after --".into());
                        }
                        script = Some(command.arguments[index].as_str());
                        index += 1;
                    }
                    value if script.is_none() => {
                        script = Some(value);
                        index += 1;
                    }
                    _ => return Err("jseval accepts [--world isolated|page] SCRIPT".into()),
                }
            }
            let script = script
                .filter(|value| !value.is_empty() && value.len() <= 64 * 1024)
                .ok_or_else(|| "jseval requires a nonempty SCRIPT of at most 64 KiB".to_owned())?;
            if !matches!(world, "isolated" | "page") {
                return Err("jseval world must be isolated or page".into());
            }
            if script.chars().any(|character| {
                character == '\0'
                    || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
            }) {
                return Err("jseval script contains a disallowed control character".into());
            }
            serde_json::json!({"world": world, "script": script})
        }
        "devtools" => match command.arguments.as_slice() {
            [] => serde_json::json!({"detach": false}),
            [flag] if flag == "--detach" => serde_json::json!({"detach": true}),
            _ => return Err("devtools accepts only the optional --detach flag".into()),
        },
        "repeat" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag, count] if flag == "--count" => {
                let count = count
                    .parse::<u32>()
                    .ok()
                    .filter(|count| (1..=100).contains(count))
                    .ok_or_else(|| "repeat --count must be 1..100".to_owned())?;
                serde_json::json!({"count": count})
            }
            _ => return Err("repeat accepts optional --count N (1..100)".into()),
        },
        "cancel" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [operation_id] => serde_json::json!({"operation_id": operation_id}),
            _ => return Err("cancel accepts at most one operation ID".into()),
        },
        "macro-record" | "macro-play" => {
            if command.arguments.len() != 1 {
                return Err(format!("{} requires REGISTER", command.name));
            }
            serde_json::json!({"register": command.arguments[0]})
        }
        "macro-stop" => {
            if !command.arguments.is_empty() {
                return Err("macro-stop does not accept arguments".into());
            }
            serde_json::json!({})
        }
        "yank" => {
            if command.arguments.is_empty()
                || !matches!(command.arguments[0].as_str(), "url" | "title" | "selection")
            {
                return Err("yank expects url, title, or selection source".into());
            }
            let source = command.arguments[0].clone();
            let mut object = serde_json::Map::from_iter([(
                "source".into(),
                serde_json::Value::String(source.clone()),
            )]);
            let mut input = None;
            for argument in &command.arguments[1..] {
                match argument.as_str() {
                    "--clean" => {
                        if source != "url" {
                            return Err("only URL copying accepts --clean".into());
                        }
                        if object
                            .insert("clean".into(), serde_json::Value::Bool(true))
                            .is_some()
                        {
                            return Err("yank --clean was specified more than once".into());
                        }
                    }
                    "--primary" => {
                        if object
                            .insert("primary".into(), serde_json::Value::Bool(true))
                            .is_some()
                        {
                            return Err("yank --primary was specified more than once".into());
                        }
                    }
                    _value if source != "url" => {
                        return Err("title and selection copying accept no URL input".into());
                    }
                    value if input.replace(value.to_owned()).is_some() => {
                        return Err("yank accepts at most one URL input".into());
                    }
                    value => input = Some(value.to_owned()),
                }
            }
            if let Some(input) = input {
                object.insert("input".into(), serde_json::Value::String(input));
            }
            serde_json::Value::Object(object)
        }
        "action" => {
            if command.arguments.len() < 2 || command.arguments.len() > 3 {
                return Err("action expects SUBJECT VERB [ARG]".into());
            }
            let subject = &command.arguments[0];
            let verb = &command.arguments[1];
            let mut object = serde_json::Map::from_iter([
                ("subject".into(), serde_json::Value::String(subject.clone())),
                ("verb".into(), serde_json::Value::String(verb.clone())),
            ]);
            if let Some(argument) = command.arguments.get(2) {
                let key = if (subject == "url" && matches!(verb.as_str(), "clean" | "explain"))
                    || (subject == "link" && matches!(verb.as_str(), "copy" | "clean-copy"))
                {
                    "url"
                } else {
                    "input"
                };
                object.insert(key.into(), serde_json::Value::String(argument.clone()));
            }
            serde_json::Value::Object(object)
        }
        "action-list" => {
            if command.arguments.len() > 1 {
                return Err("action-list accepts at most one subject".into());
            }
            command.arguments.first().map_or_else(
                || serde_json::json!({}),
                |subject| serde_json::json!({"subject": subject}),
            )
        }
        "send" => {
            let target = command
                .arguments
                .first()
                .ok_or_else(|| "send requires TARGET".to_owned())?;
            validate_command_text(target, "send target")?;
            let mut object = serde_json::Map::from_iter([(
                "target".into(),
                serde_json::Value::String(target.clone()),
            )]);
            match command.arguments.get(1..).unwrap_or_default() {
                [] => {}
                [flag] if flag == "--selection" => {
                    object.insert("selection".into(), serde_json::Value::Bool(true));
                    object.insert(
                        "send_subject".into(),
                        serde_json::Value::String("selection".into()),
                    );
                }
                [flag] if flag == "--tab" => {
                    object.insert(
                        "send_subject".into(),
                        serde_json::Value::String("tab".into()),
                    );
                }
                [flag] if flag == "--url" => {
                    object.insert(
                        "send_subject".into(),
                        serde_json::Value::String("url".into()),
                    );
                }
                [flag, url] if flag == "--url" => {
                    validate_command_text(url, "send URL")?;
                    object.insert("url".into(), serde_json::Value::String(url.clone()));
                    object.insert(
                        "send_subject".into(),
                        serde_json::Value::String("url".into()),
                    );
                }
                [input] => {
                    validate_command_text(input, "send input")?;
                    object.insert("input".into(), serde_json::Value::String(input.clone()));
                }
                _ => return Err("send expects TARGET [INPUT|--selection|--tab|--url [URL]]".into()),
            }
            serde_json::Value::Object(object)
        }
        "command-help" => {
            if command.arguments.len() != 1 {
                return Err("command-help requires COMMAND_ID".into());
            }
            validate_command_text(&command.arguments[0], "command ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "command-execute" => {
            if command.arguments.is_empty() || command.arguments.len() > 2 {
                return Err("command-execute expects COMMAND_ID [JSON_ARGUMENTS]".into());
            }
            validate_command_text(&command.arguments[0], "command ID")?;
            let arguments = match command.arguments.get(1) {
                None => serde_json::json!({}),
                Some(encoded) => {
                    let value: serde_json::Value =
                        serde_json::from_str(encoded).map_err(|error| {
                            format!("command arguments are not valid JSON: {error}")
                        })?;
                    if !value.is_object() {
                        return Err("command arguments must be a JSON object".into());
                    }
                    value
                }
            };
            serde_json::json!({"id": command.arguments[0], "arguments": arguments})
        }
        "binding-list" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag, mode] if flag == "--mode" => {
                if !matches!(
                    mode.as_str(),
                    "normal" | "insert" | "command" | "search" | "hint" | "caret" | "pass-through"
                ) {
                    return Err("binding-list --mode must be a valid input mode".into());
                }
                serde_json::json!({"mode": mode})
            }
            _ => return Err("binding-list accepts optional --mode MODE".into()),
        },
        "binding-explain" => {
            let mut keychain = None;
            let mut mode = None;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--mode" => {
                        if mode.is_some() {
                            return Err("binding-explain accepts --mode at most once".into());
                        }
                        let value = command
                            .arguments
                            .get(index + 1)
                            .ok_or_else(|| "binding-explain --mode requires a value".to_owned())?;
                        if !matches!(
                            value.as_str(),
                            "normal"
                                | "insert"
                                | "command"
                                | "search"
                                | "hint"
                                | "caret"
                                | "pass-through"
                        ) {
                            return Err("binding-explain --mode must be a valid input mode".into());
                        }
                        mode = Some(value.clone());
                        index += 2;
                    }
                    value if keychain.is_none() => {
                        keychain = Some(value.to_owned());
                        index += 1;
                    }
                    _ => {
                        return Err(
                            "binding-explain accepts KEYCHAIN and optional --mode MODE".into()
                        );
                    }
                }
            }
            let keychain =
                keychain.ok_or_else(|| "binding-explain requires KEYCHAIN".to_owned())?;
            let mut value = serde_json::Map::from_iter([(
                "keychain".into(),
                serde_json::Value::String(keychain),
            )]);
            if let Some(mode) = mode {
                value.insert("mode".into(), serde_json::Value::String(mode));
            }
            serde_json::Value::Object(value)
        }
        "learning-mode" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [state] if matches!(state.as_str(), "on" | "off" | "toggle") => {
                serde_json::json!({"state": state})
            }
            _ => return Err("learning-mode accepts optional on, off, or toggle".into()),
        },
        _ if command.arguments.is_empty() => serde_json::json!({}),
        _ => return Err(format!("{} does not accept arguments", command.name)),
    };
    let argument_object = arguments
        .as_object()
        .cloned()
        .ok_or_else(|| "command arguments must encode as an object".to_owned())?;
    crate::CommandEnvelope::new(
        command.name.clone(),
        argument_object,
        crate::CommandContext::default(),
    )
    .map(crate::CommandEnvelope::into_value)
}

fn normalize_cli_history_origin(value: &str) -> Result<String, String> {
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
