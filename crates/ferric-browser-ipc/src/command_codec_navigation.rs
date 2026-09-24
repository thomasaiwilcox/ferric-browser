//! command codec navigation command encoding.

// Each family deliberately keeps its schema mapping in one visible match.  The parent module
// owns the shared validation vocabulary used across those mappings.
#[allow(clippy::too_many_lines, clippy::wildcard_imports)]
use super::*;

#[allow(clippy::too_many_lines)]
pub(super) fn encode(command: &ParsedCommand) -> Result<Option<EncodedCommand>, String> {
    let mut context = crate::CommandContext::default();
    let arguments = match command.name.as_str() {
        "open" => {
            let mut target = None;
            let mut clean_link = false;
            let mut input = Vec::new();
            let mut options_ended = false;
            let mut index = 0;
            while index < command.arguments.len() {
                let argument = &command.arguments[index];
                if options_ended {
                    input.push(argument.clone());
                } else {
                    match argument.as_str() {
                        "--" => options_ended = true,
                        "--target" | "--profile" | "--context" => {
                            index += 1;
                            let value = command
                                .arguments
                                .get(index)
                                .ok_or_else(|| format!("open {argument} requires a value"))?;
                            validate_command_text(value, "open option")?;
                            match argument.as_str() {
                                "--target" if target.replace(value.clone()).is_some() => {
                                    return Err("open accepts --target at most once".into());
                                }
                                "--profile" if context.profile.replace(value.clone()).is_some() => {
                                    return Err("open accepts --profile at most once".into());
                                }
                                "--context" if context.context.replace(value.clone()).is_some() => {
                                    return Err("open accepts --context at most once".into());
                                }
                                _ => {}
                            }
                        }
                        "--clean-link" if !clean_link => clean_link = true,
                        "--clean-link" => {
                            return Err("open accepts --clean-link at most once".into());
                        }
                        "--ephemeral" => {
                            return Err(
                                "open --ephemeral requires a fresh GUI launch; use the CLI entry point"
                                    .into(),
                            );
                        }
                        value if value.starts_with('-') => {
                            return Err(format!("unknown open option: {value}"));
                        }
                        value => {
                            options_ended = true;
                            input.push(value.to_owned());
                        }
                    }
                }
                index += 1;
            }
            if input.is_empty() {
                return Err("open requires an input".into());
            }
            match target.as_deref() {
                None | Some("current" | "tab" | "tab-bg" | "window" | "private-window") => {}
                Some(_) => {
                    return Err(
                        "open --target must be current, tab, tab-bg, window, or private-window"
                            .into(),
                    );
                }
            }
            if clean_link && target.as_deref() == Some("tab-bg") {
                return Err("open --clean-link cannot be combined with --target tab-bg".into());
            }
            let mut value = serde_json::Map::from_iter([
                ("input".into(), serde_json::Value::String(input.join(" "))),
                ("external".into(), serde_json::Value::Bool(true)),
            ]);
            if let Some(target) = target {
                let target = if target == "current" { "tab" } else { &target };
                value.insert(
                    "target".into(),
                    serde_json::Value::String(target.to_owned()),
                );
            }
            if clean_link {
                value.insert("clean_link".into(), serde_json::Value::Bool(true));
            }
            serde_json::Value::Object(value)
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
        _ => return Ok(None),
    };
    Ok(Some(EncodedCommand::with_context(arguments, context)))
}
