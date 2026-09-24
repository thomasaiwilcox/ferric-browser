extern crate ferric_browser_engine_qt;

use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QString, QStringList,
    QUrl, QVariant,
};
use ferric_browser_config::{
    Config, ContextsConfig, ProfilesConfig, RuntimeOverrides, apply_runtime_overrides, load,
    load_contexts, load_profiles, load_runtime_overrides, profile_override_layer,
};
use ferric_browser_core::{
    CommandRegistry, ParseInput, ParsedCommand, canonical_origin, parse_chain, validate_open_target,
};
use ferric_browser_ipc::{
    ErrorCode, HelloResult, InstanceError, InstanceLock, InstancePaths, PROTOCOL_MAJOR,
    PROTOCOL_MINOR, Request, Response, current_uid, instance_paths, read_frame, write_frame,
};
use ferric_browser_storage::{
    CrashMarker, RootSpec, StorageRoots, crash_diagnostics, inspect_store, transient_marker_scan,
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
const MAX_JOURNEY_QUERY_BYTES: usize = 256;

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

fn parse_cli(arguments: &[String]) -> Result<(CliOptions, CliAction), String> {
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

fn next_cli_value(arguments: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    *index += 1;
    arguments
        .get(*index)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_subcommand(
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
fn open_from_values(
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

fn log_level_rank(level: &str) -> Option<u8> {
    match level {
        "error" => Some(0),
        "warn" => Some(1),
        "info" => Some(2),
        "debug" => Some(3),
        _ => None,
    }
}

fn emit_structured_log(configured_level: Option<&str>, level: &str, event: &str) {
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

fn cli_action_name(action: &CliAction) -> &'static str {
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

fn is_url_like_input(value: &str) -> bool {
    ["http://", "https://", "file://", "about:"]
        .iter()
        .any(|prefix| {
            value
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        })
}

fn command_from_values(values: &[String]) -> Result<(String, Option<String>), String> {
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
fn query_from_values(values: &[String]) -> Result<(String, serde_json::Value, String), String> {
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

fn format_from_values(values: &[String]) -> Result<String, String> {
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

fn validate_format(format: &str) -> Result<(), String> {
    if format == "json" {
        Ok(())
    } else {
        Err("only --format json is currently supported".into())
    }
}

fn validate_cli_text(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().any(char::is_control) {
        Err(format!(
            "{label} must be nonempty and contain no control characters"
        ))
    } else {
        Ok(())
    }
}

fn acquire_instance(
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

fn existing_instance_paths(
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

fn forward_open(
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

fn forward_focus(paths: &InstancePaths) -> Result<(), String> {
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

fn forward_command(
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

#[allow(clippy::match_same_arms, clippy::too_many_lines)]
fn command_params(command: &ParsedCommand) -> Result<serde_json::Value, String> {
    let arguments = match command.name.as_str() {
        "open" => {
            if command.arguments.is_empty() {
                return Err("open requires an input".into());
            }
            if command.arguments.first().map(String::as_str) == Some("--clean-link") {
                if command.arguments.len() < 2 {
                    return Err("open --clean-link requires an input".into());
                }
                serde_json::json!({
                    "input": command.arguments[1..].join(" "),
                    "clean_link": true,
                    "external": true
                })
            } else {
                serde_json::json!({"input": command.arguments.join(" "), "external": true})
            }
        }
        "tab-open" => {
            let mut background = false;
            let mut input = Vec::new();
            let mut options_ended = false;
            for argument in &command.arguments {
                if options_ended {
                    input.push(argument.clone());
                } else {
                    match argument.as_str() {
                        "--background" if !background => background = true,
                        "--background" => {
                            return Err("tab-open accepts --background at most once".into());
                        }
                        "--" => {
                            options_ended = true;
                        }
                        value if value.starts_with("--") => {
                            return Err(format!("unknown tab-open option: {value}"));
                        }
                        _ => input.push(argument.clone()),
                    }
                }
            }
            if input.is_empty() {
                return Err("tab-open requires an input".into());
            }
            serde_json::json!({
                "input": input.join(" "),
                "background": background
            })
        }
        "open-current" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag, target] if flag == "--target" && target == "tab" => {
                serde_json::json!({"target": "tab"})
            }
            _ => return Err("open-current accepts only --target tab".into()),
        },
        "back" | "forward" | "tab-next" | "tab-prev" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag, value] if flag == "--count" => {
                let count = value
                    .parse::<u32>()
                    .ok()
                    .filter(|value| (1..=100).contains(value))
                    .ok_or_else(|| {
                        format!("{} --count must be a value from 1 to 100", command.name)
                    })?;
                serde_json::json!({"count": count})
            }
            _ => return Err(format!("{} accepts optional --count 1..100", command.name)),
        },
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
            validate_cli_text(&key, "configuration key")?;
            if let Some(url) = url.as_deref() {
                validate_cli_text(url, "configuration URL")?;
            }
            serde_json::json!({"key": key, "url": url, "explain": explain})
        }
        "help" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [topic] => {
                validate_cli_text(topic, "help topic")?;
                serde_json::json!({"topic": topic})
            }
            _ => return Err("help accepts at most one command or setting topic".into()),
        },
        "version" | "diagnostics" => {
            if !command.arguments.is_empty() {
                return Err(format!("{} does not accept arguments", command.name));
            }
            serde_json::json!({})
        }
        "paste-open" => match command.arguments.as_slice() {
            [] => serde_json::json!({"primary": false}),
            [flag] if flag == "--primary" => serde_json::json!({"primary": true}),
            [flag, target]
                if flag == "--target" && matches!(target.as_str(), "current" | "tab") =>
            {
                serde_json::json!({"target": target, "primary": false})
            }
            [target_flag, target, primary]
                if target_flag == "--target"
                    && matches!(target.as_str(), "current" | "tab")
                    && primary == "--primary" =>
            {
                serde_json::json!({"target": target, "primary": true})
            }
            [primary, target_flag, target]
                if primary == "--primary"
                    && target_flag == "--target"
                    && matches!(target.as_str(), "current" | "tab") =>
            {
                serde_json::json!({"target": target, "primary": true})
            }
            _ => return Err("paste-open accepts [--target current|tab] [--primary]".into()),
        },
        "bookmark-add" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [flag, title] if flag == "--title" => serde_json::json!({"title": title}),
            _ => return Err("bookmark-add accepts optional --title TEXT".into()),
        },
        "bookmark-delete" => {
            if command.arguments.len() != 1 {
                return Err("bookmark-delete requires exactly one ID".into());
            }
            validate_cli_text(&command.arguments[0], "bookmark ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "bookmark-list" | "quickmark-list" | "history" => {
            if !command.arguments.is_empty() {
                return Err(format!("{} does not accept arguments", command.name));
            }
            serde_json::json!({})
        }
        "switcher" => {
            let mut value = serde_json::json!({"scope": "all", "query": ""});
            let mut scope_set = false;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--scope" => {
                        if scope_set {
                            return Err("switcher accepts --scope at most once".into());
                        }
                        let scope = command
                            .arguments
                            .get(index + 1)
                            .ok_or_else(|| "switcher --scope requires a value".to_owned())?;
                        if !matches!(
                            scope.as_str(),
                            "all"
                                | "tabs"
                                | "windows"
                                | "contexts"
                                | "commands"
                                | "history"
                                | "marks"
                                | "sessions"
                                | "downloads"
                                | "closed"
                                | "actions"
                        ) {
                            return Err("switcher --scope must be a valid switcher scope".into());
                        }
                        value["scope"] = serde_json::Value::String(scope.clone());
                        scope_set = true;
                        index += 2;
                    }
                    query if !query.starts_with('-') && value["query"] == "" => {
                        value["query"] = serde_json::Value::String(query.to_owned());
                        index += 1;
                    }
                    _ => return Err("switcher accepts optional --scope SCOPE and QUERY".into()),
                }
            }
            value
        }
        "journey" => {
            let mut value = serde_json::json!({"current": false});
            let mut current_set = false;
            let mut search_set = false;
            let mut expand_set = false;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--current" if !current_set => {
                        value["current"] = serde_json::Value::Bool(true);
                        current_set = true;
                    }
                    "--current" => {
                        return Err("journey accepts --current at most once".into());
                    }
                    "--search" | "--expand" => {
                        let key = if command.arguments[index] == "--search" {
                            "search"
                        } else {
                            "expand"
                        };
                        let already_set = if key == "search" {
                            search_set
                        } else {
                            expand_set
                        };
                        if already_set {
                            return Err(format!("journey accepts {key} at most once"));
                        }
                        let argument = command.arguments.get(index + 1).ok_or_else(|| {
                            format!("journey {} requires a value", command.arguments[index])
                        })?;
                        if key == "search"
                            && (argument.is_empty()
                                || argument.len() > MAX_JOURNEY_QUERY_BYTES
                                || argument.chars().any(char::is_control))
                        {
                            return Err(
                                "journey search must be 1..256 bytes without control characters"
                                    .into(),
                            );
                        }
                        if key == "expand" && uuid::Uuid::parse_str(argument).is_err() {
                            return Err("journey --expand requires a node UUID".into());
                        }
                        value[key] = serde_json::Value::String(argument.clone());
                        if key == "search" {
                            search_set = true;
                        } else {
                            expand_set = true;
                        }
                        index += 1;
                    }
                    _ => {
                        return Err(
                            "journey accepts --current, --search TEXT, and --expand NODE_UUID"
                                .into(),
                        );
                    }
                }
                index += 1;
            }
            value
        }
        "journey-reopen" => {
            let node = command
                .arguments
                .first()
                .ok_or_else(|| "journey-reopen requires a node UUID".to_owned())?;
            if uuid::Uuid::parse_str(node).is_err() {
                return Err("journey-reopen requires a node UUID".into());
            }
            if command.arguments.len() > 3
                || (command.arguments.len() > 1 && command.arguments[1] != "--target")
                || (command.arguments.len() == 3
                    && !matches!(command.arguments[2].as_str(), "current" | "tab" | "window"))
            {
                return Err(
                    "journey-reopen requires NODE_UUID [--target current|tab|window]".into(),
                );
            }
            let mut value = serde_json::json!({"node": node});
            if let Some(target) = command.arguments.get(2) {
                value["target"] = serde_json::Value::String(target.clone());
            }
            value
        }
        "quickmark-add" => {
            if !(1..=2).contains(&command.arguments.len()) {
                return Err("quickmark-add requires NAME and optional URL".into());
            }
            validate_cli_text(&command.arguments[0], "quickmark name")?;
            let mut value = serde_json::json!({"name": command.arguments[0]});
            if let Some(url) = command.arguments.get(1) {
                value["url"] = serde_json::Value::String(url.clone());
            }
            value
        }
        "quickmark-delete" => {
            if command.arguments.len() != 1 {
                return Err("quickmark-delete requires exactly one name".into());
            }
            validate_cli_text(&command.arguments[0], "quickmark name")?;
            serde_json::json!({"name": command.arguments[0]})
        }
        "history-clear" => {
            let mut since = None;
            let mut origin = None;
            let mut confirmed = false;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--confirm" if !confirmed => confirmed = true,
                    "--since" if since.is_none() => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "history-clear --since requires Unix seconds".to_owned()
                        })?;
                        let timestamp = value
                            .parse::<i64>()
                            .ok()
                            .filter(|value| *value >= 0)
                            .ok_or_else(|| {
                                "history-clear --since requires a nonnegative Unix timestamp"
                                    .to_owned()
                            })?;
                        since = Some(timestamp);
                        index += 1;
                    }
                    "--origin" if origin.is_none() => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "history-clear --origin requires an exact HTTP(S) origin".to_owned()
                        })?;
                        origin = Some(normalize_cli_history_origin(value)?);
                        index += 1;
                    }
                    value if value.starts_with("--") => {
                        return Err(format!("unknown history-clear option: {value}"));
                    }
                    value => return Err(format!("unexpected history-clear argument: {value}")),
                }
                index += 1;
            }
            let mut value = serde_json::json!({"confirmed": confirmed});
            if let Some(since) = since {
                value["since"] = serde_json::Value::Number(since.into());
            }
            if let Some(origin) = origin {
                value["origin"] = serde_json::Value::String(origin);
            }
            value
        }
        "url-clean" | "url-explain" => {
            if command.arguments.len() > 1 {
                return Err(format!("{} accepts at most one URL", command.name));
            }
            command.arguments.first().map_or_else(
                || serde_json::json!({}),
                |url| serde_json::json!({"url": url}),
            )
        }
        "session-save" | "session-load" | "session-delete" | "profile-delete" => {
            if command.arguments.len() != 1 {
                return Err(format!("{} requires exactly one name", command.name));
            }
            validate_cli_text(&command.arguments[0], "name")?;
            serde_json::json!({"name": command.arguments[0]})
        }
        "profile-open" => {
            if !(1..=2).contains(&command.arguments.len()) {
                return Err("profile-open requires NAME and optional URL or search input".into());
            }
            validate_cli_text(&command.arguments[0], "profile-open name")?;
            let mut value = serde_json::json!({"name": command.arguments[0]});
            if let Some(input) = command.arguments.get(1) {
                value["input"] = serde_json::Value::String(input.clone());
            }
            value
        }
        "profile-create" => {
            if !(1..=2).contains(&command.arguments.len())
                || (command.arguments.len() == 2 && command.arguments[1] != "--ephemeral")
            {
                return Err("profile-create requires NAME with optional --ephemeral".into());
            }
            validate_cli_text(&command.arguments[0], "profile-create name")?;
            serde_json::json!({
                "name": command.arguments[0],
                "ephemeral": command.arguments.get(1).is_some()
            })
        }
        "fullscreen" => {
            if command.arguments.len() > 1
                || command
                    .arguments
                    .first()
                    .is_some_and(|state| !matches!(state.as_str(), "on" | "off" | "toggle"))
            {
                return Err("fullscreen accepts optional state on, off, or toggle".into());
            }
            serde_json::json!({
                "state": command.arguments.first().map_or("toggle", String::as_str)
            })
        }
        "reload" => {
            if command.arguments.is_empty() {
                serde_json::json!({"bypass_cache": false})
            } else if command.arguments == ["--bypass-cache"] {
                serde_json::json!({"bypass_cache": true})
            } else {
                return Err("reload accepts only --bypass-cache".into());
            }
        }
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
                validate_cli_text(id, "tab ID")?;
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
        "search" => {
            let mut backward = false;
            let mut case = "smart";
            let mut case_seen = false;
            let mut query = Vec::new();
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--backward" if !backward => backward = true,
                    "--case" if !case_seen => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "search --case requires smart, sensitive, or insensitive".to_owned()
                        })?;
                        if !matches!(value.as_str(), "smart" | "sensitive" | "insensitive") {
                            return Err(
                                "search --case requires smart, sensitive, or insensitive".into()
                            );
                        }
                        case = value;
                        case_seen = true;
                        index += 1;
                    }
                    "--" => {
                        query.extend(command.arguments[index + 1..].iter().cloned());
                        break;
                    }
                    value if value.starts_with("--") => {
                        return Err(format!("unknown search option: {value}"));
                    }
                    value => query.push(value.to_owned()),
                }
                index += 1;
            }
            if query.is_empty() {
                return Err("search requires query text".into());
            }
            serde_json::json!({
                "query": query.join(" "),
                "backward": backward,
                "case": case
            })
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
                validate_cli_text(&profile, "window-new profile")?;
                value["profile"] = serde_json::Value::String(profile);
            }
            value
        }
        "tab-give" => {
            if command.arguments.len() != 1 {
                return Err("tab-give requires exactly one WINDOW_ID".into());
            }
            validate_cli_text(&command.arguments[0], "window ID")?;
            serde_json::json!({"window_id": command.arguments[0]})
        }
        "tab-select" => {
            if command.arguments.len() != 1 {
                return Err("tab-select requires exactly one tab ID or displayed index".into());
            }
            validate_cli_text(&command.arguments[0], "tab selector")?;
            serde_json::json!({"selector": command.arguments[0]})
        }
        "tab-focus" | "tab-suspend" | "tab-discard" | "tab-resume" => {
            if command.arguments.len() != 1 {
                return Err(format!("{} requires exactly one tab ID", command.name));
            }
            validate_cli_text(&command.arguments[0], "tab ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "tab-pin" | "tab-mute" => match command.arguments.as_slice() {
            [] => serde_json::json!({}),
            [state] if matches!(state.as_str(), "on" | "off" | "toggle") => {
                serde_json::json!({"state": state})
            }
            [id] => {
                validate_cli_text(id, "tab ID")?;
                serde_json::json!({"id": id})
            }
            [id, state] if matches!(state.as_str(), "on" | "off" | "toggle") => {
                validate_cli_text(id, "tab ID")?;
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
                validate_cli_text(id, "tab ID")?;
                serde_json::json!({"id": id, "direction": direction})
            }
            [id, flag, context] if flag == "--context" => {
                validate_cli_text(id, "tab ID or index")?;
                validate_cli_text(context, "context")?;
                serde_json::json!({"id": id, "context": context})
            }
            [flag, context] if flag == "--context" => {
                validate_cli_text(context, "context")?;
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
            validate_cli_text(&command.arguments[0], "window ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "window-move" => {
            if command.arguments.len() != 2 {
                return Err("window-move requires WINDOW_ID and WORKSPACE".into());
            }
            validate_cli_text(&command.arguments[0], "window ID")?;
            validate_cli_text(&command.arguments[1], "workspace")?;
            serde_json::json!({
                "id": command.arguments[0],
                "workspace": command.arguments[1]
            })
        }
        "zoom" => {
            if command.arguments.len() != 1 {
                return Err("zoom requires in, out, reset, or a factor".into());
            }
            let factor = command.arguments[0].as_str();
            if !matches!(factor, "in" | "out" | "reset")
                && factor
                    .parse::<f64>()
                    .ok()
                    .is_none_or(|value| !(0.25..=5.0).contains(&value))
            {
                return Err(
                    "zoom factor must be in, out, reset, or a value from 0.25 to 5.0".into(),
                );
            }
            serde_json::json!({"factor": command.arguments[0]})
        }
        "search-next" => {
            let mut direction = "forward";
            let mut count = None;
            let mut index = 0;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--backward" if direction == "forward" => direction = "backward",
                    "--count" if count.is_none() => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "search-next --count requires a value from 1 to 100".to_owned()
                        })?;
                        count = Some(
                            value
                                .parse::<u32>()
                                .ok()
                                .filter(|n| (1..=100).contains(n))
                                .ok_or_else(|| {
                                    "search-next --count must be a value from 1 to 100".to_owned()
                                })?,
                        );
                        index += 1;
                    }
                    option => return Err(format!("unknown search-next option: {option}")),
                }
                index += 1;
            }
            serde_json::json!({"direction": direction, "count": count.unwrap_or(1)})
        }
        "scroll" => {
            let direction = command
                .arguments
                .first()
                .ok_or_else(|| "scroll requires a direction".to_owned())?;
            if !matches!(direction.as_str(), "up" | "down" | "left" | "right") {
                return Err("scroll direction must be up, down, left, or right".into());
            }
            if command.arguments.len() == 1 {
                serde_json::json!({"direction": direction})
            } else if command.arguments.len() == 3 && command.arguments[1] == "--count" {
                let count = command.arguments[2]
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=100).contains(n))
                    .ok_or_else(|| "scroll --count must be a value from 1 to 100".to_owned())?;
                serde_json::json!({"direction": direction, "count": count})
            } else {
                return Err("scroll accepts DIRECTION [--count N]".into());
            }
        }
        "scroll-page" => {
            let direction = command
                .arguments
                .first()
                .ok_or_else(|| "scroll-page requires up or down".to_owned())?;
            if !matches!(direction.as_str(), "up" | "down") {
                return Err("scroll-page direction must be up or down".into());
            }
            let mut half = false;
            let mut count = None;
            let mut index = 1;
            while index < command.arguments.len() {
                match command.arguments[index].as_str() {
                    "--half" if !half => half = true,
                    "--count" if count.is_none() => {
                        let value = command.arguments.get(index + 1).ok_or_else(|| {
                            "scroll-page --count requires a value from 1 to 100".to_owned()
                        })?;
                        count = Some(
                            value
                                .parse::<u32>()
                                .ok()
                                .filter(|n| (1..=100).contains(n))
                                .ok_or_else(|| {
                                    "scroll-page --count must be a value from 1 to 100".to_owned()
                                })?,
                        );
                        index += 1;
                    }
                    option => return Err(format!("unknown scroll-page option: {option}")),
                }
                index += 1;
            }
            let mut value = serde_json::json!({"direction": direction, "half": half});
            if let Some(count) = count {
                value["count"] = serde_json::json!(count);
            }
            value
        }
        "scroll-to" => {
            if command.arguments.len() != 1
                || !matches!(command.arguments[0].as_str(), "top" | "bottom")
            {
                return Err("scroll-to requires top or bottom".into());
            }
            serde_json::json!({"edge": command.arguments[0]})
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
        "bookmark-edit" => {
            if command.arguments.len() != 3 || command.arguments[1] != "--title" {
                return Err("bookmark-edit requires ID --title TEXT".into());
            }
            validate_cli_text(&command.arguments[0], "bookmark ID")?;
            serde_json::json!({"id": command.arguments[0], "title": command.arguments[2]})
        }
        "bookmark-open" | "history-open" => {
            if command.arguments.len() != 1 {
                return Err(format!("{} requires exactly one ID", command.name));
            }
            validate_cli_text(&command.arguments[0], "entry ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "quickmark-edit" => {
            if command.arguments.len() != 2 {
                return Err("quickmark-edit requires NAME and URL".into());
            }
            validate_cli_text(&command.arguments[0], "quickmark name")?;
            serde_json::json!({"name": command.arguments[0], "url": command.arguments[1]})
        }
        "quickmark-open" => {
            if command.arguments.len() != 1 {
                return Err("quickmark-open requires exactly one name".into());
            }
            validate_cli_text(&command.arguments[0], "quickmark name")?;
            serde_json::json!({"name": command.arguments[0]})
        }
        "selection-search" => {
            if command.arguments.is_empty() {
                serde_json::json!({})
            } else if command.arguments.len() == 2 && command.arguments[0] == "--engine" {
                validate_cli_text(&command.arguments[1], "selection-search engine")?;
                serde_json::json!({"engine": command.arguments[1]})
            } else {
                return Err("selection-search accepts optional --engine NAME".into());
            }
        }
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
            validate_cli_text(name, "context-create name")?;
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
                        validate_cli_text(&value, &format!("context-create {option}"))?;
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
            validate_cli_text(name, "context-delete name")?;
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
            validate_cli_text(&command.arguments[0], "context-enter name")?;
            serde_json::json!({"name": command.arguments[0]})
        }
        "context-save" => {
            if command.arguments.len() > 1 {
                return Err("context-save accepts at most one name".into());
            }
            if let Some(name) = command.arguments.first() {
                validate_cli_text(name, "context-save name")?;
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
                    validate_cli_text(&command.arguments[1], "context-route pattern")?;
                    validate_cli_text(&command.arguments[2], "context-route context")?;
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
                    validate_cli_text(&command.arguments[1], "context-route route ID")?;
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
        "config-write-defaults" => {
            if command.arguments.len() != 1 {
                return Err("config-write-defaults requires exactly one path".into());
            }
            serde_json::json!({"path": command.arguments[0]})
        }
        "config-edit" | "config-reload" | "config-check" | "theme-reload" => {
            if !command.arguments.is_empty() {
                return Err(format!("{} does not accept arguments", command.name));
            }
            serde_json::json!({})
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
                validate_cli_text(tab, "tab ID")?;
                serde_json::json!({"tab": tab})
            }
            _ => return Err("site-status accepts optional --tab TAB_ID".into()),
        },
        "site-doctor" => {
            if command.arguments.len() != 1 {
                return Err("site-doctor requires one experiment name".into());
            }
            validate_cli_text(&command.arguments[0], "experiment name")?;
            serde_json::json!({"experiment": command.arguments[0]})
        }
        "site-doctor-undo" => {
            if command.arguments.len() != 1 {
                return Err("site-doctor-undo requires one experiment ID".into());
            }
            validate_cli_text(&command.arguments[0], "experiment ID")?;
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
            validate_cli_text(&command.arguments[0], "download ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
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
            validate_cli_text(target, "send target")?;
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
                    validate_cli_text(url, "send URL")?;
                    object.insert("url".into(), serde_json::Value::String(url.clone()));
                    object.insert(
                        "send_subject".into(),
                        serde_json::Value::String("url".into()),
                    );
                }
                [input] => {
                    validate_cli_text(input, "send input")?;
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
            validate_cli_text(&command.arguments[0], "command ID")?;
            serde_json::json!({"id": command.arguments[0]})
        }
        "command-execute" => {
            if command.arguments.is_empty() || command.arguments.len() > 2 {
                return Err("command-execute expects COMMAND_ID [JSON_ARGUMENTS]".into());
            }
            validate_cli_text(&command.arguments[0], "command ID")?;
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
        _ if command.arguments.is_empty() => serde_json::json!({}),
        _ => return Err(format!("{} does not accept arguments", command.name)),
    };
    let argument_object = arguments
        .as_object()
        .ok_or_else(|| "command arguments must encode as an object".to_owned())?;
    ferric_browser_ipc::validate_command_argument_fields(&command.name, argument_object)?;
    Ok(serde_json::json!({
        "command": command.name,
        "arguments": arguments
    }))
}

fn normalize_cli_history_origin(value: &str) -> Result<String, String> {
    let authority = value
        .split_once("://")
        .map(|(_, authority)| authority)
        .filter(|authority| !authority.is_empty() && !authority.contains(['/', '?', '#', '@']))
        .ok_or_else(|| "history-clear --origin requires an exact HTTP(S) origin".to_owned())?;
    if authority.contains('\\') {
        return Err("history-clear --origin requires an exact HTTP(S) origin".into());
    }
    canonical_origin(value)
        .ok_or_else(|| "history-clear --origin requires an exact HTTP(S) origin".to_owned())
}

fn add_cli_context(params: &mut serde_json::Value, options: &CliOptions, window: Option<&str>) {
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

fn forward_query(
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

fn forward_activate(paths: &InstancePaths, kind: &str, id: &str) -> Result<(), String> {
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

fn print_response_result(response: Response, format: &str) -> Result<(), String> {
    validate_format(format)?;
    let result = response.result.unwrap_or_else(|| serde_json::json!({}));
    println!(
        "{}",
        serde_json::to_string_pretty(&result)
            .map_err(|error| format!("could not format JSON response: {error}"))?
    );
    Ok(())
}

fn print_diagnostics(
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

fn storage_diagnostics(roots: &StorageRoots) -> serde_json::Value {
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

fn finish_forward(
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

fn instance_request(paths: &InstancePaths, request: &Request) -> Result<Response, String> {
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

fn ipc_roundtrip(stream: &mut UnixStream, request: &Request) -> Result<Response, String> {
    let payload = serde_json::to_vec(request)
        .map_err(|error| format!("could not encode IPC request: {error}"))?;
    write_frame(stream, &payload).map_err(|error| error.to_string())?;
    let payload = read_frame(stream)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "running instance closed the IPC connection".to_owned())?;
    serde_json::from_slice(&payload).map_err(|error| format!("invalid IPC response: {error}"))
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
    if let Some(_app) = app.as_mut() {
        ferric_browser_engine_qt::set_desktop_identity();
        let platform = ferric_browser_engine_qt::qt_platform_name();
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
mod tests {
    use super::*;

    #[test]
    fn generated_command_usage_preserves_typed_flag_shapes() {
        let registry = CommandRegistry::default_v1();
        let open = registry.resolve("open").expect("open definition");
        assert_eq!(
            command_usage(open),
            "open <input> [--target <TARGET>] [--profile <PROFILE>] [--context <CONTEXT>] [--ephemeral] [--clean-link]"
        );
        let search = registry.resolve("search").expect("search definition");
        assert_eq!(
            command_usage(search),
            "search <query> [--backward] [--case <CASE>]"
        );
        let search_next = registry
            .resolve("search-next")
            .expect("search-next definition");
        assert_eq!(
            command_usage(search_next),
            "search-next [--backward] [--count <COUNT>]"
        );
        let context = registry
            .resolve("context-create")
            .expect("context-create definition");
        assert_eq!(
            command_usage(context),
            "context-create <name> [--label <LABEL>] --profile <PROFILE> [--workspace <WORKSPACE>]"
        );
    }

    #[test]
    fn structured_logging_uses_bounded_levels_and_safe_event_names() {
        assert_eq!(log_level_rank("error"), Some(0));
        assert_eq!(log_level_rank("warn"), Some(1));
        assert_eq!(log_level_rank("info"), Some(2));
        assert_eq!(log_level_rank("debug"), Some(3));
        assert_eq!(log_level_rank("trace"), None);
        assert_eq!(cli_action_name(&CliAction::ConfigCheck), "config-check");
        assert_eq!(
            cli_action_name(&CliAction::Open {
                input: "https://private.example/?token=secret".into(),
                additional_inputs: Vec::new(),
                target: None,
                clean_link: false,
                explicit_input: true,
                ephemeral: false,
            }),
            "open"
        );
    }

    #[test]
    fn diagnostics_bound_and_order_profile_inspection() {
        let base = std::env::temp_dir().join(format!(
            "ferric-browser-diagnostics-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("storage roots");
        let profiles = roots.data.join("profiles");
        for index in 0..(MAX_DIAGNOSTIC_PROFILES + 2) {
            let id = Uuid::from_u128(index as u128).to_string();
            std::fs::create_dir_all(profiles.join(id)).expect("profile directory");
        }

        let report = storage_diagnostics(&roots);
        assert_eq!(report["profiles"].as_array().map(Vec::len), Some(128));
        assert_eq!(report["profile_limit"], MAX_DIAGNOSTIC_PROFILES);
        assert_eq!(report["profiles_truncated"], true);
        let ids = report["profiles"]
            .as_array()
            .expect("profile report")
            .iter()
            .map(|profile| profile["id"].as_str().expect("profile ID"))
            .collect::<Vec<_>>();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            ids.first().copied(),
            Some("00000000-0000-0000-0000-000000000000")
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn default_browser_is_an_explicit_cli_operation() {
        let (_, status) = parse_cli(&["default-browser".into(), "status".into()])
            .expect("valid default-browser status");
        assert!(matches!(
            status,
            CliAction::DefaultBrowser {
                operation: DefaultBrowserOperation::Status
            }
        ));

        let (_, set) = parse_cli(&["default-browser".into(), "set".into()])
            .expect("valid default-browser set");
        assert!(matches!(
            set,
            CliAction::DefaultBrowser {
                operation: DefaultBrowserOperation::Set
            }
        ));
        assert!(parse_cli(&["default-browser".into()]).is_err());
        assert!(parse_cli(&["default-browser".into(), "set".into(), "extra".into()]).is_err());
    }

    #[test]
    fn reset_data_requires_explicit_confirmation() {
        let (_, action) =
            parse_cli(&["reset-data".into(), "--confirm".into()]).expect("explicit reset command");
        assert!(matches!(action, CliAction::ResetData));
        assert!(parse_cli(&["reset-data".into()]).is_err());
        assert!(parse_cli(&["reset-data".into(), "--force".into()]).is_err());
    }

    #[test]
    fn cli_startup_scalar_options_are_not_overwritten() {
        for arguments in [
            vec![
                "--basedir".into(),
                "/tmp/one".into(),
                "--basedir".into(),
                "/tmp/two".into(),
            ],
            vec![
                "--config".into(),
                "/tmp/one.toml".into(),
                "--config".into(),
                "/tmp/two.toml".into(),
            ],
            vec![
                "--profile".into(),
                "one".into(),
                "--profile".into(),
                "two".into(),
            ],
            vec![
                "--context".into(),
                "one".into(),
                "--context".into(),
                "two".into(),
            ],
            vec![
                "--instance".into(),
                "one".into(),
                "--instance".into(),
                "two".into(),
            ],
            vec![
                "--log-level".into(),
                "info".into(),
                "--log-level".into(),
                "debug".into(),
            ],
            vec!["--safe-mode".into(), "--safe-mode".into()],
        ] {
            assert!(parse_cli(&arguments).is_err(), "arguments: {arguments:?}");
        }
    }

    #[test]
    fn cli_startup_settings_are_bounded() {
        let mut arguments = Vec::new();
        for index in 0..MAX_STARTUP_SETTINGS {
            arguments.push("--set".into());
            arguments.push(format!("setting{index}=value"));
        }
        assert!(parse_cli(&arguments).is_ok());
        arguments.push("--set".into());
        arguments.push("overflow=value".into());
        assert!(parse_cli(&arguments).is_err());
    }

    #[test]
    fn cli_selection_search_validates_engine_name_at_the_envelope_boundary() {
        let valid = command_params(&ParsedCommand {
            name: "selection-search".into(),
            arguments: vec!["--engine".into(), "ddg".into()],
        })
        .expect("valid selection-search envelope");
        assert_eq!(valid["arguments"]["engine"], "ddg");
        for engine in [String::new(), "bad\nengine".into()] {
            assert!(
                command_params(&ParsedCommand {
                    name: "selection-search".into(),
                    arguments: vec!["--engine".into(), engine],
                })
                .is_err()
            );
        }
    }

    #[test]
    fn cli_paste_open_preserves_target_and_primary_channel() {
        let typed = command_params(&ParsedCommand {
            name: "paste-open".into(),
            arguments: vec!["--target".into(), "tab".into(), "--primary".into()],
        })
        .expect("typed paste-open envelope");
        assert_eq!(
            typed["arguments"],
            serde_json::json!({"target": "tab", "primary": true})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "paste-open".into(),
                arguments: vec!["--target".into(), "window".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_context_create_validates_text_fields_at_the_envelope_boundary() {
        let valid = command_params(&ParsedCommand {
            name: "context-create".into(),
            arguments: vec![
                "work".into(),
                "--label".into(),
                "Work".into(),
                "--profile".into(),
                "work".into(),
                "--workspace".into(),
                "2".into(),
            ],
        })
        .expect("valid context-create envelope");
        assert_eq!(valid["arguments"]["name"], "work");
        assert_eq!(valid["arguments"]["label"], "Work");

        assert!(
            command_params(&ParsedCommand {
                name: "context-create".into(),
                arguments: vec!["work".into()],
            })
            .is_err()
        );

        for arguments in [
            vec![String::new()],
            vec!["work".into(), "--label".into(), "bad\nlabel".into()],
            vec!["work".into(), "--profile".into(), String::new()],
            vec!["work".into(), "--workspace".into(), "bad\tworkspace".into()],
        ] {
            assert!(
                command_params(&ParsedCommand {
                    name: "context-create".into(),
                    arguments,
                })
                .is_err()
            );
        }
    }

    #[test]
    fn cli_tab_move_context_preserves_optional_index() {
        let active = command_params(&ParsedCommand {
            name: "tab-move".into(),
            arguments: vec!["--context".into(), "work".into()],
        })
        .expect("active tab context move envelope");
        assert_eq!(active["arguments"], serde_json::json!({"context": "work"}));
        let indexed = command_params(&ParsedCommand {
            name: "tab-move".into(),
            arguments: vec!["2".into(), "--context".into(), "work".into()],
        })
        .expect("indexed context move envelope");
        assert_eq!(
            indexed["arguments"],
            serde_json::json!({"id": "2", "context": "work"})
        );
    }

    #[test]
    fn cli_tab_move_context_rejects_untrusted_text() {
        for arguments in [
            vec!["--context".into(), "work\nprod".into()],
            vec!["2\tstale".into(), "--context".into(), "work".into()],
        ] {
            assert!(
                command_params(&ParsedCommand {
                    name: "tab-move".into(),
                    arguments,
                })
                .is_err()
            );
        }
    }

    #[test]
    fn cli_tab_window_and_download_ids_reject_untrusted_text() {
        for (name, arguments) in [
            ("tab-give", vec!["window\n1".into()]),
            ("tab-select", vec!["tab\t1".into()]),
            ("tab-focus", vec!["tab\n1".into()]),
            ("tab-close", vec!["--id".into(), "tab\n1".into()]),
            ("window-focus", vec!["window\n1".into()]),
            (
                "window-move",
                vec!["window-1".into(), "workspace\t1".into()],
            ),
            ("download-open", vec!["download\n1".into()]),
        ] {
            assert!(
                command_params(&ParsedCommand {
                    name: name.into(),
                    arguments,
                })
                .is_err(),
                "{name} accepted untrusted identifier text"
            );
        }
    }

    #[test]
    fn cli_profile_and_context_names_validate_at_the_envelope_boundary() {
        for command_name in [
            "context-delete",
            "context-enter",
            "context-save",
            "profile-create",
            "profile-delete",
            "profile-open",
            "session-delete",
            "session-load",
            "session-save",
        ] {
            assert!(
                command_params(&ParsedCommand {
                    name: command_name.into(),
                    arguments: vec!["bad\nname".into()],
                })
                .is_err(),
                "command should reject control characters: {command_name}"
            );
        }
    }

    #[test]
    fn cli_library_identifiers_validate_at_the_envelope_boundary() {
        for (name, arguments) in [
            ("bookmark-delete", vec!["bad\nID".into()]),
            (
                "bookmark-edit",
                vec!["bad\nID".into(), "--title".into(), "Title".into()],
            ),
            ("bookmark-open", vec!["bad\nID".into()]),
            ("history-open", vec!["bad\nID".into()]),
            ("quickmark-add", vec!["bad\nname".into()]),
            ("quickmark-delete", vec!["bad\nname".into()]),
            (
                "quickmark-edit",
                vec!["bad\nname".into(), "https://example.test".into()],
            ),
            ("quickmark-open", vec!["bad\nname".into()]),
        ] {
            assert!(
                command_params(&ParsedCommand {
                    name: name.into(),
                    arguments,
                })
                .is_err(),
                "command should reject control characters: {name}"
            );
        }
    }
    use ferric_browser_config::{ContextDefinition, ProfileDefinition};
    use std::collections::BTreeMap;

    #[test]
    fn cli_open_keeps_colon_input_as_data() {
        let arguments = vec!["open".into(), "--".into(), ":https://example.test".into()];
        let (_, action) = parse_cli(&arguments).expect("valid open command");
        assert!(matches!(
            action,
            CliAction::Open { input, target: None, .. } if input == ":https://example.test"
        ));
    }

    #[test]
    fn cli_open_accepts_explicit_clean_link() {
        let arguments = vec![
            "open".into(),
            "--clean-link".into(),
            "https://example.test/?utm_source=demo".into(),
        ];
        let (_, action) = parse_cli(&arguments).expect("valid clean-link open");
        assert!(matches!(
            action,
            CliAction::Open {
                input,
                clean_link: true,
                explicit_input: true,
                ..
            } if input == "https://example.test/?utm_source=demo"
        ));
    }

    #[test]
    fn cli_open_accepts_typed_route_options_and_joins_input_tail() {
        let arguments = vec![
            "open".into(),
            "--target".into(),
            "current".into(),
            "--profile".into(),
            "work".into(),
            "--context".into(),
            "calendar".into(),
            "quarterly".into(),
            "report".into(),
        ];
        let (options, action) = parse_cli(&arguments).expect("valid typed open command");
        assert_eq!(options.profile.as_deref(), Some("work"));
        assert_eq!(options.context.as_deref(), Some("calendar"));
        assert!(matches!(
            action,
            CliAction::Open {
                input,
                target: Some(target),
                explicit_input: true,
                ..
            } if input == "quarterly report" && target == "tab"
        ));
    }

    #[test]
    fn cli_open_accepts_cold_start_background_target() {
        let (_, action) = parse_cli(&[
            "open".into(),
            "--target".into(),
            "tab-bg".into(),
            "https://example.test/one".into(),
        ])
        .expect("valid cold-start background open");
        assert!(matches!(
            action,
            CliAction::Open {
                input,
                target: Some(target),
                explicit_input: true,
                ..
            } if input == "https://example.test/one" && target == "tab-bg"
        ));
    }

    #[test]
    fn cli_open_rejects_clean_link_with_cold_start_background_target() {
        let error = parse_cli(&[
            "open".into(),
            "--target".into(),
            "tab-bg".into(),
            "--clean-link".into(),
            "https://example.test/one?utm_source=demo".into(),
        ])
        .expect_err("clean-link background open must be rejected before startup");
        assert!(error.contains("cannot be combined with --target tab-bg"));
    }

    #[test]
    fn cli_open_keeps_multiple_url_arguments_as_separate_inputs() {
        let (_, action) = parse_cli(&[
            "open".into(),
            "https://example.test/one".into(),
            "https://example.test/two".into(),
            "file:///tmp/three.html".into(),
        ])
        .expect("valid multiple-url open command");
        assert!(matches!(
            action,
            CliAction::Open {
                input,
                additional_inputs,
                explicit_input: true,
                ..
            } if input == "https://example.test/one"
                && additional_inputs == [
                    "https://example.test/two".to_owned(),
                    "file:///tmp/three.html".to_owned()
                ]
        ));

        let (_, action) = parse_cli(&["open".into(), "hello".into(), "world".into()])
            .expect("valid search input");
        assert!(matches!(
            action,
            CliAction::Open {
                input,
                additional_inputs,
                ..
            } if input == "hello world" && additional_inputs.is_empty()
        ));
    }

    #[test]
    fn cli_open_option_terminator_preserves_option_like_input_tail() {
        let arguments = vec![
            "open".into(),
            "--".into(),
            "--profile".into(),
            "literal".into(),
        ];
        let (_, action) = parse_cli(&arguments).expect("valid data-only open command");
        assert!(matches!(
            action,
            CliAction::Open { input, .. } if input == "--profile literal"
        ));
    }

    #[test]
    fn cli_catalog_commands_are_forwarded_instead_of_becoming_open_input() {
        let (_, action) = parse_cli(&["reload".into(), "--bypass-cache".into()])
            .expect("registered commands use the direct CLI entry point");
        assert!(matches!(
            action,
            CliAction::Command { text, window: None } if text == "reload --bypass-cache"
        ));

        let (_, action) = parse_cli(&["profile-list".into()])
            .expect("registered no-argument commands use the direct CLI entry point");
        assert!(matches!(
            action,
            CliAction::Command { text, window: None } if text == "profile-list"
        ));
    }

    #[test]
    fn cli_builtin_aliases_expand_before_typed_forwarding() {
        let commands =
            parse_chain("o https://example.test", ParseInput::Cli).expect("alias command parses");
        let expanded = CommandRegistry::default_v1()
            .expand_chain(commands)
            .expect("built-in alias expands");
        assert_eq!(expanded[0].name, "open");
        assert_eq!(expanded[0].arguments, vec!["https://example.test"]);
    }

    #[test]
    fn startup_profile_selection_follows_context_profile_definition() {
        let options = CliOptions {
            context: Some("calendar".into()),
            ..CliOptions::default()
        };
        let profiles = ProfilesConfig {
            profiles: vec![ProfileDefinition {
                name: "work".into(),
                label: "Work".into(),
                default: false,
                overrides: BTreeMap::default(),
            }],
            ..ProfilesConfig::default()
        };
        let contexts = ContextsConfig {
            contexts: vec![ContextDefinition {
                name: "calendar".into(),
                label: "Calendar".into(),
                profile: "work".into(),
                sessions: Vec::new(),
                workspace: None,
                accent: None,
                default_target: "reuse-or-window".into(),
            }],
            ..ContextsConfig::default()
        };
        let (_, name, label, startup_context) =
            select_startup_profile(&options, &profiles, &contexts, "default", "Default")
                .expect("context selects a valid startup profile");
        assert_eq!(name, "work");
        assert_eq!(label, "Work");
        assert_eq!(startup_context.as_deref(), Some("calendar"));
    }

    #[test]
    fn startup_profile_selection_rejects_conflicting_context_profile() {
        let options = CliOptions {
            profile: Some("personal".into()),
            context: Some("calendar".into()),
            ..CliOptions::default()
        };
        let contexts = ContextsConfig {
            contexts: vec![ContextDefinition {
                name: "calendar".into(),
                label: "Calendar".into(),
                profile: "work".into(),
                sessions: Vec::new(),
                workspace: None,
                accent: None,
                default_target: "reuse-or-window".into(),
            }],
            ..ContextsConfig::default()
        };
        let error = select_startup_profile(
            &options,
            &ProfilesConfig::default(),
            &contexts,
            "default",
            "Default",
        )
        .expect_err("conflicting selectors must be rejected");
        assert!(error.contains("profile and context selectors disagree"));
    }

    #[test]
    fn cli_open_accepts_ephemeral_profile_flag() {
        let (_, action) = parse_cli(&[
            "open".into(),
            "--ephemeral".into(),
            "--".into(),
            "https://example.test".into(),
        ])
        .expect("valid ephemeral open");
        assert!(matches!(
            action,
            CliAction::Open {
                ephemeral: true,
                ..
            }
        ));
    }

    #[test]
    fn cli_hint_accepts_ephemeral_target_without_rapid_mode() {
        let command = parse_chain("hint --target ephemeral links", ParseInput::Cli)
            .expect("valid ephemeral hint command")
            .pop()
            .expect("hint command");
        let params = command_params(&command).expect("typed ephemeral hint parameters");
        assert_eq!(params["arguments"]["target"], "ephemeral");
        assert_eq!(params["arguments"]["kind"], "links");
    }

    #[test]
    fn cli_profile_create_accepts_ephemeral_flag() {
        let command = parse_chain("profile-create task --ephemeral", ParseInput::Cli)
            .expect("valid ephemeral profile command")
            .pop()
            .expect("profile command");
        let params = command_params(&command).expect("typed profile parameters");
        assert_eq!(
            params["arguments"],
            serde_json::json!({"name": "task", "ephemeral": true})
        );
    }

    #[test]
    fn cli_profile_open_accepts_an_optional_url() {
        let command = parse_chain("profile-open work https://example.test", ParseInput::Cli)
            .expect("valid profile-open command")
            .pop()
            .expect("profile-open command");
        let params = command_params(&command).expect("typed profile-open parameters");
        assert_eq!(
            params["arguments"],
            serde_json::json!({"name": "work", "input": "https://example.test"})
        );
    }

    #[test]
    fn cli_profile_delete_keeps_the_name_typed() {
        let command = parse_chain("profile-delete work", ParseInput::Cli)
            .expect("valid profile-delete command")
            .pop()
            .expect("profile-delete command");
        let params = command_params(&command).expect("typed profile-delete parameters");
        assert_eq!(params["arguments"], serde_json::json!({"name": "work"}));
    }

    #[test]
    fn cli_fullscreen_defaults_to_toggle_and_validates_state() {
        let command = parse_chain("fullscreen", ParseInput::Cli)
            .expect("valid fullscreen command")
            .pop()
            .expect("fullscreen command");
        let params = command_params(&command).expect("typed fullscreen parameters");
        assert_eq!(params["arguments"], serde_json::json!({"state": "toggle"}));
        let command = parse_chain("fullscreen off", ParseInput::Cli)
            .expect("valid fullscreen off command")
            .pop()
            .expect("fullscreen command");
        assert_eq!(
            command_params(&command).expect("fullscreen off parameters")["arguments"],
            serde_json::json!({"state": "off"})
        );
        assert!(
            parse_chain("fullscreen sideways", ParseInput::Cli)
                .ok()
                .and_then(|mut commands| commands.pop())
                .and_then(|command| command_params(&command).ok())
                .is_none()
        );
    }

    #[test]
    fn cli_window_new_types_profile_and_private_options() {
        let command = parse_chain("window-new --profile work --private", ParseInput::Cli)
            .expect("valid window-new command")
            .pop()
            .expect("window-new command");
        let params = command_params(&command).expect("typed window-new parameters");
        assert_eq!(
            params["arguments"],
            serde_json::json!({"profile": "work", "private": true})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "window-new".into(),
                arguments: vec!["--private".into(), "--private".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_tab_open_types_input_and_background_flag() {
        let command = parse_chain(
            "tab-open --background -- https://example.test",
            ParseInput::Cli,
        )
        .expect("valid tab-open command")
        .pop()
        .expect("tab-open command");
        assert_eq!(
            command_params(&command).expect("typed tab-open parameters")["arguments"],
            serde_json::json!({"input": "https://example.test", "background": true})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "tab-open".into(),
                arguments: vec!["--background".into(), "--background".into(), "x".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_history_commands_type_bounded_counts() {
        let command = parse_chain("back --count 4", ParseInput::Cli)
            .expect("valid history command")
            .pop()
            .expect("back command");
        assert_eq!(
            command_params(&command).expect("typed history parameters")["arguments"],
            serde_json::json!({"count": 4})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "forward".into(),
                arguments: vec!["--count".into(), "101".into()],
            })
            .is_err()
        );
        let command = parse_chain("tab-next --count 4", ParseInput::Cli)
            .expect("valid tab traversal command")
            .pop()
            .expect("tab-next command");
        assert_eq!(
            command_params(&command).expect("typed tab traversal parameters")["arguments"],
            serde_json::json!({"count": 4})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "tab-prev".into(),
                arguments: vec!["--count".into(), "101".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_history_clear_forwards_exact_scope_filters() {
        let command = parse_chain(
            "history-clear --since 1757894400 --origin https://example.test --confirm",
            ParseInput::Cli,
        )
        .expect("valid history clear command")
        .pop()
        .expect("history clear command");
        assert_eq!(
            command_params(&command).expect("typed history clear parameters"),
            serde_json::json!({
                "command": "history-clear",
                "arguments": {
                    "since": 1_757_894_400,
                    "origin": "https://example.test",
                    "confirmed": true
                }
            })
        );
        assert!(
            command_params(&ParsedCommand {
                name: "history-clear".into(),
                arguments: vec![
                    "--origin".into(),
                    "https://example.test/path".into(),
                    "--confirm".into()
                ],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_tab_give_types_the_target_window_id() {
        let command = parse_chain("tab-give window-42", ParseInput::Cli)
            .expect("valid tab-give command")
            .pop()
            .expect("tab-give command");
        let params = command_params(&command).expect("typed tab-give parameters");
        assert_eq!(
            params["arguments"],
            serde_json::json!({"window_id": "window-42"})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "tab-give".into(),
                arguments: Vec::new(),
            })
            .is_err()
        );
    }

    #[test]
    fn cli_reload_types_bypass_cache() {
        let command = parse_chain("reload --bypass-cache", ParseInput::Cli)
            .expect("valid reload command")
            .pop()
            .expect("reload command");
        let params = command_params(&command).expect("typed reload parameters");
        assert_eq!(
            params["arguments"],
            serde_json::json!({"bypass_cache": true})
        );
        let command = parse_chain("reload", ParseInput::Cli)
            .expect("valid default reload command")
            .pop()
            .expect("reload command");
        assert_eq!(
            command_params(&command).expect("typed default reload parameters")["arguments"],
            serde_json::json!({"bypass_cache": false})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "reload".into(),
                arguments: vec!["--bypass-cache".into(), "--bypass-cache".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_tab_close_types_optional_id_and_count() {
        let command = parse_chain("tab-close --id tab-42", ParseInput::Cli)
            .expect("valid targeted tab-close command")
            .pop()
            .expect("tab-close command");
        assert_eq!(
            command_params(&command).expect("typed targeted tab-close parameters")["arguments"],
            serde_json::json!({"id": "tab-42"})
        );
        let command = parse_chain("tab-close --count 3", ParseInput::Cli)
            .expect("valid bulk tab-close command")
            .pop()
            .expect("tab-close command");
        assert_eq!(
            command_params(&command).expect("typed bulk tab-close parameters")["arguments"],
            serde_json::json!({"count": 3})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "tab-close".into(),
                arguments: vec!["--id".into(), "tab-42".into(), "--count".into(), "2".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_search_types_query_direction_and_case() {
        let command = parse_chain(
            "search --backward --case sensitive \"Ferric Browser\"",
            ParseInput::Cli,
        )
        .expect("valid search command")
        .pop()
        .expect("search command");
        let params = command_params(&command).expect("typed search parameters");
        assert_eq!(
            params["arguments"],
            serde_json::json!({
                "query": "Ferric Browser",
                "backward": true,
                "case": "sensitive"
            })
        );
        assert!(
            command_params(&ParsedCommand {
                name: "search".into(),
                arguments: vec!["--case".into(), "bogus".into(), "query".into()],
            })
            .is_err()
        );
        assert!(
            command_params(&ParsedCommand {
                name: "search".into(),
                arguments: vec![
                    "--case".into(),
                    "sensitive".into(),
                    "--case".into(),
                    "smart".into(),
                    "query".into(),
                ],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_journey_accepts_current_filter() {
        let command = parse_chain("journey --current", ParseInput::Cli)
            .expect("valid journey command")
            .pop()
            .expect("journey command");
        let params = command_params(&command).expect("typed journey parameters");
        assert_eq!(params["arguments"], serde_json::json!({"current": true}));

        for arguments in [
            vec!["--current".into(), "--current".into()],
            vec![
                "--search".into(),
                "one".into(),
                "--search".into(),
                "two".into(),
            ],
            vec![
                "--expand".into(),
                "one".into(),
                "--expand".into(),
                "two".into(),
            ],
        ] {
            assert!(
                command_params(&ParsedCommand {
                    name: "journey".into(),
                    arguments,
                })
                .is_err()
            );
        }
    }

    #[test]
    fn cli_journey_accepts_bounded_search_and_expansion_filters() {
        let command = parse_chain("journey --search \"example.test\"", ParseInput::Cli)
            .expect("valid journey search command")
            .pop()
            .expect("journey command");
        let params = command_params(&command).expect("typed journey search parameters");
        assert_eq!(params["arguments"]["search"], "example.test");

        for search in ["", "bad\nsearch", &"x".repeat(257)] {
            assert!(
                command_params(&ParsedCommand {
                    name: "journey".into(),
                    arguments: vec!["--search".into(), search.into()],
                })
                .is_err()
            );
        }

        let command = parse_chain(
            "journey --expand 123e4567-e89b-12d3-a456-426614174000",
            ParseInput::Cli,
        )
        .expect("valid journey expansion command")
        .pop()
        .expect("journey command");
        let params = command_params(&command).expect("typed journey expansion parameters");
        assert_eq!(
            params["arguments"]["expand"],
            "123e4567-e89b-12d3-a456-426614174000"
        );
        assert!(
            command_params(&ParsedCommand {
                name: "journey".into(),
                arguments: vec!["--expand".into(), "not-a-uuid".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_journey_reopen_accepts_uuid_and_target() {
        let command = parse_chain(
            "journey-reopen 123e4567-e89b-12d3-a456-426614174000 --target tab",
            ParseInput::Cli,
        )
        .expect("valid journey reopen command")
        .pop()
        .expect("journey reopen command");
        let params = command_params(&command).expect("typed journey reopen parameters");
        assert_eq!(
            params["arguments"],
            serde_json::json!({
                "node": "123e4567-e89b-12d3-a456-426614174000",
                "target": "tab"
            })
        );
        assert!(
            command_params(&ParsedCommand {
                name: "journey-reopen".into(),
                arguments: vec!["not-a-uuid".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_journey_reopen_accepts_window_target() {
        let command = parse_chain(
            "journey-reopen 123e4567-e89b-12d3-a456-426614174000 --target window",
            ParseInput::Cli,
        )
        .expect("valid window journey reopen command")
        .pop()
        .expect("journey command");
        let params = command_params(&command).expect("typed window journey parameters");
        assert_eq!(params["arguments"]["target"], "window");
    }

    #[test]
    fn cli_query_selects_active_tab_without_starting_a_gui() {
        let arguments = vec![
            "query".into(),
            "active-tab".into(),
            "--format".into(),
            "json".into(),
        ];
        let (_, action) = parse_cli(&arguments).expect("valid query command");
        assert!(matches!(
            action,
            CliAction::Query { method, params, format }
                if method == "tabs.query"
                    && params["active_only"] == true
                    && format == "json"
        ));
    }

    #[test]
    fn cli_query_exposes_live_window_registry() {
        let arguments = vec![
            "query".into(),
            "windows".into(),
            "--format".into(),
            "json".into(),
        ];
        let (_, action) = parse_cli(&arguments).expect("valid windows query command");
        assert!(matches!(
            action,
            CliAction::Query { method, params, format }
                if method == "windows.query" && params == serde_json::json!({}) && format == "json"
        ));
    }

    #[test]
    fn cli_switcher_command_uses_typed_scope_and_query_fields() {
        let command = parse_chain("switcher --scope tabs rust", ParseInput::Cli)
            .expect("valid switcher command")
            .remove(0);
        let params = command_params(&command).expect("typed switcher parameters");
        assert_eq!(params["arguments"]["scope"], "tabs");
        assert_eq!(params["arguments"]["query"], "rust");
        assert!(
            command_params(&ParsedCommand {
                name: "switcher".into(),
                arguments: vec![
                    "--scope".into(),
                    "tabs".into(),
                    "--scope".into(),
                    "history".into(),
                ],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_activate_accepts_only_stable_tab_ids() {
        let (_, action) = parse_cli(&["activate".into(), "tab".into(), "tab-123".into()])
            .expect("valid tab activation");
        assert!(matches!(
            action,
            CliAction::Activate { kind, id } if kind == "tab" && id == "tab-123"
        ));
        assert!(parse_cli(&["activate".into(), "window".into(), "window-123".into()]).is_err());
    }

    #[test]
    fn cli_tab_and_window_commands_use_typed_subject_fields() {
        let select = command_params(&ParsedCommand {
            name: "tab-select".into(),
            arguments: vec!["tab-42".into()],
        })
        .expect("typed tab-select parameters");
        assert_eq!(select["arguments"]["selector"], "tab-42");

        let mute = command_params(&ParsedCommand {
            name: "tab-mute".into(),
            arguments: vec!["tab-42".into(), "off".into()],
        })
        .expect("typed tab-mute parameters");
        assert_eq!(mute["arguments"]["id"], "tab-42");
        assert_eq!(mute["arguments"]["state"], "off");
        let mute_active = command_params(&ParsedCommand {
            name: "tab-mute".into(),
            arguments: vec!["toggle".into()],
        })
        .expect("typed active-tab mute parameters");
        assert!(mute_active["arguments"].get("id").is_none());
        assert_eq!(mute_active["arguments"]["state"], "toggle");

        let move_tab = command_params(&ParsedCommand {
            name: "tab-move".into(),
            arguments: vec!["tab-42".into(), "left".into()],
        })
        .expect("typed tab-move parameters");
        assert_eq!(move_tab["arguments"]["direction"], "left");
        let move_active = command_params(&ParsedCommand {
            name: "tab-move".into(),
            arguments: vec!["right".into()],
        })
        .expect("typed active-tab move parameters");
        assert!(move_active["arguments"].get("id").is_none());
        assert_eq!(move_active["arguments"]["direction"], "right");

        let focus = command_params(&ParsedCommand {
            name: "window-focus".into(),
            arguments: vec!["window-7".into()],
        })
        .expect("typed window-focus parameters");
        assert_eq!(focus["arguments"]["id"], "window-7");
    }

    #[test]
    fn cli_navigation_and_configuration_commands_use_typed_arguments() {
        let tab_open = command_params(&ParsedCommand {
            name: "tab-open".into(),
            arguments: vec!["--background".into(), "https://example.test/a".into()],
        })
        .expect("typed tab-open parameters");
        assert_eq!(tab_open["arguments"]["input"], "https://example.test/a");
        assert_eq!(tab_open["arguments"]["background"], true);

        let scroll = command_params(&ParsedCommand {
            name: "scroll-page".into(),
            arguments: vec!["down".into(), "--half".into(), "--count".into(), "3".into()],
        })
        .expect("typed scroll-page parameters");
        assert_eq!(scroll["arguments"]["direction"], "down");
        assert_eq!(scroll["arguments"]["half"], true);
        assert_eq!(scroll["arguments"]["count"], 3);

        let binding = command_params(&ParsedCommand {
            name: "bind".into(),
            arguments: vec![
                "--mode".into(),
                "normal".into(),
                "gg".into(),
                "scroll-page down".into(),
            ],
        })
        .expect("typed bind parameters");
        assert_eq!(binding["arguments"]["mode"], "normal");
        assert_eq!(binding["arguments"]["keychain"], "gg");
        assert_eq!(binding["arguments"]["command"], "scroll-page down");

        let search = command_params(&ParsedCommand {
            name: "search-next".into(),
            arguments: vec!["--backward".into(), "--count".into(), "4".into()],
        })
        .expect("typed search-next parameters");
        assert_eq!(search["arguments"]["direction"], "backward");
        assert_eq!(search["arguments"]["count"], 4);

        assert!(
            command_params(&ParsedCommand {
                name: "zoom".into(),
                arguments: vec!["9".into()],
            })
            .is_err()
        );
        assert!(
            command_params(&ParsedCommand {
                name: "search-next".into(),
                arguments: vec!["--count".into(), "2".into(), "--count".into(), "3".into()],
            })
            .is_err()
        );
        assert!(
            command_params(&ParsedCommand {
                name: "bind".into(),
                arguments: vec!["--mode".into(), "invalid".into(), "g".into(), "help".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn every_registered_command_example_has_a_typed_cli_envelope() {
        let registry = CommandRegistry::default_v1();
        let mut checked = 0;
        for definition in registry.definitions() {
            for example in &definition.examples {
                let commands = parse_chain(example, ParseInput::Cli).unwrap_or_else(|error| {
                    panic!("{} example does not parse: {error}", definition.name)
                });
                let commands = registry.expand_chain(commands).unwrap_or_else(|error| {
                    panic!("{} example does not expand: {error}", definition.name)
                });
                for command in commands {
                    command_params(&command).unwrap_or_else(|error| {
                        panic!(
                            "{} example has no typed CLI envelope: {error}",
                            definition.name
                        )
                    });
                    checked += 1;
                }
            }
        }
        assert!(
            checked >= 100,
            "expected broad command example coverage, got {checked}"
        );
    }

    #[test]
    fn cli_query_supports_bindings_and_config_keys() {
        let (_, action) = parse_cli(&[
            "query".into(),
            "bindings".into(),
            "--mode".into(),
            "normal".into(),
        ])
        .expect("valid binding query");
        assert!(matches!(
            action,
            CliAction::Query { method, params, .. }
                if method == "bindings.query" && params["mode"] == "normal"
        ));

        let (_, action) =
            parse_cli(&["query".into(), "operations".into()]).expect("valid operations query");
        assert!(matches!(
            action,
            CliAction::Query { method, params, .. }
                if method == "operations.query" && params == serde_json::json!({})
        ));

        let (_, action) =
            parse_cli(&["query".into(), "blocking".into()]).expect("valid blocking query");
        assert!(matches!(
            action,
            CliAction::Query { method, params, .. }
                if method == "blocking.status" && params == serde_json::json!({})
        ));

        let (_, action) = parse_cli(&[
            "query".into(),
            "config".into(),
            "ui.font_size_pt".into(),
            "--explain".into(),
        ])
        .expect("valid config query");
        assert!(matches!(
            action,
            CliAction::Query { method, params, .. }
                if method == "config.get"
                    && params["key"] == "ui.font_size_pt"
                    && params["explain"] == true
        ));

        let (_, action) = parse_cli(&[
            "query".into(),
            "config".into(),
            "content.zoom".into(),
            "--url".into(),
            "https://example.test/docs".into(),
        ])
        .expect("valid site config query");
        assert!(matches!(
            action,
            CliAction::Query { method, params, .. }
                if method == "config.get"
                    && params["key"] == "content.zoom"
                    && params["url"] == "https://example.test/docs"
        ));

        let (_, action) = parse_cli(&["query".into(), "contexts".into(), "--members".into()])
            .expect("valid context query");
        assert!(matches!(
            action,
            CliAction::Query { method, params, .. }
                if method == "contexts.query" && params["include_members"] == true
        ));

        let (_, action) = parse_cli(&[
            "query".into(),
            "switcher".into(),
            "--scope".into(),
            "history".into(),
            "rust".into(),
            "--limit".into(),
            "12".into(),
        ])
        .expect("valid switcher query");
        assert!(matches!(
            action,
            CliAction::Query { method, params, .. }
                if method == "switcher.query"
                    && params["scope"] == "history"
                    && params["query"] == "rust"
                    && params["limit"] == 12
        ));

        let (_, action) = parse_cli(&[
            "query".into(),
            "binding-explain".into(),
            "2gt".into(),
            "--mode".into(),
            "normal".into(),
        ])
        .expect("valid binding explanation query");
        assert!(matches!(
            action,
            CliAction::Query { method, params, .. }
                if method == "bindings.explain"
                    && params["keychain"] == "2gt"
                    && params["mode"] == "normal"
        ));
    }

    #[test]
    fn cli_blocking_toggle_forwards_optional_site_flag() {
        let site = ParsedCommand {
            name: "blocking-toggle".into(),
            arguments: vec!["--site".into()],
        };
        assert_eq!(
            command_params(&site).expect("site toggle params")["arguments"],
            serde_json::json!({"site": true})
        );
        let global = ParsedCommand {
            name: "blocking-toggle".into(),
            arguments: Vec::new(),
        };
        assert_eq!(
            command_params(&global).expect("global toggle params")["arguments"],
            serde_json::json!({})
        );
    }

    #[test]
    fn cli_site_status_forwards_optional_tab_target() {
        let active = ParsedCommand {
            name: "site-status".into(),
            arguments: Vec::new(),
        };
        assert_eq!(
            command_params(&active).expect("active site status params")["arguments"],
            serde_json::json!({})
        );

        let targeted = ParsedCommand {
            name: "site-status".into(),
            arguments: vec!["--tab".into(), "tab-42".into()],
        };
        assert_eq!(
            command_params(&targeted).expect("targeted site status params")["arguments"],
            serde_json::json!({"tab": "tab-42"})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "site-status".into(),
                arguments: vec!["--tab".into(), "bad\nid".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_link_commands_forward_an_optional_url_field() {
        let command = parse_chain(
            "url-explain https://example.test/?utm_source=demo",
            ParseInput::Cli,
        )
        .expect("valid URL explanation command")
        .remove(0);
        assert_eq!(
            command_params(&command).expect("typed command parameters"),
            serde_json::json!({
                "command": "url-explain",
                "arguments": {"url": "https://example.test/?utm_source=demo"}
            })
        );
        let current = parse_chain("url-clean", ParseInput::Cli)
            .expect("valid current URL command")
            .remove(0);
        assert_eq!(
            command_params(&current).expect("current URL parameters"),
            serde_json::json!({"command": "url-clean", "arguments": {}})
        );
    }

    #[test]
    fn cli_hint_commands_forward_rapid_target_options() {
        let command = parse_chain("hint --rapid --target yank links", ParseInput::Cli)
            .expect("valid rapid hint command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed rapid hint parameters"),
            serde_json::json!({
                "command": "hint",
                "arguments": {"kind": "links", "target": "yank", "rapid": true}
            })
        );
    }

    #[test]
    fn cli_hint_commands_accept_qutebrowser_one_shot_targets() {
        for target in ["tab", "tab-bg", "window", "yank", "clean-yank", "download"] {
            let command = parse_chain(&format!("hint --target {target} links"), ParseInput::Cli)
                .expect("valid one-shot hint command")
                .remove(0);
            let params = command_params(&command).expect("typed one-shot hint parameters");
            assert_eq!(params["arguments"]["target"], target);
            assert_eq!(params["arguments"]["kind"], "links");
        }
    }

    #[test]
    fn cli_hint_commands_forward_userscript_target() {
        let command = parse_chain(
            "hint --target userscript --script video links",
            ParseInput::Cli,
        )
        .expect("valid userscript hint command")
        .remove(0);
        assert_eq!(
            command_params(&command).expect("typed userscript hint parameters"),
            serde_json::json!({
                "command": "hint",
                "arguments": {
                    "kind": "links",
                    "target": "userscript",
                    "script": "video"
                }
            })
        );
        let command = parse_chain(
            "hint --rapid --target userscript --script video links",
            ParseInput::Cli,
        )
        .expect("valid rapid userscript hint command")
        .remove(0);
        assert_eq!(
            command_params(&command).expect("typed rapid userscript hint parameters"),
            serde_json::json!({
                "command": "hint",
                "arguments": {
                    "kind": "links",
                    "target": "userscript",
                    "script": "video",
                    "rapid": true
                }
            })
        );
    }

    #[test]
    fn cli_download_commands_use_a_typed_url_field() {
        let command = parse_chain("download https://example.test/file.zip", ParseInput::Cli)
            .expect("valid download command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed download parameters"),
            serde_json::json!({
                "command": "download",
                "arguments": {"input": "https://example.test/file.zip"}
            })
        );
    }

    #[test]
    fn cli_print_pdf_command_uses_a_typed_path_field() {
        let command = parse_chain("print-pdf /tmp/page.pdf", ParseInput::Cli)
            .expect("valid print-pdf command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed print-pdf parameters"),
            serde_json::json!({
                "command": "print-pdf",
                "arguments": {"path": "/tmp/page.pdf"}
            })
        );
    }

    #[test]
    fn cli_save_page_command_uses_a_typed_path_field() {
        let command = parse_chain("save-page /tmp/page.html", ParseInput::Cli)
            .expect("valid save-page command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed save-page parameters"),
            serde_json::json!({
                "command": "save-page",
                "arguments": {"path": "/tmp/page.html"}
            })
        );
    }

    #[test]
    fn cli_script_run_command_uses_a_typed_name_field() {
        let command = parse_chain("script-run video", ParseInput::Cli)
            .expect("valid script-run command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed script-run parameters"),
            serde_json::json!({
                "command": "script-run",
                "arguments": {"name": "video"}
            })
        );
    }

    #[test]
    fn cli_jseval_command_uses_typed_world_and_script_fields() {
        let command = parse_chain(
            "jseval --world page \"document.body.dataset.ferric-browser = '1'\"",
            ParseInput::Cli,
        )
        .expect("valid jseval command")
        .remove(0);
        assert_eq!(
            command_params(&command).expect("typed jseval parameters"),
            serde_json::json!({
                "command": "jseval",
                "arguments": {
                    "world": "page",
                    "script": "document.body.dataset.ferric-browser = '1'"
                }
            })
        );
        let default_world = parse_chain("jseval \"1 + 1\"", ParseInput::Cli)
            .expect("default jseval world")
            .remove(0);
        assert_eq!(
            command_params(&default_world).expect("default jseval parameters"),
            serde_json::json!({"command": "jseval", "arguments": {"world": "isolated", "script": "1 + 1"}})
        );
    }

    #[test]
    fn cli_print_command_uses_an_empty_typed_argument_object() {
        let command = parse_chain("print", ParseInput::Cli)
            .expect("valid print command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed print parameters"),
            serde_json::json!({
                "command": "print",
                "arguments": {}
            })
        );
    }

    #[test]
    fn cli_devtools_command_uses_typed_detach_flag() {
        let command = parse_chain("devtools --detach", ParseInput::Cli)
            .expect("valid devtools command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed devtools parameters"),
            serde_json::json!({
                "command": "devtools",
                "arguments": {"detach": true}
            })
        );
        let toggle = parse_chain("devtools", ParseInput::Cli)
            .expect("valid devtools toggle")
            .remove(0);
        assert_eq!(
            command_params(&toggle).expect("typed devtools toggle parameters"),
            serde_json::json!({
                "command": "devtools",
                "arguments": {"detach": false}
            })
        );
    }

    #[test]
    fn cli_configuration_lifecycle_commands_use_typed_paths_and_no_arguments() {
        for name in [
            "config-edit",
            "config-reload",
            "config-check",
            "theme-reload",
        ] {
            let command = ParsedCommand {
                name: name.into(),
                arguments: Vec::new(),
            };
            assert_eq!(
                command_params(&command).expect("typed lifecycle command")["arguments"],
                serde_json::json!({})
            );
            assert!(
                command_params(&ParsedCommand {
                    name: name.into(),
                    arguments: vec!["unexpected".into()],
                })
                .is_err()
            );
        }
        let command = parse_chain(
            "config-write-defaults /tmp/ferric-browser-defaults.toml",
            ParseInput::Cli,
        )
        .expect("valid config-write-defaults command")
        .pop()
        .expect("config-write-defaults command");
        assert_eq!(
            command_params(&command).expect("typed config-write-defaults")["arguments"],
            serde_json::json!({"path": "/tmp/ferric-browser-defaults.toml"})
        );
    }

    #[test]
    fn cli_get_command_uses_typed_key_url_and_explain_fields() {
        let command = parse_chain(
            "get content.zoom --url https://example.test/docs --explain",
            ParseInput::Cli,
        )
        .expect("valid get command")
        .remove(0);
        assert_eq!(
            command_params(&command).expect("typed get parameters")["arguments"],
            serde_json::json!({
                "key": "content.zoom",
                "url": "https://example.test/docs",
                "explain": true
            })
        );
        assert!(
            command_params(&ParsedCommand {
                name: "get".into(),
                arguments: vec![
                    "content.zoom".into(),
                    "--explain".into(),
                    "--explain".into()
                ],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_support_commands_use_bounded_typed_arguments() {
        let command = parse_chain("help content.zoom", ParseInput::Cli)
            .expect("valid help command")
            .pop()
            .expect("help command");
        assert_eq!(
            command_params(&command).expect("typed help parameters")["arguments"],
            serde_json::json!({"topic": "content.zoom"})
        );
        for name in ["version", "diagnostics"] {
            let command = ParsedCommand {
                name: name.into(),
                arguments: Vec::new(),
            };
            assert_eq!(
                command_params(&command).expect("typed support parameters")["arguments"],
                serde_json::json!({})
            );
        }
        assert!(
            command_params(&ParsedCommand {
                name: "help".into(),
                arguments: vec!["one".into(), "two".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_permission_commands_use_typed_fields() {
        let command = parse_chain("permissions https://example.test", ParseInput::Cli)
            .expect("valid permissions command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed permissions parameters"),
            serde_json::json!({
                "command": "permissions",
                "arguments": {"origin": "https://example.test"}
            })
        );
        let command = parse_chain(
            "permission-reset https://example.test notifications",
            ParseInput::Cli,
        )
        .expect("valid permission reset command")
        .remove(0);
        assert_eq!(
            command_params(&command).expect("typed permission reset parameters"),
            serde_json::json!({
                "command": "permission-reset",
                "arguments": {
                    "origin": "https://example.test",
                    "permission": "notifications"
                }
            })
        );
    }

    #[test]
    fn cli_site_data_clear_uses_typed_origin_and_confirmation() {
        let command = parse_chain(
            "site-data-clear https://example.test --confirm",
            ParseInput::Cli,
        )
        .expect("valid site-data clear command")
        .remove(0);
        assert_eq!(
            command_params(&command).expect("typed site-data clear parameters"),
            serde_json::json!({
                "command": "site-data-clear",
                "arguments": {"origin": "https://example.test", "confirmed": true}
            })
        );
    }

    #[test]
    fn cli_download_desktop_commands_use_a_typed_id_field() {
        for name in [
            "download-open",
            "download-show",
            "download-cancel",
            "download-pause",
            "download-resume",
            "download-retry",
        ] {
            let command = parse_chain(&format!("{name} download-1"), ParseInput::Cli)
                .expect("valid download desktop command")
                .remove(0);
            assert_eq!(
                command_params(&command).expect("typed download desktop parameters"),
                serde_json::json!({
                    "command": name,
                    "arguments": {"id": "download-1"}
                })
            );
        }
    }

    #[test]
    fn cli_action_commands_use_typed_subject_and_verb_fields() {
        let command = parse_chain(
            "action url explain https://example.test/?utm_source=demo",
            ParseInput::Cli,
        )
        .expect("valid action command")
        .remove(0);
        assert_eq!(
            command_params(&command).expect("typed action parameters"),
            serde_json::json!({
                "command": "action",
                "arguments": {
                    "subject": "url",
                    "verb": "explain",
                    "url": "https://example.test/?utm_source=demo"
                }
            })
        );
        let listing = parse_chain("action-list url", ParseInput::Cli)
            .expect("valid action list")
            .remove(0);
        assert_eq!(
            command_params(&listing).expect("typed action list parameters"),
            serde_json::json!({"command": "action-list", "arguments": {"subject": "url"}})
        );
    }

    #[test]
    fn cli_send_and_command_management_use_typed_envelopes() {
        for (text, expected) in [
            (
                "send mpv --selection",
                serde_json::json!({
                    "target": "mpv",
                    "selection": true,
                    "send_subject": "selection"
                }),
            ),
            (
                "send mpv --url https://example.test",
                serde_json::json!({
                    "target": "mpv",
                    "url": "https://example.test",
                    "send_subject": "url"
                }),
            ),
        ] {
            let command = parse_chain(text, ParseInput::Cli)
                .expect("valid send command")
                .remove(0);
            assert_eq!(
                command_params(&command).expect("typed send parameters")["arguments"],
                expected
            );
        }

        let help = parse_chain("command-help open", ParseInput::Cli)
            .expect("valid command help")
            .remove(0);
        assert_eq!(
            command_params(&help).expect("typed command help parameters")["arguments"],
            serde_json::json!({"id": "open"})
        );
        let execute = parse_chain(
            r#"command-execute open '{"target":"tab"}'"#,
            ParseInput::Cli,
        )
        .expect("valid command execute")
        .remove(0);
        assert_eq!(
            command_params(&execute).expect("typed command execute parameters")["arguments"],
            serde_json::json!({"id": "open", "arguments": {"target": "tab"}})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "send".into(),
                arguments: vec!["mpv".into(), "--selection".into(), "extra".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn cli_context_route_commands_use_typed_fields() {
        let add = parse_chain(
            "context-route add 'https://*.company.test/*' work",
            ParseInput::Cli,
        )
        .expect("valid context route add")
        .remove(0);
        assert_eq!(
            command_params(&add).expect("typed context route add parameters"),
            serde_json::json!({
                "command": "context-route",
                "arguments": {
                    "action": "add",
                    "pattern": "https://*.company.test/*",
                    "context": "work"
                }
            })
        );
        let configured = parse_chain(
            "context-route add 'https://*.company.test/*' work --priority -4 --behavior suggest --entry-point explicit-open --entry-point typed-initial-url",
            ParseInput::Cli,
        )
        .expect("valid configured context route add")
        .remove(0);
        assert_eq!(
            command_params(&configured).expect("typed configured route parameters"),
            serde_json::json!({
                "command": "context-route",
                "arguments": {
                    "action": "add",
                    "pattern": "https://*.company.test/*",
                    "context": "work",
                    "priority": -4,
                    "behavior": "suggest",
                    "entry_points": ["explicit-open", "typed-initial-url"]
                }
            })
        );
        let remove = parse_chain("context-route remove company-work", ParseInput::Cli)
            .expect("valid context route remove")
            .remove(0);
        assert_eq!(
            command_params(&remove).expect("typed context route remove parameters"),
            serde_json::json!({
                "command": "context-route",
                "arguments": {"action": "remove", "id": "company-work"}
            })
        );
        let open = parse_chain("open https://example.test", ParseInput::Cli)
            .expect("valid external open")
            .remove(0);
        assert_eq!(
            command_params(&open).expect("typed external open parameters"),
            serde_json::json!({
                "command": "open",
                "arguments": {
                    "input": "https://example.test",
                    "external": true
                }
            })
        );

        for arguments in [
            vec!["add".into(), "bad\npattern".into(), "work".into()],
            vec![
                "add".into(),
                "https://*.company.test/*".into(),
                "bad\tcontext".into(),
            ],
            vec!["remove".into(), String::new()],
        ] {
            assert!(
                command_params(&ParsedCommand {
                    name: "context-route".into(),
                    arguments,
                })
                .is_err()
            );
        }
    }

    #[test]
    fn cli_yank_commands_use_typed_url_and_clean_fields() {
        let command = parse_chain("yank url --clean", ParseInput::Cli)
            .expect("valid clean URL copy command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed URL copy parameters"),
            serde_json::json!({
                "command": "yank",
                "arguments": {"source": "url", "clean": true}
            })
        );

        let command = parse_chain("yank selection --primary", ParseInput::Cli)
            .expect("valid primary selection copy command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed primary selection parameters"),
            serde_json::json!({
                "command": "yank",
                "arguments": {"source": "selection", "primary": true}
            })
        );
        let command = parse_chain("yank title --primary", ParseInput::Cli)
            .expect("valid title copy command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed title parameters"),
            serde_json::json!({
                "command": "yank",
                "arguments": {"source": "title", "primary": true}
            })
        );
        let duplicate = parse_chain("yank url --primary --primary", ParseInput::Cli)
            .expect("parse duplicate option for typed validation")
            .remove(0);
        assert!(command_params(&duplicate).is_err());
    }

    #[test]
    fn cli_caret_commands_use_typed_fields() {
        let mode = parse_chain("mode-enter caret", ParseInput::Cli)
            .expect("valid mode command")
            .remove(0);
        assert_eq!(
            command_params(&mode).expect("typed mode parameters"),
            serde_json::json!({
                "command": "mode-enter",
                "arguments": {"mode": "caret"}
            })
        );

        let movement = parse_chain("caret-move word-next --count 2", ParseInput::Cli)
            .expect("valid caret movement")
            .remove(0);
        assert_eq!(
            command_params(&movement).expect("typed movement parameters"),
            serde_json::json!({
                "command": "caret-move",
                "arguments": {"direction": "word-next", "count": 2}
            })
        );

        let selection = parse_chain("caret-select toggle", ParseInput::Cli)
            .expect("valid caret selection")
            .remove(0);
        assert_eq!(
            command_params(&selection).expect("typed selection parameters"),
            serde_json::json!({
                "command": "caret-select",
                "arguments": {"state": "toggle"}
            })
        );
    }

    #[test]
    fn cli_spawn_commands_forward_direct_argv() {
        let command = parse_chain("spawn -- /usr/bin/printf {title}", ParseInput::Cli)
            .expect("valid spawn command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed spawn parameters"),
            serde_json::json!({
                "command": "spawn",
                "arguments": {"argv": ["/usr/bin/printf", "{title}"]}
            })
        );
    }

    #[test]
    fn cli_spawn_userscript_forwards_manifest_name() {
        let command = parse_chain("spawn --userscript video", ParseInput::Cli)
            .expect("valid userscript command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed userscript parameters"),
            serde_json::json!({
                "command": "spawn",
                "arguments": {"userscript": "video"}
            })
        );
    }

    #[test]
    fn implicit_config_uses_basedir_but_temp_launches_ignore_it() {
        let directory = std::env::temp_dir().join(format!(
            "ferric-browser-implicit-config-{}",
            std::process::id()
        ));
        fs::create_dir_all(directory.join("config")).expect("config directory");
        fs::write(
            directory.join("config/config.toml"),
            "[ui]\nfont_size_pt = 11.0\n",
        )
        .expect("config");
        let options = CliOptions {
            basedir: Some(directory.to_string_lossy().into_owned()),
            ..CliOptions::default()
        };
        assert!(default_config_path(&options).is_some());
        let temporary = CliOptions {
            temp_basedir: true,
            basedir: options.basedir.clone(),
            ..CliOptions::default()
        };
        assert!(default_config_path(&temporary).is_none());
        let safe = CliOptions {
            safe_mode: true,
            basedir: options.basedir.clone(),
            ..CliOptions::default()
        };
        assert!(default_config_path(&safe).is_none());
        let _ = fs::remove_file(directory.join("config/config.toml"));
        let _ = fs::remove_dir(directory.join("config"));
        let _ = fs::remove_dir(directory);
    }

    #[test]
    fn safe_mode_is_a_gui_only_startup_option() {
        let (options, action) = parse_cli(&["--safe-mode".into()]).expect("safe mode startup");
        assert!(options.safe_mode);
        assert!(matches!(action, CliAction::Open { .. }));
    }

    #[test]
    fn userscripts_off_is_a_normal_profile_gui_only_startup_option() {
        let (options, action) =
            parse_cli(&["--userscripts-off".into()]).expect("userscripts-off startup");
        assert!(options.userscripts_off);
        assert!(!options.safe_mode);
        assert!(matches!(action, CliAction::Open { .. }));
    }

    #[test]
    fn software_rendering_is_a_gui_only_startup_option() {
        let (options, action) =
            parse_cli(&["--software-rendering".into()]).expect("software rendering startup");
        assert!(options.software_rendering);
        assert!(matches!(action, CliAction::Open { .. }));
    }

    #[test]
    fn cli_errors_use_stable_codes() {
        let error = run(&["--unknown".into()]).expect_err("unknown option is rejected");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(error.status(), 2);
        let config_error = CliError::from_run(RunError::config("invalid test config"));
        assert_eq!(config_error.code, ErrorCode::Config);
        assert_eq!(config_error.status(), 2);
        let storage_error = CliError::from_run(RunError::storage("unavailable test storage"));
        assert_eq!(storage_error.code, ErrorCode::Storage);
        assert_eq!(storage_error.status(), 1);
        let instance_error = CliError::from_run(RunError::no_instance("missing test instance"));
        assert_eq!(instance_error.code, ErrorCode::NoInstance);
        assert_eq!(instance_error.status(), 3);
        assert_eq!(error.user_message, "The command-line request is invalid.");
        let error = run(&[
            "--safe-mode".into(),
            "--basedir".into(),
            "/tmp/ferric-invalid-combination".into(),
        ])
        .expect_err("post-parse option conflict is rejected");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(error.status(), 2);
        assert!(wants_structured_error(&[
            "query".into(),
            "tabs".into(),
            "--format".into(),
            "json".into(),
        ]));
        assert!(!wants_structured_error(&["query".into(), "tabs".into()]));
        let structured = serde_json::from_str::<serde_json::Value>(&structured_cli_error_with_id(
            ErrorCode::Timeout,
            6,
            "slow",
            "err-test",
        ))
        .expect("structured CLI errors are JSON");
        assert_eq!(structured["error"]["code"], "E_TIMEOUT");
        assert_eq!(structured["error"]["status"], 6);
        assert_eq!(structured["error"]["message"], "slow");
        assert_eq!(
            structured["error"]["preserved"],
            "the timed-out operation was not committed"
        );
        assert_eq!(
            structured["error"]["next_action"],
            "check the target and retry"
        );
        assert!(
            structured["error"]["correlation_id"]
                .as_str()
                .is_some_and(|value| value.starts_with("err-"))
        );
    }

    #[test]
    fn cli_command_parses_chain_as_data_for_ipc() {
        let arguments = vec![
            "command".into(),
            "--".into(),
            ":open".into(),
            "https://example.test".into(),
        ];
        let (_, action) = parse_cli(&arguments).expect("valid command command");
        assert!(matches!(
            action,
            CliAction::Command { text, window: None }
                if text == ":open https://example.test"
        ));
    }

    #[test]
    fn cli_binding_commands_use_typed_mode_and_keychain_fields() {
        let list = parse_chain("binding-list --mode normal", ParseInput::Cli)
            .expect("binding list command")
            .pop()
            .expect("binding list parsed command");
        assert_eq!(
            command_params(&list).expect("typed binding list")["arguments"],
            serde_json::json!({"mode": "normal"})
        );
        let explain = parse_chain("binding-explain gg --mode normal", ParseInput::Cli)
            .expect("binding explain command")
            .pop()
            .expect("binding explain parsed command");
        assert_eq!(
            command_params(&explain).expect("typed binding explain")["arguments"],
            serde_json::json!({"keychain": "gg", "mode": "normal"})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "binding-explain".into(),
                arguments: Vec::new(),
            })
            .is_err()
        );
        let learning = parse_chain("learning-mode toggle", ParseInput::Cli)
            .expect("learning mode command")
            .pop()
            .expect("learning mode parsed command");
        assert_eq!(
            command_params(&learning).expect("typed learning mode")["arguments"],
            serde_json::json!({"state": "toggle"})
        );
        assert!(
            command_params(&ParsedCommand {
                name: "learning-mode".into(),
                arguments: vec!["invalid".into()],
            })
            .is_err()
        );
    }

    #[test]
    fn forwarded_cli_context_preserves_cli_source() {
        let mut params = serde_json::json!({
            "command": "back",
            "arguments": {}
        });
        add_cli_context(&mut params, &CliOptions::default(), None);
        assert_eq!(params["context"]["source"], "cli");
    }

    #[test]
    fn engine_security_flag_guard_rejects_boolean_forms() {
        assert_eq!(
            find_disabling_engine_flag("--disable-web-security --foo"),
            Some("--disable-web-security")
        );
        assert_eq!(
            find_disabling_engine_flag("--disable-web-security=true"),
            Some("--disable-web-security")
        );
        assert_eq!(
            find_disabling_engine_flag("--single-process=false"),
            Some("--single-process")
        );
        assert_eq!(find_disabling_engine_flag("--disable-gpu"), None);
    }
}
