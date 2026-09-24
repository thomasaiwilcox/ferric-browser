//! Command-line token parsing and validation.

use super::{
    CliAction, CliOptions, CommandRegistry, DefaultBrowserOperation, MAX_STARTUP_INPUTS,
    MAX_STARTUP_SETTINGS, validate_open_target,
};

pub(super) fn parse_cli(arguments: &[String]) -> Result<(CliOptions, CliAction), String> {
    let mut options = CliOptions::default();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        match argument.as_str() {
            "--temp-basedir" if !options.temp_basedir => options.temp_basedir = true,
            "--temp-basedir" => return Err("--temp-basedir may be specified only once".into()),
            "--ephemeral" if !options.ephemeral => options.ephemeral = true,
            "--ephemeral" => return Err("--ephemeral may be specified only once".into()),
            "--safe-mode" if !options.safe_mode => options.safe_mode = true,
            "--safe-mode" => return Err("--safe-mode may be specified only once".into()),
            "--userscripts-off" if !options.userscripts_off => options.userscripts_off = true,
            "--userscripts-off" => {
                return Err("--userscripts-off may be specified only once".into());
            }
            "--software-rendering" if !options.software_rendering => {
                options.software_rendering = true;
            }
            "--software-rendering" => {
                return Err("--software-rendering may be specified only once".into());
            }
            "--basedir" if options.basedir.is_none() => {
                options.basedir = Some(next_cli_value(arguments, &mut index, argument)?);
            }
            "--basedir" => return Err("--basedir may be specified only once".into()),
            "--config" if options.config_path.is_none() => {
                options.config_path = Some(next_cli_value(arguments, &mut index, argument)?);
            }
            "--config" => return Err("--config may be specified only once".into()),
            "--profile" if options.profile.is_none() => {
                options.profile = Some(next_cli_value(arguments, &mut index, argument)?);
            }
            "--profile" => return Err("--profile may be specified only once".into()),
            "--context" if options.context.is_none() => {
                options.context = Some(next_cli_value(arguments, &mut index, argument)?);
            }
            "--context" => return Err("--context may be specified only once".into()),
            "--instance" if options.instance.is_none() => {
                options.instance = Some(next_cli_value(arguments, &mut index, argument)?);
            }
            "--instance" => return Err("--instance may be specified only once".into()),
            "--set" if options.settings.len() < MAX_STARTUP_SETTINGS => options
                .settings
                .push(next_cli_value(arguments, &mut index, argument)?),
            "--set" => {
                return Err(format!(
                    "--set accepts at most {MAX_STARTUP_SETTINGS} values"
                ));
            }
            "--log-level" if options.log_level.is_none() => {
                options.log_level = Some(next_cli_value(arguments, &mut index, argument)?);
            }
            "--log-level" => return Err("--log-level may be specified only once".into()),
            "--" => {
                let remaining = arguments[index + 1..].to_vec();
                let ephemeral = options.ephemeral;
                let action = open_from_values(&mut options, &remaining, None, ephemeral)?;
                return Ok((options, action));
            }
            value if value.starts_with('-') => return Err(format!("unknown option: {value}")),
            command => {
                let tail = arguments[index + 1..].to_vec();
                return parse_subcommand(&mut options, command, &tail);
            }
        }
        index += 1;
    }
    let ephemeral = options.ephemeral;
    Ok((
        options,
        CliAction::Open {
            input: "about:blank".into(),
            additional_inputs: Vec::new(),
            target: None,
            clean_link: false,
            explicit_input: false,
            ephemeral,
        },
    ))
}

pub(super) fn next_cli_value(
    arguments: &[String],
    index: &mut usize,
    option: &str,
) -> Result<String, String> {
    *index += 1;
    arguments
        .get(*index)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| format!("{option} requires a value"))
}

pub(super) fn parse_subcommand(
    options: &mut CliOptions,
    command: &str,
    tail: &[String],
) -> Result<(CliOptions, CliAction), String> {
    match command {
        "open" => {
            let ephemeral = options.ephemeral;
            let action = open_from_values(options, tail, None, ephemeral)?;
            Ok((std::mem::take(options), action))
        }
        "command" => {
            let (text, window) = command_from_values(tail)?;
            Ok((std::mem::take(options), CliAction::Command { text, window }))
        }
        "query" => {
            let (method, params, format) = query_from_values(tail)?;
            Ok((
                std::mem::take(options),
                CliAction::Query {
                    method,
                    params,
                    format,
                },
            ))
        }
        "activate" => {
            if tail.len() != 2 {
                return Err("activate requires KIND and ID".into());
            }
            let kind = tail[0].clone();
            let id = tail[1].clone();
            validate_cli_text(&kind, "activation kind")?;
            validate_cli_text(&id, "activation ID")?;
            if kind != "tab" {
                return Err("activate currently supports only tab results".into());
            }
            Ok((std::mem::take(options), CliAction::Activate { kind, id }))
        }
        "diagnostics" => Ok((
            std::mem::take(options),
            CliAction::Diagnostics {
                format: format_from_values(tail)?,
            },
        )),
        "default-browser" => {
            let operation = match tail {
                [operation] if operation == "status" => DefaultBrowserOperation::Status,
                [operation] if operation == "set" => DefaultBrowserOperation::Set,
                [] => return Err("default-browser requires status or set".into()),
                _ => return Err("default-browser accepts only status or set".into()),
            };
            Ok((
                std::mem::take(options),
                CliAction::DefaultBrowser { operation },
            ))
        }
        "config" if tail == ["check"] => Ok((std::mem::take(options), CliAction::ConfigCheck)),
        "reset-data" if tail == ["--confirm"] => {
            Ok((std::mem::take(options), CliAction::ResetData))
        }
        "reset-data" => {
            Err("reset-data requires --confirm because it erases Ferric-owned data".into())
        }
        value => {
            if CommandRegistry::default_v1().resolve(value).is_ok() {
                let mut command_arguments = Vec::with_capacity(tail.len() + 1);
                command_arguments.push(value.to_owned());
                command_arguments.extend(tail.iter().cloned());
                let text = command_arguments.join(" ");
                validate_cli_text(&text, "command text")?;
                return Ok((
                    std::mem::take(options),
                    CliAction::Command { text, window: None },
                ));
            }
            if tail.is_empty() {
                Ok((
                    std::mem::take(options),
                    CliAction::Open {
                        input: value.into(),
                        additional_inputs: Vec::new(),
                        target: None,
                        clean_link: false,
                        explicit_input: true,
                        ephemeral: options.ephemeral,
                    },
                ))
            } else {
                Err(format!("unknown command: {value}"))
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn open_from_values(
    options: &mut CliOptions,
    values: &[String],
    target: Option<String>,
    ephemeral_default: bool,
) -> Result<CliAction, String> {
    let mut target = target;
    let mut clean_link = false;
    let mut ephemeral = ephemeral_default;
    let mut input = Vec::new();
    let mut options_ended = false;
    let mut index = 0;
    while index < values.len() {
        let value = &values[index];
        if options_ended {
            input.push(value.clone());
        } else {
            match value.as_str() {
                "--" => options_ended = true,
                "--target" => {
                    index += 1;
                    let value = values
                        .get(index)
                        .cloned()
                        .ok_or_else(|| "--target requires a value".to_owned())?;
                    if target.replace(value).is_some() {
                        return Err("--target may be specified only once".into());
                    }
                }
                "--clean-link" => {
                    if clean_link {
                        return Err("--clean-link may be specified only once".into());
                    }
                    clean_link = true;
                }
                "--ephemeral" => {
                    if ephemeral {
                        return Err("--ephemeral may be specified only once".into());
                    }
                    ephemeral = true;
                }
                option @ ("--profile" | "--context") => {
                    index += 1;
                    let value = values
                        .get(index)
                        .cloned()
                        .ok_or_else(|| format!("{option} requires a value"))?;
                    let destination = if option == "--profile" {
                        &mut options.profile
                    } else {
                        &mut options.context
                    };
                    if destination.replace(value).is_some() {
                        return Err(format!("{option} may be specified only once"));
                    }
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown option: {value}"));
                }
                value => input.push(value.to_owned()),
            }
        }
        index += 1;
    }
    let explicit_input = !input.is_empty();
    if clean_link && !explicit_input {
        return Err("open --clean-link requires an input".into());
    }
    if clean_link && input.len() > 1 {
        return Err("open --clean-link accepts one input".into());
    }
    let additional_inputs = if input.len() > 1 && input.iter().all(|value| is_url_like_input(value))
    {
        if input.len() > MAX_STARTUP_INPUTS {
            return Err(format!(
                "open accepts at most {MAX_STARTUP_INPUTS} URL inputs"
            ));
        }
        input.iter().skip(1).cloned().collect()
    } else {
        Vec::new()
    };
    let input = if explicit_input {
        if additional_inputs.is_empty() {
            input.join(" ")
        } else {
            input[0].clone()
        }
    } else {
        "about:blank".into()
    };
    validate_cli_text(&input, "open input")?;
    for additional in &additional_inputs {
        validate_cli_text(additional, "additional open input")?;
    }
    if let Some(target) = target.as_mut() {
        if target == "current" {
            *target = "tab".into();
        }
        if !matches!(
            target.as_str(),
            "tab" | "tab-bg" | "window" | "private-window"
        ) {
            return Err("--target must be current, tab, tab-bg, window, or private-window".into());
        }
    }
    validate_open_target(target.as_deref(), clean_link).map_err(str::to_owned)?;
    Ok(CliAction::Open {
        input,
        additional_inputs,
        target,
        clean_link,
        explicit_input,
        ephemeral,
    })
}

pub(super) fn log_level_rank(level: &str) -> Option<u8> {
    match level {
        "error" => Some(0),
        "warn" => Some(1),
        "info" => Some(2),
        "debug" => Some(3),
        _ => None,
    }
}

pub(super) fn emit_structured_log(configured_level: Option<&str>, level: &str, event: &str) {
    let Some(configured_rank) = configured_level.and_then(log_level_rank) else {
        return;
    };
    let Some(event_rank) = log_level_rank(level) else {
        return;
    };
    if event_rank > configured_rank {
        return;
    }
    eprintln!(
        "{}",
        serde_json::json!({
            "schema": 1,
            "level": level,
            "event": event,
            "privacy": "redacted",
        })
    );
}

pub(super) fn cli_action_name(action: &CliAction) -> &'static str {
    match action {
        CliAction::Open { .. } => "open",
        CliAction::Command { .. } => "command",
        CliAction::Query { .. } => "query",
        CliAction::Activate { .. } => "activate",
        CliAction::Diagnostics { .. } => "diagnostics",
        CliAction::DefaultBrowser { .. } => "default-browser",
        CliAction::ConfigCheck => "config-check",
        CliAction::ResetData => "reset-data",
    }
}

pub(super) fn is_url_like_input(value: &str) -> bool {
    ["http://", "https://", "file://", "about:"]
        .iter()
        .any(|prefix| {
            value
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        })
}

pub(super) fn command_from_values(values: &[String]) -> Result<(String, Option<String>), String> {
    let mut window = None;
    let mut command = Vec::new();
    let mut after_separator = false;
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--" => after_separator = true,
            "--window" if !after_separator => {
                index += 1;
                window = Some(
                    values
                        .get(index)
                        .cloned()
                        .ok_or_else(|| "--window requires a value".to_owned())?,
                );
            }
            value if value.starts_with('-') && !after_separator => {
                return Err(format!("unknown command option: {value}"));
            }
            value => command.push(value.to_owned()),
        }
        index += 1;
    }
    if command.is_empty() {
        return Err("command requires command text after --".into());
    }
    let text = command.join(" ");
    validate_cli_text(&text, "command text")?;
    Ok((text, window))
}

#[allow(clippy::too_many_lines)]
pub(super) fn query_from_values(
    values: &[String],
) -> Result<(String, serde_json::Value, String), String> {
    let mut kind = None;
    let mut format = "json".to_owned();
    let mut window = None;
    let mut mode = None;
    let mut key = None;
    let mut url = None;
    let mut explain = false;
    let mut include_members = false;
    let mut switcher_scope = None;
    let mut switcher_limit = None;
    let mut include_private = false;
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--format" => {
                index += 1;
                format = values
                    .get(index)
                    .cloned()
                    .ok_or_else(|| "--format requires a value".to_owned())?;
            }
            "--window" => {
                index += 1;
                window = Some(
                    values
                        .get(index)
                        .cloned()
                        .ok_or_else(|| "--window requires a value".to_owned())?,
                );
            }
            "--mode" => {
                index += 1;
                mode = Some(
                    values
                        .get(index)
                        .cloned()
                        .ok_or_else(|| "--mode requires a value".to_owned())?,
                );
            }
            "--url" => {
                index += 1;
                url = Some(
                    values
                        .get(index)
                        .cloned()
                        .ok_or_else(|| "--url requires a value".to_owned())?,
                );
            }
            "--explain" => explain = true,
            "--members" => include_members = true,
            "--private" => include_private = true,
            "--scope" => {
                index += 1;
                switcher_scope = Some(
                    values
                        .get(index)
                        .cloned()
                        .ok_or_else(|| "--scope requires a value".to_owned())?,
                );
            }
            "--limit" => {
                index += 1;
                switcher_limit = Some(
                    values
                        .get(index)
                        .cloned()
                        .ok_or_else(|| "--limit requires a value".to_owned())?,
                );
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown query option: {value}"));
            }
            value => {
                if kind.is_none() {
                    kind = Some(value.to_owned());
                } else if key.is_none() {
                    key = Some(value.to_owned());
                } else {
                    return Err("query accepts one query kind and one key".into());
                }
            }
        }
        index += 1;
    }
    let kind = kind.ok_or_else(|| "query requires a query kind".to_owned())?;
    if kind != "config" && url.is_some() {
        return Err("--url is only supported by config queries".into());
    }
    let (method, params) = match kind.as_str() {
        "tabs" => {
            if mode.is_some() || key.is_some() || explain || include_members {
                return Err("tabs query accepts only --window and --format".into());
            }
            ("tabs.query", serde_json::json!({"window": window}))
        }
        "active-tab" => {
            if mode.is_some() || key.is_some() || explain || include_members {
                return Err("active-tab query accepts only --window and --format".into());
            }
            (
                "tabs.query",
                serde_json::json!({"active_only": true, "window": window}),
            )
        }
        "windows" => {
            if window.is_some()
                || mode.is_some()
                || key.is_some()
                || explain
                || include_members
                || include_private
                || switcher_scope.is_some()
                || switcher_limit.is_some()
            {
                return Err("windows query accepts only --format".into());
            }
            ("windows.query", serde_json::json!({}))
        }
        "operations" => {
            if window.is_some() || mode.is_some() || key.is_some() || explain || include_members {
                return Err("operations query accepts only --format".into());
            }
            ("operations.query", serde_json::json!({}))
        }
        "blocking" => {
            if window.is_some()
                || mode.is_some()
                || key.is_some()
                || explain
                || include_members
                || include_private
            {
                return Err("blocking query accepts only --format".into());
            }
            ("blocking.status", serde_json::json!({}))
        }
        "permissions" => {
            if window.is_some() || mode.is_some() || explain || include_members {
                return Err("permissions query accepts an optional origin and --format".into());
            }
            ("permissions.query", serde_json::json!({"origin": key}))
        }
        "bindings" => {
            if window.is_some() || key.is_some() || explain || include_members {
                return Err("bindings query accepts only --mode and --format".into());
            }
            ("bindings.query", serde_json::json!({"mode": mode}))
        }
        "binding-explain" => {
            if window.is_some()
                || explain
                || include_members
                || include_private
                || switcher_scope.is_some()
                || switcher_limit.is_some()
            {
                return Err(
                    "binding-explain accepts KEYCHAIN, optional --mode, and --format".into(),
                );
            }
            let keychain = key.ok_or_else(|| "binding-explain requires KEYCHAIN".to_owned())?;
            (
                "bindings.explain",
                serde_json::json!({"keychain": keychain, "mode": mode}),
            )
        }
        "contexts" => {
            if window.is_some() || mode.is_some() || key.is_some() || explain {
                return Err("contexts query accepts only --members and --format".into());
            }
            (
                "contexts.query",
                serde_json::json!({"include_members": include_members}),
            )
        }
        "config" => {
            if window.is_some() || mode.is_some() || include_members {
                return Err("config query accepts a key, --url, --explain, and --format".into());
            }
            let key = key.ok_or_else(|| "config query requires a key".to_owned())?;
            (
                "config.get",
                serde_json::json!({"key": key, "url": url, "explain": explain}),
            )
        }
        "switcher" => {
            if window.is_some() || mode.is_some() || explain || include_members {
                return Err(
                    "switcher query accepts a search, --scope, --limit, --private, and --format"
                        .into(),
                );
            }
            let limit = switcher_limit
                .as_deref()
                .map(|limit| {
                    limit
                        .parse::<u64>()
                        .map_err(|_| "switcher --limit must be an integer".to_owned())
                })
                .transpose()?;
            (
                "switcher.query",
                serde_json::json!({
                    "query": key.unwrap_or_default(),
                    "scope": switcher_scope.unwrap_or_else(|| "all".into()),
                    "limit": limit,
                    "include_private": include_private
                }),
            )
        }
        _ => {
            return Err(
                "query supports tabs, active-tab, windows, operations, blocking, permissions, bindings, binding-explain, contexts, switcher, and config".into(),
            );
        }
    };
    validate_format(&format)?;
    Ok((method.into(), params, format))
}

pub(super) fn format_from_values(values: &[String]) -> Result<String, String> {
    let mut format = "json".to_owned();
    let mut index = 0;
    while index < values.len() {
        if values[index] != "--format" {
            return Err(format!("unknown diagnostics option: {}", values[index]));
        }
        index += 1;
        format = values
            .get(index)
            .cloned()
            .ok_or_else(|| "--format requires a value".to_owned())?;
        index += 1;
    }
    validate_format(&format)?;
    Ok(format)
}

pub(super) fn validate_format(format: &str) -> Result<(), String> {
    if format == "json" {
        Ok(())
    } else {
        Err("only --format json is currently supported".into())
    }
}

pub(super) fn validate_cli_text(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().any(char::is_control) {
        Err(format!(
            "{label} must be nonempty and contain no control characters"
        ))
    } else {
        Ok(())
    }
}
