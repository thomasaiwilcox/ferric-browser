//! command codec contexts command encoding.

// The family encoder shares the parent's schema helpers by design.
#[allow(clippy::wildcard_imports)]
use super::*;

#[allow(clippy::too_many_lines)]
pub(super) fn encode(command: &ParsedCommand) -> Result<Option<EncodedCommand>, String> {
    let arguments = match command.name.as_str() {
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
        _ => return Ok(None),
    };
    Ok(Some(EncodedCommand::new(arguments)))
}
