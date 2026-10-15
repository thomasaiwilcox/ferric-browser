//! Readable diagnostic facts; raw records stay in the explicitly reviewed report.
use serde_json::Value;

pub(super) fn engine_notice(status: &str) -> (&'static str, &'static str) {
    match status {
        "qualified" => ("qualified", ""),
        "blocked" => (
            "blocked",
            "This web engine version is blocked. Update it with your system package manager.",
        ),
        "unqualified" => (
            "unqualified",
            "This web engine version is outside the browser's compatibility checks. Review Diagnostics before relying on it.",
        ),
        _ => (
            "unavailable",
            "The web engine version couldn't be checked. Review Diagnostics.",
        ),
    }
}

pub(super) fn summary(snapshot: &Value, private: bool) -> Vec<String> {
    let version = |path| {
        snapshot
            .pointer(path)
            .and_then(Value::as_str)
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 64
                    && value.chars().all(|c| c.is_ascii_digit() || c == '.')
            })
            .unwrap_or("Unavailable")
    };
    let engine = match snapshot
        .pointer("/build/engine_update_policy/status")
        .and_then(Value::as_str)
    {
        Some("qualified") => "Covered by current compatibility checks",
        Some("blocked") => "Blocked version — update required",
        Some("unqualified") => "Outside current compatibility checks",
        _ => "Version checks unavailable",
    };
    let storage = if private {
        "Private session — kept in memory"
    } else {
        match snapshot
            .pointer("/storage/health/status")
            .and_then(Value::as_str)
        {
            Some("available")
                if snapshot
                    .pointer("/storage/health/value")
                    .and_then(Value::as_str)
                    == Some("ok") =>
            {
                "Integrity check passed"
            }
            Some("available") => "Result unavailable — inspect report details",
            Some("missing") => "No database has been created",
            Some("unavailable") => "Could not be checked",
            Some("corrupt") => "Integrity check failed — preserve the profile and review details",
            Some("unsupported") => "Requires a newer Ferric Browser",
            _ => "Not checked",
        }
    };
    vec![
        format!("Ferric Browser: {}", version("/application/version")),
        format!("Web engine: {}", version("/build/qt_webengine/value")),
        format!("Compatibility: {engine}"),
        format!("Profile storage: {storage}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn storage_summary_distinguishes_observed_results() {
        for (status, value, expected) in [
            ("available", "ok", "Integrity check passed"),
            ("available", "unexpected", "Result unavailable"),
            ("corrupt", "failed", "preserve the profile"),
            ("unsupported", "ok", "newer Ferric"),
            ("missing", "not-created", "No database"),
            ("unavailable", "not-checked", "Could not be checked"),
        ] {
            let snapshot = json!({"storage":{"health":{"status":status,"value":value}}});
            assert!(summary(&snapshot, false).join("\n").contains(expected));
        }
    }

    #[test]
    fn absent_and_untrusted_facts_do_not_look_successful() {
        let snapshot = json!({"application":{"version":"<b>secret</b>"},"build":{"qt_webengine":{"value":"6.11.2"},"engine_update_policy":{"status":"invented"}}});
        let text = summary(&snapshot, false).join("\n");
        assert!(!text.contains("secret"));
        assert!(text.contains("6.11.2"));
        assert!(text.contains("Version checks unavailable"));
        assert!(text.contains("Not checked"));
        assert!(
            summary(&snapshot, true)
                .join("\n")
                .contains("kept in memory")
        );
        assert_eq!(engine_notice("unexpected").0, "unavailable");
    }
}
