//! Bounded local userscript manifests, page-script metadata, and result
//! validation. This module contains no Qt calls, so its policy boundary can be
//! tested independently of a display server.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
pub const MAX_PROTOCOL_LINE_BYTES: usize = 64 * 1024;
pub const MAX_PROTOCOL_BYTES: usize = 1024 * 1024;
pub const MAX_PAGE_SCRIPT_BYTES: u64 = 256 * 1024;
pub const MAX_ARGV_VALUE_BYTES: usize = 16 * 1024;
const MAX_PAGE_SCRIPT_MANIFESTS: usize = 256;
const MAX_REGISTERED_ACTIONS: usize = 512;
const MAX_ACTION_TEXT_BYTES: usize = 256;
const MAX_INSTALL_ASSET_BYTES: u64 = 16 * 1024 * 1024;

const KNOWN_CONTEXT_FIELDS: &[&str] = &[
    "url",
    "title",
    "mode",
    "profile",
    "tab_id",
    "window_id",
    "context_name",
    "document_revision",
    "private",
    "hint_url",
    "selection",
    "download_id",
    "history_id",
    "bookmark_id",
    "quickmark_name",
    "session_name",
    "command_id",
];
const KNOWN_RESULTS: &[&str] = &["message", "open", "yank", "command"];
const KNOWN_COMMANDS: &[&str] = &["open", "yank", "spawn"];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u64,
    pub name: String,
    pub executable: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub context_fields: Vec<String>,
    #[serde(default)]
    pub allowed_results: Vec<String>,
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub allow_private: bool,
    #[serde(default)]
    pub actions: Vec<ActionManifest>,
    #[serde(default, alias = "match")]
    pub matches: Vec<String>,
    #[serde(default, alias = "exclude")]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub run_at: RunAt,
    #[serde(default)]
    pub frames: FrameScope,
    #[serde(default)]
    pub page_world: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RunAt {
    #[serde(rename = "document_start")]
    Start,
    #[serde(rename = "document_end")]
    End,
    #[default]
    #[serde(rename = "document_idle")]
    Idle,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FrameScope {
    #[default]
    Top,
    All,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionManifest {
    pub id: String,
    pub subject: String,
    pub verb: String,
    pub label: String,
    #[serde(default)]
    pub required_fields: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredAction {
    pub script: String,
    pub id: String,
    pub subject: String,
    pub verb: String,
    pub label: String,
    pub required_fields: Vec<String>,
    pub allow_private: bool,
}

#[derive(Clone, Debug)]
pub struct LoadedManifest {
    pub manifest: Manifest,
    pub path: PathBuf,
    pub executable: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstalledScript {
    pub name: String,
    pub enabled: bool,
    pub page_world: bool,
    pub run_at: RunAt,
    pub allow_private: bool,
    pub matches: Vec<String>,
    pub actions: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct PageScript {
    pub name: String,
    pub source: String,
    pub run_at: &'static str,
    pub runs_on_sub_frames: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserscriptAction {
    Open { url: String },
    Yank { url: String, clean: bool },
    Command { name: String, arguments: Value },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedOutput {
    pub message: String,
    pub action: Option<UserscriptAction>,
}

fn default_timeout() -> u64 {
    30
}

fn default_enabled() -> bool {
    true
}

fn validate_known_unique_values(
    values: &[String],
    known: &[&str],
    label: &str,
) -> Result<(), String> {
    if values.len() > known.len() || values.iter().any(|value| !known.contains(&value.as_str())) {
        return Err(format!(
            "userscript {label} contains an unknown or excessive value"
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        if !unique.insert(value) {
            return Err(format!(
                "userscript {label} contains a duplicate value: {value}"
            ));
        }
    }
    Ok(())
}

/// Loads and validates one installed userscript manifest.
///
/// # Errors
///
/// Returns an error when the name, manifest file, manifest data, or executable
/// fails the bounded userscript policy.
pub fn load(root: &Path, name: &str) -> Result<LoadedManifest, String> {
    validate_name(name)?;
    let path = root.join("userscripts").join(format!("{name}.toml"));
    let metadata = fs::metadata(&path)
        .map_err(|error| format!("userscript manifest is unavailable: {error}"))?;
    if !metadata.is_file() {
        return Err("userscript manifest is not a regular file".into());
    }
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err("userscript manifest exceeds 64 KiB".into());
    }
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("could not read userscript manifest: {error}"))?;
    let manifest: Manifest =
        toml::from_str(&source).map_err(|error| format!("invalid userscript manifest: {error}"))?;
    validate(&manifest, name)?;
    let executable = if Path::new(&manifest.executable).is_absolute() {
        PathBuf::from(&manifest.executable)
    } else {
        path.parent().unwrap_or(root).join(&manifest.executable)
    };
    Ok(LoadedManifest {
        manifest,
        path,
        executable,
    })
}

/// Installs one explicitly selected local manifest into the profile's
/// userscript directory. Relative executable and page-source assets are copied
/// into a private, script-specific asset directory and the installed manifest
/// is rewritten to refer to those copies. Existing manifests and asset
/// directories are never replaced.
///
/// # Errors
///
/// Returns an error when the selected file, manifest, dependency, or target
/// violates the bounded installation policy.
pub fn install_manifest(root: &Path, source_path: &Path) -> Result<String, String> {
    let source_metadata = fs::symlink_metadata(source_path)
        .map_err(|error| format!("userscript manifest is unavailable: {error}"))?;
    if !source_metadata.is_file() {
        return Err("selected userscript manifest is not a regular file".into());
    }
    if source_path.extension().and_then(|value| value.to_str()) != Some("toml") {
        return Err("userscript manifest must use the .toml extension".into());
    }
    if source_metadata.len() > MAX_MANIFEST_BYTES {
        return Err("userscript manifest exceeds 64 KiB".into());
    }
    let name = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "userscript manifest name is not valid UTF-8".to_owned())?;
    validate_name(name)?;
    let source = fs::read_to_string(source_path)
        .map_err(|error| format!("could not read userscript manifest: {error}"))?;
    let mut manifest: Manifest =
        toml::from_str(&source).map_err(|error| format!("invalid userscript manifest: {error}"))?;
    validate(&manifest, name)?;

    let userscripts = root.join("userscripts");
    fs::create_dir_all(&userscripts)
        .map_err(|error| format!("could not create userscript directory: {error}"))?;
    let userscripts_metadata = fs::symlink_metadata(&userscripts)
        .map_err(|error| format!("could not inspect userscript directory: {error}"))?;
    if userscripts_metadata.file_type().is_symlink() || !userscripts_metadata.is_dir() {
        return Err("userscript directory must be a real directory".into());
    }
    set_private_directory_permissions(&userscripts)?;
    let manifest_path = userscripts.join(format!("{name}.toml"));
    if fs::symlink_metadata(&manifest_path).is_ok() {
        return Err(format!("userscript {name} is already installed"));
    }
    let asset_directory = userscripts.join(format!("{name}.assets"));
    if fs::symlink_metadata(&asset_directory).is_ok() {
        return Err(format!(
            "userscript asset directory already exists for {name}"
        ));
    }

    let mut copied_assets = BTreeMap::new();
    let mut asset_directory_created = false;
    let install_result = (|| {
        let manifest_parent = source_path.parent().unwrap_or_else(|| Path::new("."));
        if Path::new(&manifest.executable).is_absolute() {
            validate_executable(Path::new(&manifest.executable))?;
        } else {
            asset_directory_created = true;
            let relative = install_relative_asset(
                manifest_parent,
                &manifest.executable,
                &asset_directory,
                &mut copied_assets,
                true,
            )?;
            manifest.executable = relative;
        }
        if let Some(source) = manifest.source.clone() {
            if Path::new(&source).is_absolute() {
                validate_page_source(Path::new(&source))?;
            } else {
                asset_directory_created = true;
                let relative = install_relative_asset(
                    manifest_parent,
                    &source,
                    &asset_directory,
                    &mut copied_assets,
                    false,
                )?;
                manifest.source = Some(relative);
            }
        }
        validate(&manifest, name)?;
        let installed = toml::to_string(&manifest)
            .map_err(|error| format!("could not encode installed userscript: {error}"))?;
        if installed.len() > MAX_MANIFEST_BYTES as usize {
            return Err("installed userscript manifest exceeds 64 KiB".into());
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&manifest_path)
            .map_err(|error| format!("could not create installed userscript: {error}"))?;
        file.write_all(installed.as_bytes())
            .map_err(|error| format!("could not write installed userscript: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("could not sync installed userscript: {error}"))?;
        set_private_file_permissions(&manifest_path)?;
        Ok::<(), String>(())
    })();
    if let Err(error) = install_result {
        let _ = fs::remove_file(&manifest_path);
        if asset_directory_created {
            let _ = fs::remove_dir_all(&asset_directory);
        }
        return Err(error);
    }
    Ok(name.to_owned())
}

fn install_relative_asset(
    manifest_parent: &Path,
    value: &str,
    asset_directory: &Path,
    copied_assets: &mut BTreeMap<PathBuf, PathBuf>,
    executable: bool,
) -> Result<String, String> {
    let relative = safe_relative_asset(value)?;
    let source = manifest_parent.join(&relative);
    let destination = asset_directory.join(&relative);
    if !copied_assets.contains_key(&relative) {
        ensure_asset_directory(asset_directory)?;
        copy_install_asset(&source, &destination, executable)?;
        copied_assets.insert(relative.clone(), destination.clone());
    }
    if executable {
        validate_executable(&destination)?;
    } else {
        validate_page_source(&destination)?;
    }
    Ok(format!(
        "{}/{}",
        asset_directory
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "userscript asset directory name is invalid".to_owned())?,
        relative.to_string_lossy().replace('\\', "/")
    ))
}

fn ensure_asset_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("userscript asset directory must be a real directory".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)
                .map_err(|error| format!("could not create userscript asset directory: {error}"))?;
        }
        Err(error) => {
            return Err(format!(
                "could not inspect userscript asset directory: {error}"
            ));
        }
    }
    set_private_directory_permissions(path)
}

fn safe_relative_asset(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.len() > 256 || Path::new(value).is_absolute() {
        return Err("userscript asset path must be a bounded relative path".into());
    }
    let mut relative = PathBuf::new();
    for component in Path::new(value).components() {
        match component {
            std::path::Component::Normal(component) => relative.push(component),
            std::path::Component::CurDir => {}
            _ => return Err("userscript asset path contains an unsafe component".into()),
        }
    }
    if relative.as_os_str().is_empty() {
        return Err("userscript asset path is empty".into());
    }
    Ok(relative)
}

fn copy_install_asset(source: &Path, destination: &Path, executable: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("userscript asset is unavailable: {error}"))?;
    if !metadata.is_file() {
        return Err("userscript asset is not a regular file".into());
    }
    if metadata.len() > MAX_INSTALL_ASSET_BYTES {
        return Err("userscript asset exceeds 16 MiB".into());
    }
    if executable {
        validate_executable(source)?;
    } else {
        validate_page_source(source)?;
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create userscript asset directory: {error}"))?;
        set_private_directory_permissions(parent)?;
    }
    fs::copy(source, destination)
        .map_err(|error| format!("could not copy userscript asset: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            destination,
            fs::Permissions::from_mode(if executable {
                metadata.permissions().mode() | 0o111
            } else {
                0o600
            }),
        )
        .map_err(|error| format!("could not set userscript asset permissions: {error}"))?;
    }
    Ok(())
}

fn validate_page_source(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("userscript source is unavailable: {error}"))?;
    if !metadata.is_file() {
        return Err("userscript source is not a regular file".into());
    }
    if metadata.len() > MAX_PAGE_SCRIPT_BYTES {
        return Err("userscript source exceeds 256 KiB".into());
    }
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read userscript source: {error}"))?;
    if source.contains('\0') {
        return Err("userscript source contains a NUL byte".into());
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("could not set userscript permissions: {error}"))?;
    }
    Ok(())
}

fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not set userscript directory permissions: {error}"))?;
    }
    Ok(())
}

/// Validates a decoded userscript manifest against its requested name.
///
/// # Errors
///
/// Returns an error for unknown fields, invalid names, unsafe executable or
/// argument data, unsupported scopes/results, or exceeded policy limits.
pub fn validate(manifest: &Manifest, requested_name: &str) -> Result<(), String> {
    if manifest.schema_version != 1 {
        return Err("userscript schema_version must be 1".into());
    }
    if manifest.name != requested_name {
        return Err("userscript manifest name does not match its file name".into());
    }
    for (label, value) in [
        ("name", manifest.name.as_str()),
        ("executable", manifest.executable.as_str()),
    ] {
        if value.is_empty()
            || value.len() > MAX_ARGV_VALUE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(format!(
                "userscript {label} is empty, oversized, or contains a control character"
            ));
        }
    }
    for (label, value) in [("executable", manifest.executable.as_str())]
        .into_iter()
        .chain(manifest.source.as_deref().map(|source| ("source", source)))
    {
        if value.is_empty()
            || value.len() > MAX_ARGV_VALUE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(format!(
                "userscript {label} path is empty, oversized, or contains a control character"
            ));
        }
        if !Path::new(value).is_absolute() {
            safe_relative_asset(value)
                .map_err(|error| format!("userscript {label} path is invalid: {error}"))?;
        }
    }
    if manifest.argv.len() > 256
        || manifest.argv.iter().any(|value| {
            value.is_empty()
                || value.len() > MAX_ARGV_VALUE_BYTES
                || value.chars().any(char::is_control)
        })
    {
        return Err(
            "userscript argv must contain at most 256 nonempty values of at most 16 KiB without controls".into(),
        );
    }
    if !(1..=300).contains(&manifest.timeout_seconds) {
        return Err("userscript timeout_seconds must be 1..300".into());
    }
    validate_known_unique_values(
        &manifest.context_fields,
        KNOWN_CONTEXT_FIELDS,
        "context_fields",
    )?;
    validate_known_unique_values(&manifest.allowed_results, KNOWN_RESULTS, "allowed_results")?;
    validate_known_unique_values(
        &manifest.allowed_commands,
        KNOWN_COMMANDS,
        "allowed_commands",
    )?;
    for action in &manifest.actions {
        if action.id.len() > MAX_ACTION_TEXT_BYTES
            || action.subject.len() > MAX_ACTION_TEXT_BYTES
            || action.verb.len() > MAX_ACTION_TEXT_BYTES
            || action.label.len() > MAX_ACTION_TEXT_BYTES
        {
            return Err("userscript action fields must be at most 256 bytes".into());
        }
        if !matches!(
            action.subject.as_str(),
            "url"
                | "link"
                | "tab"
                | "window"
                | "context"
                | "selection"
                | "download"
                | "history-entry"
                | "bookmark"
                | "quickmark"
                | "session"
                | "command"
        ) {
            return Err(format!(
                "unknown userscript action subject: {}",
                action.subject
            ));
        }
        let subject_fields = match action.subject.as_str() {
            "link" => &["url", "hint_url"][..],
            "selection" => &["selection"][..],
            "url" => &["url"][..],
            "tab" => &["tab_id"][..],
            "window" => &["window_id"][..],
            "context" => &["context_name"][..],
            "download" => &["download_id"][..],
            "history-entry" => &["history_id"][..],
            "bookmark" => &["bookmark_id"][..],
            "quickmark" => &["quickmark_name"][..],
            "session" => &["session_name"][..],
            "command" => &["command_id"][..],
            _ => &[][..],
        };
        if !action
            .required_fields
            .iter()
            .any(|field| subject_fields.contains(&field.as_str()))
        {
            return Err(format!(
                "userscript {} action must require one of: {}",
                action.subject,
                subject_fields.join(", ")
            ));
        }
        if action.required_fields.len() > KNOWN_CONTEXT_FIELDS.len()
            || action.required_fields.iter().any(|field| {
                !KNOWN_CONTEXT_FIELDS.contains(&field.as_str())
                    || action
                        .required_fields
                        .iter()
                        .filter(|candidate| *candidate == field)
                        .count()
                        > 1
            })
        {
            return Err("userscript action required_fields are invalid or duplicated".into());
        }
        if action.id.is_empty()
            || !action
                .id
                .starts_with(&format!("userscript.{requested_name}."))
        {
            return Err(format!(
                "userscript action id must be namespaced under userscript.{requested_name}."
            ));
        }
        for (label, value) in [
            ("id", action.id.as_str()),
            ("subject", action.subject.as_str()),
            ("verb", action.verb.as_str()),
            ("label", action.label.as_str()),
        ] {
            if value.is_empty() || value.chars().any(char::is_control) {
                return Err(format!("userscript action {label} is empty or invalid"));
            }
        }
        for field in &action.required_fields {
            if !manifest
                .context_fields
                .iter()
                .any(|allowed| allowed == field)
            {
                return Err(format!(
                    "userscript action requires undeclared context field: {field}"
                ));
            }
        }
    }
    if manifest.actions.len() > 64 {
        return Err("userscript manifest may register at most 64 actions".into());
    }
    if manifest.matches.len() > 64 || manifest.excludes.len() > 64 {
        return Err("userscript match and exclude lists must contain at most 64 patterns".into());
    }
    for pattern in manifest.matches.iter().chain(&manifest.excludes) {
        validate_url_pattern(pattern)?;
    }
    if !manifest.page_world && manifest.source.is_some() {
        return Err("userscript source requires explicit page_world = true".into());
    }
    if manifest.page_world {
        if manifest.matches.is_empty() {
            return Err("page-world userscript must declare at least one match pattern".into());
        }
        let source = manifest
            .source
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "page-world userscript requires a local source path".to_owned())?;
        if source.chars().any(char::is_control) {
            return Err("page-world userscript source path contains a control character".into());
        }
    }
    Ok(())
}

/// Load the installed, manifest-backed action registrations in stable order.
/// Missing or removed manifests therefore disappear from every generated
/// action surface on the next query.
/// Discovers the stable, bounded action list from installed manifests.
///
/// # Errors
///
/// Returns an error if the userscript directory cannot be read or a discovered
/// manifest cannot be loaded and validated.
pub fn registered_actions(root: &Path) -> Result<Vec<RegisteredAction>, String> {
    let directory = root.join("userscripts");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not list userscripts: {error}")),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not inspect userscript: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("toml")
            && let Some(name) = path.file_stem().and_then(|value| value.to_str())
        {
            validate_name(name)?;
            names.push(name.to_owned());
        }
    }
    names.sort_unstable();

    let mut actions = Vec::new();
    for name in names {
        let manifest = load(root, &name)?.manifest;
        if !manifest.enabled {
            continue;
        }
        for action in manifest.actions {
            actions.push(RegisteredAction {
                script: name.clone(),
                id: action.id,
                subject: action.subject,
                verb: action.verb,
                label: action.label,
                required_fields: action.required_fields,
                allow_private: manifest.allow_private,
            });
            if actions.len() > MAX_REGISTERED_ACTIONS {
                return Err("installed userscripts register more than 512 actions".into());
            }
        }
    }
    actions.sort_by(|left, right| left.id.cmp(&right.id));
    for pair in actions.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(format!("duplicate userscript action id: {}", pair[0].id));
        }
    }
    Ok(actions)
}

/// Returns bounded, path-free metadata for the installed userscript manager.
///
/// # Errors
///
/// Returns an error when the userscript directory or any discovered manifest
/// cannot be read, validated, or represented within the inventory bounds.
pub fn installed_scripts(root: &Path) -> Result<Vec<InstalledScript>, String> {
    let directory = root.join("userscripts");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not list userscripts: {error}")),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not inspect userscript: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("toml")
            && let Some(name) = path.file_stem().and_then(|value| value.to_str())
        {
            validate_name(name)?;
            names.push(name.to_owned());
        }
    }
    names.sort_unstable();
    if names.len() > MAX_PAGE_SCRIPT_MANIFESTS {
        return Err("userscript directory exceeds 256 manifests".into());
    }
    names
        .into_iter()
        .map(|name| {
            let manifest = load(root, &name)?.manifest;
            Ok(InstalledScript {
                name,
                enabled: manifest.enabled,
                page_world: manifest.page_world,
                run_at: manifest.run_at,
                allow_private: manifest.allow_private,
                matches: manifest.matches,
                actions: manifest.actions.len(),
            })
        })
        .collect()
}

/// Changes one installed manifest's enabled state using an atomic replacement.
///
/// # Errors
///
/// Returns an error when the name or manifest is invalid, the replacement file
/// cannot be created safely, or the durable replacement cannot be installed.
pub fn set_enabled(root: &Path, name: &str, enabled: bool) -> Result<(), String> {
    validate_name(name)?;
    let loaded = load(root, name)?;
    let metadata = fs::symlink_metadata(&loaded.path)
        .map_err(|error| format!("could not inspect userscript manifest: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("userscript manifest must be a real file".into());
    }
    let mut manifest = loaded.manifest;
    manifest.enabled = enabled;
    validate(&manifest, name)?;
    let encoded = toml::to_string(&manifest)
        .map_err(|error| format!("could not encode userscript manifest: {error}"))?;
    if encoded.len() > MAX_MANIFEST_BYTES as usize {
        return Err("userscript manifest exceeds 64 KiB".into());
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "userscript clock is before the Unix epoch".to_owned())?
        .as_nanos();
    let temporary = loaded
        .path
        .with_extension(format!("toml.tmp-{}-{stamp}", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("could not create userscript update: {error}"))?;
        file.write_all(encoded.as_bytes())
            .map_err(|error| format!("could not write userscript update: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("could not sync userscript update: {error}"))?;
        set_private_file_permissions(&temporary)?;
        fs::rename(&temporary, &loaded.path)
            .map_err(|error| format!("could not install userscript update: {error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Removes one installed userscript and its private copied assets.
///
/// The manifest is renamed out of the live namespace before the asset
/// directory is removed, so a failed asset deletion can restore the manifest.
/// Symlinked manifests and asset directories are refused.
///
/// # Errors
///
/// Returns an error when the manifest or asset directory is invalid, the
/// removal cannot be completed, or the directory cannot be synchronized.
pub fn remove(root: &Path, name: &str) -> Result<(), String> {
    validate_name(name)?;
    let loaded = load(root, name)?;
    let manifest_metadata = fs::symlink_metadata(&loaded.path)
        .map_err(|error| format!("could not inspect userscript manifest: {error}"))?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err("userscript manifest must be a real file".into());
    }
    let assets = root.join("userscripts").join(format!("{name}.assets"));
    inspect_asset_directory(&assets)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "userscript clock is before the Unix epoch".to_owned())?
        .as_nanos();
    let tombstone = loaded
        .path
        .with_extension(format!("toml.remove-{}-{stamp}", std::process::id()));
    if fs::symlink_metadata(&tombstone).is_ok() {
        return Err("userscript removal marker already exists".into());
    }
    fs::rename(&loaded.path, &tombstone)
        .map_err(|error| format!("could not stage userscript removal: {error}"))?;
    let cleanup = (|| {
        if inspect_asset_directory(&assets)? {
            fs::remove_dir_all(&assets)
                .map_err(|error| format!("could not remove userscript assets: {error}"))?;
        }
        fs::remove_file(&tombstone)
            .map_err(|error| format!("could not remove userscript manifest: {error}"))?;
        sync_directory(loaded.path.parent().unwrap_or_else(|| Path::new(".")))
    })();
    if let Err(error) = cleanup {
        if fs::symlink_metadata(&tombstone).is_ok() {
            let _ = fs::rename(&tombstone, &loaded.path);
        }
        return Err(error);
    }
    Ok(())
}

fn inspect_asset_directory(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("userscript asset directory must be a real directory".into());
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("could not inspect userscript assets: {error}")),
    }
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("could not sync userscript directory: {error}"))
}

/// Returns page scripts whose manifest patterns match the supplied page.
///
/// # Errors
///
/// Returns an error if an installed manifest cannot be read or validated, or
/// if the page URL is not a valid bounded HTTP(S) target.
pub fn matching_page_scripts(
    root: &Path,
    url: &str,
    private_profile: bool,
) -> Result<Vec<PageScript>, String> {
    if !is_http_url(url) {
        return Ok(Vec::new());
    }
    let directory = root.join("userscripts");
    let mut names = Vec::new();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not list userscripts: {error}")),
    };
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not inspect userscript: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("toml") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        validate_name(name)?;
        names.push(name.to_owned());
        if names.len() > MAX_PAGE_SCRIPT_MANIFESTS {
            return Err("userscript directory exceeds 256 manifests".into());
        }
    }
    names.sort_unstable();

    let mut scripts = Vec::new();
    for name in names {
        let loaded = load(root, &name)?;
        let manifest = &loaded.manifest;
        if !manifest.enabled {
            continue;
        }
        if !manifest.page_world
            || (private_profile && !manifest.allow_private)
            || !manifest
                .matches
                .iter()
                .any(|pattern| url_pattern_matches(pattern, url))
            || manifest
                .excludes
                .iter()
                .any(|pattern| url_pattern_matches(pattern, url))
        {
            continue;
        }
        let source_name = manifest
            .source
            .as_deref()
            .ok_or_else(|| "page-world userscript source path is missing".to_owned())?;
        let source_path = resolve_manifest_path(root, &loaded.path, source_name);
        let metadata = fs::metadata(&source_path)
            .map_err(|error| format!("userscript source is unavailable: {error}"))?;
        if !metadata.is_file() {
            return Err("userscript source is not a regular file".into());
        }
        if metadata.len() > MAX_PAGE_SCRIPT_BYTES {
            return Err("userscript source exceeds 256 KiB".into());
        }
        let source = fs::read_to_string(&source_path)
            .map_err(|error| format!("could not read userscript source: {error}"))?;
        if source.contains('\0') {
            return Err("userscript source contains a NUL byte".into());
        }
        scripts.push(PageScript {
            name,
            source,
            run_at: match manifest.run_at {
                RunAt::Start => "document_start",
                RunAt::End => "document_end",
                RunAt::Idle => "document_idle",
            },
            runs_on_sub_frames: manifest.frames == FrameScope::All,
        });
    }
    Ok(scripts)
}

fn resolve_manifest_path(root: &Path, manifest_path: &Path, value: &str) -> PathBuf {
    if Path::new(value).is_absolute() {
        PathBuf::from(value)
    } else {
        manifest_path.parent().unwrap_or(root).join(value)
    }
}

fn validate_url_pattern(pattern: &str) -> Result<(), String> {
    if pattern.is_empty() || pattern.len() > 2048 || pattern.contains('?') {
        return Err("userscript URL patterns must be 1..2048 bytes without '?'".into());
    }
    let Some((scheme, remainder)) = pattern.split_once("://") else {
        return Err("userscript URL patterns require http:// or https://".into());
    };
    if !matches!(scheme, "http" | "https") {
        return Err("userscript URL patterns only support http:// and https://".into());
    }
    let authority = remainder.split('/').next().unwrap_or_default();
    let host = authority.split(':').next().unwrap_or_default();
    let port_is_valid = authority
        .split_once(':')
        .is_none_or(|(_, port)| port == "*" || port.parse::<u16>().is_ok());
    if host.is_empty()
        || authority.contains('@')
        || authority.matches(':').count() > 1
        || !port_is_valid
        || authority.contains("**")
        || host.contains('*') && !host.starts_with("*.")
        || pattern.chars().any(char::is_control)
    {
        return Err("userscript URL pattern has an invalid authority or character".into());
    }
    Ok(())
}

fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn url_pattern_matches(pattern: &str, url: &str) -> bool {
    glob_matches(pattern, url.split('#').next().unwrap_or(url))
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let (mut pattern_index, mut value_index) = (0, 0);
    let (mut star, mut retry) = (None, 0);
    while value_index < value.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == value[value_index] {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star = Some(pattern_index);
            pattern_index += 1;
            retry = value_index;
        } else if let Some(star_index) = star {
            pattern_index = star_index + 1;
            retry += 1;
            value_index = retry;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

/// Checks that a userscript executable is a regular executable file.
///
/// # Errors
///
/// Returns an error when the path is missing, not a regular file, or lacks the
/// executable permission on Unix.
pub fn validate_executable(path: &Path) -> Result<(), String> {
    if path.as_os_str().len() > MAX_ARGV_VALUE_BYTES {
        return Err("userscript executable path is oversized".into());
    }
    let metadata = fs::metadata(path)
        .map_err(|error| format!("userscript executable is unavailable: {error}"))?;
    if !metadata.is_file() {
        return Err("userscript executable is not a regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err("userscript executable is not executable".into());
        }
    }
    Ok(())
}

pub fn protocol_input(operation_id: &str, context: &Value) -> Value {
    json!({
        "protocol_version": 1,
        "invocation_id": operation_id,
        "context": context,
    })
}

/// Parses and validates bounded newline-delimited userscript output.
///
/// # Errors
///
/// Returns an error for oversized, malformed, undeclared, duplicated, or
/// otherwise unsafe result messages/actions.
pub fn parse_output(bytes: &[u8], manifest: &Manifest) -> Result<ParsedOutput, String> {
    if bytes.len() > MAX_PROTOCOL_BYTES {
        return Err("userscript stdout exceeds 1 MiB".into());
    }
    if bytes.is_empty() {
        return Err("userscript produced no JSON result".into());
    }
    let mut messages = Vec::new();
    let mut action = None;
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_PROTOCOL_LINE_BYTES {
            return Err("userscript result line exceeds 64 KiB".into());
        }
        let value: Value = serde_json::from_slice(line)
            .map_err(|error| format!("invalid userscript JSON result: {error}"))?;
        let result_type = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| "userscript result requires a string type".to_owned())?;
        if !manifest
            .allowed_results
            .iter()
            .any(|allowed| allowed == result_type)
        {
            return Err(format!(
                "userscript result type is not allowed: {result_type}"
            ));
        }
        match result_type {
            "message" => {
                ensure_keys(&value, &["type", "text"])?;
                let message = value
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "userscript message result requires text".to_owned())?;
                if message.chars().any(char::is_control) {
                    return Err("userscript message contains a control character".into());
                }
                messages.push(message.chars().take(256).collect::<String>());
            }
            "open" => {
                ensure_action_is_unique(action.as_ref())?;
                ensure_keys(&value, &["type", "url"])?;
                let url = bounded_text(&value, "url", "userscript open result")?;
                action = Some(UserscriptAction::Open { url });
            }
            "yank" => {
                ensure_action_is_unique(action.as_ref())?;
                ensure_keys(&value, &["type", "url", "clean"])?;
                let url = bounded_text(&value, "url", "userscript yank result")?;
                let clean = value.get("clean").map_or(Ok(false), |value| {
                    value
                        .as_bool()
                        .ok_or_else(|| "userscript yank clean must be a boolean".to_owned())
                })?;
                action = Some(UserscriptAction::Yank { url, clean });
            }
            "command" => {
                ensure_action_is_unique(action.as_ref())?;
                ensure_keys(&value, &["type", "command", "arguments"])?;
                let name = bounded_text(&value, "command", "userscript command result")?;
                if !manifest
                    .allowed_commands
                    .iter()
                    .any(|allowed| allowed == &name)
                {
                    return Err(format!("userscript command is not allowed: {name}"));
                }
                let arguments = value
                    .get("arguments")
                    .cloned()
                    .ok_or_else(|| "userscript command result requires arguments".to_owned())?;
                if !arguments.is_object() {
                    return Err("userscript command result arguments must be an object".into());
                }
                action = Some(UserscriptAction::Command { name, arguments });
            }
            _ => {
                return Err(format!(
                    "userscript result type is not implemented: {result_type}"
                ));
            }
        }
    }
    if messages.is_empty() && action.is_none() {
        return Err("userscript produced no JSON result".into());
    }
    Ok(ParsedOutput {
        message: messages.join(" | "),
        action,
    })
}

fn ensure_action_is_unique(action: Option<&UserscriptAction>) -> Result<(), String> {
    if action.is_some() {
        Err("userscript produced more than one action result".into())
    } else {
        Ok(())
    }
}

fn ensure_keys(value: &Value, allowed: &[&str]) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "userscript result must be an object".to_owned())?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("userscript result contains an unknown field".into());
    }
    Ok(())
}

fn bounded_text(value: &Value, key: &str, label: &str) -> Result<String, String> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 8192)
        .ok_or_else(|| format!("{label} requires a nonempty value of at most 8192 bytes"))?;
    if text.chars().any(char::is_control) {
        return Err(format!("{label} contains a control character"));
    }
    Ok(text.to_owned())
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || name.chars().any(|character| {
            !character.is_ascii_alphanumeric() && character != '-' && character != '_'
        })
    {
        return Err("userscript name must be 1..64 ASCII letters, digits, '-' or '_'".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_validation_rejects_unknown_fields_and_unscoped_actions() {
        let manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "video"
executable = "/bin/true"
context_fields = ["url"]
allowed_results = ["message"]
[[actions]]
id = "other.send"
subject = "link"
verb = "send"
label = "Send"
"#,
        )
        .expect("manifest parses");
        assert!(validate(&manifest, "video").is_err());
    }

    #[test]
    fn manifest_validation_rejects_oversized_argv_values() {
        let mut manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "video"
executable = "/bin/true"
argv = ["--safe"]
"#,
        )
        .expect("manifest parses");
        validate(&manifest, "video").expect("bounded argv is valid");
        manifest.argv = vec!["x".repeat(MAX_ARGV_VALUE_BYTES + 1)];
        assert!(validate(&manifest, "video").is_err());

        manifest.argv = Vec::new();
        manifest.executable = format!("/{}", "x".repeat(MAX_ARGV_VALUE_BYTES));
        assert!(validate(&manifest, "video").is_err());

        manifest.executable = "/bin/true".into();
        manifest.page_world = true;
        manifest.matches = vec!["https://example.test/*".into()];
        manifest.source = Some(format!("/{}", "x".repeat(MAX_ARGV_VALUE_BYTES)));
        assert!(validate(&manifest, "video").is_err());
    }

    #[test]
    fn manifest_validation_rejects_duplicate_context_permissions() {
        let mut manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "video"
executable = "/bin/true"
context_fields = ["url", "url"]
allowed_results = ["message", "message"]
allowed_commands = ["open", "open"]
"#,
        )
        .expect("manifest parses");
        assert!(
            validate(&manifest, "video")
                .expect_err("duplicate context fields must be rejected")
                .contains("context_fields contains a duplicate")
        );

        manifest.context_fields = vec!["url".into()];
        assert!(
            validate(&manifest, "video")
                .expect_err("duplicate results must be rejected")
                .contains("allowed_results contains a duplicate")
        );

        manifest.allowed_results = vec!["message".into()];
        assert!(
            validate(&manifest, "video")
                .expect_err("duplicate commands must be rejected")
                .contains("allowed_commands contains a duplicate")
        );
    }

    #[test]
    fn manifest_validation_rejects_relative_path_traversal_but_accepts_dot_prefix() {
        let mut manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "video"
executable = "./bin/video"
page_world = true
matches = ["https://example.test/*"]
source = "./page.js"
"#,
        )
        .expect("manifest parses");
        validate(&manifest, "video").expect("dot-prefixed paths are valid");

        manifest.executable = "../video".into();
        assert!(
            validate(&manifest, "video")
                .expect_err("executable traversal must be rejected")
                .contains("executable path is invalid")
        );

        manifest.executable = "./bin/video".into();
        manifest.source = Some("assets/../page.js".into());
        assert!(
            validate(&manifest, "video")
                .expect_err("source traversal must be rejected")
                .contains("source path is invalid")
        );
    }

    #[test]
    fn manifest_validation_requires_subject_context_fields() {
        let manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "video"
executable = "/bin/true"
context_fields = ["url"]
[[actions]]
id = "userscript.video.send"
subject = "link"
verb = "send"
label = "Send"
"#,
        )
        .expect("manifest parses");
        let error = validate(&manifest, "video").expect_err("link field is required");
        assert!(error.contains("userscript link action must require one of"));

        let hint_manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "hint"
executable = "/bin/true"
context_fields = ["hint_url"]
[[actions]]
id = "userscript.hint.send"
subject = "link"
verb = "send"
label = "Send hinted link"
required_fields = ["hint_url"]
"#,
        )
        .expect("hint manifest parses");
        validate(&hint_manifest, "hint").expect("hint-only link is valid");
    }

    #[test]
    fn manifest_validation_accepts_typed_window_and_context_fields() {
        let manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "organizer"
executable = "/bin/true"
context_fields = ["tab_id", "window_id", "context_name", "download_id", "history_id", "bookmark_id", "quickmark_name"]
[[actions]]
id = "userscript.organizer.focus"
subject = "window"
verb = "focus"
label = "Focus window"
required_fields = ["window_id"]
"#,
        )
        .expect("manifest parses");
        assert!(validate(&manifest, "organizer").is_ok());
    }

    #[test]
    fn manifest_validation_accepts_stored_subject_fields() {
        let manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "library-tools"
executable = "/bin/true"
context_fields = ["download_id", "history_id", "bookmark_id", "quickmark_name", "session_name", "command_id"]
[[actions]]
id = "userscript.library-tools.download"
subject = "download"
verb = "show"
label = "Show download"
required_fields = ["download_id"]
[[actions]]
id = "userscript.library-tools.history"
subject = "history-entry"
verb = "open"
label = "Open history"
required_fields = ["history_id"]
[[actions]]
id = "userscript.library-tools.bookmark"
subject = "bookmark"
verb = "open"
label = "Open bookmark"
required_fields = ["bookmark_id"]
[[actions]]
id = "userscript.library-tools.quickmark"
subject = "quickmark"
verb = "open"
label = "Open quickmark"
required_fields = ["quickmark_name"]
[[actions]]
id = "userscript.library-tools.session"
subject = "session"
verb = "load"
label = "Load session"
required_fields = ["session_name"]
[[actions]]
id = "userscript.library-tools.command"
subject = "command"
verb = "help"
label = "Command help"
required_fields = ["command_id"]
"#,
        )
        .expect("manifest parses");
        assert!(validate(&manifest, "library-tools").is_ok());
    }

    #[test]
    fn output_requires_declared_message_and_enforces_limits() {
        let manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "test"
executable = "/bin/true"
allowed_results = ["message"]
"#,
        )
        .expect("manifest parses");
        assert_eq!(
            parse_output(br#"{"type":"message","text":"ok"}"#, &manifest)
                .unwrap()
                .message,
            "ok"
        );
        assert!(
            parse_output(
                br#"{"type":"open","url":"https://example.test"}"#,
                &manifest
            )
            .is_err()
        );
    }

    #[test]
    fn output_parses_allowlisted_typed_actions() {
        let manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "test"
executable = "/bin/true"
allowed_results = ["open", "yank", "command"]
allowed_commands = ["open"]
"#,
        )
        .expect("manifest parses");
        assert_eq!(
            parse_output(
                br#"{"type":"command","command":"open","arguments":{"input":"https://example.test"}}"#,
                &manifest
            )
            .unwrap()
            .action,
            Some(UserscriptAction::Command {
                name: "open".into(),
                arguments: json!({"input": "https://example.test"})
            })
        );
        assert!(
            parse_output(
                br#"{"type":"command","command":"spawn","arguments":{}}"#,
                &manifest
            )
            .is_err()
        );
        assert!(
            parse_output(
                br#"{"type":"open","url":"https://example.test","extra":true}"#,
                &manifest
            )
            .is_err()
        );
    }

    #[test]
    fn page_world_metadata_requires_explicit_scope_and_source() {
        let manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "page"
executable = "/bin/true"
matches = ["https://example.test/*"]
page_world = true
"#,
        )
        .expect("manifest parses");
        assert!(validate(&manifest, "page").is_err());

        let manifest: Manifest = toml::from_str(
            r#"
schema_version = 1
name = "page"
executable = "/bin/true"
matches = ["https://example.test/*"]
source = "page.js"
"#,
        )
        .expect("manifest parses");
        assert!(validate(&manifest, "page").is_err());
    }

    #[test]
    fn page_url_patterns_are_bounded_and_excludes_win() {
        assert!(validate_url_pattern("https://example.test/*").is_ok());
        assert!(validate_url_pattern("https://*.example.test/*").is_ok());
        assert!(validate_url_pattern("http://127.0.0.1:*/*").is_ok());
        assert!(validate_url_pattern("http://127.0.0.1:bad/*").is_err());
        assert!(validate_url_pattern("file:///tmp/*").is_err());
        assert!(url_pattern_matches(
            "https://example.test/*",
            "https://example.test/a#b"
        ));
        assert!(!url_pattern_matches(
            "https://example.test/private/*",
            "https://example.test/public"
        ));
    }

    #[test]
    fn matching_page_scripts_loads_local_source_and_honors_private_gate() {
        let root = std::env::temp_dir().join(format!(
            "ferric-browser-userscript-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        let directory = root.join("userscripts");
        std::fs::create_dir_all(&directory).expect("create userscript test directory");
        std::fs::write(
            directory.join("page.toml"),
            r#"
schema_version = 1
name = "page"
executable = "/bin/true"
matches = ["https://example.test/*"]
page_world = true
source = "page.js"
"#,
        )
        .expect("write userscript manifest");
        std::fs::write(directory.join("page.js"), "document.title = 'ok';")
            .expect("write userscript source");

        let scripts = matching_page_scripts(&root, "https://example.test/path", false)
            .expect("load matching page script");
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].name, "page");
        assert_eq!(scripts[0].run_at, "document_idle");
        assert_eq!(scripts[0].source, "document.title = 'ok';");
        assert!(
            matching_page_scripts(&root, "https://example.test/path", true)
                .expect("private filtering succeeds")
                .is_empty()
        );
        std::fs::remove_dir_all(root).expect("remove userscript test directory");
    }

    #[test]
    fn installed_script_enable_state_is_atomic_and_affects_matching() {
        let root = std::env::temp_dir().join(format!(
            "ferric-browser-userscript-enable-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        let directory = root.join("userscripts");
        std::fs::create_dir_all(&directory).expect("create userscript enable directory");
        std::fs::write(
            directory.join("page.toml"),
            r#"
schema_version = 1
name = "page"
executable = "/bin/true"
enabled = false
matches = ["https://example.test/*"]
page_world = true
source = "page.js"
"#,
        )
        .expect("write disabled manifest");
        std::fs::write(directory.join("page.js"), "document.title = 'ok';")
            .expect("write page source");

        assert!(
            matching_page_scripts(&root, "https://example.test/path", false)
                .expect("disabled script is filtered")
                .is_empty()
        );
        assert!(!installed_scripts(&root).expect("inventory loads")[0].enabled);

        set_enabled(&root, "page", true).expect("enable userscript");
        assert!(installed_scripts(&root).expect("enabled inventory loads")[0].enabled);
        assert_eq!(
            matching_page_scripts(&root, "https://example.test/path", false)
                .expect("enabled script matches")
                .len(),
            1
        );

        set_enabled(&root, "page", false).expect("disable userscript");
        assert!(
            registered_actions(&root)
                .expect("disabled actions are filtered")
                .is_empty()
        );
        std::fs::remove_dir_all(root).expect("remove userscript enable directory");
    }

    #[test]
    fn remove_deletes_manifest_and_private_assets() {
        let root = std::env::temp_dir().join(format!(
            "ferric-browser-userscript-remove-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        let directory = root.join("userscripts");
        let assets = directory.join("page.assets");
        std::fs::create_dir_all(&assets).expect("create userscript asset directory");
        std::fs::write(
            directory.join("page.toml"),
            r#"
schema_version = 1
name = "page"
executable = "/bin/true"
"#,
        )
        .expect("write userscript manifest");
        std::fs::write(assets.join("asset.txt"), "private asset").expect("write userscript asset");

        remove(&root, "page").expect("remove userscript");
        assert!(!directory.join("page.toml").exists());
        assert!(!assets.exists());
        assert!(
            installed_scripts(&root)
                .expect("read empty inventory")
                .is_empty()
        );
        std::fs::remove_dir_all(root).expect("remove userscript removal directory");
    }

    #[cfg(unix)]
    #[test]
    fn remove_refuses_symlinked_private_assets_and_keeps_manifest() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "ferric-browser-userscript-remove-symlink-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        let directory = root.join("userscripts");
        let outside = root.join("outside");
        std::fs::create_dir_all(&directory).expect("create userscript directory");
        std::fs::create_dir_all(&outside).expect("create outside directory");
        std::fs::write(
            directory.join("page.toml"),
            r#"
schema_version = 1
name = "page"
executable = "/bin/true"
"#,
        )
        .expect("write userscript manifest");
        symlink(&outside, directory.join("page.assets")).expect("create asset symlink");

        let error = remove(&root, "page").expect_err("symlinked assets must be refused");
        assert!(error.contains("real directory"));
        assert!(directory.join("page.toml").is_file());
        assert!(outside.is_dir());
        std::fs::remove_file(directory.join("page.assets")).expect("remove asset symlink");
        std::fs::remove_dir_all(root).expect("remove symlink safety directory");
    }

    #[test]
    fn registered_actions_are_stable_and_removed_with_the_manifest() {
        let root = std::env::temp_dir().join(format!(
            "ferric-browser-userscript-actions-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        let directory = root.join("userscripts");
        std::fs::create_dir_all(&directory).expect("create userscript action directory");
        std::fs::write(
            directory.join("video.toml"),
            r#"
schema_version = 1
name = "video"
executable = "/bin/true"
context_fields = ["url"]
[[actions]]
id = "userscript.video.send-link"
subject = "link"
verb = "send"
label = "Send link to video player"
required_fields = ["url"]
"#,
        )
        .expect("write userscript action manifest");

        let actions = registered_actions(&root).expect("load registered action");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "userscript.video.send-link");
        assert_eq!(actions[0].required_fields, vec!["url"]);

        std::fs::remove_file(directory.join("video.toml")).expect("remove manifest");
        assert!(
            registered_actions(&root)
                .expect("requery after removal")
                .is_empty()
        );
        std::fs::remove_dir_all(root).expect("remove userscript action directory");
    }

    #[test]
    fn install_manifest_copies_relative_page_assets_without_overwriting() {
        let root = std::env::temp_dir().join(format!(
            "ferric-browser-userscript-install-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        let bundle = root.join("bundle");
        std::fs::create_dir_all(&bundle).expect("create userscript bundle");
        std::fs::write(
            bundle.join("page.toml"),
            r#"
schema_version = 1
name = "page"
executable = "/bin/true"
matches = ["https://example.test/*"]
page_world = true
source = "page.js"
"#,
        )
        .expect("write source manifest");
        std::fs::write(bundle.join("page.js"), "document.title = 'installed';")
            .expect("write source asset");

        assert_eq!(
            install_manifest(&root, &bundle.join("page.toml")).unwrap(),
            "page"
        );
        let installed = load(&root, "page").expect("load installed manifest");
        assert_eq!(
            installed.manifest.source.as_deref(),
            Some("page.assets/page.js")
        );
        assert_eq!(
            matching_page_scripts(&root, "https://example.test/path", false)
                .expect("load installed page source")[0]
                .source,
            "document.title = 'installed';"
        );
        let second = install_manifest(&root, &bundle.join("page.toml"));
        assert!(
            second.is_err(),
            "installer must not replace an installed script"
        );
        std::fs::remove_dir_all(root).expect("remove installed userscript test");
    }
}
