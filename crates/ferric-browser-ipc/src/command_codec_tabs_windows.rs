//! command codec tabs windows command encoding.

// The family encoder shares the parent's schema helpers by design.
#[allow(clippy::wildcard_imports)]
use super::*;

#[allow(clippy::too_many_lines)]
pub(super) fn encode(command: &ParsedCommand) -> Result<Option<EncodedCommand>, String> {
    let arguments = match command.name.as_str() {
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
        _ => return Ok(None),
    };
    Ok(Some(EncodedCommand::new(arguments)))
}
