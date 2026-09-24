//! command codec automation command encoding.

// The family encoder shares the parent's schema helpers by design.
#[allow(clippy::wildcard_imports)]
use super::*;

#[allow(clippy::too_many_lines)]
pub(super) fn encode(command: &ParsedCommand) -> Result<Option<EncodedCommand>, String> {
    let arguments = match command.name.as_str() {
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
        _ => return Ok(None),
    };
    Ok(Some(EncodedCommand::new(arguments)))
}
