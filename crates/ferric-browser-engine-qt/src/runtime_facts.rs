//! Qt runtime/build facts normalized for diagnostics.
//!
//! This module is deliberately Qt-type-free. The bridge gathers raw strings;
//! these functions validate and place those strings in the diagnostic schema.

use serde_json::Value;

pub(super) fn apply_runtime_facts(
    snapshot: &mut Value,
    platform: String,
    backend: String,
    webengine_version: String,
    chromium_version: String,
    chromium_security_patch_version: String,
) {
    let backend_fact = if backend.is_empty() || backend == "unknown" {
        serde_json::json!({
            "status": "unknown",
            "value": null,
            "reason": "Qt has not exposed a selected graphics backend",
            "provenance": "runtime-qt"
        })
    } else {
        serde_json::json!({
            "status": "available",
            "value": backend,
            "reason": "reported by QQuickWindow::graphicsApi",
            "provenance": "runtime-qt"
        })
    };
    if let Some(runtime) = snapshot.get_mut("runtime").and_then(Value::as_object_mut) {
        runtime.insert(
            "window_system".into(),
            serde_json::json!({
                "status": if platform.is_empty() { "unknown" } else { "available" },
                "value": if platform.is_empty() { Value::Null } else { Value::String(platform) },
                "reason": "reported by QGuiApplication::platformName",
                "provenance": "runtime-qt"
            }),
        );
        runtime.insert("graphics_backend".into(), backend_fact);
    }
    apply_webengine_fact(
        snapshot,
        webengine_version,
        chromium_version,
        chromium_security_patch_version,
    );
}

pub(super) fn apply_webengine_fact(
    snapshot: &mut Value,
    version: String,
    chromium: String,
    security_patch: String,
) {
    let fact = if version.is_empty() {
        serde_json::json!({
            "status": "unknown",
            "value": null,
            "reason": "QtWebEngine did not expose its public module version",
            "provenance": "compile-time-qtwebengine"
        })
    } else {
        serde_json::json!({
            "status": "available",
            "value": version,
            "reason": "reported by the public QtWebEngineCore version header",
            "provenance": "compile-time-qtwebengine"
        })
    };
    if let Some(build) = snapshot.get_mut("build").and_then(Value::as_object_mut) {
        build.insert("qt_webengine".into(), fact);
        build.insert(
            "chromium_base".into(),
            chromium_fact(
                chromium,
                "reported by QtWebEngine's public Chromium version API",
            ),
        );
        build.insert(
            "chromium_security_patch".into(),
            chromium_fact(
                security_patch,
                "reported by QtWebEngine's public Chromium security-patch API",
            ),
        );
    }
}

pub(super) fn chromium_fact(value: String, reason: &str) -> Value {
    if value.is_empty() {
        serde_json::json!({
            "status": "unknown",
            "value": null,
            "reason": "QtWebEngine returned an empty Chromium version",
            "provenance": "runtime-qtwebengine"
        })
    } else if !is_bounded_runtime_version(&value) {
        serde_json::json!({
            "status": "unknown",
            "value": null,
            "reason": "QtWebEngine returned an invalid or unbounded Chromium version",
            "provenance": "runtime-qtwebengine"
        })
    } else {
        serde_json::json!({
            "status": "available",
            "value": value,
            "reason": reason,
            "provenance": "runtime-qtwebengine"
        })
    }
}

fn is_bounded_runtime_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
        && value.split('.').all(|part| !part.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chromium_facts_are_bounded_and_explicit() {
        let available = chromium_fact("140.0.7339.225".into(), "runtime API");
        assert_eq!(available["status"], "available");
        assert_eq!(available["value"], "140.0.7339.225");
        assert_eq!(available["provenance"], "runtime-qtwebengine");

        let unavailable = chromium_fact(String::new(), "runtime API");
        assert_eq!(unavailable["status"], "unknown");
        assert!(unavailable["value"].is_null());
        assert_eq!(unavailable["provenance"], "runtime-qtwebengine");

        let malformed = chromium_fact("140.0.7339.225\nsecret".into(), "runtime API");
        assert_eq!(malformed["status"], "unknown");
        assert!(malformed["value"].is_null());
    }
}
