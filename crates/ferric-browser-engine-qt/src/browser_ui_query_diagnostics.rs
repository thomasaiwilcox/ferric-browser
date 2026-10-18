//! Diagnostics and event-subscription query projections.

#[allow(clippy::wildcard_imports)]
use super::*;

const SAFE_INTERACTION_ERROR_CATEGORIES: &[&str] = &[
    "stale-target",
    "wrong-owner",
    "wrong-frame",
    "blocked-by-overlay",
    "not-interactable",
    "ambiguous-proxy",
    "unsupported-geometry",
    "unsupported-capability",
    "invalid-selector",
    "selector-validation-unavailable",
    "config-revision-changed",
    "native-prompt-active",
    "dispatch-expired",
    "dispatch-uncertain",
    "traversal-limit",
    "no-visible-targets",
];

fn interaction_error_summary(records: &[Value]) -> Value {
    let mut summary = action_error_summary(records);
    if let Some(categories) = summary.get("categories").and_then(Value::as_array) {
        let mut counts = BTreeMap::<String, u64>::new();
        for entry in categories {
            let category = entry
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let safe_category = if SAFE_INTERACTION_ERROR_CATEGORIES.contains(&category) {
                category
            } else {
                "other"
            };
            let count = entry.get("count").and_then(Value::as_u64).unwrap_or(0);
            *counts.entry(safe_category.to_owned()).or_default() += count;
        }
        summary["categories"] = Value::Array(
            counts
                .into_iter()
                .map(|(category, count)| serde_json::json!({"category": category, "count": count}))
                .collect(),
        );
    }
    summary["reason"] = Value::String(
        "bounded in-memory failures are summarized by allowlisted redacted category".into(),
    );
    summary
}

impl qobject::BrowserUi {
    pub(super) fn ipc_interaction_status(&self, params: &Value) -> Result<Value, String> {
        query_empty_object_or_null(params, "interaction.status")?;
        let rust = self.rust();
        let active_index = rust.active_tab_index;
        let current_url = rust
            .tab
            .and_then(|tab| rust.state.as_ref()?.tabs().get(&tab))
            .and_then(|tab| tab.url.as_deref())
            .unwrap_or("about:blank");
        let focus_rows = rust
            .focus_observations
            .iter()
            .filter(|((index, _), observation)| {
                *index == active_index
                    && observation.epoch == rust.input_epoch
                    && observation.url == current_url
            })
            .take(16)
            .collect::<Vec<_>>();
        let editable_count = focus_rows
            .iter()
            .filter(|(_, observation)| observation.editable)
            .count();
        let focus_category = if focus_rows.is_empty() {
            "unknown"
        } else if editable_count == 0 {
            "page"
        } else if editable_count == focus_rows.len() {
            "editable"
        } else {
            "mixed"
        };
        let active_prompt = rust.native_prompt_leases.top();
        let prompt_owner = active_prompt.map(|lease| {
            serde_json::json!({
                "view_bound": lease.owner.view.is_some(),
                "document_known": lease.owner.document.is_some()
            })
        });
        let shortcut_policy = rust.mixed_policy.shortcut_policy.as_str();
        let shortcut_policy_source =
            rust.mixed_policy
                .shortcut_policy_source
                .map_or("default", |source| match source {
                    ferric_browser_core::MixedPolicySource::Global => "global",
                    ferric_browser_core::MixedPolicySource::Profile => "profile",
                    ferric_browser_core::MixedPolicySource::Site => "site",
                    ferric_browser_core::MixedPolicySource::Runtime => "runtime",
                });
        let site_rule_config = serde_json::from_value::<Config>(rust.config.clone()).ok();
        let site_rule_for =
            |setting: &str, source: Option<ferric_browser_core::MixedPolicySource>| {
                if source != Some(ferric_browser_core::MixedPolicySource::Site) {
                    return None;
                }
                let config = site_rule_config.as_ref()?;
                matching_site_rules(config, &rust.current_url.to_string())
                    .into_iter()
                    .rev()
                    .find(|rule| rule.set.contains_key(setting))
                    .map(|rule| rule.id.clone())
            };
        Ok(serde_json::json!({
            "schema_version": 1,
            "mode": super::ipc_mode_name(rust.core_mode),
            "input_epoch": rust.input_epoch,
            "configuration_revision": rust.config_revision,
            "shortcut_policy_revision": rust.policy_revision,
            "target": {
                "document_known": rust.tab.and_then(|tab| rust.page_document_identities.get(tab)).is_some()
            },
            "focus": {
                "category": focus_category,
                "frame_count": focus_rows.len()
            },
            "shortcut_policy": {
                "value": shortcut_policy,
                "source": shortcut_policy_source,
                "page_key_count": rust.mixed_policy.page_keys.len().min(64),
                "policy_site_rule": site_rule_for(
                    "input.shortcut_policy",
                    rust.mixed_policy.shortcut_policy_source
                ),
                "page_keys_site_rule": site_rule_for(
                    "input.page_keys",
                    rust.mixed_policy.page_keys_source
                )
            },
            "scroll_policy": {
                "status": "unknown",
                "policy": null,
                "source": "page-local policy is not retained by the native projection"
            },
            "native_prompt": {
                "active": rust.native_prompt_leases.is_active(),
                "category": rust.native_prompt_leases.active_category().unwrap_or("none"),
                "depth": rust.native_prompt_leases.depth(),
                "limit": super::native_prompt_lease::MAX_NATIVE_PROMPT_DEPTH,
                "last_outcome": rust.native_prompt_leases.last_result(),
                "owner": prompt_owner,
            },
            "recent_action_errors": interaction_error_summary(&rust.action_audit),
            "privacy": "on-demand bounded facts; no URL, title, page label, DOM data, key stream, or input values"
        }))
    }

    pub(super) fn refresh_interaction_status(self: Pin<&mut Self>) -> QString {
        match self.as_ref().ipc_interaction_status(&Value::Null) {
            Ok(value) => QString::from(value.to_string()),
            Err(_) => QString::from("{}"),
        }
    }

    pub(super) fn capture_diagnostic_input(&self) -> diagnostic_report::ReportInput {
        let mut result = serde_json::json!({"build":{},"runtime":{},"capabilities":{}});
        apply_qt_runtime_facts(&mut result);
        diagnostics_snapshot::apply_primary_selection(
            &mut result,
            Some(qobject::ferric_browser_primary_selection_available()),
        );
        result["build"]["engine_update_policy"] =
            diagnostics::engine_update_policy(&result["build"]["qt_webengine"]);
        result["protocol"] = serde_json::json!({
            "major": ferric_browser_ipc::PROTOCOL_MAJOR,
            "minor": ferric_browser_ipc::PROTOCOL_MINOR
        });
        result["profile_startup"] = self
            .rust()
            .profile_startup
            .snapshot(self.rust().profile_bootstrap_pending);
        result["private_state"] = serde_json::json!("memory-only");
        result["macros"] = macro_state_value(self.rust());
        result["action_audit"] = serde_json::json!({
            "status": "available",
            "records": &self.rust().action_audit,
            "limit": MAX_ACTION_AUDIT_RECORDS,
            "fields": "action_id, operation_id, outcome, and redacted category only"
        });
        result["recent_errors"] = action_error_summary(&self.rust().action_audit);
        result["error_correlations"] = serde_json::json!({
            "status": "available",
            "records": ferric_browser_ipc::recent_error_correlations(),
            "limit": 32,
            "fields": "correlation_id and stable error code only",
            "provenance": "process-memory"
        });
        result["request_resolutions"] = serde_json::json!({
            "status": "available",
            "counts": self
                .rust()
                .request_resolution_counts
                .iter()
                .map(|((kind, outcome), count)| {
                    serde_json::json!({
                        "kind": kind,
                        "outcome": outcome,
                        "count": count
                    })
                })
                .collect::<Vec<_>>(),
            "reason": "bounded Qt request resolution outcomes; stale and duplicate responses are no-ops"
        });
        if let Ok(blocking) = self.ipc_blocking_status(&Value::Null) {
            let loaded_lists = blocking
                .get("loaded_lists")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let skipped_lists = blocking
                .get("skipped_lists")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let blocked_rules = blocking
                .get("blocked_rules")
                .and_then(Value::as_u64)
                .map_or(0, |value| usize::try_from(value).unwrap_or(usize::MAX));
            result["capabilities"]["network_blocking"] = diagnostics::network_blocking_fact(
                blocking
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                loaded_lists,
                skipped_lists,
                blocked_rules,
            );
        }
        let private_profile = self.active_profile_is_transient();
        result["maintenance_traffic"] = maintenance::snapshot(
            Some(&self.rust().config),
            !private_profile,
            self.rust().storage_roots.is_some(),
        );
        diagnostic_report::ReportInput {
            key: self.diagnostics_key(),
            live: result,
            roots: self.rust().storage_roots.clone(),
            profile: self.rust().profile_id,
            config: self.rust().config.clone(),
            private: private_profile,
        }
    }

    pub(super) fn ipc_events_subscribe(&self, params: &Value) -> Result<Value, String> {
        Self::ipc_event_filter(params)?;
        Ok(serde_json::json!({
            "status": "subscribed",
            "sequence": self.rust().ipc_sequence
        }))
    }

    pub(super) fn ipc_event_filter(params: &Value) -> Result<Option<Vec<String>>, String> {
        let object = params
            .as_object()
            .ok_or_else(|| "events.subscribe params must be an object".to_owned())?;
        if object.keys().any(|key| key != "event_types") {
            return Err("events.subscribe contains an unknown field".into());
        }
        if let Some(event_types) = object.get("event_types") {
            let types = event_types
                .as_array()
                .ok_or_else(|| "event_types must be an array".to_owned())?;
            if types.len() > 32
                || types.iter().any(|event_type| {
                    event_type.as_str().is_none_or(|event_type| {
                        event_type.is_empty()
                            || event_type.len() > 128
                            || event_type.chars().any(char::is_control)
                    })
                })
            {
                return Err(
                    "event_types must contain at most 32 nonempty strings without control characters"
                        .into(),
                );
            }
            return Ok((!types.is_empty()).then(|| {
                types
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            }));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod interaction_status_tests {
    use super::interaction_error_summary;
    use serde_json::json;

    #[test]
    fn interaction_errors_are_bounded_allowlisted_and_drop_audit_identifiers() {
        let records = vec![
            json!({
                "action_id": "action-private",
                "operation_id": "operation-private",
                "outcome": "failed",
                "category": "https://example.test/path?token=private"
            }),
            json!({
                "action_id": "another-private-action",
                "operation_id": "another-private-operation",
                "outcome": "rejected",
                "category": "stale-target"
            }),
        ];

        let summary = interaction_error_summary(&records);
        assert_eq!(summary["total"], 2);
        assert_eq!(summary["categories"].as_array().map(Vec::len), Some(2));
        assert_eq!(summary["categories"][0]["category"], "other");
        assert_eq!(summary["categories"][1]["category"], "stale-target");
        let encoded = summary.to_string();
        for private in [
            "action-private",
            "operation-private",
            "example.test",
            "token=private",
        ] {
            assert!(!encoded.contains(private));
        }
        assert!(summary["categories"].to_string().len() < 512);
    }
}
