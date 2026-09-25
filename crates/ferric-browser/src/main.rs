extern crate ferric_browser_engine_qt;

mod cli_parse;
mod ipc_client;

use cli_parse::{cli_action_name, emit_structured_log, parse_cli, validate_format};
use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QString, QStringList,
    QUrl, QVariant,
};
use ferric_browser_config::{
    Config, ContextsConfig, ProfilesConfig, RuntimeOverrides, apply_runtime_overrides, load,
    load_contexts, load_profiles, load_runtime_overrides, profile_override_layer,
};
#[cfg(test)]
use ferric_browser_core::ParsedCommand;
use ferric_browser_core::{CommandRegistry, ParseInput, parse_chain, validate_open_target};
use ferric_browser_ipc::{
    ErrorCode, HelloResult, InstanceError, InstanceLock, InstancePaths, PROTOCOL_MAJOR,
    PROTOCOL_MINOR, Request, Response, current_uid, encode_command as command_params,
    instance_paths, read_frame, write_frame,
};
use ferric_browser_storage::{
    CrashMarker, RootSpec, StorageRoots, crash_diagnostics, inspect_store, transient_marker_scan,
};
use ipc_client::{
    acquire_instance, existing_instance_paths, finish_forward, forward_activate, forward_command,
    forward_focus, forward_open, forward_query, print_diagnostics,
};
use std::{
    collections::BTreeSet,
    fs,
    os::unix::net::UnixStream,
    path::Path,
    process::{Command as ProcessCommand, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

struct InstanceRuntime {
    _lock: InstanceLock,
    paths: InstancePaths,
}

enum InstanceStart {
    Owner(InstanceRuntime),
    Existing(InstancePaths),
}

const DISABLING_ENGINE_FLAGS: [&str; 3] =
    ["--no-sandbox", "--single-process", "--disable-web-security"];
#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
struct CliOptions {
    temp_basedir: bool,
    ephemeral: bool,
    safe_mode: bool,
    userscripts_off: bool,
    software_rendering: bool,
    basedir: Option<String>,
    config_path: Option<String>,
    profile: Option<String>,
    context: Option<String>,
    instance: Option<String>,
    settings: Vec<String>,
    log_level: Option<String>,
}

#[derive(Debug)]
enum CliAction {
    Open {
        input: String,
        additional_inputs: Vec<String>,
        target: Option<String>,
        clean_link: bool,
        explicit_input: bool,
        ephemeral: bool,
    },
    Command {
        text: String,
        window: Option<String>,
    },
    Query {
        method: String,
        params: serde_json::Value,
        format: String,
    },
    Activate {
        kind: String,
        id: String,
    },
    Diagnostics {
        format: String,
    },
    ConfigCheck,
    ResetData,
    DefaultBrowser {
        operation: DefaultBrowserOperation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefaultBrowserOperation {
    Status,
    Set,
}

const DESKTOP_ENTRY_ID: &str = "io.github.ferricbrowser.FerricBrowser.desktop";
const MAX_STARTUP_INPUTS: usize = 32;
const MAX_DIAGNOSTIC_PROFILES: usize = 128;
const MAX_STARTUP_SETTINGS: usize = 256;

/// A CLI-visible failure with a stable public code.
///
/// The diagnostic context is intentionally kept separate from the message
/// presented to users and IPC callers.  The CLI must not reconstruct a code
/// by searching the diagnostic prose.
#[derive(Debug)]
struct CliError {
    code: ErrorCode,
    user_message: &'static str,
    // Kept separate from the public message so future local diagnostics can
    // retain cause context without turning that prose into a public protocol.
    #[allow(dead_code)]
    diagnostic_context: String,
}

impl CliError {
    fn invalid(diagnostic_context: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidArgument,
            user_message: "The command-line request is invalid.",
            diagnostic_context: diagnostic_context.into(),
        }
    }

    fn engine(diagnostic_context: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Engine,
            user_message: "Ferric Browser could not complete that request.",
            diagnostic_context: diagnostic_context.into(),
        }
    }

    fn from_run(error: RunError) -> Self {
        match error {
            RunError::Config(context) => Self {
                code: ErrorCode::Config,
                user_message: "Ferric Browser configuration is invalid.",
                diagnostic_context: context,
            },
            RunError::Storage(context) => Self {
                code: ErrorCode::Storage,
                user_message: "Ferric Browser storage is unavailable.",
                diagnostic_context: context,
            },
            RunError::NoInstance(context) => Self {
                code: ErrorCode::NoInstance,
                user_message: "No running Ferric Browser instance was found.",
                diagnostic_context: context,
            },
            RunError::Engine(context) => Self::engine(context),
        }
    }

    const fn status(&self) -> i32 {
        match self.code {
            ErrorCode::InvalidArgument | ErrorCode::Config => 2,
            ErrorCode::NotFound | ErrorCode::NoInstance | ErrorCode::StaleTarget => 3,
            ErrorCode::Denied => 4,
            ErrorCode::Unsupported => 5,
            ErrorCode::Busy | ErrorCode::Timeout => 6,
            ErrorCode::Protocol => 7,
            ErrorCode::ConfirmationRequired
            | ErrorCode::Io
            | ErrorCode::Storage
            | ErrorCode::Engine
            | ErrorCode::Cancelled => 1,
        }
    }
}

#[derive(Debug)]
enum RunError {
    Config(String),
    Storage(String),
    NoInstance(String),
    Engine(String),
}

impl RunError {
    fn config(context: impl Into<String>) -> Self {
        Self::Config(context.into())
    }

    fn storage(context: impl Into<String>) -> Self {
        Self::Storage(context.into())
    }

    fn no_instance(context: impl Into<String>) -> Self {
        Self::NoInstance(context.into())
    }
}

impl From<String> for RunError {
    fn from(context: String) -> Self {
        Self::Engine(context)
    }
}

impl From<&str> for RunError {
    fn from(context: &str) -> Self {
        Self::Engine(context.into())
    }
}

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if let Err(error) = run(&arguments) {
        let code = error.code;
        let status = error.status();
        let correlation_id = format!("err-{}", Uuid::new_v4());
        if wants_structured_error(&arguments) {
            eprintln!(
                "{}",
                structured_cli_error_with_id(code, status, error.user_message, &correlation_id)
            );
        } else {
            let (preserved, next_action) = code.guidance();
            eprintln!(
                "ferric-browser: {}: {} Preserved: {preserved}. Next action: {next_action}. [correlation {correlation_id}]",
                code.as_str(),
                error.user_message,
            );
        }
        emit_structured_log(None, "error", &format!("cli.failure: {}", code.as_str()));
        std::process::exit(status);
    }
}

fn wants_structured_error(arguments: &[String]) -> bool {
    arguments
        .windows(2)
        .any(|window| window[0] == "--format" && window[1] == "json")
}

fn structured_cli_error_with_id(
    code: ErrorCode,
    status: i32,
    message: &str,
    correlation_id: &str,
) -> String {
    let (preserved, next_action) = code.guidance();
    serde_json::json!({
        "error": {
            "code": code.as_str(),
            "status": status,
            "message": message,
            "correlation_id": correlation_id,
            "preserved": preserved,
            "next_action": next_action,
        }
    })
    .to_string()
}

fn select_startup_profile(
    options: &CliOptions,
    profiles: &ProfilesConfig,
    contexts: &ContextsConfig,
    default_name: &str,
    default_label: &str,
) -> Result<(RuntimeOverrides, String, String, Option<String>), String> {
    let context = options.context.as_deref().map(|name| {
        contexts
            .contexts
            .iter()
            .find(|context| context.name == name)
            .ok_or_else(|| format!("context not found: {name}"))
    });
    let context = match context {
        Some(context) => Some(context?),
        None => None,
    };
    if let (Some(profile), Some(context)) = (options.profile.as_deref(), context)
        && profile != context.profile
    {
        return Err(format!(
            "profile and context selectors disagree (context {} belongs to {})",
            context.name, context.profile
        ));
    }
    let name = options
        .profile
        .as_deref()
        .or_else(|| context.map(|context| context.profile.as_str()))
        .unwrap_or(default_name);
    let definition = profiles
        .profiles
        .iter()
        .find(|profile| profile.name == name);
    let overrides = definition
        .map(profile_override_layer)
        .transpose()
        .map_err(|error| format!("profile overrides are invalid: {error}"))?
        .unwrap_or_default();
    let label = definition.map_or_else(
        || {
            if name == default_name {
                default_label.to_owned()
            } else {
                name.to_owned()
            }
        },
        |profile| profile.label.clone(),
    );
    Ok((
        overrides,
        name.to_owned(),
        label,
        context.map(|context| context.name.clone()),
    ))
}

fn run(arguments: &[String]) -> Result<(), CliError> {
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_help();
        return Ok(());
    }
    if arguments
        .iter()
        .any(|argument| argument == "--version" || argument == "-V")
    {
        println!("ferric-browser {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let request = parse_cli(arguments).map_err(CliError::invalid)?;
    validate_cli_request(&request.0, &request.1).map_err(CliError::invalid)?;
    run_parsed(request).map_err(CliError::from_run)
}

fn validate_cli_request(options: &CliOptions, action: &CliAction) -> Result<(), String> {
    if options.safe_mode
        && (options.basedir.is_some()
            || options.config_path.is_some()
            || options.profile.is_some()
            || options.context.is_some()
            || !options.settings.is_empty())
    {
        return Err(
            "invalid --safe-mode combination: it uses a temporary profile and built-in configuration; remove --basedir, --config, --profile, --context, and --set"
                .into(),
        );
    }
    if options.safe_mode && options.userscripts_off {
        return Err("invalid recovery-mode combination: --safe-mode already disables userscripts and uses a disposable profile".into());
    }
    if options.safe_mode && !matches!(&action, CliAction::Open { .. }) {
        return Err("invalid --safe-mode use: it only applies to GUI startup".into());
    }
    if options.userscripts_off && !matches!(&action, CliAction::Open { .. }) {
        return Err("invalid --userscripts-off use: it only applies to GUI startup".into());
    }
    if options.software_rendering && !matches!(&action, CliAction::Open { .. }) {
        return Err("invalid --software-rendering use: it only applies to GUI startup".into());
    }
    if options.temp_basedir && options.basedir.is_some() {
        return Err("--basedir and --temp-basedir are mutually exclusive".into());
    }
    if let Some(path) = options.basedir.as_ref() {
        StorageRoots::resolve(RootSpec::Base(std::path::PathBuf::from(path)))
            .map_err(|error| format!("invalid --basedir: {error}"))?;
    }

    if let Some(instance) = options.instance.as_deref()
        && (instance.is_empty() || instance.len() > 128)
    {
        return Err("--instance must be a nonempty identifier of at most 128 bytes".into());
    }
    if let Some(level) = options.log_level.as_deref()
        && !matches!(level, "error" | "warn" | "info" | "debug")
    {
        return Err("--log-level must be one of error, warn, info, or debug".into());
    }
    for setting in &options.settings {
        if setting.is_empty() || !setting.contains('=') || setting.chars().any(char::is_control) {
            return Err("--set requires KEY=VALUE without control characters".into());
        }
    }
    let rejects_startup_options = matches!(action, CliAction::ResetData)
        || matches!(action, CliAction::DefaultBrowser { .. });
    if rejects_startup_options
        && (options.temp_basedir
            || options.ephemeral
            || options.safe_mode
            || options.userscripts_off
            || options.software_rendering
            || options.basedir.is_some()
            || options.config_path.is_some()
            || options.profile.is_some()
            || options.context.is_some()
            || options.instance.is_some()
            || !options.settings.is_empty()
            || options.log_level.is_some())
    {
        return Err(match action {
            CliAction::ResetData => {
                "reset-data does not accept browser startup or profile options".into()
            }
            CliAction::DefaultBrowser { .. } => {
                "default-browser does not accept browser startup or profile options".into()
            }
            _ => unreachable!("startup-option rejection was guarded above"),
        });
    }
    if options.ephemeral && !matches!(action, CliAction::Open { .. }) {
        return Err("--ephemeral only applies to GUI open".into());
    }
    let private_initial = matches!(
        action,
        CliAction::Open {
            target: Some(target),
            ..
        } if target == "private-window"
    );
    let ephemeral_open = matches!(
        action,
        CliAction::Open {
            ephemeral: true,
            ..
        }
    );
    if private_initial && ephemeral_open {
        return Err("--target private-window and --ephemeral are mutually exclusive".into());
    }
    if private_initial && (options.profile.is_some() || options.context.is_some()) {
        return Err("private-window cannot select a durable profile or context".into());
    }
    if options.context.is_some() && (options.temp_basedir || ephemeral_open) {
        return Err("durable contexts require a non-temporary profile".into());
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn run_parsed((options, action): (CliOptions, CliAction)) -> Result<(), RunError> {
    emit_structured_log(
        options.log_level.as_deref(),
        "debug",
        cli_action_name(&action),
    );
    let config_path = if options.safe_mode {
        None
    } else {
        options
            .config_path
            .clone()
            .or_else(|| default_config_path(&options))
    };
    if let CliAction::ConfigCheck = action {
        check_config(config_path.as_deref()).map_err(RunError::config)?;
        return Ok(());
    }
    if let CliAction::ResetData = action {
        let roots = StorageRoots::resolve(RootSpec::Xdg).map_err(|error| {
            RunError::storage(format!("could not resolve Ferric storage roots: {error}"))
        })?;
        let report = roots
            .reset_owned_contents()
            .map_err(|error| RunError::storage(format!("could not reset Ferric data: {error}")))?;
        roots.initialize_current_schema().map_err(|error| {
            RunError::storage(format!("could not initialize Ferric data: {error}"))
        })?;
        println!(
            "Reset Ferric Browser data: cleared {} roots and removed {} entries. Legacy browser data was not modified.",
            report.roots_cleared, report.entries_removed
        );
        return Ok(());
    }
    if let CliAction::DefaultBrowser { operation } = &action {
        return run_default_browser_operation(*operation).map_err(RunError::from);
    }
    let temporary_storage = options.temp_basedir
        || options.safe_mode
        || matches!(
            &action,
            CliAction::Open {
                ephemeral: true,
                ..
            }
        )
        || matches!(
            &action,
            CliAction::Open {
                target: Some(target),
                ..
            } if target == "private-window"
        );
    if !temporary_storage {
        let roots = if let Some(base) = options.basedir.as_deref() {
            StorageRoots::resolve(RootSpec::Base(base.into()))
        } else {
            StorageRoots::resolve(RootSpec::Xdg)
        }
        .map_err(|error| {
            RunError::storage(format!("could not resolve Ferric storage roots: {error}"))
        })?;
        if roots.requires_schema_reset().map_err(|error| {
            format!("could not inspect Ferric data for the clean-break schema: {error}")
        })? {
            return Err(
                "existing Ferric Browser data is incompatible with this pre-alpha release; run `ferric-browser reset-data --confirm` to erase Ferric-owned data before starting. Legacy browser data will not be touched"
                    .into(),
            );
        }
        roots.initialize_current_schema().map_err(|error| {
            RunError::storage(format!("could not initialize Ferric data schema: {error}"))
        })?;
    }
    let (
        mut config,
        config_source,
        contexts_json,
        profile_overrides,
        normal_profile_name,
        normal_profile_label,
        profiles,
    ) = if let Some(path) = config_path.as_deref() {
        let loaded = load(path)
            .map_err(|error| RunError::config(format!("configuration is invalid: {error}")))?;
        let config_directory = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
        let contexts_path = config_directory.join("contexts.toml");
        let contexts = if contexts_path.exists() {
            load_contexts(&contexts_path).map_err(|error| {
                RunError::config(format!("contexts configuration is invalid: {error}"))
            })?
        } else {
            ContextsConfig::default()
        };
        let profiles_path = config_directory.join("profiles.toml");
        let profiles = if profiles_path.exists() {
            load_profiles(&profiles_path).map_err(|error| {
                RunError::config(format!("profiles configuration is invalid: {error}"))
            })?
        } else {
            ProfilesConfig::default()
        };
        let (profile_overrides, profile_name, profile_label) =
            if let Some(profile) = profiles.default_profile() {
                (
                    profile_override_layer(profile).map_err(|error| {
                        RunError::config(format!("profile overrides are invalid: {error}"))
                    })?,
                    profile.name.clone(),
                    profile.label.clone(),
                )
            } else {
                (
                    RuntimeOverrides::default(),
                    "default".into(),
                    "Default".into(),
                )
            };
        (
            loaded.config,
            "user",
            serde_json::to_string(&contexts).map_err(|error| {
                RunError::config(format!(
                    "could not serialize contexts configuration: {error}"
                ))
            })?,
            profile_overrides,
            profile_name,
            profile_label,
            profiles,
        )
    } else {
        (
            Config::default(),
            "built-in",
            serde_json::to_string(&ContextsConfig::default()).map_err(|error| {
                RunError::config(format!("could not serialize default contexts: {error}"))
            })?,
            RuntimeOverrides::default(),
            "default".into(),
            "Default".into(),
            ProfilesConfig::default(),
        )
    };
    if let CliAction::Diagnostics { format } = action {
        return print_diagnostics(
            &format,
            options.basedir.as_deref(),
            options.instance.as_deref(),
            !options.temp_basedir,
        )
        .map_err(RunError::from);
    }

    let (profile_overrides, normal_profile_name, normal_profile_label, startup_context) =
        if matches!(&action, CliAction::Open { .. }) {
            select_startup_profile(
                &options,
                &profiles,
                &serde_json::from_str(&contexts_json).map_err(|error| {
                    RunError::config(format!("contexts configuration is invalid: {error}"))
                })?,
                &normal_profile_name,
                &normal_profile_label,
            )
            .map_err(RunError::config)?
        } else {
            (
                profile_overrides,
                normal_profile_name,
                normal_profile_label,
                None,
            )
        };
    let base_config_json = serde_json::to_string(&config).map_err(|error| {
        RunError::config(format!("could not serialize base configuration: {error}"))
    })?;
    config = apply_runtime_overrides(&config, &profile_overrides)
        .map_err(|error| RunError::config(format!("profile overrides are invalid: {error}")))?;

    let ephemeral_profile = match &action {
        CliAction::Open { ephemeral, .. } => *ephemeral,
        _ => false,
    };
    let private_initial = matches!(
        &action,
        CliAction::Open {
            target: Some(target),
            ..
        } if target == "private-window"
    );
    let temporary_profile =
        options.temp_basedir || options.safe_mode || ephemeral_profile || private_initial;
    let (ephemeral_profile_name, ephemeral_profile_label) = if ephemeral_profile {
        let id = Uuid::new_v4().simple().to_string();
        let short_id = &id[..8];
        (format!("ephemeral-{id}"), format!("Ephemeral · {short_id}"))
    } else if private_initial {
        ("private".into(), "Private".into())
    } else {
        (normal_profile_name, normal_profile_label)
    };
    let (storage_base, temporary_roots) = resolve_storage_base(
        temporary_profile,
        if options.safe_mode {
            None
        } else {
            options.basedir.clone()
        },
    )
    .map_err(RunError::storage)?;
    let runtime_roots = if let Some(base) = storage_base.as_deref() {
        StorageRoots::resolve(RootSpec::Base(base.to_owned()))
    } else {
        StorageRoots::resolve(RootSpec::Xdg)
    }
    .map_err(|error| {
        RunError::storage(format!("could not resolve runtime override roots: {error}"))
    })?;
    let runtime_overrides = if temporary_profile {
        RuntimeOverrides::default()
    } else {
        load_runtime_overrides(runtime_roots.state.join("runtime-overrides.toml"))
            .map_err(|error| RunError::config(format!("runtime overrides are invalid: {error}")))?
    };
    let mut cli_overrides = RuntimeOverrides::default();
    for setting in &options.settings {
        let (key, literal) = setting
            .split_once('=')
            .ok_or_else(|| "--set requires KEY=VALUE without control characters".to_owned())?;
        cli_overrides.set_literal(key, literal).map_err(|error| {
            RunError::config(format!("invalid runtime setting {key:?}: {error}"))
        })?;
    }
    config = apply_runtime_overrides(&config, &runtime_overrides)
        .map_err(|error| RunError::config(format!("runtime overrides are invalid: {error}")))?;
    config = apply_runtime_overrides(&config, &cli_overrides)
        .map_err(|error| RunError::config(format!("CLI runtime overrides are invalid: {error}")))?;
    let config_json = serde_json::to_string(&config)
        .map_err(|error| RunError::config(format!("could not serialize configuration: {error}")))?;
    let cli_overrides_json = serde_json::to_string(&cli_overrides)
        .map_err(|error| RunError::config(format!("could not serialize CLI overrides: {error}")))?;
    let profile_overrides_json = serde_json::to_string(&profile_overrides).map_err(|error| {
        RunError::config(format!("could not serialize profile overrides: {error}"))
    })?;
    if let CliAction::Open {
        input,
        additional_inputs,
        target,
        clean_link,
        explicit_input,
        ..
    } = action
    {
        let instance = match acquire_instance(storage_base.as_deref(), options.instance.as_deref())
            .map_err(RunError::storage)?
        {
            InstanceStart::Owner(instance) => instance,
            InstanceStart::Existing(paths) => {
                if options.software_rendering {
                    return finish_forward(
                        Err("software-rendering startup requested but another instance is already running".into()),
                        temporary_roots,
                    )
                    .map_err(RunError::from);
                }
                if !explicit_input {
                    let result = if ephemeral_profile {
                        Err("ephemeral open requires a fresh GUI owner; the running instance was not changed".into())
                    } else {
                        forward_focus(&paths)
                    };
                    return finish_forward(result, temporary_roots).map_err(RunError::from);
                }
                let result = if ephemeral_profile {
                    Err("ephemeral open requires a fresh GUI owner; the running instance was not changed".into())
                } else {
                    forward_open(
                        &paths,
                        &input,
                        &additional_inputs,
                        target.as_deref(),
                        clean_link,
                        &options,
                    )
                };
                return finish_forward(result, temporary_roots).map_err(RunError::from);
            }
        };
        if options.software_rendering {
            ferric_browser_engine_qt::enable_software_rendering();
        }
        if let Err(error) = validate_engine_security_environment() {
            drop(instance);
            return finish_forward(Err(error), temporary_roots).map_err(RunError::from);
        }
        let startup_background = target.as_deref() == Some("tab-bg");
        let startup_url = if startup_background {
            "about:blank"
        } else {
            input.as_str()
        };
        let startup_additional_inputs = if startup_background {
            let mut inputs = Vec::with_capacity(additional_inputs.len() + 1);
            inputs.push(input.clone());
            inputs.extend(additional_inputs);
            inputs
        } else {
            additional_inputs
        };
        let marker = match begin_marker(storage_base.as_deref()) {
            Ok(marker) => marker,
            Err(error) => {
                if let Some(roots) = temporary_roots {
                    let _ = roots.cleanup();
                }
                return Err(RunError::storage(format!(
                    "could not create crash marker: {error}"
                )));
            }
        };
        let recovery_available =
            !options.safe_mode && !ephemeral_profile && marker.previous_unclean();
        let initial_entry_point = if explicit_input {
            "external-open"
        } else {
            "typed-initial-url"
        };
        emit_structured_log(options.log_level.as_deref(), "info", "browser.start");
        let result = run_gui(&GuiLaunchPlan {
            initial_url: startup_url,
            additional_inputs: &startup_additional_inputs,
            initial_entry_point,
            temporary_profile,
            ephemeral_profile,
            safe_mode: options.safe_mode,
            userscripts_off: options.userscripts_off,
            software_rendering: options.software_rendering,
            instance_lock_path: &instance.paths.lock,
            instance_selector: options.instance.as_deref(),
            storage_base: storage_base.as_deref(),
            recovery_available,
            base_config_json: &base_config_json,
            config_json: &config_json,
            config_source,
            config_path: config_path.as_deref(),
            cli_overrides_json: &cli_overrides_json,
            profile_overrides_json: &profile_overrides_json,
            contexts_json: &contexts_json,
            profile_name: &ephemeral_profile_name,
            profile_label: &ephemeral_profile_label,
            startup_context: startup_context.as_deref(),
            startup_clean_link: clean_link,
            startup_background,
        });
        emit_structured_log(
            options.log_level.as_deref(),
            if result.is_ok() { "info" } else { "error" },
            if result.is_ok() {
                "browser.stop"
            } else {
                "browser.startup-failed"
            },
        );
        return finish_runtime(
            marker,
            result,
            temporary_roots,
            instance,
            ferric_browser_engine_qt::shutdown_was_forced(),
        )
        .map_err(RunError::from);
    }

    let paths = existing_instance_paths(storage_base.as_deref(), options.instance.as_deref())
        .map_err(RunError::no_instance)?;
    let result = match action {
        CliAction::Command { text, window } => {
            forward_command(&paths, &text, window.as_deref(), &options)
        }
        CliAction::Query {
            method,
            params,
            format,
        } => forward_query(&paths, &method, params, &format),
        CliAction::Activate { kind, id } => forward_activate(&paths, &kind, &id),
        CliAction::Open { .. }
        | CliAction::Diagnostics { .. }
        | CliAction::ConfigCheck
        | CliAction::ResetData => {
            unreachable!("non-open CLI actions were handled above")
        }
        CliAction::DefaultBrowser { .. } => {
            unreachable!("default-browser was handled before instance setup")
        }
    };
    finish_forward(result, temporary_roots).map_err(RunError::from)
}

fn run_default_browser_operation(operation: DefaultBrowserOperation) -> Result<(), String> {
    let current = xdg_settings_default_browser("get", None)?;
    match operation {
        DefaultBrowserOperation::Status => {
            println!(
                "{}",
                serde_json::json!({
                    "desktop_entry": DESKTOP_ENTRY_ID,
                    "is_default": current == DESKTOP_ENTRY_ID,
                    "current": current,
                })
            );
            Ok(())
        }
        DefaultBrowserOperation::Set => {
            if current != DESKTOP_ENTRY_ID {
                xdg_settings_default_browser("set", Some(DESKTOP_ENTRY_ID))?;
            }
            let verified = xdg_settings_default_browser("get", None)?;
            if verified != DESKTOP_ENTRY_ID {
                return Err(format!(
                    "desktop handler did not verify Ferric Browser as the default browser (reported {verified:?})"
                ));
            }
            println!(
                "Ferric Browser is now the default browser ({DESKTOP_ENTRY_ID}); no other browser defaults were changed"
            );
            Ok(())
        }
    }
}

fn xdg_settings_default_browser(
    operation: &str,
    desktop_entry: Option<&str>,
) -> Result<String, String> {
    let mut command = ProcessCommand::new("xdg-settings");
    command
        .arg(operation)
        .arg("default-web-browser")
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(desktop_entry) = desktop_entry {
        command.arg(desktop_entry);
    }
    let output = command
        .output()
        .map_err(|error| format!("could not run xdg-settings: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("xdg-settings {operation} default-web-browser failed")
        } else {
            format!("xdg-settings {operation} default-web-browser failed: {detail}")
        });
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if value.len() > 256 || value.chars().any(char::is_control) {
        return Err("xdg-settings returned an invalid default-browser identifier".into());
    }
    Ok(value)
}

fn validate_engine_security_environment() -> Result<(), String> {
    if current_uid() == Some(0) {
        return Err("production GUI startup is not permitted as root".into());
    }

    for variable in ["QTWEBENGINE_CHROMIUM_FLAGS", "QTWEBENGINE_FLAGS"] {
        let Ok(value) = std::env::var(variable) else {
            continue;
        };
        if let Some(flag) = find_disabling_engine_flag(&value) {
            return Err(format!(
                "refusing GUI startup because {variable} contains the disabling flag {flag}"
            ));
        }
    }
    Ok(())
}

fn find_disabling_engine_flag(value: &str) -> Option<&'static str> {
    value.split_whitespace().find_map(|token| {
        let flag = token.split_once('=').map_or(token, |(flag, _)| flag);
        DISABLING_ENGINE_FLAGS
            .iter()
            .copied()
            .find(|blocked| flag == *blocked)
    })
}

fn instance_identity(instance_selector: Option<&str>) -> String {
    let identity = format!(
        "user={};session={};wayland={}",
        std::env::var("USER").unwrap_or_default(),
        std::env::var("XDG_SESSION_ID").unwrap_or_default(),
        std::env::var("WAYLAND_DISPLAY").unwrap_or_default()
    );
    instance_selector.map_or(identity.clone(), |selector| {
        format!("{identity};instance={selector}")
    })
}

fn resolve_storage_base(
    temp_basedir: bool,
    basedir: Option<String>,
) -> Result<(Option<std::path::PathBuf>, Option<StorageRoots>), String> {
    if temp_basedir {
        let roots = StorageRoots::resolve(RootSpec::Temporary)
            .map_err(|error| format!("could not create temporary base directory: {error}"))?;
        let base = roots
            .temporary_root()
            .ok_or_else(|| "temporary storage root was not recorded".to_owned())?
            .to_owned();
        Ok((Some(base), Some(roots)))
    } else if let Some(path) = basedir {
        Ok((Some(std::path::PathBuf::from(path)), None))
    } else {
        Ok((None, None))
    }
}

fn begin_marker(storage_base: Option<&std::path::Path>) -> Result<CrashMarker, String> {
    let roots = if let Some(base) = storage_base {
        StorageRoots::resolve(RootSpec::Base(base.to_owned()))
    } else {
        StorageRoots::resolve(RootSpec::Xdg)
    }
    .map_err(|error| format!("could not resolve runtime storage roots: {error}"))?;
    CrashMarker::begin(&roots).map_err(|error| format!("could not create crash marker: {error}"))
}

fn finish_runtime(
    marker: CrashMarker,
    result: Result<(), String>,
    temporary_roots: Option<StorageRoots>,
    _instance: InstanceRuntime,
    forced_shutdown: bool,
) -> Result<(), String> {
    if !forced_shutdown {
        marker
            .finish()
            .map_err(|error| format!("could not finalize crash marker: {error}"))?;
    }
    temporary_roots
        .map(StorageRoots::cleanup)
        .transpose()
        .map_err(|error| format!("could not clean temporary base directory: {error}"))?;
    result
}

fn check_config(path: Option<&str>) -> Result<(), String> {
    if let Some(path) = path {
        let loaded = load(path).map_err(|error| format!("configuration is invalid: {error}"))?;
        let contexts_path = Path::new(path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("contexts.toml");
        if contexts_path.exists() {
            load_contexts(&contexts_path)
                .map_err(|error| format!("contexts configuration is invalid: {error}"))?;
        }
        let profiles_path = Path::new(path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("profiles.toml");
        if profiles_path.exists() {
            load_profiles(&profiles_path)
                .map_err(|error| format!("profiles configuration is invalid: {error}"))?;
        }
        println!(
            "configuration valid ({} source file{})",
            loaded.sources.len(),
            if contexts_path.exists() {
                "; contexts.toml valid"
            } else {
                ""
            }
        );
    } else {
        println!("built-in configuration defaults valid");
    }
    Ok(())
}

fn default_config_path(options: &CliOptions) -> Option<String> {
    if options.temp_basedir || options.safe_mode {
        return None;
    }
    let path = if let Some(base) = options.basedir.as_deref() {
        Path::new(base).join("config").join("config.toml")
    } else if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        Path::new(&config_home)
            .join("ferric-browser")
            .join("config.toml")
    } else {
        std::env::var_os("HOME")
            .map(|home| Path::new(&home).join(".config/ferric-browser/config.toml"))?
    };
    path.exists().then(|| path.to_string_lossy().into_owned())
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy)]
struct GuiLaunchPlan<'a> {
    initial_url: &'a str,
    additional_inputs: &'a [String],
    initial_entry_point: &'a str,
    temporary_profile: bool,
    ephemeral_profile: bool,
    safe_mode: bool,
    userscripts_off: bool,
    software_rendering: bool,
    instance_lock_path: &'a Path,
    instance_selector: Option<&'a str>,
    storage_base: Option<&'a Path>,
    recovery_available: bool,
    base_config_json: &'a str,
    config_json: &'a str,
    config_source: &'a str,
    config_path: Option<&'a str>,
    cli_overrides_json: &'a str,
    profile_overrides_json: &'a str,
    contexts_json: &'a str,
    profile_name: &'a str,
    profile_label: &'a str,
    startup_context: Option<&'a str>,
    startup_clean_link: bool,
    startup_background: bool,
}

#[allow(clippy::too_many_lines)]
fn run_gui(plan: &GuiLaunchPlan<'_>) -> Result<(), String> {
    let plan = *plan;
    let GuiLaunchPlan {
        initial_url,
        additional_inputs,
        initial_entry_point,
        temporary_profile,
        ephemeral_profile,
        safe_mode,
        userscripts_off,
        software_rendering,
        instance_lock_path,
        instance_selector,
        storage_base,
        recovery_available,
        base_config_json,
        config_json,
        config_source,
        config_path,
        cli_overrides_json,
        profile_overrides_json,
        contexts_json,
        profile_name,
        profile_label,
        startup_context,
        startup_clean_link,
        startup_background,
    } = plan;
    ferric_browser_engine_qt::register_internal_scheme();
    cxx_qt::init_crate!(ferric_browser_engine_qt);
    cxx_qt::init_qml_module!("io.github.ferricbrowser");
    let mut app = QGuiApplication::new();
    let mut native_wayland = false;
    if let Some(_app) = app.as_mut() {
        ferric_browser_engine_qt::set_desktop_identity();
        let platform = ferric_browser_engine_qt::qt_platform_name();
        native_wayland = platform.starts_with("wayland");
        if matches!(platform.as_str(), "xcb" | "xwayland")
            && std::env::var("FERRIC_BROWSER_ALLOW_NON_WAYLAND").as_deref() != Ok("1")
        {
            return Err(format!(
                "unsupported Qt platform '{platform}': Ferric Browser requires native Wayland; set FERRIC_BROWSER_ALLOW_NON_WAYLAND=1 only for explicit development use"
            ));
        }
        if matches!(platform.as_str(), "xcb" | "xwayland") {
            eprintln!(
                "warning: running on Qt platform '{platform}' via explicit development override; this is not a native Wayland-qualified run"
            );
        }
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        let qml_failed = Arc::new(AtomicBool::new(false));
        let qml_failed_signal = Arc::clone(&qml_failed);
        let _qml_failure = engine.as_mut().on_object_creation_failed(move |_, _| {
            qml_failed_signal.store(true, Ordering::Relaxed);
        });
        let initial_url = QString::from(initial_url);
        let mut initial_properties: QMap<QMapPair_QString_QVariant> = QMap::default();
        initial_properties.insert(QString::from("startupUrl"), QVariant::from(&initial_url));
        let additional_inputs = additional_inputs
            .iter()
            .map(|input| QString::from(input.as_str()))
            .collect::<QStringList>();
        initial_properties.insert(
            QString::from("startupAdditionalUrls"),
            QVariant::from(&additional_inputs),
        );
        let initial_entry_point = QString::from(initial_entry_point);
        initial_properties.insert(
            QString::from("startupEntryPoint"),
            QVariant::from(&initial_entry_point),
        );
        let startup_trusted_local_input = initial_entry_point.to_string() == "external-open";
        initial_properties.insert(
            QString::from("startupTrustedLocalInput"),
            QVariant::from(&startup_trusted_local_input),
        );
        initial_properties.insert(
            QString::from("temporaryProfile"),
            QVariant::from(&temporary_profile),
        );
        initial_properties.insert(
            QString::from("ephemeralProfile"),
            QVariant::from(&ephemeral_profile),
        );
        let profile_name = QString::from(profile_name);
        initial_properties.insert(QString::from("profileName"), QVariant::from(&profile_name));
        let profile_label = QString::from(profile_label);
        initial_properties.insert(
            QString::from("profileLabel"),
            QVariant::from(&profile_label),
        );
        let startup_context = QString::from(startup_context.unwrap_or_default());
        initial_properties.insert(
            QString::from("startupContext"),
            QVariant::from(&startup_context),
        );
        initial_properties.insert(
            QString::from("startupCleanLink"),
            QVariant::from(&startup_clean_link),
        );
        initial_properties.insert(
            QString::from("startupBackground"),
            QVariant::from(&startup_background),
        );
        initial_properties.insert(QString::from("safeMode"), QVariant::from(&safe_mode));
        initial_properties.insert(
            QString::from("userscriptsOff"),
            QVariant::from(&userscripts_off),
        );
        initial_properties.insert(
            QString::from("softwareRendering"),
            QVariant::from(&software_rendering),
        );
        initial_properties.insert(
            QString::from("nativeWayland"),
            QVariant::from(&native_wayland),
        );
        let instance_lock_path = QString::from(instance_lock_path.to_string_lossy().as_ref());
        initial_properties.insert(
            QString::from("instanceLockPath"),
            QVariant::from(&instance_lock_path),
        );
        let instance_selector = QString::from(instance_selector.unwrap_or_default());
        initial_properties.insert(
            QString::from("instanceSelector"),
            QVariant::from(&instance_selector),
        );
        let storage_base = QString::from(
            storage_base
                .map(std::path::Path::to_string_lossy)
                .as_deref()
                .unwrap_or_default(),
        );
        initial_properties.insert(
            QString::from("storageBasePath"),
            QVariant::from(&storage_base),
        );
        initial_properties.insert(
            QString::from("recoveryAvailable"),
            QVariant::from(&recovery_available),
        );
        let config_json = QString::from(config_json);
        initial_properties.insert(
            QString::from("startupConfigJson"),
            QVariant::from(&config_json),
        );
        let base_config_json = QString::from(base_config_json);
        initial_properties.insert(
            QString::from("startupConfigBaseJson"),
            QVariant::from(&base_config_json),
        );
        let config_source = QString::from(config_source);
        initial_properties.insert(
            QString::from("startupConfigSource"),
            QVariant::from(&config_source),
        );
        let config_path = QString::from(config_path.unwrap_or_default());
        initial_properties.insert(
            QString::from("startupConfigPath"),
            QVariant::from(&config_path),
        );
        let cli_overrides_json = QString::from(cli_overrides_json);
        initial_properties.insert(
            QString::from("startupCliOverridesJson"),
            QVariant::from(&cli_overrides_json),
        );
        let profile_overrides_json = QString::from(profile_overrides_json);
        initial_properties.insert(
            QString::from("startupProfileOverridesJson"),
            QVariant::from(&profile_overrides_json),
        );
        let contexts_json = QString::from(contexts_json);
        initial_properties.insert(
            QString::from("startupContextsJson"),
            QVariant::from(&contexts_json),
        );
        engine.as_mut().set_initial_properties(&initial_properties);
        engine.load(&QUrl::from(
            "qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml",
        ));
        if qml_failed.load(Ordering::Relaxed) {
            return Err("could not load the browser QML surface".into());
        }
    } else {
        return Err("could not create QML engine".into());
    }
    if let Some(app) = app.as_mut() {
        app.exec();
        Ok(())
    } else {
        Err("could not create Qt GUI application".into())
    }
}

fn print_help() {
    println!("Ferric Browser — keyboard-driven browser");
    println!();
    println!("Usage: ferric-browser [OPTIONS] [COMMAND]");
    println!();
    println!("Options:");
    println!("  --basedir PATH  redirect browser-owned storage to an absolute directory");
    println!("  --temp-basedir  start with an isolated disposable core profile");
    println!("  --ephemeral     start a fresh isolated ephemeral profile");
    println!(
        "  --safe-mode     start a disposable diagnostic profile with built-in config and no userscripts"
    );
    println!(
        "  --userscripts-off  open the selected normal profile with page userscripts disabled"
    );
    println!("  --software-rendering  start with explicit software rendering for diagnostics");
    println!("  --config PATH   validate and use a TOML configuration");
    println!("  --profile NAME  select a profile for forwarded commands");
    println!("  --context NAME  select a context for forwarded commands");
    println!("  --instance ID   select an explicit instance identifier");
    println!("  --set KEY=VALUE provide a validated runtime setting");
    println!("  --log-level L   select error, warn, info, or debug logging");
    println!();
    println!("Commands:");
    println!(
        "  open [--target current|tab|tab-bg|window|private-window] [--profile NAME] [--context NAME] [--clean-link] [--ephemeral] [--] INPUT..."
    );
    println!("  command [--window WINDOW] -- COMMAND");
    println!("  profile-list");
    println!("  profile-open NAME [URL]");
    println!("  profile-create NAME [--ephemeral]");
    println!("  window-new [--profile NAME] [--private]");
    println!("  tab-give WINDOW_ID");
    println!("  tab-close [--id TAB_ID] [--count N]");
    println!("  reload [--bypass-cache]");
    println!("  search [--backward] [--case smart|sensitive|insensitive] TEXT");
    println!("  fullscreen [on|off|toggle]");
    println!("    action SUBJECT VERB [ARG...] | action-list [SUBJECT]");
    println!(
        "  query tabs|active-tab|windows|operations|blocking|permissions|bindings|binding-explain|contexts|switcher|config [--url URL] --format json"
    );
    println!("  activate tab TAB_ID");
    println!("  diagnostics --format json");
    println!("  default-browser status|set");
    println!("  reset-data --confirm  erase incompatible Ferric-owned data");
    println!("  command -- blocking-toggle [--site]");
    println!("  config check    validate configuration without starting the GUI");
    println!("  -h, --help      show this help");
    println!("  -V, --version   show the application version");
    println!();
    println!("Registry command catalog:");
    let registry = CommandRegistry::default_v1();
    for definition in registry.definitions() {
        let usage = command_usage(definition);
        let aliases = registry
            .alias_definitions()
            .iter()
            .filter(|alias| {
                alias
                    .expansion
                    .first()
                    .is_some_and(|command| command.name == definition.name)
            })
            .map(|alias| alias.name.as_str())
            .chain(definition.aliases.iter().map(String::as_str))
            .collect::<Vec<_>>();
        if aliases.is_empty() {
            println!("  {usage:<48} {}", definition.description);
        } else {
            println!(
                "  {usage:<48} {} (aliases: {})",
                definition.description,
                aliases.join(", ")
            );
        }
    }
}

fn command_usage(definition: &ferric_browser_core::CommandDefinition) -> String {
    let arguments = definition
        .arguments
        .iter()
        .map(|argument| command_argument_usage(definition.name.as_str(), argument))
        .collect::<String>();
    format!("{}{}", definition.name, arguments)
}

fn command_argument_usage(
    command: &str,
    argument: &ferric_browser_core::ArgumentDefinition,
) -> String {
    let is_flag = argument.kind == ferric_browser_core::ArgumentKind::Boolean
        || command_argument_is_flag(command, argument.name.as_str());
    let display_name = argument.name.replace('_', "-");
    if !is_flag {
        return if argument.required {
            format!(" <{}>", argument.name)
        } else {
            format!(" [{}]", argument.name)
        };
    }

    let flag = if command == "search-next" && argument.name == "direction" {
        "--backward".to_owned()
    } else {
        format!("--{display_name}")
    };
    let rendered = if argument.kind == ferric_browser_core::ArgumentKind::Boolean
        || (command == "search-next" && argument.name == "direction")
    {
        flag
    } else {
        format!("{flag} <{}>", argument.name.to_ascii_uppercase())
    };
    if argument.required {
        format!(" {rendered}")
    } else {
        format!(" [{rendered}]")
    }
}

fn command_argument_is_flag(command: &str, argument: &str) -> bool {
    match command {
        "open" => matches!(
            argument,
            "target" | "profile" | "context" | "ephemeral" | "clean_link"
        ),
        "tab-open" => argument == "background",
        "paste-open" => matches!(argument, "target" | "primary"),
        "reload" => argument == "bypass_cache",
        "search" => matches!(argument, "backward" | "case"),
        "search-next" => matches!(argument, "direction" | "count"),
        "scroll" | "back" | "forward" | "tab-next" | "tab-prev" | "caret-move" => {
            argument == "count"
        }
        "scroll-page" => matches!(argument, "half" | "count"),
        "tab-close" => matches!(argument, "id" | "count"),
        "tab-move" => argument == "context",
        "window-new" => argument == "profile",
        "hint" => matches!(argument, "target" | "rapid" | "script"),
        "yank" => matches!(argument, "clean" | "primary"),
        "history-clear" => matches!(argument, "since" | "origin"),
        "session-load" => argument == "append",
        "switcher" => argument == "scope",
        "journey" => matches!(argument, "current" | "search" | "expand"),
        "journey-reopen" => argument == "target",
        "set" | "unset" => argument == "pattern",
        "get" => argument == "url",
        "binding-list" | "binding-explain" | "bind" | "unbind" => argument == "mode",
        "site-status" => argument == "tab",
        "selection-search" => argument == "engine",
        "jseval" => argument == "world",
        "devtools" => argument == "detach",
        "blocking-toggle" => argument == "site",
        "context-create" => matches!(argument, "label" | "profile" | "workspace"),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
