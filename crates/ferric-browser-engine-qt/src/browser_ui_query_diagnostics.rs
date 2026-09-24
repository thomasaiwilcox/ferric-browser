//! Diagnostics and event-subscription query projections.

#[allow(clippy::wildcard_imports)]
use super::*;

impl qobject::BrowserUi {
    pub(super) fn ipc_diagnostics(&self, params: &Value) -> Result<Value, String> {
        query_empty_object_or_null(params, "diagnostics.get")?;
        let mut result = diagnostics::snapshot_with_primary_selection(Some(
            qobject::ferric_browser_primary_selection_available(),
        ));
        apply_qt_runtime_facts(&mut result);
        result["crash"] = self.rust().storage_roots.as_ref().map_or_else(
            || {
                serde_json::json!({
                    "schema": 1,
                    "marker": {
                        "status": "unavailable",
                        "reason": "the active profile has no durable storage root",
                        "provenance": "not-probed"
                    }
                })
            },
            crash_diagnostics,
        );
        result["protocol"] = serde_json::json!({
            "major": ferric_browser_ipc::PROTOCOL_MAJOR,
            "minor": ferric_browser_ipc::PROTOCOL_MINOR
        });
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
        result["link_cleaning"] =
            link_cleaning_policy::diagnostic_snapshot(self.rust().storage_roots.as_ref());
        let config =
            serde_json::from_value::<Config>(self.rust().config.clone()).unwrap_or_default();
        let private_profile = self
            .rust()
            .state
            .as_ref()
            .and_then(|state| {
                let tab = state.active_tab()?;
                let profile = state.profiles().get(&tab.profile)?;
                Some(profile.privacy.is_transient())
            })
            .unwrap_or(false);
        result["maintenance_traffic"] = maintenance::snapshot(
            Some(&self.rust().config),
            !private_profile,
            self.rust().storage_roots.is_some(),
        );
        let contrast = load_theme_palette(&config.theme)
            .map(|palette| serde_json::to_value(theme_contrast_report(&palette)))
            .ok()
            .and_then(Result::ok)
            .unwrap_or_else(|| {
                serde_json::json!({
                    "status": "unknown",
                    "checks": [],
                    "failing": [],
                    "reason": "the configured theme palette could not be assessed"
                })
            });
        result["theme"] = serde_json::json!({
            "contrast": contrast,
            "user_theme_remains_importable": true,
            "security_surfaces": "opaque semantic surfaces retain readable text requirements"
        });
        if let (Some(roots), Some(profile_id)) =
            (self.rust().storage_roots.as_ref(), self.rust().profile_id)
        {
            result["storage"]["health"] = diagnostics::storage_health(
                roots
                    .data
                    .join("profiles")
                    .join(profile_id.to_string())
                    .join("browser.sqlite"),
            );
        }
        Ok(result)
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
