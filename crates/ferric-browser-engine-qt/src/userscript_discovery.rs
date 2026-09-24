//! Read-only userscript inventory, registered actions, and page matching.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::userscript::{
    FrameScope, MAX_PAGE_SCRIPT_BYTES, MAX_PAGE_SCRIPT_MANIFESTS, MAX_REGISTERED_ACTIONS, RunAt,
    load, validate_name,
};

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

pub(crate) fn validate_url_pattern(pattern: &str) -> Result<(), String> {
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

pub(crate) fn url_pattern_matches(pattern: &str, url: &str) -> bool {
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
