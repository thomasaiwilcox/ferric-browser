use crate::hyprland;
use ferric_browser_config::{HyprlandConfig, TriState};
use ferric_browser_storage::inspect_store;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

const ACTION_ERROR_RECORD_LIMIT: usize = 128;
const ACTION_ERROR_CATEGORY_LIMIT: usize = 32;
const ACTION_ERROR_CATEGORIES: &[&str] = &[
    "invalid-subject-or-parameters",
    "not-found",
    "stale-target",
    "confirmation-required",
    "missing-capability",
    "denied-source-or-privacy",
    "cancelled",
    "executor-failure",
];

pub(crate) fn hyprland_version_fact() -> Value {
    let adapter = hyprland::HyprlandAdapter::from_config(&HyprlandConfig {
        enabled: TriState::On,
        workspace_routing: true,
    });
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return json!({
            "status": "unknown",
            "value": null,
            "reason": "the active compositor does not expose a Hyprland instance signature",
            "provenance": "runtime-hyprland"
        });
    }
    match adapter.version() {
        Ok(version) => json!({
            "status": "available",
            "value": version,
            "reason": "Hyprland j/version IPC response was observed",
            "provenance": "runtime-hyprland"
        }),
        Err(error) => hyprland_version_error_fact(error),
    }
}

fn hyprland_version_error_fact(_error: impl std::fmt::Display) -> Value {
    json!({
        "status": "unavailable",
        "value": null,
        "reason": "the Hyprland version probe failed",
        "provenance": "runtime-hyprland"
    })
}

/// Returns a privacy-safe health fact for one Rust-owned database. The probe
/// is read-only and deliberately omits the profile path and user metadata.
#[must_use]
pub fn storage_health(path: impl AsRef<Path>) -> Value {
    let inspection = inspect_store(path);
    json!({
        "status": inspection.status,
        "value": inspection.integrity,
        "reason": inspection.reason,
        "provenance": "observed",
        "schema_version": inspection.schema_version,
        "recovery": inspection.recovery
    })
}

/// Summarizes the bounded in-memory action audit without returning operation,
/// action, argument, URL, or page data. The caller owns the audit ring; this
/// helper only exposes stable failure categories for diagnostics.
#[must_use]
pub fn action_error_summary(records: &[Value]) -> Value {
    let mut counts = BTreeMap::<String, u64>::new();
    let mut total = 0_u64;
    for record in records.iter().rev().take(ACTION_ERROR_RECORD_LIMIT) {
        let failed = matches!(
            record.get("outcome").and_then(Value::as_str),
            Some("failed" | "rejected")
        );
        if !failed {
            continue;
        }
        let category = record
            .get("category")
            .and_then(Value::as_str)
            .filter(|value| ACTION_ERROR_CATEGORIES.contains(value))
            .unwrap_or("unknown")
            .to_owned();
        *counts.entry(category).or_default() += 1;
        total += 1;
    }
    let categories = counts
        .into_iter()
        .take(ACTION_ERROR_CATEGORY_LIMIT)
        .map(|(category, count)| json!({"category": category, "count": count}))
        .collect::<Vec<_>>();
    json!({
        "status": "available",
        "categories": categories,
        "total": total,
        "limit": ACTION_ERROR_RECORD_LIMIT,
        "reason": "bounded in-memory action failures are summarized by redacted category",
        "provenance": "runtime-memory"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_compositor_probe_diagnostic_uses_a_fixed_reason() {
        let fact =
            hyprland_version_error_fact("private /run/user/1000/hyprland.sock: profile secret");
        assert_eq!(fact["reason"], "the Hyprland version probe failed");
        assert!(!fact.to_string().contains("/run/user/1000/hyprland.sock"));
        assert!(!fact.to_string().contains("profile secret"));
    }

    #[test]
    fn action_error_summary_keeps_only_closed_failure_categories() {
        let summary = action_error_summary(&[json!({
            "outcome": "failed",
            "category": "private /home/person/profile.sqlite"
        })]);
        assert!(!summary.to_string().contains("/home/person/profile.sqlite"));
        assert_eq!(summary["categories"][0]["category"], "unknown");

        let stable = action_error_summary(&[json!({
            "outcome": "failed",
            "category": "stale-target"
        })]);
        assert_eq!(stable["categories"][0]["category"], "stale-target");
    }
}
