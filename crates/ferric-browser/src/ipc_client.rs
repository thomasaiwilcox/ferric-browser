//! Local instance discovery, forwarding, diagnostics, and IPC transport.

use super::{
    BTreeSet, CliError, CliOptions, CommandRegistry, Duration, ErrorCode, HelloResult,
    InstanceError, InstanceLock, InstancePaths, InstanceRuntime, InstanceStart,
    MAX_DIAGNOSTIC_PROFILES, PROTOCOL_MAJOR, PROTOCOL_MINOR, ParseInput, Path, Request, Response,
    RootSpec, StorageRoots, UnixStream, Uuid, command_params, crash_diagnostics,
    ferric_browser_engine_qt, fs, inspect_store, instance_identity, instance_paths, parse_chain,
    read_frame, transient_marker_scan, validate_format, write_frame,
};

pub(super) fn acquire_instance(
    storage_base: Option<&Path>,
    instance_selector: Option<&str>,
) -> Result<InstanceStart, String> {
    let roots = if let Some(base) = storage_base {
        StorageRoots::resolve(RootSpec::Base(base.to_owned()))
    } else {
        StorageRoots::resolve(RootSpec::Xdg)
    }
    .map_err(|error| format!("could not resolve instance roots: {error}"))?;
    roots
        .ensure()
        .map_err(|error| format!("could not prepare instance roots: {error}"))?;
    let base = storage_base.map_or_else(|| roots.data.clone(), Path::to_owned);
    let canonical_base = fs::canonicalize(&base).unwrap_or(base);
    let paths = instance_paths(
        &roots.runtime,
        &canonical_base,
        &instance_identity(instance_selector),
    );
    let lock = match InstanceLock::acquire(&paths) {
        Ok(lock) => lock,
        Err(InstanceError::Busy { .. }) => return Ok(InstanceStart::Existing(paths)),
        Err(error) => return Err(error.to_string()),
    };
    let listener = ferric_browser_ipc::bind_listener(&paths).map_err(|error| error.to_string())?;
    ferric_browser_engine_qt::spawn_ipc_server(listener, Uuid::new_v4().to_string());
    Ok(InstanceStart::Owner(InstanceRuntime { _lock: lock, paths }))
}

pub(super) fn existing_instance_paths(
    storage_base: Option<&Path>,
    instance_selector: Option<&str>,
) -> Result<InstancePaths, String> {
    let roots = if let Some(base) = storage_base {
        StorageRoots::resolve(RootSpec::Base(base.to_owned()))
    } else {
        StorageRoots::resolve(RootSpec::Xdg)
    }
    .map_err(|error| format!("could not resolve instance roots: {error}"))?;
    roots
        .ensure()
        .map_err(|error| format!("could not prepare instance roots: {error}"))?;
    let base = storage_base.map_or_else(|| roots.data.clone(), Path::to_owned);
    let canonical_base = fs::canonicalize(&base).unwrap_or(base);
    Ok(instance_paths(
        &roots.runtime,
        &canonical_base,
        &instance_identity(instance_selector),
    ))
}

pub(super) fn forward_open(
    paths: &InstancePaths,
    input: &str,
    additional_inputs: &[String],
    target: Option<&str>,
    clean_link: bool,
    options: &CliOptions,
) -> Result<(), CliError> {
    let mut arguments = serde_json::json!({"input": input, "external": true});
    if let Some(target) = target {
        arguments["target"] = serde_json::Value::String(target.to_owned());
    }
    if clean_link {
        arguments["clean_link"] = serde_json::Value::Bool(true);
    }
    let mut params = serde_json::json!({
        "command": "open",
        "arguments": arguments
    });
    add_cli_context(&mut params, options, None)?;
    let response = instance_request(
        paths,
        &Request {
            id: format!("open-{}", Uuid::new_v4()),
            method: "command.execute".into(),
            params,
        },
    )?;
    if let Some(error) = response.error {
        return Err(CliError::Protocol(error));
    }
    for (offset, additional) in additional_inputs.iter().enumerate() {
        let mut params = if let Some(target) = target {
            serde_json::json!({
                "command": "open",
                "arguments": {"input": additional, "external": true, "target": target}
            })
        } else {
            serde_json::json!({
                "command": "tab-open",
                "arguments": {"input": additional, "background": false}
            })
        };
        add_cli_context(&mut params, options, None)?;
        let response = instance_request(
            paths,
            &Request {
                id: format!("open-additional-{}", Uuid::new_v4()),
                method: "command.execute".into(),
                params,
            },
        )
        .map_err(|error| error.after_accepted("startup input", offset + 1))?;
        if let Some(error) = response.error {
            return Err(CliError::Protocol(error).after_accepted("startup input", offset + 1));
        }
    }
    println!(
        "Forwarded {} startup input{} to the running Ferric Browser instance",
        additional_inputs.len().saturating_add(1),
        if additional_inputs.is_empty() {
            ""
        } else {
            "s"
        }
    );
    Ok(())
}

pub(super) fn forward_focus(paths: &InstancePaths) -> Result<(), CliError> {
    let response = instance_request(
        paths,
        &Request {
            id: format!("focus-{}", Uuid::new_v4()),
            method: "window.focus".into(),
            params: serde_json::json!({}),
        },
    )?;
    if let Some(error) = response.error {
        return Err(CliError::Protocol(error));
    }
    println!("Focused the running Ferric Browser instance");
    Ok(())
}

pub(super) fn forward_command(
    paths: &InstancePaths,
    text: &str,
    window: Option<&str>,
    options: &CliOptions,
) -> Result<(), CliError> {
    let commands =
        parse_chain(text, ParseInput::Cli).map_err(|error| CliError::command_error(&error))?;
    let commands = CommandRegistry::default_v1()
        .expand_chain(commands)
        .map_err(|error| CliError::command_error(&error))?;
    ferric_browser_engine_qt::reject_external_hint_interaction_chain(&commands).map_err(
        |error| {
            CliError::local(
                ErrorCode::Denied,
                "Use hints from the browser window.",
                error,
            )
        },
    )?;
    // Finish local encoding before sending any request in the chain.
    let registry = CommandRegistry::default_v1();
    let parameters = commands
        .iter()
        .map(|command| {
            let mut params = command_params(command).map_err(|cause| {
                let message = if registry
                    .resolve(&command.name)
                    .is_ok_and(|definition| definition.arguments.is_empty())
                {
                    format!("{} does not accept arguments.", command.name)
                } else {
                    format!(
                        "Invalid arguments for {}. Check :help {} in the browser.",
                        command.name, command.name
                    )
                };
                CliError::local(ErrorCode::InvalidArgument, &message, cause)
            })?;
            add_cli_context(&mut params, options, window)?;
            Ok(params)
        })
        .collect::<Result<Vec<_>, CliError>>()?;
    for (accepted, params) in parameters.into_iter().enumerate() {
        let response = instance_request(
            paths,
            &Request {
                id: format!("command-{}", Uuid::new_v4()),
                method: "command.execute".into(),
                params,
            },
        )
        .map_err(|error| error.after_accepted("command", accepted))?;
        if let Some(error) = response.error {
            return Err(CliError::Protocol(error).after_accepted("command", accepted));
        }
    }
    println!("Command accepted by the running Ferric Browser instance");
    Ok(())
}

pub(super) fn add_cli_context(
    params: &mut serde_json::Value,
    options: &CliOptions,
    window: Option<&str>,
) -> Result<(), CliError> {
    let object = params
        .as_object_mut()
        .ok_or_else(|| CliError::invalid("command envelope is not an object"))?;
    let mut context = match object.get("context") {
        Some(serde_json::Value::Object(context)) => context.clone(),
        None | Some(serde_json::Value::Null) => serde_json::Map::new(),
        Some(_) => return Err(CliError::invalid("command context is not an object")),
    };
    for (key, requested) in [
        ("window", window),
        ("profile", options.profile.as_deref()),
        ("context", options.context.as_deref()),
    ] {
        if let Some(requested) = requested {
            if requested.is_empty()
                || context
                    .get(key)
                    .is_some_and(|value| !value.is_null() && value.as_str() != Some(requested))
            {
                return Err(CliError::local(
                    ErrorCode::InvalidArgument,
                    &format!(
                        "Conflicting or empty {key} selectors. Use one {key} for this request."
                    ),
                    "CLI selector disagrees with the encoded command",
                ));
            }
            context.insert(key.into(), serde_json::Value::String(requested.into()));
        }
    }
    context.insert("source".into(), serde_json::Value::String("cli".into()));
    object.insert("context".into(), serde_json::Value::Object(context));
    Ok(())
}

pub(super) fn forward_query(
    paths: &InstancePaths,
    method: &str,
    params: serde_json::Value,
    format: &str,
) -> Result<(), CliError> {
    let response = instance_request(
        paths,
        &Request {
            id: format!("query-{}", Uuid::new_v4()),
            method: method.into(),
            params,
        },
    )?;
    if let Some(error) = response.error {
        return Err(CliError::Protocol(error));
    }
    print_response_result(response, format).map_err(CliError::from)
}

pub(super) fn forward_activate(
    paths: &InstancePaths,
    kind: &str,
    id: &str,
) -> Result<(), CliError> {
    let response = instance_request(
        paths,
        &Request {
            id: format!("activate-{}", Uuid::new_v4()),
            method: "switcher.activate".into(),
            params: serde_json::json!({"kind": kind, "id": id}),
        },
    )?;
    if let Some(error) = response.error {
        return Err(CliError::Protocol(error));
    }
    print_response_result(response, "json").map_err(CliError::from)
}

pub(super) fn print_response_result(response: Response, format: &str) -> Result<(), String> {
    validate_format(format)?;
    let result = response.result.unwrap_or_else(|| serde_json::json!({}));
    println!(
        "{}",
        serde_json::to_string_pretty(&result)
            .map_err(|error| format!("could not format JSON response: {error}"))?
    );
    Ok(())
}

pub(super) fn print_diagnostics(
    format: &str,
    storage_base: Option<&str>,
    instance_selector: Option<&str>,
    allow_live_instance: bool,
) -> Result<(), CliError> {
    validate_format(format)?;
    if allow_live_instance
        && let Ok(paths) = existing_instance_paths(storage_base.map(Path::new), instance_selector)
        && paths.socket.exists()
    {
        match instance_request(
            &paths,
            &Request {
                id: format!("diagnostics-{}", Uuid::new_v4()),
                method: "diagnostics.get".into(),
                params: serde_json::Value::Null,
            },
        ) {
            Ok(response) => {
                if let Some(error) = response.error {
                    return Err(CliError::Protocol(error));
                }
                return print_response_result(response, format).map_err(CliError::from);
            }
            Err(error) if error.code() == ErrorCode::NoInstance => {}
            Err(error) => return Err(error),
        }
    }
    let mut diagnostics = ferric_browser_engine_qt::diagnostics_snapshot();
    let roots = if let Some(base) = storage_base {
        StorageRoots::resolve(RootSpec::Base(base.into()))
    } else {
        StorageRoots::resolve(RootSpec::Xdg)
    }
    .map_err(|error| format!("could not resolve diagnostics storage roots: {error}"))?;
    diagnostics["crash"] = crash_diagnostics(&roots);
    diagnostics["protocol"] = serde_json::json!({
        "major": PROTOCOL_MAJOR,
        "minor": PROTOCOL_MINOR
    });
    let marker_scan = transient_marker_scan(&roots, "ephemeral-");
    diagnostics["private_state"] = serde_json::json!({
        "status": "memory-only",
        "marker_scan": marker_scan,
        "outside_guarantee": [
            "engine and OS swap",
            "core dumps",
            "downloaded files",
            "clipboard managers",
            "sites and network observers"
        ]
    });
    diagnostics["storage"] = storage_diagnostics(&roots);
    println!(
        "{}",
        serde_json::to_string_pretty(&diagnostics)
            .map_err(|error| format!("could not format diagnostics: {error}"))?
    );
    Ok(())
}

pub(super) fn storage_diagnostics(roots: &StorageRoots) -> serde_json::Value {
    let profiles_dir = roots.data.join("profiles");
    let mut profile_ids: BTreeSet<String> = BTreeSet::new();
    let mut profiles_truncated = false;
    let entries = match fs::read_dir(&profiles_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return serde_json::json!({
                "health": {
                    "status": "not-created",
                    "reason": "no profile database directory exists",
                    "provenance": "observed"
                },
                "profiles": []
            });
        }
        Err(_) => {
            return serde_json::json!({
                "health": {
                    "status": "unavailable",
                    "reason": "profile database directory could not be inspected",
                    "provenance": "observed"
                },
                "profiles": []
            });
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() || path.file_name().and_then(|name| name.to_str()).is_none() {
            continue;
        }
        let Some(profile_id) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if uuid::Uuid::parse_str(profile_id).is_err() {
            continue;
        }
        if profile_ids.len() >= MAX_DIAGNOSTIC_PROFILES {
            profiles_truncated = true;
            if profile_ids
                .last()
                .is_some_and(|last| profile_id < last.as_str())
            {
                profile_ids.pop_last();
                profile_ids.insert(profile_id.to_owned());
            }
            continue;
        }
        profile_ids.insert(profile_id.to_owned());
    }
    let profiles = profile_ids
        .into_iter()
        .map(|profile_id| {
            let inspection = inspect_store(profiles_dir.join(&profile_id).join("browser.sqlite"));
            serde_json::json!({
                "id": profile_id,
                "inspection": inspection
            })
        })
        .collect::<Vec<_>>();
    let status = if profiles
        .iter()
        .any(|profile| profile["inspection"]["status"] == "corrupt")
    {
        "corrupt"
    } else if profiles
        .iter()
        .any(|profile| profile["inspection"]["status"] == "unsupported")
    {
        "unsupported"
    } else if profiles
        .iter()
        .any(|profile| profile["inspection"]["status"] == "unavailable")
    {
        "unavailable"
    } else if profiles
        .iter()
        .any(|profile| profile["inspection"]["status"] == "available")
    {
        "available"
    } else {
        "not-created"
    };
    serde_json::json!({
        "health": {
            "status": status,
            "reason": "profile databases were inspected read-only",
            "provenance": "observed"
        },
        "profiles": profiles,
        "profile_limit": MAX_DIAGNOSTIC_PROFILES,
        "profiles_truncated": profiles_truncated
    })
}

pub(super) fn finish_forward(
    result: Result<(), CliError>,
    temporary_roots: Option<StorageRoots>,
) -> Result<(), CliError> {
    let cleanup = temporary_roots
        .map(StorageRoots::cleanup)
        .transpose()
        .map_err(|error| format!("could not clean temporary base directory: {error}"));
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(CliError::local(
            ErrorCode::Io,
            "The request was accepted, but temporary files couldn't be removed.",
            error,
        )
        .completion_unknown()),
        (Ok(()), Ok(_)) => Ok(()),
    }
}

pub(super) fn instance_request(
    paths: &InstancePaths,
    request: &Request,
) -> Result<Response, CliError> {
    let mut stream = UnixStream::connect(&paths.socket).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
            CliError::no_instance(error.to_string())
        }
        _ => transport_io(error.kind(), error.to_string()),
    })?;
    let timeout = Some(Duration::from_secs(5));
    stream
        .set_read_timeout(timeout)
        .and_then(|()| stream.set_write_timeout(timeout))
        .map_err(|error| transport_io(error.kind(), error.to_string()))?;

    let hello = Request {
        id: format!("hello-{}", std::process::id()),
        method: "hello".into(),
        params: serde_json::json!({
            "protocol_major": PROTOCOL_MAJOR,
            "protocol_minor": PROTOCOL_MINOR,
            "client": "ferric-browser-cli"
        }),
    };
    let hello_response = ipc_roundtrip(&mut stream, &hello)?;
    if let Some(error) = hello_response.error {
        return Err(CliError::Protocol(error));
    }
    let hello_result = hello_response.result.ok_or_else(|| {
        CliError::local(
            ErrorCode::Protocol,
            "The running browser did not accept the connection.",
            "missing handshake result",
        )
    })?;
    let hello_result: HelloResult = serde_json::from_value(hello_result).map_err(|error| {
        CliError::local(
            ErrorCode::Protocol,
            "The running browser sent an invalid handshake.",
            error.to_string(),
        )
    })?;
    if hello_result.protocol_major != PROTOCOL_MAJOR {
        return Err(CliError::local(
            ErrorCode::Protocol,
            "The running browser uses an incompatible protocol. Restart it with the installed version.",
            "protocol major version mismatch",
        ));
    }

    // Live diagnostic collection has a 15-second server deadline. Keep the
    // ordinary handshake/write bounds, but allow its deferred response to
    // arrive before deciding that the running instance is unavailable.
    if request.method == "diagnostics.get" {
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .map_err(|error| transport_io(error.kind(), error.to_string()))?;
    }

    ipc_roundtrip(&mut stream, request)
}

pub(super) fn ipc_roundtrip(
    stream: &mut UnixStream,
    request: &Request,
) -> Result<Response, CliError> {
    let payload = serde_json::to_vec(request).map_err(|error| {
        CliError::local(
            ErrorCode::Protocol,
            "The browser request couldn't be encoded.",
            error.to_string(),
        )
    })?;
    let result = (|| {
        write_frame(stream, &payload).map_err(frame_failure)?;
        let payload = read_frame(stream).map_err(frame_failure)?.ok_or_else(|| {
            CliError::local(
                ErrorCode::Io,
                "The browser closed the connection before replying.",
                "IPC response EOF",
            )
        })?;
        let response: Response = serde_json::from_slice(&payload).map_err(|error| {
            CliError::local(
                ErrorCode::Protocol,
                "The running browser sent an invalid response.",
                error.to_string(),
            )
        })?;
        if response.id != request.id || response.error.is_some() == response.result.is_some() {
            return Err(CliError::local(
                ErrorCode::Protocol,
                "The browser response did not match the request.",
                "mismatched response ID or result/error shape",
            ));
        }
        Ok(response)
    })();
    result.map_err(|error| {
        if request.method == "hello" {
            error
        } else {
            error.completion_unknown()
        }
    })
}

fn frame_failure(error: ferric_browser_ipc::FrameError) -> CliError {
    match error {
        ferric_browser_ipc::FrameError::Io { kind, message } => transport_io(kind, message),
        _ => CliError::local(
            ErrorCode::Protocol,
            "The running browser sent an invalid message.",
            error.to_string(),
        ),
    }
}

fn transport_io(kind: std::io::ErrorKind, context: String) -> CliError {
    match kind {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => CliError::local(
            ErrorCode::Timeout,
            "The running browser did not reply in time.",
            context,
        ),
        std::io::ErrorKind::PermissionDenied => CliError::local(
            ErrorCode::Denied,
            "Access to the browser connection was denied.",
            context,
        ),
        _ => CliError::local(ErrorCode::Io, "The browser connection failed.", context),
    }
}
