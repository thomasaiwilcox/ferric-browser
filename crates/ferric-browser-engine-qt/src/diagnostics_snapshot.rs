use crate::{
    compatibility,
    diagnostics::{
        SCHEMA_VERSION, dictionary_probe, display_fact, engine_update_policy, fact,
        gpu_driver_fact, installed_engine_package_fact, media_manifest, pipewire_probe,
        portal_probe, qt_runtime_fact, sandbox_fact, secret_storage_policy,
        software_rendering_fact,
    },
    link_cleaning_policy, maintenance,
};
use ferric_browser_config::{ThemePalette, theme_contrast_report};
use serde_json::{Value, json};

#[must_use]
#[allow(clippy::too_many_lines)]
fn unknown_compositor_fact() -> Value {
    json!({
        "status": "not-tested",
        "value": null,
        "reason": "compositor identity/version is not read on the Qt UI thread",
        "provenance": "not-probed"
    })
}

pub fn snapshot() -> Value {
    snapshot_with_compositor(unknown_compositor_fact())
}

/// Adds runtime clipboard capability evidence when a live Qt application is
/// available. The display-free CLI intentionally leaves this capability
/// unprobed because it has no `QGuiApplication` or compositor session.
pub fn snapshot_with_primary_selection(available: Option<bool>) -> Value {
    let mut snapshot = snapshot();
    if let Some(available) = available {
        snapshot["capabilities"]["primary_selection"] = if available {
            fact(
                "available",
                Some("Qt QClipboard::Selection"),
                "the active Qt clipboard supports the Wayland primary-selection mode",
                "observed",
            )
        } else {
            fact(
                "unavailable",
                None,
                "the active Qt clipboard does not support the Wayland primary-selection mode",
                "observed",
            )
        };
    }
    snapshot
}

/// Builds a snapshot with a caller-supplied compositor fact. The live Qt
/// surface uses [`snapshot`] so diagnostics never perform compositor IPC on
/// the GUI thread; the display-free CLI supplies its bounded probe result.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn snapshot_with_compositor(compositor: Value) -> Value {
    let portal = portal_probe();
    let pipewire = pipewire_probe();
    let qt_runtime = qt_runtime_fact();
    let engine_package = installed_engine_package_fact();
    let engine_policy = engine_update_policy(&qt_runtime);
    let dictionaries = dictionary_probe();
    let theme_contrast = serde_json::to_value(theme_contrast_report(&ThemePalette::default()))
        .unwrap_or_else(|_| {
            json!({
                "status": "unknown",
                "checks": [],
                "failing": [],
                "reason": "contrast report unavailable"
            })
        });
    json!({
        "schema": SCHEMA_VERSION,
        "application": {
            "name": "ferric-browser",
            "version": env!("CARGO_PKG_VERSION"),
            "commit": fact(
                "not-tested",
                None,
                "release metadata was not supplied to this build",
                "not-probed"
            )
        },
        "build": {
            "rust": fact(
                "configured",
                Some("Rust 1.85+"),
                "workspace rust-version constraint",
                "configured"
            ),
            "bridge": fact(
                "configured",
                Some("CXX-Qt 0.10.0"),
                "workspace dependency pin",
                "configured"
            ),
            "qt_compile": fact(
                "available",
                Some("Qt 6"),
                "Qt Quick and Qt WebEngineQuick modules are linked",
                "observed"
            ),
            "qt_runtime": qt_runtime,
            "engine_package": engine_package,
            "user_agent": compatibility::user_agent_snapshot(),
            "chromium_base": fact(
                "not-tested",
                None,
                "the browser user agent is not an authoritative patch report",
                "not-probed"
            ),
            "chromium_security_patch": fact(
                "not-tested",
                None,
                "the installed QtWebEngine package probe does not expose Chromium security-patch metadata",
                "not-probed"
            ),
            "engine_update_policy": engine_policy
        },
        "runtime": {
            "display": display_fact(),
            "native_wayland": fact(
                "not-tested",
                None,
                "compositor-visible verification is not part of this snapshot",
                "not-probed"
            ),
            "compositor": compositor,
            "graphics_backend": fact(
                "unknown",
                None,
                "Qt selected backend is not exposed by this adapter",
                "not-probed"
            ),
            "software_rendering": software_rendering_fact(),
            "gpu_driver": gpu_driver_fact(),
            "sandbox": sandbox_fact(),
            "portal": portal["service"].clone(),
            "pipewire": pipewire.clone()
        },
        "capabilities": {
            "webengine": fact(
                "available",
                Some("QtWebEngineQuick"),
                "the compiled adapter links the public WebEngineView module",
                "observed"
            ),
            "request_interception": fact(
                "available",
                Some("QWebEngineUrlRequestInterceptor"),
                "the public profile interceptor adapter is attached to live profiles",
                "observed"
            ),
            "network_blocking": fact(
                "not-tested",
                None,
                "no blocklist snapshot is loaded yet",
                "not-probed"
            ),
            "gpu_decode": if software_rendering_fact()["status"] == "degraded" {
                fact(
                    "unavailable",
                    None,
                    "GPU compositing is explicitly disabled for this session",
                    "observed",
                )
            } else {
                fact(
                    "not-tested",
                    None,
                    "GPU compositing must not be used as a decode inference",
                    "not-probed",
                )
            },
            "hardware_decode": fact(
                "not-tested",
                None,
                "GPU compositing and media decode are reported independently; no decode matrix has run",
                "not-probed"
            ),
            "codecs": fact(
                "not-tested",
                None,
                "codec playback matrix is not yet run",
                "not-probed"
            ),
            "drm": fact(
                "not-tested",
                None,
                "DRM playback matrix is not yet run",
                "not-probed"
            ),
            "media_manifest": media_manifest(),
            "webauthn": fact(
                "not-tested",
                None,
                "WebAuthn transport qualification is not yet run",
                "not-probed"
            ),
            "system_audio": fact(
                "not-tested",
                None,
                "system-audio capture selection and PipeWire stream ownership are not yet qualified",
                "not-probed"
            ),
            "page_picture_in_picture": fact(
                "not-tested",
                Some("QtWebEngine"),
                "page media picture-in-picture has no separately qualified browser-owned surface",
                "not-probed"
            ),
            "document_picture_in_picture": fact(
                "not-tested",
                Some("QtWebEngine"),
                "document picture-in-picture capability and window ownership have not been qualified",
                "not-probed"
            ),
            "media_session": fact(
                "not-tested",
                Some("MPRIS Player.PlayPause"),
                "the scoped MPRIS bridge is compiled; live session-bus registration and desktop-player discovery require a running session",
                "not-probed"
            ),
            "printing_pdf": fact(
                "available",
                Some("QtWebEngine"),
                "the adapter uses the public PDF printing API",
                "observed"
            ),
            "notifications": fact(
                "not-tested",
                None,
                "notification delivery is not yet qualified",
                "not-probed"
            ),
            "push": fact(
                "not-tested",
                None,
                "push behavior is not yet qualified",
                "not-probed"
            ),
            "spellcheck": dictionaries,
            "primary_selection": fact(
                "not-tested",
                None,
                "primary-selection availability is session/compositor dependent and has not been probed here",
                "not-probed"
            ),
            "per_site_settings": fact(
                "available",
                Some("Rust origin policy"),
                "the browser-owned origin policy exposes only bounded public per-site settings",
                "observed"
            ),
            "portal_file_chooser": portal["interfaces"]["file_chooser"].clone(),
            "portal_screen_cast": portal["interfaces"]["screen_cast"].clone(),
            "portal_open_uri": portal["interfaces"]["open_uri"].clone(),
            "portal_notifications": portal["interfaces"]["notifications"].clone(),
            "pipewire": pipewire
        },
        "storage": {
            "health": fact(
                "not-tested",
                None,
                "profile storage health is checked on open, not in this snapshot",
                "not-probed"
            )
        },
        "workarounds": compatibility::diagnostic_snapshot(),
        "link_cleaning": link_cleaning_policy::diagnostic_snapshot(None),
        "maintenance_traffic": maintenance::snapshot(None, false, false),
        "theme": {
            "contrast": theme_contrast,
            "user_theme_remains_importable": true,
            "security_surfaces": "opaque semantic surfaces retain readable text requirements"
        },
        "recent_errors": fact(
            "available",
            Some("empty-standalone-audit"),
            "standalone diagnostics has no live action-audit owner; a running instance supplies bounded recent categories through diagnostics.get",
            "standalone-process"
        ),
        "request_resolutions": {
            "status": "available",
            "counts": [],
            "reason": "bounded Qt request resolution outcomes; live GUI counters are reported by the IPC surface"
        },
        "privacy": {
            "browsing_urls": "excluded",
            "account_names": "excluded",
            "cookies": "excluded",
            "credential_paths": "excluded",
            "private_session_data": "excluded",
            "page_console_logs": "excluded"
        },
        "secret_storage": secret_storage_policy(),
        "logging": {
            "default_level": "off",
            "page_console_logs": "excluded",
            "navigation_traces": "excluded",
            "debug_redaction": "mandatory",
            "reason": "diagnostics never enable page logging or raw navigation tracing"
        },
        "export": {
            "status": "preview",
            "requires_explicit_user_action": true,
            "automatic_upload": false,
            "redaction": "sensitive values and URL query data are excluded"
        }
    })
}
