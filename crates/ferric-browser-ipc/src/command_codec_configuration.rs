//! command codec configuration command encoding.

// The family encoder shares the parent's schema helpers by design.
#[allow(clippy::wildcard_imports)]
use super::*;

#[allow(clippy::too_many_lines)]
pub(super) fn encode(command: &ParsedCommand) -> Result<Option<EncodedCommand>, String> {
    let arguments = match command.name.as_str() {
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
        "version" | "diagnostics" | "config-edit" | "config-reload" | "config-check"
        | "theme-reload" => {
            if !command.arguments.is_empty() {
                return Err(format!("{} does not accept arguments", command.name));
            }
            serde_json::json!({})
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
        "config-write-defaults" => {
            if command.arguments.len() != 1 {
                return Err("config-write-defaults requires exactly one path".into());
            }
            serde_json::json!({"path": command.arguments[0]})
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
        _ => return Ok(None),
    };
    Ok(Some(EncodedCommand::new(arguments)))
}
