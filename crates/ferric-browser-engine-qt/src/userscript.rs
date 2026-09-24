//! Bounded local userscript manifests, page-script metadata, and result
//! validation. This module contains no Qt calls, so its policy boundary can be
//! tested independently of a display server.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde_json::json;

pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
pub const MAX_PROTOCOL_LINE_BYTES: usize = 64 * 1024;
pub const MAX_PROTOCOL_BYTES: usize = 1024 * 1024;
pub const MAX_PAGE_SCRIPT_BYTES: u64 = 256 * 1024;
pub const MAX_ARGV_VALUE_BYTES: usize = 16 * 1024;
pub(crate) const MAX_PAGE_SCRIPT_MANIFESTS: usize = 256;
pub(crate) const MAX_REGISTERED_ACTIONS: usize = 512;
const MAX_ACTION_TEXT_BYTES: usize = 256;
pub(crate) const MAX_INSTALL_ASSET_BYTES: u64 = 16 * 1024 * 1024;

#[cfg(test)]
use crate::userscript_discovery::url_pattern_matches;
use crate::userscript_discovery::validate_url_pattern;
pub use crate::userscript_discovery::{
    InstalledScript, PageScript, RegisteredAction, installed_scripts, matching_page_scripts,
    registered_actions,
};
pub use crate::userscript_install::install_manifest;
pub use crate::userscript_manifest::{ActionManifest, FrameScope, LoadedManifest, Manifest, RunAt};
pub use crate::userscript_protocol::{
    ParsedOutput, UserscriptAction, parse_output, protocol_input,
};
pub use crate::userscript_storage::{remove, set_enabled};
pub(crate) use crate::userscript_storage::{
    set_private_directory_permissions, set_private_file_permissions,
};

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

pub(crate) fn safe_relative_asset(value: &str) -> Result<PathBuf, String> {
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

pub(crate) fn validate_page_source(path: &Path) -> Result<(), String> {
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

pub(crate) fn validate_name(name: &str) -> Result<(), String> {
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
