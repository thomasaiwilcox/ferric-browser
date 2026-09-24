//! Local instance discovery, forwarding, diagnostics, and IPC transport.

use super::{
    BTreeSet, CliOptions, CommandRegistry, Duration, HelloResult, InstanceError, InstanceLock,
    InstancePaths, InstanceRuntime, InstanceStart, MAX_DIAGNOSTIC_PROFILES, PROTOCOL_MAJOR,
    PROTOCOL_MINOR, ParseInput, Path, Request, Response, RootSpec, StorageRoots, UnixStream, Uuid,
    command_params, crash_diagnostics, ferric_browser_engine_qt, fs, inspect_store,
    instance_identity, instance_paths, parse_chain, read_frame, transient_marker_scan,
    validate_format, write_frame,
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
) -> Result<(), String> {
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
    add_cli_context(&mut params, options, None);
    let response = instance_request(
        paths,
        &Request {
            id: format!("open-{}", Uuid::new_v4()),
            method: "command.execute".into(),
            params,
        },
    )?;
    if let Some(error) = response.error {
        return Err(format!(
            "instance rejected startup input ({}): {}",
            error.code, error.message
        ));
    }
    for additional in additional_inputs {
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
        add_cli_context(&mut params, options, None);
        let response = instance_request(
            paths,
            &Request {
                id: format!("open-additional-{}", Uuid::new_v4()),
                method: "command.execute".into(),
                params,
            },
        )?;
        if let Some(error) = response.error {
            return Err(format!(
                "additional startup input was rejected ({}): {}",
                error.code, error.message
            ));
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

pub(super) fn forward_focus(paths: &InstancePaths) -> Result<(), String> {
    let response = instance_request(
        paths,
        &Request {
            id: format!("focus-{}", Uuid::new_v4()),
            method: "window.focus".into(),
            params: serde_json::json!({}),
        },
    )?;
    if let Some(error) = response.error {
        return Err(format!(
            "focus request failed ({}): {}",
            error.code, error.message
        ));
    }
    println!("Focused the running Ferric Browser instance");
    Ok(())
}

pub(super) fn forward_command(
    paths: &InstancePaths,
    text: &str,
    window: Option<&str>,
    options: &CliOptions,
) -> Result<(), String> {
    let commands = parse_chain(text, ParseInput::Cli).map_err(|error| error.to_string())?;
    let commands = CommandRegistry::default_v1()
        .expand_chain(commands)
        .map_err(|error| error.to_string())?;
    for command in commands {
        let name = command.name.clone();
        let mut params = command_params(&command)?;
        add_cli_context(&mut params, options, window);
        let response = instance_request(
            paths,
            &Request {
                id: format!("command-{}", Uuid::new_v4()),
                method: "command.execute".into(),
                params,
            },
        )?;
        if let Some(error) = response.error {
            return Err(format!(
                "command {name} failed ({}): {}",
                error.code, error.message
            ));
        }
    }
    println!("Command accepted by the running Ferric Browser instance");
    Ok(())
}

pub(super) fn add_cli_context(
    params: &mut serde_json::Value,
    options: &CliOptions,
    window: Option<&str>,
) {
    let mut context = serde_json::Map::new();
    context.insert("source".into(), serde_json::Value::String("cli".into()));
    if let Some(window) = window {
        context.insert("window".into(), serde_json::Value::String(window.into()));
    }
    if let Some(profile) = options.profile.as_deref() {
        context.insert("profile".into(), serde_json::Value::String(profile.into()));
    }
    if let Some(context_name) = options.context.as_deref() {
        context.insert(
            "context".into(),
            serde_json::Value::String(context_name.into()),
        );
    }
    if let Some(object) = params.as_object_mut()
        && !context.is_empty()
    {
        object.insert("context".into(), serde_json::Value::Object(context));
    }
}

pub(super) fn forward_query(
    paths: &InstancePaths,
    method: &str,
    params: serde_json::Value,
    format: &str,
) -> Result<(), String> {
    let response = instance_request(
        paths,
        &Request {
            id: format!("query-{}", Uuid::new_v4()),
            method: method.into(),
            params,
        },
    )?;
    if let Some(error) = response.error {
        return Err(format!("query failed ({}): {}", error.code, error.message));
    }
    print_response_result(response, format)
}

pub(super) fn forward_activate(paths: &InstancePaths, kind: &str, id: &str) -> Result<(), String> {
    let response = instance_request(
        paths,
        &Request {
            id: format!("activate-{}", Uuid::new_v4()),
            method: "switcher.activate".into(),
            params: serde_json::json!({"kind": kind, "id": id}),
        },
    )?;
    if let Some(error) = response.error {
        return Err(format!(
            "activation failed ({}): {}",
            error.code, error.message
        ));
    }
    print_response_result(response, "json")
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
) -> Result<(), String> {
    validate_format(format)?;
    if allow_live_instance
        && let Ok(paths) = existing_instance_paths(storage_base.map(Path::new), instance_selector)
        && paths.socket.exists()
        && let Ok(response) = instance_request(
            &paths,
            &Request {
                id: format!("diagnostics-{}", Uuid::new_v4()),
                method: "diagnostics.get".into(),
                params: serde_json::Value::Null,
            },
        )
    {
        if let Some(error) = response.error {
            return Err(format!(
                "running instance diagnostics failed ({}): {}",
                error.code, error.message
            ));
        }
        return print_response_result(response, format);
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
    result: Result<(), String>,
    temporary_roots: Option<StorageRoots>,
) -> Result<(), String> {
    let cleanup = temporary_roots
        .map(StorageRoots::cleanup)
        .transpose()
        .map_err(|error| format!("could not clean temporary base directory: {error}"));
    match (result, cleanup) {
        (Err(error), _) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(_)) => Ok(()),
    }
}

pub(super) fn instance_request(
    paths: &InstancePaths,
    request: &Request,
) -> Result<Response, String> {
    let mut stream = UnixStream::connect(&paths.socket)
        .map_err(|error| format!("could not connect to running instance: {error}"))?;
    let timeout = Some(Duration::from_secs(5));
    stream
        .set_read_timeout(timeout)
        .and_then(|()| stream.set_write_timeout(timeout))
        .map_err(|error| format!("could not configure IPC timeout: {error}"))?;

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
        return Err(format!(
            "running instance rejected handshake ({}): {}",
            error.code, error.message
        ));
    }
    let hello_result = hello_response
        .result
        .ok_or_else(|| "running instance rejected IPC handshake".to_owned())?;
    let hello_result: HelloResult = serde_json::from_value(hello_result)
        .map_err(|error| format!("running instance returned an invalid handshake: {error}"))?;
    if hello_result.protocol_major != PROTOCOL_MAJOR {
        return Err("running instance uses an incompatible IPC protocol".into());
    }

    ipc_roundtrip(&mut stream, request)
}

pub(super) fn ipc_roundtrip(
    stream: &mut UnixStream,
    request: &Request,
) -> Result<Response, String> {
    let payload = serde_json::to_vec(request)
        .map_err(|error| format!("could not encode IPC request: {error}"))?;
    write_frame(stream, &payload).map_err(|error| error.to_string())?;
    let payload = read_frame(stream)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "running instance closed the IPC connection".to_owned())?;
    serde_json::from_slice(&payload).map_err(|error| format!("invalid IPC response: {error}"))
}
