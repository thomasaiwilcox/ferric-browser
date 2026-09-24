use serde_json::{Map, Value, json};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(crate) const SCHEMA_VERSION: u32 = 1;
const PROBE_TIMEOUT: Duration = Duration::from_millis(750);
const PROBE_OUTPUT_LIMIT: u64 = 64 * 1024;
pub(crate) use crate::diagnostics_overview::hyprland_version_fact;
pub use crate::diagnostics_overview::{action_error_summary, storage_health};

pub(crate) fn fact(status: &str, value: Option<&str>, reason: &str, provenance: &str) -> Value {
    let mut result = json!({
        "status": status,
        "reason": reason,
        "provenance": provenance,
    });
    if let Some(value) = value {
        result["value"] = Value::String(value.to_owned());
    }
    result
}

/// Describes the live blocker state without exposing rule bodies, list URLs, or
/// page/request data.
#[must_use]
pub(crate) fn network_blocking_fact(
    enabled: bool,
    loaded_lists: usize,
    skipped_lists: usize,
    blocked_rules: usize,
) -> Value {
    if !enabled {
        return fact(
            "available",
            Some("disabled"),
            "network blocking is disabled by the active profile configuration",
            "live-profile",
        );
    }
    if loaded_lists > 0 {
        return json!({
            "status": "available",
            "value": "enabled",
            "reason": "the live profile has one or more compiled blocklists",
            "provenance": "live-profile",
            "loaded_lists": loaded_lists,
            "skipped_lists": skipped_lists,
            "blocked_rules": blocked_rules
        });
    }
    json!({
        "status": "unknown",
        "value": "enabled-without-loaded-lists",
        "reason": "network blocking is enabled but no compiled blocklist is loaded",
        "provenance": "live-profile",
        "loaded_lists": 0,
        "skipped_lists": skipped_lists,
        "blocked_rules": blocked_rules
    })
}

pub(crate) fn display_fact() -> Value {
    match std::env::var("WAYLAND_DISPLAY") {
        Ok(display) if !display.is_empty() => fact(
            "available",
            Some("wayland"),
            "WAYLAND_DISPLAY is set",
            "observed",
        ),
        _ => fact(
            "unavailable",
            Some("wayland"),
            "WAYLAND_DISPLAY is not set in this process",
            "observed",
        ),
    }
}

pub(crate) fn software_rendering_fact() -> Value {
    let quick_backend = std::env::var("QT_QUICK_BACKEND").unwrap_or_default();
    let chromium_flags = std::env::var("QTWEBENGINE_CHROMIUM_FLAGS").unwrap_or_default();
    let active = quick_backend == "software"
        || chromium_flags
            .split_whitespace()
            .any(|flag| flag == "--disable-gpu" || flag == "--disable-gpu-compositing");
    if active {
        fact(
            "degraded",
            Some("software"),
            "explicit software-rendering mode disables GPU compositing",
            "observed",
        )
    } else {
        fact(
            "not-active",
            Some("qualified graphics path"),
            "software-rendering mode was not requested",
            "observed",
        )
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn gpu_driver_fact() -> Value {
    for index in 0..8 {
        let driver_path = format!("/sys/class/drm/card{index}/device/driver");
        let Ok(driver) = std::fs::read_link(driver_path) else {
            continue;
        };
        let Some(name) = driver.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if valid_driver_name(name) {
            return fact(
                "available",
                Some(name),
                "Linux DRM sysfs reported the bound graphics driver",
                "runtime-sysfs",
            );
        }
    }
    fact(
        "unknown",
        None,
        "no bounded Linux DRM driver binding was observed",
        "runtime-sysfs",
    )
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn gpu_driver_fact() -> Value {
    fact(
        "not-tested",
        None,
        "the supported graphics-driver probe is Linux DRM-specific",
        "not-probed",
    )
}

fn valid_driver_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(target_os = "linux")]
pub(crate) fn sandbox_fact() -> Value {
    match std::fs::read_to_string("/proc/self/status") {
        Ok(status) => {
            let mut fact = sandbox_fact_from_status(&status);
            fact["renderer_helpers"] = renderer_helpers_fact(std::process::id());
            fact
        }
        Err(_) => {
            let mut result = fact(
                "unknown",
                None,
                "the Linux process-security status file is unavailable",
                "not-probed",
            );
            result["renderer_helpers"] = fact(
                "unknown",
                None,
                "the Linux process directory is unavailable for a bounded renderer-helper probe",
                "not-probed",
            );
            result
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn sandbox_fact() -> Value {
    let mut result = fact(
        "unknown",
        None,
        "this platform does not expose the bounded Linux process-security probe",
        "not-probed",
    );
    result["renderer_helpers"] = fact(
        "unknown",
        None,
        "this platform does not expose a bounded renderer-helper process probe",
        "not-probed",
    );
    result
}

#[cfg(target_os = "linux")]
fn sandbox_fact_from_status(status: &str) -> Value {
    let security = process_security_from_status(status);
    let no_new_privs = security.no_new_privs;
    let seccomp_mode = security.seccomp_mode;
    let seccomp_filters = security.seccomp_filters;

    match (no_new_privs, seccomp_mode) {
        (Some(true), Some(2)) if seccomp_filters.is_none_or(|count| count > 0) => json!({
            "status": "available",
            "value": "seccomp",
            "reason": "the current application process reports seccomp filtering and no-new-privileges",
            "provenance": "observed",
            "process": {
                "no_new_privileges": true,
                "seccomp_mode": 2,
                "seccomp_filters": seccomp_filters
            },
            "scope": "application-process"
        }),
        (Some(false), Some(0)) => fact(
            "unknown",
            Some("unconfined-application-process"),
            "the current application process reports no seccomp filter; renderer-helper posture is reported separately",
            "observed",
        ),
        _ => json!({
            "status": "unknown",
            "reason": "process security fields were incomplete or did not establish a sandboxed posture",
            "provenance": "observed",
            "process": {
                "no_new_privileges": no_new_privs,
                "seccomp_mode": seccomp_mode,
                "seccomp_filters": seccomp_filters
            },
            "scope": "application-process"
        }),
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Default)]
struct ProcessSecurity {
    parent_pid: Option<u32>,
    no_new_privs: Option<bool>,
    seccomp_mode: Option<u8>,
    seccomp_filters: Option<u32>,
}

#[cfg(target_os = "linux")]
fn process_security_from_status(status: &str) -> ProcessSecurity {
    let field = |name: &str| {
        status.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key == name).then(|| value.trim())
        })
    };
    ProcessSecurity {
        parent_pid: field("PPid").and_then(|value| value.parse::<u32>().ok()),
        no_new_privs: field("NoNewPrivs").and_then(|value| match value {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }),
        seccomp_mode: field("Seccomp").and_then(|value| match value {
            "0" => Some(0_u8),
            "1" => Some(1_u8),
            "2" => Some(2_u8),
            _ => None,
        }),
        seccomp_filters: field("Seccomp_filters").and_then(|value| value.parse::<u32>().ok()),
    }
}

#[cfg(target_os = "linux")]
fn renderer_helper_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("qtwebengineproc")
        || name.contains("qtwebengineprocess")
        || name == "chrome"
        || name == "chromium"
}

#[cfg(target_os = "linux")]
fn renderer_helper_fact_from_status(status: &str) -> Option<bool> {
    let security = process_security_from_status(status);
    match (security.no_new_privs, security.seccomp_mode) {
        (Some(true), Some(2)) if security.seccomp_filters.is_none_or(|count| count > 0) => {
            Some(true)
        }
        (Some(_), Some(_)) => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn renderer_helpers_fact(parent_pid: u32) -> Value {
    const MAX_PROC_ENTRIES: usize = 256;
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return fact(
            "unknown",
            None,
            "the Linux process directory is unavailable for a bounded renderer-helper probe",
            "not-probed",
        );
    };

    let mut observed = 0_u32;
    let mut secure = 0_u32;
    let mut incomplete = false;
    for entry in entries.flatten().take(MAX_PROC_ENTRIES) {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let status_path = entry.path().join("status");
        let Ok(status) = std::fs::read_to_string(status_path) else {
            continue;
        };
        let process = process_security_from_status(&status);
        let process_name = status.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key == "Name").then(|| value.trim())
        });
        if process.parent_pid != Some(parent_pid)
            || !process_name.is_some_and(renderer_helper_name)
            || pid == parent_pid
        {
            continue;
        }
        observed = observed.saturating_add(1);
        match renderer_helper_fact_from_status(&status) {
            Some(true) => secure = secure.saturating_add(1),
            Some(false) | None => incomplete = true,
        }
    }

    if observed == 0 {
        return json!({
            "status": "unknown",
            "reason": "no direct QtWebEngine renderer helper was observed during the bounded probe",
            "provenance": "observed",
            "scope": "renderer-helper-processes",
            "observed_helpers": 0
        });
    }
    if incomplete || secure != observed {
        return json!({
            "status": "unknown",
            "reason": "one or more observed renderer helpers did not expose a uniformly sandboxed posture",
            "provenance": "observed",
            "scope": "renderer-helper-processes",
            "observed_helpers": observed,
            "sandboxed_helpers": secure
        });
    }
    json!({
        "status": "available",
        "value": "seccomp",
        "reason": "all observed direct QtWebEngine renderer helpers report seccomp filtering and no-new-privileges",
        "provenance": "observed",
        "scope": "renderer-helper-processes",
        "observed_helpers": observed,
        "sandboxed_helpers": secure
    })
}

pub(crate) fn qt_runtime_fact() -> Value {
    match run_probe("qmake6", &["-query", "QT_VERSION"]) {
        Ok(output) => {
            let version = output.trim();
            if !version.is_empty()
                && version.split('.').count() >= 2
                && version
                    .split('.')
                    .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
            {
                fact(
                    "available",
                    Some(version),
                    "qmake6 reported the installed Qt runtime version",
                    "observed",
                )
            } else {
                fact(
                    "unavailable",
                    Some("Qt 6"),
                    "qmake6 returned no parseable Qt runtime version",
                    "observed",
                )
            }
        }
        Err(reason) => fact("unavailable", Some("Qt 6"), reason, "observed"),
    }
}

fn valid_package_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '+' | ':' | '-' | '~')
        })
}

fn package_version_from_output(output: &str, field: Option<&str>) -> Option<String> {
    let value = field
        .and_then(|field| {
            output.lines().find_map(|line| {
                let remainder = line.trim().strip_prefix(field)?.trim();
                let value = remainder.strip_prefix(':').unwrap_or(remainder).trim();
                valid_package_version(value).then(|| value.to_owned())
            })
        })
        .or_else(|| {
            let value = output.trim();
            valid_package_version(value).then(|| value.to_owned())
        })?;
    Some(value)
}

pub(crate) fn installed_engine_package_fact() -> Value {
    let probes = [
        ("pacman", &["-Qi", "qt6-webengine"][..], Some("Version")),
        (
            "dpkg-query",
            &["-W", "-f=${Version}\\n", "qt6-webengine"][..],
            None,
        ),
        (
            "rpm",
            &["-q", "--qf", "%{VERSION}-%{RELEASE}\\n", "qt6-qtwebengine"][..],
            None,
        ),
    ];
    for (program, arguments, field) in probes {
        if let Ok(output) = run_probe(program, arguments)
            && let Some(version) = package_version_from_output(&output, field)
        {
            return fact(
                "available",
                Some(&version),
                "the system package manager reported the installed QtWebEngine package version",
                "observed",
            );
        }
    }
    fact(
        "unavailable",
        Some("QtWebEngine package"),
        "no supported package manager reported an installed QtWebEngine package version",
        "observed",
    )
}

fn version_parts(value: &str) -> Option<[u32; 3]> {
    let mut parts = value.split('.').map(str::parse::<u32>);
    Some([
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ])
}

pub(crate) fn engine_update_policy(runtime: &Value) -> Value {
    let manifest = include_str!("../../../packaging/engine-version-policy.toml");
    let parsed = toml::from_str::<toml::Value>(manifest)
        .ok()
        .and_then(|value| serde_json::to_value(value).ok())
        .unwrap_or_default();
    let qualified_versions = parsed
        .get("qualified_versions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let known_bad_versions = parsed
        .get("known_bad_versions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let runtime_version = runtime.get("value").and_then(Value::as_str);
    let runtime_status = match runtime_version {
        Some(version)
            if known_bad_versions
                .iter()
                .any(|item| item.as_str() == Some(version)) =>
        {
            "blocked"
        }
        Some(version)
            if qualified_versions
                .iter()
                .any(|item| item.as_str() == Some(version)) =>
        {
            "qualified"
        }
        Some(version) if version_parts(version).is_some() => "unqualified",
        _ => "unavailable",
    };
    json!({
        "status": runtime_status,
        "runtime_qt": runtime,
        "qualified_versions": qualified_versions,
        "known_bad_versions": known_bad_versions,
        "security_patch": {
            "status": "not-tested",
            "reason": "the installed Qt/Chromium package security patch level is not exposed by this adapter"
        },
        "update_authority": "system-package-manager",
        "automatic_update": false,
        "reason": match runtime_status {
            "qualified" => "runtime Qt version matches the checked-in qualified set",
            "blocked" => "runtime Qt version is in the checked-in known-bad set",
            "unqualified" => "runtime Qt version is parseable but has not been qualified by this release",
            _ => "runtime Qt version could not be compared with the qualified set",
        },
        "provenance": "compiled-policy-and-runtime-probe"
    })
}

fn run_probe(program: &str, arguments: &[&str]) -> Result<String, &'static str> {
    run_probe_stream(program, arguments, false, false)
}

fn run_probe_stderr(program: &str, arguments: &[&str]) -> Result<String, &'static str> {
    run_probe_stream(program, arguments, true, true)
}

fn run_probe_stream(
    program: &str,
    arguments: &[&str],
    capture_stderr: bool,
    accept_failure: bool,
) -> Result<String, &'static str> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(if capture_stderr {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stderr(if capture_stderr {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => "probe executable is not installed",
            _ => "probe process could not be started",
        })?;
    let Some(mut output) = (if capture_stderr {
        child
            .stderr
            .take()
            .map(|stream| Box::new(stream) as Box<dyn Read + Send>)
    } else {
        child
            .stdout
            .take()
            .map(|stream| Box::new(stream) as Box<dyn Read + Send>)
    }) else {
        return Err("probe output is unavailable");
    };
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        output
            .by_ref()
            .take(PROBE_OUTPUT_LIMIT)
            .read_to_end(&mut bytes)
            .map(|_| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|_| "probe output could not be read")
    });
    let deadline = Instant::now() + PROBE_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("probe timed out");
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("probe process status is unavailable");
            }
        }
    };
    let output = reader.join().map_err(|_| "probe output reader stopped")??;
    if status.success() || accept_failure {
        Ok(output)
    } else {
        Err("probe process reported failure")
    }
}

fn dictionary_names(output: &str) -> Vec<String> {
    let mut in_inventory = false;
    let mut names = std::collections::BTreeSet::new();
    for line in output.lines() {
        let line = line.trim();
        if line
            .eq_ignore_ascii_case("AVAILABLE DICTIONARIES (path is not mandatory for -d option):")
        {
            in_inventory = true;
            continue;
        }
        if !in_inventory || line.is_empty() || line.chars().any(char::is_whitespace) {
            continue;
        }
        let path = Path::new(line);
        let extension = path.extension().and_then(|value| value.to_str());
        if !matches!(extension, Some("aff" | "dic")) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if stem.is_empty()
            || stem.len() > 128
            || !stem.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
        {
            continue;
        }
        names.insert(stem.to_owned());
        if names.len() == 64 {
            break;
        }
    }
    names.into_iter().collect()
}

/// Returns only bounded, path-free dictionary identifiers for the UI.  The
/// probe is deliberately read-only; `Ferric Browser` never installs dictionaries
/// as a side effect of configuring spellcheck.
pub fn installed_dictionary_names() -> Vec<String> {
    run_probe_stderr("hunspell", &["-D"])
        .map(|output| dictionary_names(&output))
        .unwrap_or_default()
}

pub(crate) fn dictionary_probe() -> Value {
    let dictionaries = installed_dictionary_names();
    if dictionaries.is_empty() {
        fact(
            "unavailable",
            Some("hunspell"),
            "hunspell found no usable dictionary files",
            "observed",
        )
    } else {
        json!({
            "status": "available",
            "value": dictionaries,
            "reason": "hunspell reported installed dictionary identifiers",
            "provenance": "observed"
        })
    }
}

fn portal_interface_version(output: &str, interface: &str) -> Option<String> {
    let mut in_interface = false;
    for line in output.lines() {
        let columns = line.split_whitespace().collect::<Vec<_>>();
        if columns.get(1) == Some(&"interface") {
            in_interface = columns.first() == Some(&interface);
            continue;
        }
        if in_interface && columns.first() == Some(&".version") {
            return columns
                .get(3)
                .filter(|value| value.chars().all(|character| character.is_ascii_digit()))
                .map(ToString::to_string);
        }
    }
    None
}

fn portal_interface_fact(output: &str, interface: &str, label: &str) -> Value {
    if let Some(version) = portal_interface_version(output, interface) {
        fact(
            "available",
            Some(&format!("{label} v{version}")),
            "portal introspection advertised the interface and version",
            "observed",
        )
    } else {
        fact(
            "unavailable",
            Some(label),
            "portal introspection did not advertise the interface",
            "observed",
        )
    }
}

pub(crate) fn portal_probe() -> Value {
    let mut interfaces = Map::new();
    let output = match run_probe(
        "busctl",
        &[
            "--user",
            "introspect",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "--no-pager",
        ],
    ) {
        Ok(output) => output,
        Err(reason) => {
            for (key, label) in [
                ("file_chooser", "FileChooser"),
                ("screen_cast", "ScreenCast"),
                ("open_uri", "OpenURI"),
                ("notifications", "Notification"),
            ] {
                interfaces.insert(
                    key.to_owned(),
                    fact("unavailable", Some(label), reason, "observed"),
                );
            }
            return json!({
                "service": fact(
                    "unavailable",
                    Some("org.freedesktop.portal.Desktop"),
                    reason,
                    "observed"
                ),
                "interfaces": interfaces,
            });
        }
    };
    for (key, interface, label) in [
        (
            "file_chooser",
            "org.freedesktop.portal.FileChooser",
            "FileChooser",
        ),
        (
            "screen_cast",
            "org.freedesktop.portal.ScreenCast",
            "ScreenCast",
        ),
        ("open_uri", "org.freedesktop.portal.OpenURI", "OpenURI"),
        (
            "notifications",
            "org.freedesktop.portal.Notification",
            "Notification",
        ),
    ] {
        interfaces.insert(
            key.to_owned(),
            portal_interface_fact(&output, interface, label),
        );
    }
    json!({
        "service": fact(
            "available",
            Some("org.freedesktop.portal.Desktop"),
            "user D-Bus introspection completed",
            "observed"
        ),
        "interfaces": interfaces,
    })
}

/// Probes the GNOME-compatible desktop setting without sourcing shell
/// configuration. Unknown schemas remain unavailable, so system mode can
/// retain its conservative fallback.
pub(crate) fn reduced_motion_probe() -> Value {
    match run_probe(
        "gsettings",
        &["get", "org.gnome.desktop.interface", "enable-animations"],
    ) {
        Ok(output) => match reduced_motion_value(&output) {
            Some(false) => json!({
                "status": "available",
                "value": false,
                "reason": "desktop animations are enabled",
                "provenance": "runtime-gsettings"
            }),
            Some(true) => json!({
                "status": "available",
                "value": true,
                "reason": "desktop animations are disabled",
                "provenance": "runtime-gsettings"
            }),
            _ => json!({
                "status": "unknown",
                "value": null,
                "reason": "desktop animation preference returned an unknown value",
                "provenance": "runtime-gsettings"
            }),
        },
        Err(reason) => json!({
            "status": "unavailable",
            "value": null,
            "reason": reason,
            "provenance": "runtime-gsettings"
        }),
    }
}

pub(crate) fn system_font_scale_probe() -> Value {
    match run_probe(
        "gsettings",
        &["get", "org.gnome.desktop.interface", "text-scaling-factor"],
    ) {
        Ok(output) => match font_scale_value(&output) {
            Some(value) => json!({
                "status": "available",
                "value": value,
                "reason": "desktop text scaling factor observed",
                "provenance": "runtime-gsettings"
            }),
            None => json!({
                "status": "unknown",
                "value": null,
                "reason": "desktop text scaling preference returned an unknown value",
                "provenance": "runtime-gsettings"
            }),
        },
        Err(reason) => json!({
            "status": "unavailable",
            "value": null,
            "reason": reason,
            "provenance": "runtime-gsettings"
        }),
    }
}

fn reduced_motion_value(output: &str) -> Option<bool> {
    match output.trim() {
        "true" => Some(false),
        "false" => Some(true),
        _ => None,
    }
}

fn font_scale_value(output: &str) -> Option<f64> {
    let value = output.trim().parse::<f64>().ok()?;
    (0.5..=3.0).contains(&value).then_some(value)
}

pub(crate) fn pipewire_probe() -> Value {
    match run_probe("pw-cli", &["info", "0"]) {
        Ok(output) if output.contains("PipeWire:Interface:Core") => {
            let version = pipewire_version_from_output(&output);
            fact(
                "available",
                version.as_deref().or(Some("PipeWire")),
                "PipeWire core introspection completed",
                "observed",
            )
        }
        Ok(_) => fact(
            "unavailable",
            Some("PipeWire"),
            "PipeWire core introspection returned no PipeWire core",
            "observed",
        ),
        Err(reason) => fact("unavailable", Some("PipeWire"), reason, "observed"),
    }
}

fn pipewire_version_from_output(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.trim().split_once(':')?;
        let value = value.trim().trim_matches('"');
        (key == "version" && valid_package_version(value)).then(|| value.to_owned())
    })
}

pub(crate) fn media_manifest() -> Value {
    let source = include_str!("../../../packaging/media-capabilities.toml");
    let fallback = || {
        json!({
            "schema": 1,
            "status": "unknown",
            "reason": "media capability manifest failed conservative validation"
        })
    };
    let Ok(manifest) = toml::from_str::<toml::Value>(source) else {
        return fallback();
    };
    if !media_manifest_is_conservative(&manifest) {
        return fallback();
    }
    serde_json::to_value(manifest).unwrap_or_else(|_| fallback())
}

fn media_manifest_is_conservative(manifest: &toml::Value) -> bool {
    let Some(table) = manifest.as_table() else {
        return false;
    };
    if table.get("schema").and_then(toml::Value::as_integer) != Some(1) {
        return false;
    }
    let qualification_status = table
        .get("qualification")
        .and_then(toml::Value::as_table)
        .and_then(|qualification| qualification.get("status"))
        .and_then(toml::Value::as_str);
    if !matches!(
        qualification_status,
        Some("not-tested" | "qualified" | "blocked")
    ) {
        return false;
    }
    ["codecs", "drm"].into_iter().all(|section| {
        table
            .get(section)
            .and_then(toml::Value::as_array)
            .is_some_and(|entries| {
                !entries.is_empty()
                    && entries.iter().all(|entry| {
                        entry
                            .as_table()
                            .and_then(|entry| entry.get("status"))
                            .and_then(toml::Value::as_str)
                            .is_some_and(|status| {
                                matches!(status, "not-tested" | "qualified" | "blocked")
                            })
                    })
            })
    })
}

pub(crate) fn secret_storage_policy() -> Value {
    json!({
        "status": "explicit-boundary",
        "built_in_vault": {
            "enabled": false,
            "status": "not-implemented",
            "reason": "V1 does not provide a password vault"
        },
        "external_password_manager": {
            "enabled": false,
            "status": "unconfigured",
            "reason": "external password-manager integration requires a separate reviewed adapter"
        },
        "authentication_requests": {
            "storage": "transient-engine-memory",
            "export": false,
            "logging": false
        },
        "userscripts_as_credential_handlers": false,
        "persistent_secret_fields": [],
        "provenance": "compiled-policy"
    })
}

/// Produces the privacy-safe, versioned diagnostic snapshot shared by the
/// display-free CLI and the live IPC surface. Values are intentionally
/// conservative: compiled support is not treated as runtime qualification.
#[cfg(test)]
use crate::diagnostics_snapshot::snapshot;
pub use crate::diagnostics_snapshot::{snapshot_with_compositor, snapshot_with_primary_selection};

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        action_error_summary, dictionary_names, engine_update_policy, font_scale_value,
        media_manifest_is_conservative, network_blocking_fact, package_version_from_output,
        pipewire_version_from_output, portal_interface_fact, portal_interface_version,
        reduced_motion_value, renderer_helper_fact_from_status, renderer_helper_name,
        sandbox_fact_from_status, snapshot, snapshot_with_primary_selection, valid_driver_name,
    };

    #[test]
    fn action_errors_are_summarized_without_audit_identifiers() {
        let records = vec![
            json!({
                "action_id": "action.secret",
                "operation_id": "op-secret",
                "outcome": "failed",
                "category": "stale-target"
            }),
            json!({
                "action_id": "action.other",
                "operation_id": "op-other",
                "outcome": "rejected",
                "category": "stale-target"
            }),
            json!({
                "action_id": "action.ok",
                "operation_id": "op-ok",
                "outcome": "accepted",
                "category": null
            }),
        ];
        let summary = action_error_summary(&records);
        assert_eq!(summary["status"], "available");
        assert_eq!(summary["total"], 2);
        assert_eq!(summary["categories"][0]["category"], "stale-target");
        let encoded = summary.to_string();
        assert!(!encoded.contains("action.secret"));
        assert!(!encoded.contains("op-secret"));
    }

    #[test]
    fn live_network_blocking_fact_exposes_only_bounded_counts() {
        let active = network_blocking_fact(true, 2, 1, 42);
        assert_eq!(active["status"], "available");
        assert_eq!(active["value"], "enabled");
        assert_eq!(active["loaded_lists"], 2);
        assert_eq!(active["blocked_rules"], 42);

        let missing = network_blocking_fact(true, 0, 2, 0);
        assert_eq!(missing["status"], "unknown");
        assert_eq!(missing["value"], "enabled-without-loaded-lists");

        let disabled = network_blocking_fact(false, 0, 0, 0);
        assert_eq!(disabled["status"], "available");
        assert_eq!(disabled["value"], "disabled");
        assert!(!disabled.to_string().contains("example"));
    }

    #[test]
    fn dictionary_inventory_redacts_paths_and_deduplicates_formats() {
        let output = "SEARCH PATH:\n/home/tom/private\nAVAILABLE DICTIONARIES (path is not mandatory for -d option):\n/usr/share/hunspell/en_US.aff\n/home/tom/private/en_US.dic\n/home/tom/private/custom dictionary.aff\n";
        assert_eq!(dictionary_names(output), vec!["en_US"]);
    }

    #[test]
    fn package_probe_parses_only_a_bounded_version_value() {
        assert_eq!(
            package_version_from_output(
                "Name            : qt6-webengine\nVersion         : 6.11.2-1\n",
                Some("Version")
            ),
            Some("6.11.2-1".into())
        );
        assert_eq!(
            package_version_from_output("6.11.2-1\n", None),
            Some("6.11.2-1".into())
        );
        assert!(package_version_from_output("/home/tom/private\n", None).is_none());
        assert!(package_version_from_output(&"a".repeat(129), None).is_none());
    }

    #[test]
    fn snapshot_has_explicit_statuses_and_privacy_contract() {
        let value = snapshot();
        assert_eq!(value["schema"], 1);
        assert!(matches!(
            value["build"]["qt_runtime"]["status"].as_str(),
            Some("available" | "unavailable")
        ));
        if value["build"]["qt_runtime"]["status"] == "available" {
            assert!(
                value["build"]["qt_runtime"]["value"]
                    .as_str()
                    .is_some_and(|version| version.split('.').count() >= 2)
            );
        }
        assert!(matches!(
            value["build"]["engine_package"]["status"].as_str(),
            Some("available" | "unavailable")
        ));
        assert_eq!(value["workarounds"]["status"], "available");
        assert_eq!(value["workarounds"]["entry_count"], 0);
        assert!(value["workarounds"]["active_ids"].is_array());
        assert_eq!(value["maintenance_traffic"]["telemetry"]["enabled"], false);
        assert_eq!(
            value["maintenance_traffic"]["crash_upload"]["enabled"],
            false
        );
        assert_eq!(
            value["maintenance_traffic"]["history_sync"]["enabled"],
            false
        );
        assert!(matches!(
            value["runtime"]["sandbox"]["status"].as_str(),
            Some("available" | "unknown")
        ));
        assert_eq!(
            value["runtime"]["sandbox"]["renderer_helpers"]["scope"],
            "renderer-helper-processes"
        );
        assert!(
            value["runtime"]["sandbox"]["renderer_helpers"]["observed_helpers"]
                .as_u64()
                .is_some()
        );
        assert_eq!(value["capabilities"]["gpu_decode"]["status"], "not-tested");
        assert_eq!(
            value["capabilities"]["hardware_decode"]["status"],
            "not-tested"
        );
        assert_eq!(
            value["capabilities"]["system_audio"]["status"],
            "not-tested"
        );
        assert_eq!(
            value["capabilities"]["page_picture_in_picture"]["status"],
            "not-tested"
        );
        assert_eq!(
            value["capabilities"]["document_picture_in_picture"]["status"],
            "not-tested"
        );
        assert_eq!(
            value["capabilities"]["media_session"]["status"],
            "not-tested"
        );
        assert_eq!(
            value["capabilities"]["media_session"]["value"],
            "MPRIS Player.PlayPause"
        );
        assert_eq!(
            value["capabilities"]["primary_selection"]["status"],
            "not-tested"
        );
        assert_eq!(
            value["capabilities"]["per_site_settings"]["status"],
            "available"
        );
        assert_eq!(value["capabilities"]["media_manifest"]["schema"], 1);
        assert_eq!(
            value["capabilities"]["media_manifest"]["qualification"]["status"],
            "not-tested"
        );
        assert_eq!(value["privacy"]["browsing_urls"], "excluded");
        assert_eq!(value["secret_storage"]["status"], "explicit-boundary");
        assert_eq!(value["request_resolutions"]["status"], "available");
        assert!(value["request_resolutions"]["counts"].is_array());
        assert_eq!(value["secret_storage"]["built_in_vault"]["enabled"], false);
        assert_eq!(
            value["secret_storage"]["authentication_requests"]["storage"],
            "transient-engine-memory"
        );
        assert_eq!(
            value["secret_storage"]["userscripts_as_credential_handlers"],
            false
        );
        assert_eq!(value["logging"]["default_level"], "off");
        assert_eq!(value["export"]["status"], "preview");
        assert_eq!(value["export"]["automatic_upload"], false);
        assert_eq!(value["theme"]["contrast"]["status"], "pass");
        assert!(
            value["theme"]["contrast"]["checks"]
                .as_array()
                .is_some_and(|checks| !checks.is_empty())
        );
        assert_eq!(value["workarounds"]["active_ids"], serde_json::json!([]));
        assert_eq!(
            value["build"]["engine_update_policy"]["update_authority"],
            "system-package-manager"
        );
        assert_eq!(
            value["build"]["engine_update_policy"]["automatic_update"],
            false
        );
        let serialized = value.to_string();
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("file://"));
    }

    #[test]
    fn primary_selection_runtime_fact_distinguishes_available_and_missing() {
        let available = snapshot_with_primary_selection(Some(true));
        assert_eq!(
            available["capabilities"]["primary_selection"]["status"],
            "available"
        );
        assert_eq!(
            available["capabilities"]["primary_selection"]["provenance"],
            "observed"
        );

        let unavailable = snapshot_with_primary_selection(Some(false));
        assert_eq!(
            unavailable["capabilities"]["primary_selection"]["status"],
            "unavailable"
        );
        assert_eq!(
            unavailable["capabilities"]["primary_selection"]["value"],
            Value::Null
        );
    }

    #[test]
    fn display_probe_never_returns_the_display_name() {
        let value = snapshot();
        let display = &value["runtime"]["display"];
        assert!(display["value"] == "wayland" || display["value"].is_null());
        assert!(
            display["value"]
                .as_str()
                .is_none_or(|value| value == "wayland")
        );
    }

    #[test]
    fn portal_probe_parses_only_known_interface_versions() {
        let output = "org.freedesktop.portal.FileChooser interface - - -\n.version property u 4 emits-change\norg.freedesktop.portal.ScreenCast interface - - -\n.version property u 6 emits-change\n";
        assert_eq!(
            portal_interface_version(output, "org.freedesktop.portal.FileChooser"),
            Some("4".into())
        );
        assert_eq!(
            portal_interface_version(output, "org.freedesktop.portal.ScreenCast"),
            Some("6".into())
        );
        assert_eq!(
            portal_interface_version(output, "org.freedesktop.portal.OpenURI"),
            None
        );
    }

    #[test]
    fn portal_interface_facts_distinguish_available_missing_and_malformed_versions() {
        let output = concat!(
            "org.freedesktop.portal.FileChooser interface - - -\n",
            ".version property u 4 emits-change\n",
            "org.freedesktop.portal.ScreenCast interface - - -\n",
            ".version property u not-a-version emits-change\n",
        );

        let file_chooser =
            portal_interface_fact(output, "org.freedesktop.portal.FileChooser", "FileChooser");
        assert_eq!(file_chooser["status"], "available");
        assert_eq!(file_chooser["value"], "FileChooser v4");

        let screen_cast =
            portal_interface_fact(output, "org.freedesktop.portal.ScreenCast", "ScreenCast");
        assert_eq!(screen_cast["status"], "unavailable");
        assert_eq!(screen_cast["value"], "ScreenCast");

        let open_uri = portal_interface_fact(output, "org.freedesktop.portal.OpenURI", "OpenURI");
        assert_eq!(open_uri["status"], "unavailable");
        assert_eq!(open_uri["value"], "OpenURI");
    }

    #[test]
    fn media_manifest_rejects_schema_and_capability_overclaims() {
        let valid: toml::Value = toml::from_str(
            "schema = 1\nqualification = { status = \"not-tested\" }\ncodecs = [{ status = \"not-tested\" }]\ndrm = [{ status = \"blocked\" }]\n",
        )
        .expect("valid media manifest");
        assert!(media_manifest_is_conservative(&valid));

        let mut wrong_schema = valid.clone();
        wrong_schema["schema"] = toml::Value::Integer(2);
        assert!(!media_manifest_is_conservative(&wrong_schema));

        let mut available_codec = valid;
        available_codec["codecs"][0]["status"] = toml::Value::String("available".into());
        assert!(!media_manifest_is_conservative(&available_codec));
    }

    #[test]
    fn reduced_motion_parser_maps_only_boolean_gsettings_values() {
        assert_eq!(reduced_motion_value("true\n"), Some(false));
        assert_eq!(reduced_motion_value("false\n"), Some(true));
        assert_eq!(reduced_motion_value("maybe\n"), None);
        assert_eq!(reduced_motion_value("true\nextra"), None);
    }

    #[test]
    fn font_scale_parser_accepts_only_bounded_numeric_values() {
        assert_eq!(font_scale_value("1.25\n"), Some(1.25));
        assert_eq!(font_scale_value("0.49"), None);
        assert_eq!(font_scale_value("3.01"), None);
        assert_eq!(font_scale_value("1.0\nextra"), None);
    }

    #[test]
    fn gpu_driver_names_are_bounded_and_path_free() {
        assert!(valid_driver_name("amdgpu"));
        assert!(valid_driver_name("i915"));
        assert!(!valid_driver_name("/sys/devices/amdgpu"));
        assert!(!valid_driver_name(""));
        assert!(!valid_driver_name(&"x".repeat(65)));
    }

    #[test]
    fn pipewire_version_parser_rejects_unbounded_values() {
        assert_eq!(
            pipewire_version_from_output(
                "object.serial: 1\ninterface.name: PipeWire:Interface:Core\nversion: \"1.6.8\"\n"
            ),
            Some("1.6.8".into())
        );
        assert_eq!(
            pipewire_version_from_output(
                "interface.name: PipeWire:Interface:Core\nversion: \"/private/path\"\n"
            ),
            None
        );
    }

    #[test]
    fn engine_update_policy_distinguishes_qualified_and_unqualified_versions() {
        let qualified = engine_update_policy(&json!({
            "status": "available",
            "value": "6.11.2"
        }));
        assert_eq!(qualified["status"], "qualified");

        let unqualified = engine_update_policy(&json!({
            "status": "available",
            "value": "6.12.0"
        }));
        assert_eq!(unqualified["status"], "unqualified");
        assert_eq!(unqualified["security_patch"]["status"], "not-tested");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sandbox_probe_is_bounded_to_process_security_fields() {
        let value = sandbox_fact_from_status(
            "Name:\tferric_browser\nNoNewPrivs:\t1\nSeccomp:\t2\nSeccomp_filters:\t3\nUid:\t1000\n",
        );
        assert_eq!(value["status"], "available");
        assert_eq!(value["value"], "seccomp");
        assert_eq!(value["scope"], "application-process");
        assert!(value.to_string().contains("seccomp_filters"));
        assert!(!value.to_string().contains("1000"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn renderer_helper_probe_requires_uniform_security_fields() {
        let secure = "Name:\tQtWebEngineProcess\nPPid:\t42\nNoNewPrivs:\t1\nSeccomp:\t2\nSeccomp_filters:\t2\n";
        let unconfined = "Name:\tQtWebEngineProcess\nPPid:\t42\nNoNewPrivs:\t0\nSeccomp:\t0\n";
        assert!(renderer_helper_name("QtWebEngineProc"));
        assert_eq!(renderer_helper_fact_from_status(secure), Some(true));
        assert_eq!(renderer_helper_fact_from_status(unconfined), Some(false));
        assert_eq!(
            renderer_helper_fact_from_status("Name:\tQtWebEngineProcess\n"),
            None
        );
    }
}
