//! command codec content command encoding.

// The family encoder shares the parent's schema helpers by design.
#[allow(clippy::wildcard_imports)]
use super::*;

#[allow(clippy::too_many_lines)]
pub(super) fn encode(command: &ParsedCommand) -> Result<Option<EncodedCommand>, String> {
    let arguments = match command.name.as_str() {
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
        _ => return Ok(None),
    };
    Ok(Some(EncodedCommand::new(arguments)))
}
