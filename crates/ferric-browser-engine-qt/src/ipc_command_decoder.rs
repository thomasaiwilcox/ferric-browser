//! Typed IPC command decoding outside the Qt presentation boundary.
//!
//! This module accepts only the documented wire envelope and produces a
//! canonical command plus routing request. It does not inspect `QObject` state
//! or execute browser policy.

use crate::{
    command_options::normalize_history_clear_origin,
    input_validation::{
        is_bounded_journey_query, is_bounded_untrusted_text, validate_jseval_script,
    },
    ipc_contract::{IpcCommandError, IpcRoute},
    ipc_route::typed_ipc_route,
    ipc_schema::validate_command_argument_envelope,
    open_policy::parse_interactive as parse_interactive_open,
    parse_ipc_mode,
    switcher_policy::valid_scope as valid_switcher_scope,
};
use ferric_browser_core::ParsedCommand;
use serde_json::Value;
use uuid::Uuid;

pub(super) fn interactive_open_command(
    command: &ParsedCommand,
) -> Result<Option<(ParsedCommand, IpcRoute)>, String> {
    let Some(open) = parse_interactive_open(command)? else {
        return Ok(None);
    };
    let mut params = serde_json::json!({
        "command": "open",
        "arguments": {
            "input": open.input,
            "clean_link": open.clean_link,
        }
    });
    if let Some(target) = open.target {
        params["arguments"]["target"] = Value::String(target);
    }
    let mut route_context = serde_json::Map::new();
    if let Some(profile) = open.profile {
        route_context.insert("profile".into(), Value::String(profile));
    }
    if let Some(context) = open.context {
        route_context.insert("context".into(), Value::String(context));
    }
    if !route_context.is_empty() {
        params["context"] = Value::Object(route_context);
    }
    typed_ipc_command(&params)
        .map(Some)
        .map_err(|error| error.to_string())
}

pub(super) fn typed_ipc_command(
    params: &Value,
) -> Result<(ParsedCommand, IpcRoute), IpcCommandError> {
    let object = params
        .as_object()
        .ok_or_else(|| "command.execute params must be an object".to_owned())?;
    let name = object
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "command.execute requires a string command".to_owned())?;
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err("command name is empty, too long, or contains a control character".into());
    }
    let arguments = object.get("arguments").map_or(Ok(None), |value| {
        value
            .as_object()
            .map(Some)
            .ok_or_else(|| "command arguments must be an object".to_owned())
    })?;
    let route = typed_ipc_route(object, arguments, name)?;
    let text_argument = |key: &str| -> Result<String, String> {
        arguments
            .and_then(|arguments| arguments.get(key))
            .and_then(Value::as_str)
            .filter(|value| is_bounded_untrusted_text(value))
            .map(ToOwned::to_owned)
            .ok_or_else(|| format!("command argument {key} must be a nonempty string"))
    };
    let optional_text_argument = |key: &str| -> Result<Option<String>, String> {
        arguments
            .and_then(|arguments| arguments.get(key))
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| is_bounded_untrusted_text(value))
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| format!("command argument {key} must be a nonempty string"))
            })
            .transpose()
    };
    let jseval_script_argument = || -> Result<String, String> {
        let script = arguments
            .and_then(|arguments| arguments.get("script"))
            .and_then(Value::as_str)
            .ok_or_else(|| "command argument script must be a nonempty string".to_owned())?;
        validate_jseval_script(script)?;
        Ok(script.to_owned())
    };
    let optional_timestamp_argument = |key: &str| -> Result<Option<i64>, String> {
        arguments
            .and_then(|arguments| arguments.get(key))
            .map(|value| {
                value
                    .as_i64()
                    .filter(|timestamp| *timestamp >= 0)
                    .ok_or_else(|| {
                        format!("command argument {key} must be a nonnegative Unix timestamp")
                    })
            })
            .transpose()
    };
    let args = match name {
        "back" | "forward" | "tab-next" | "tab-prev" => {
            let count = arguments
                .and_then(|arguments| arguments.get("count"))
                .map(|value| {
                    let count = value
                        .as_u64()
                        .and_then(|value| u32::try_from(value).ok())
                        .filter(|value| (1..=100).contains(value))
                        .ok_or_else(|| {
                            format!("command argument count must be 1 to 100 for {name}")
                        })?;
                    Ok::<u32, String>(count)
                })
                .transpose()?;
            count
                .map(|count| vec!["--count".into(), count.to_string()])
                .unwrap_or_default()
        }
        "tab-open" => {
            let input = text_argument("input")?;
            let background = match arguments.and_then(|arguments| arguments.get("background")) {
                None => false,
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| "command argument background must be a boolean".to_owned())?,
            };
            if background {
                vec!["--background".into(), input]
            } else {
                vec![input]
            }
        }
        "open-current" => match optional_text_argument("target")?.as_deref() {
            None => Vec::new(),
            Some("tab") => vec!["--target".into(), "tab".into()],
            Some(_) => return Err("command argument target must be tab".into()),
        },
        "open" => {
            let input = text_argument("input")?;
            if let Some(external) = arguments.and_then(|arguments| arguments.get("external"))
                && external.as_bool().is_none()
            {
                return Err("command argument external must be a boolean".into());
            }
            if arguments
                .and_then(|arguments| arguments.get("clean_link"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                vec!["--clean-link".into(), input]
            } else {
                vec![input]
            }
        }
        "fullscreen" => match optional_text_argument("state")? {
            None => Vec::new(),
            Some(state) if matches!(state.as_str(), "on" | "off" | "toggle") => vec![state],
            Some(_) => return Err("command argument state must be on, off, or toggle".into()),
        },
        "reload" => {
            let bypass_cache = match arguments.and_then(|arguments| arguments.get("bypass_cache")) {
                None => false,
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| "command argument bypass_cache must be a boolean".to_owned())?,
            };
            if bypass_cache {
                vec!["--bypass-cache".into()]
            } else {
                Vec::new()
            }
        }
        "search" => {
            let query = text_argument("query")?;
            let backward = match arguments.and_then(|arguments| arguments.get("backward")) {
                None => false,
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| "command argument backward must be a boolean".to_owned())?,
            };
            let case = optional_text_argument("case")?.unwrap_or_else(|| "smart".into());
            if !matches!(case.as_str(), "smart" | "sensitive" | "insensitive") {
                return Err(
                    "command argument case must be smart, sensitive, or insensitive".into(),
                );
            }
            let mut args = Vec::new();
            if backward {
                args.push("--backward".into());
            }
            if case != "smart" {
                args.extend(["--case".into(), case]);
            }
            args.push(query);
            args
        }
        "window-new" => {
            let mut args = Vec::new();
            if let Some(profile) = optional_text_argument("profile")? {
                args.extend(["--profile".into(), profile]);
            }
            if let Some(private) = arguments.and_then(|arguments| arguments.get("private")) {
                if private.as_bool().is_none() {
                    return Err("command argument private must be a boolean".into());
                }
                if private.as_bool() == Some(true) {
                    args.push("--private".into());
                }
            }
            args
        }
        "tab-close" => {
            let id = optional_text_argument("id")?;
            let count = arguments
                .and_then(|arguments| arguments.get("count"))
                .map(|value| {
                    let count = value
                        .as_u64()
                        .and_then(|value| u32::try_from(value).ok())
                        .filter(|value| (1..=100).contains(value))
                        .ok_or_else(|| "command argument count must be 1 to 100".to_owned())?;
                    Ok::<u32, String>(count)
                })
                .transpose()?;
            if id.is_some() && count.is_some() {
                return Err("tab-close cannot combine id and count".into());
            }
            let mut args = Vec::new();
            if let Some(id) = id {
                args.extend(["--id".into(), id]);
            }
            if let Some(count) = count {
                args.extend(["--count".into(), count.to_string()]);
            }
            args
        }
        "tab-give" => vec![text_argument("window_id")?],
        "set" => {
            let key = text_argument("key")?;
            let value = text_argument("value")?;
            let mut args = Vec::new();
            if arguments
                .and_then(|arguments| arguments.get("temporary"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--temp".into());
            }
            if let Some(pattern) = optional_text_argument("pattern")? {
                args.extend(["--pattern".into(), pattern]);
            }
            args.push(format!("{key}={value}"));
            args
        }
        "unset" => {
            let key = text_argument("key")?;
            let mut args = if arguments
                .and_then(|arguments| arguments.get("temporary"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                vec!["--temp".into()]
            } else {
                Vec::new()
            };
            if let Some(pattern) = optional_text_argument("pattern")? {
                args.extend(["--pattern".into(), pattern]);
            }
            args.push(key);
            args
        }
        "get" => {
            let key = text_argument("key")?;
            let mut args = vec![key];
            if let Some(url) = optional_text_argument("url")? {
                args.extend(["--url".into(), url]);
            }
            if arguments
                .and_then(|arguments| arguments.get("explain"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--explain".into());
            }
            args
        }
        "help" => optional_text_argument("topic")?
            .map(|topic| vec![topic])
            .unwrap_or_default(),
        "config-export" | "config-write-defaults" | "print-pdf" | "save-page" => {
            vec![text_argument("path")?]
        }
        "bind" => {
            let mode = optional_text_argument("mode")?.unwrap_or_else(|| "normal".into());
            if parse_ipc_mode(&mode).is_err() {
                return Err("command argument mode is invalid".into());
            }
            vec![
                "--mode".into(),
                mode,
                text_argument("keychain")?,
                text_argument("command")?,
            ]
        }
        "unbind" => {
            let mode = optional_text_argument("mode")?.unwrap_or_else(|| "normal".into());
            if parse_ipc_mode(&mode).is_err() {
                return Err("command argument mode is invalid".into());
            }
            vec!["--mode".into(), mode, text_argument("keychain")?]
        }
        "binding-list" => {
            let mode = optional_text_argument("mode")?;
            if let Some(mode) = &mode {
                parse_ipc_mode(mode)?;
            }
            mode.map(|mode| vec!["--mode".into(), mode])
                .unwrap_or_default()
        }
        "binding-explain" => {
            let mode = optional_text_argument("mode")?;
            if let Some(mode) = &mode {
                parse_ipc_mode(mode)?;
            }
            let mut args = vec![text_argument("keychain")?];
            if let Some(mode) = mode {
                args.extend(["--mode".into(), mode]);
            }
            args
        }
        "learning-mode" => match optional_text_argument("state")? {
            None => Vec::new(),
            Some(state) if matches!(state.as_str(), "on" | "off" | "toggle") => vec![state],
            Some(_) => return Err("command argument state must be on, off, or toggle".into()),
        },
        "bookmark-open" | "bookmark-delete" | "history-open" | "quickmark-open"
        | "quickmark-delete" => vec![text_argument(
            if name == "quickmark-open" || name == "quickmark-delete" {
                "name"
            } else {
                "id"
            },
        )?],
        "bookmark-edit" => {
            let id = text_argument("id")?;
            let title = text_argument("title")?;
            vec![id, "--title".into(), title]
        }
        "quickmark-edit" => vec![text_argument("name")?, text_argument("url")?],
        "quickmark-add" => {
            let mut args = vec![text_argument("name")?];
            if let Some(url) = optional_text_argument("url")? {
                args.push(url);
            }
            args
        }
        "bookmark-add" => optional_text_argument("title")?
            .map(|title| vec!["--title".into(), title])
            .unwrap_or_default(),
        "journey" => {
            let mut args = Vec::new();
            if arguments
                .and_then(|arguments| arguments.get("current"))
                .is_some_and(|value| value.as_bool().is_none())
            {
                return Err("command argument current must be a boolean".into());
            }
            if arguments
                .and_then(|arguments| arguments.get("current"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--current".into());
            }
            if let Some(search) = optional_text_argument("search")? {
                if !is_bounded_journey_query(&search) {
                    return Err(
                        "command argument search must be 1..256 bytes without control characters"
                            .into(),
                    );
                }
                args.extend(["--search".into(), search]);
            }
            if let Some(expand) = optional_text_argument("expand")? {
                if Uuid::parse_str(&expand).is_err() {
                    return Err("command argument expand must be a node UUID".into());
                }
                args.extend(["--expand".into(), expand]);
            }
            args
        }
        "switcher" => {
            let mut args = Vec::new();
            if let Some(scope) = optional_text_argument("scope")? {
                if !valid_switcher_scope(&scope) {
                    return Err("command argument scope must be a valid switcher scope".into());
                }
                args.extend(["--scope".into(), scope]);
            }
            if let Some(query) = optional_text_argument("query")? {
                args.push(query);
            }
            args
        }
        "journey-reopen" => {
            let node = text_argument("node")?;
            if Uuid::parse_str(&node).is_err() {
                return Err("command argument node must be a UUID".into());
            }
            let mut args = vec![node];
            if let Some(target) = optional_text_argument("target")? {
                if !matches!(target.as_str(), "current" | "tab" | "window") {
                    return Err("command argument target must be current, tab, or window".into());
                }
                args.extend(["--target".into(), target]);
            }
            args
        }
        "history-clear" => {
            let since = optional_timestamp_argument("since")?;
            let origin = optional_text_argument("origin")?
                .map(|origin| normalize_history_clear_origin(&origin))
                .transpose()?;
            let mut args = Vec::new();
            if let Some(since) = since {
                args.extend(["--since".into(), since.to_string()]);
            }
            if let Some(origin) = origin {
                args.extend(["--origin".into(), origin]);
            }
            if arguments
                .and_then(|arguments| arguments.get("confirmed"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--confirm".into());
            }
            args
        }
        "url-clean" | "url-explain" => optional_text_argument("url")?
            .map(|url| vec![url])
            .unwrap_or_default(),
        "hint" => {
            let mut args = Vec::new();
            if arguments
                .and_then(|arguments| arguments.get("rapid"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--rapid".into());
            }
            if let Some(target) = optional_text_argument("target")? {
                args.extend(["--target".into(), target]);
            }
            if let Some(script) = optional_text_argument("script")? {
                args.extend(["--script".into(), script]);
            }
            if let Some(kind) = optional_text_argument("kind")? {
                args.push(kind);
            }
            args
        }
        "mode-enter" => vec![text_argument("mode")?],
        "caret-move" => {
            let mut args = vec![text_argument("direction")?];
            if let Some(count) = arguments.and_then(|arguments| arguments.get("count")) {
                let count = count
                    .as_u64()
                    .filter(|count| (1..=9_999).contains(count))
                    .ok_or_else(|| "command argument count must be 1..9999".to_owned())?;
                args.extend(["--count".into(), count.to_string()]);
            }
            args
        }
        "caret-select" => optional_text_argument("state")?
            .map(|state| vec![state])
            .unwrap_or_default(),
        "download" => vec![text_argument("input")?],
        "permissions" => optional_text_argument("origin")?
            .map(|origin| vec![origin])
            .unwrap_or_default(),
        "permission-reset" => vec![text_argument("origin")?, text_argument("permission")?],
        "site-doctor" => vec![text_argument("experiment")?],
        "site-doctor-undo" => vec![text_argument("id")?],
        "site-status" => optional_text_argument("tab")?
            .map(|tab| vec!["--tab".into(), tab])
            .unwrap_or_default(),
        "site-data-clear" => {
            let mut args = vec![text_argument("origin")?];
            if arguments
                .and_then(|arguments| arguments.get("confirmed"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--confirm".into());
            }
            args
        }
        "blocking-toggle" => {
            if arguments
                .and_then(|arguments| arguments.get("site"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                vec!["--site".into()]
            } else {
                Vec::new()
            }
        }
        "download-open" | "download-show" | "download-cancel" | "download-pause"
        | "download-resume" | "download-retry" => {
            vec![text_argument("id")?]
        }
        "tab-select" => vec![text_argument("selector")?],
        "tab-focus" | "tab-suspend" | "tab-discard" | "tab-resume" => {
            vec![text_argument("id")?]
        }
        "tab-mute" | "tab-pin" => {
            let mut args = optional_text_argument("id")?
                .map(|id| vec![id])
                .unwrap_or_default();
            if let Some(state) = arguments.and_then(|arguments| arguments.get("state")) {
                let state = state
                    .as_str()
                    .ok_or_else(|| "command argument state must be a string".to_owned())?;
                if !matches!(state, "on" | "off" | "toggle") {
                    return Err("command argument state must be on, off, or toggle".into());
                }
                args.push(state.to_owned());
            }
            args
        }
        "tab-move" => {
            if let Some(context) = optional_text_argument("context")? {
                if arguments
                    .and_then(|arguments| arguments.get("direction"))
                    .is_some()
                {
                    return Err("tab-move context cannot be combined with direction".into());
                }
                let mut args = Vec::with_capacity(3);
                if let Some(id) = optional_text_argument("id")? {
                    args.push(id);
                }
                args.extend(["--context".into(), context]);
                args
            } else {
                let direction = text_argument("direction")?;
                if !matches!(direction.as_str(), "left" | "right") {
                    return Err("command argument direction must be left or right".into());
                }
                let mut args = optional_text_argument("id")?
                    .map(|id| vec![id])
                    .unwrap_or_default();
                args.push(direction);
                args
            }
        }
        "zoom" => vec![text_argument("factor")?],
        "search-next" => {
            let mut args = Vec::new();
            if let Some(direction) = optional_text_argument("direction")? {
                match direction.as_str() {
                    "forward" => {}
                    "backward" => args.push("--backward".into()),
                    _ => {
                        return Err("command argument direction must be forward or backward".into());
                    }
                }
            }
            if let Some(count) = arguments.and_then(|arguments| arguments.get("count")) {
                let count = count
                    .as_u64()
                    .filter(|count| (1..=100).contains(count))
                    .ok_or_else(|| "command argument count must be 1..100".to_owned())?;
                args.extend(["--count".into(), count.to_string()]);
            }
            args
        }
        "scroll" => {
            let direction = text_argument("direction")?;
            if !matches!(direction.as_str(), "up" | "down" | "left" | "right") {
                return Err("command argument direction must be up, down, left, or right".into());
            }
            let mut args = vec![direction];
            if let Some(count) = arguments.and_then(|arguments| arguments.get("count")) {
                let count = count
                    .as_u64()
                    .filter(|count| (1..=9_999).contains(count))
                    .ok_or_else(|| "command argument count must be 1..9999".to_owned())?;
                args.extend(["--count".into(), count.to_string()]);
            }
            args
        }
        "scroll-page" => {
            let direction = text_argument("direction")?;
            if !matches!(direction.as_str(), "up" | "down") {
                return Err("command argument direction must be up or down".into());
            }
            let mut args = vec![direction];
            if arguments
                .and_then(|arguments| arguments.get("half"))
                .is_some_and(|value| value.as_bool().is_none())
            {
                return Err("command argument half must be a boolean".into());
            }
            if arguments
                .and_then(|arguments| arguments.get("half"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--half".into());
            }
            if let Some(count) = arguments.and_then(|arguments| arguments.get("count")) {
                let count = count
                    .as_u64()
                    .filter(|count| (1..=9_999).contains(count))
                    .ok_or_else(|| "command argument count must be 1..9999".to_owned())?;
                args.extend(["--count".into(), count.to_string()]);
            }
            args
        }
        "scroll-to" => {
            let edge = text_argument("edge")?;
            if !matches!(edge.as_str(), "top" | "bottom") {
                return Err("command argument edge must be top or bottom".into());
            }
            vec![edge]
        }
        "window-focus" => vec![text_argument("id")?],
        "window-move" => vec![text_argument("id")?, text_argument("workspace")?],
        "command-help" => vec![text_argument("id")?],
        "command-execute" => {
            let mut args = vec![text_argument("id")?];
            if let Some(command_arguments) =
                arguments.and_then(|arguments| arguments.get("arguments"))
            {
                if !command_arguments.is_object() {
                    return Err("command argument arguments must be an object".into());
                }
                let encoded = serde_json::to_string(command_arguments)
                    .map_err(|error| format!("command arguments could not be encoded: {error}"))?;
                if encoded.len() > 64 * 1024 {
                    return Err("command argument arguments exceed 64 KiB".into());
                }
                args.push(encoded);
            }
            args
        }
        "selection-search" => optional_text_argument("engine")?
            .map(|engine| vec![engine])
            .unwrap_or_default(),
        "spawn" => {
            let arguments = arguments
                .ok_or_else(|| "spawn requires argv or userscript arguments".to_owned())?;
            match (arguments.get("argv"), arguments.get("userscript")) {
                (Some(_), Some(_)) | (None, None) => {
                    return Err("spawn requires exactly one of argv or userscript".into());
                }
                (Some(values), None) => {
                    let values = values
                        .as_array()
                        .ok_or_else(|| "command argument argv must be an array".to_owned())?;
                    if values.is_empty() || values.len() > 256 {
                        return Err("command argument argv must contain 1..256 values".into());
                    }
                    values
                        .iter()
                        .map(|value| {
                            value
                                .as_str()
                                .filter(|value| is_bounded_untrusted_text(value))
                                .map(ToOwned::to_owned)
                                .ok_or_else(|| {
                                    "command argument argv values must be nonempty strings"
                                        .to_owned()
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?
                }
                (None, Some(name)) => {
                    let name = name
                        .as_str()
                        .filter(|value| is_bounded_untrusted_text(value))
                        .ok_or_else(|| {
                            "command argument userscript must be a nonempty string".to_owned()
                        })?;
                    vec!["--userscript".into(), name.into()]
                }
            }
        }
        "script-run" => {
            let name = text_argument("name")?;
            if !is_bounded_untrusted_text(&name) {
                return Err("command argument name must be a nonempty bounded string".into());
            }
            vec![name]
        }
        "jseval" => {
            let script = jseval_script_argument()?;
            let world = optional_text_argument("world")?.unwrap_or_else(|| "isolated".into());
            if !matches!(world.as_str(), "isolated" | "page") {
                return Err("command argument world must be isolated or page".into());
            }
            vec!["--world".into(), world, script]
        }
        "devtools" => match arguments.and_then(|arguments| arguments.get("detach")) {
            None => Vec::new(),
            Some(Value::Bool(true)) => vec!["--detach".into()],
            Some(Value::Bool(false)) => Vec::new(),
            Some(_) => return Err("command argument detach must be a boolean".into()),
        },
        "send" => {
            let target = text_argument("target")?;
            let mut args = vec![target];
            match arguments
                .and_then(|arguments| arguments.get("send_subject"))
                .and_then(Value::as_str)
            {
                Some("selection") => args.push("--selection".into()),
                Some("tab") => args.push("--tab".into()),
                Some("link") => {
                    if let Some(input) =
                        optional_text_argument("input")?.or(optional_text_argument("url")?)
                    {
                        args.push(input);
                    }
                }
                Some("url") => {
                    args.push("--url".into());
                    if let Some(input) =
                        optional_text_argument("input")?.or(optional_text_argument("url")?)
                    {
                        args.push(input);
                    }
                }
                Some(other) => {
                    return Err(format!("command argument send_subject is invalid: {other}").into());
                }
                None => {
                    if arguments
                        .and_then(|arguments| arguments.get("selection"))
                        .is_some_and(|value| value.as_bool() == Some(true))
                    {
                        args.push("--selection".into());
                    } else if let Some(input) =
                        optional_text_argument("input")?.or(optional_text_argument("url")?)
                    {
                        args.push(input);
                    }
                }
            }
            args
        }
        "repeat" => match arguments.and_then(|arguments| arguments.get("count")) {
            None => Vec::new(),
            Some(count) => vec![
                "--count".into(),
                count
                    .as_u64()
                    .filter(|count| (1..=100).contains(count))
                    .ok_or_else(|| "command argument count must be 1..100".to_owned())?
                    .to_string(),
            ],
        },
        "cancel" => arguments
            .and_then(|arguments| arguments.get("operation_id"))
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| is_bounded_untrusted_text(value))
                    .map(|value| vec![value.to_owned()])
                    .ok_or_else(|| {
                        "command argument operation_id must be a nonempty string".to_owned()
                    })
            })
            .transpose()?
            .unwrap_or_default(),
        "macro-record" | "macro-play" => vec![text_argument("register")?],
        "yank" => {
            let source = optional_text_argument("source")?.unwrap_or_else(|| "url".into());
            if !matches!(source.as_str(), "url" | "title" | "selection") {
                return Err("yank source must be url, title, or selection".into());
            }
            if matches!(source.as_str(), "title" | "selection")
                && arguments.is_some_and(|arguments| arguments.contains_key("input"))
            {
                return Err("title and selection copying do not accept an input URL".into());
            }
            if source != "url"
                && arguments
                    .and_then(|arguments| arguments.get("clean"))
                    .and_then(Value::as_bool)
                    == Some(true)
            {
                return Err("title and selection copying do not accept --clean".into());
            }
            let mut args = vec![source];
            if let Some(input) = optional_text_argument("input")? {
                args.push(input);
            }
            if arguments
                .and_then(|arguments| arguments.get("clean"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--clean".into());
            }
            if arguments
                .and_then(|arguments| arguments.get("primary"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--primary".into());
            }
            args
        }
        "paste-open" => {
            let mut args = Vec::new();
            if let Some(target) = optional_text_argument("target")? {
                if !matches!(target.as_str(), "current" | "tab") {
                    return Err("paste-open target must be current or tab".into());
                }
                args.extend(["--target".into(), target]);
            }
            if arguments
                .and_then(|arguments| arguments.get("primary"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--primary".into());
            }
            args
        }
        "action" => {
            let mut args = vec![text_argument("subject")?, text_argument("verb")?];
            let input = optional_text_argument("input")?;
            let url = optional_text_argument("url")?;
            if input.is_some() && url.is_some() {
                return Err("action accepts one typed input or URL argument".into());
            }
            if let Some(value) = input.or(url) {
                args.push(value);
            }
            args
        }
        "action-list" => optional_text_argument("subject")?
            .map(|subject| vec![subject])
            .unwrap_or_default(),
        "session-save" | "session-delete" | "profile-open" | "profile-create"
        | "profile-delete" | "context-create" | "context-delete" | "context-enter" => {
            let mut args = vec![text_argument("name")?];
            if name == "profile-open"
                && let Some(input) = optional_text_argument("input")?
            {
                args.push(input);
            }
            if name == "profile-create"
                && arguments
                    .and_then(|arguments| arguments.get("ephemeral"))
                    .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--ephemeral".into());
            }
            if name == "context-create" {
                if let Some(label) = optional_text_argument("label")? {
                    args.extend(["--label".into(), label]);
                }
                let profile = text_argument("profile")?;
                args.extend(["--profile".into(), profile]);
                if let Some(workspace) = optional_text_argument("workspace")? {
                    args.extend(["--workspace".into(), workspace]);
                }
            }
            if name == "context-delete"
                && arguments
                    .and_then(|arguments| arguments.get("confirmed"))
                    .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--confirm".into());
            }
            args
        }
        "session-load" => {
            let mut args = Vec::new();
            if arguments
                .and_then(|arguments| arguments.get("append"))
                .is_some_and(|value| value.as_bool() == Some(true))
            {
                args.push("--append".into());
            }
            args.push(text_argument("name")?);
            args
        }
        "context-save" => arguments
            .and_then(|arguments| arguments.get("name"))
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| is_bounded_untrusted_text(value))
                    .map(|value| vec![value.to_owned()])
                    .ok_or_else(|| "command argument name must be a nonempty string".to_owned())
            })
            .transpose()?
            .unwrap_or_default(),
        "context-route" => {
            let action = text_argument("action")?;
            if !matches!(action.as_str(), "add" | "remove" | "list") {
                return Err("context-route action must be add, remove, or list".into());
            }
            match action.as_str() {
                "list" => vec![action],
                "add" => {
                    let mut args =
                        vec![action, text_argument("pattern")?, text_argument("context")?];
                    if let Some(priority) =
                        arguments.and_then(|arguments| arguments.get("priority"))
                    {
                        let priority = priority
                            .as_i64()
                            .and_then(|value| i32::try_from(value).ok())
                            .ok_or_else(|| {
                                "command argument priority must be a 32-bit integer".to_owned()
                            })?;
                        args.extend(["--priority".into(), priority.to_string()]);
                    }
                    if let Some(behavior) = optional_text_argument("behavior")? {
                        if !matches!(behavior.as_str(), "prompt" | "suggest") {
                            return Err(
                                "command argument behavior must be prompt or suggest".into()
                            );
                        }
                        args.extend(["--behavior".into(), behavior]);
                    }
                    if let Some(entry_points) =
                        arguments.and_then(|arguments| arguments.get("entry_points"))
                    {
                        let entry_points = entry_points.as_array().ok_or_else(|| {
                            "command argument entry_points must be an array".to_owned()
                        })?;
                        if entry_points.is_empty() || entry_points.len() > 3 {
                            return Err(
                                "command argument entry_points must contain 1..3 values".into()
                            );
                        }
                        let mut seen = std::collections::BTreeSet::new();
                        for entry_point in entry_points {
                            let entry_point = entry_point
                                .as_str()
                                .filter(|value| is_bounded_untrusted_text(value))
                                .ok_or_else(|| {
                                    "command argument entry_points values must be nonempty strings"
                                        .to_owned()
                                })?;
                            if !matches!(
                                entry_point,
                                "external-open" | "explicit-open" | "typed-initial-url"
                            ) {
                                return Err("command argument entry_points contains an unsupported entry point".into());
                            }
                            if !seen.insert(entry_point) {
                                return Err(
                                    "command argument entry_points must not contain duplicates"
                                        .into(),
                                );
                            }
                            args.extend(["--entry-point".into(), entry_point.into()]);
                        }
                    }
                    args
                }
                "remove" => vec![action, text_argument("id")?],
                _ => unreachable!("context-route action was validated"),
            }
        }
        _ => Vec::new(),
    };
    validate_command_argument_envelope(name, arguments)?;
    Ok((
        ParsedCommand {
            name: name.to_owned(),
            arguments: args,
        },
        route,
    ))
}
