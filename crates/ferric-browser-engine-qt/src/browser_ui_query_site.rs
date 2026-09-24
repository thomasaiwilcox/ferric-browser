//! Site, blocking, and diagnostic-status query projections.

#[allow(clippy::wildcard_imports)]
use super::*;

impl qobject::BrowserUi {
    pub(super) fn ipc_site_status(&self, params: &Value) -> Result<Value, String> {
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let tab = match params {
            Value::Null => state.active_tab(),
            Value::Object(object) => {
                if object.keys().any(|key| key != "tab") {
                    return Err("site.status contains an unknown field".into());
                }
                match object.get("tab") {
                    None => state.active_tab(),
                    Some(Value::String(id))
                        if !id.is_empty()
                            && id.len() <= 128
                            && !id.chars().any(char::is_control) =>
                    {
                        let tab = state
                            .tabs()
                            .values()
                            .find(|candidate| candidate.id.to_string() == *id)
                            .ok_or_else(|| "site.status tab target is stale".to_owned())?;
                        if tab.existence != ExistenceState::Live {
                            return Err("site.status tab target is stale".into());
                        }
                        Some(tab)
                    }
                    Some(_) => return Err("site.status tab must be a bounded stable tab ID".into()),
                }
            }
            _ => return Err("site.status params must be an object or null".into()),
        };
        let window = tab.and_then(|tab| state.windows().get(&tab.window));
        let profile = tab.and_then(|tab| state.profiles().get(&tab.profile));
        let private = tab
            .and_then(|tab| state.profiles().get(&tab.profile))
            .is_some_and(|profile| profile.privacy.is_transient());
        let origin = tab
            .and_then(|tab| tab.url.as_deref())
            .and_then(safe_site_origin);
        let current_site_url = tab
            .and_then(|tab| tab.url.clone())
            .unwrap_or_else(|| self.rust().current_url.to_string());
        let permission_metadata_ready = private
            || (self.rust().storage_library.is_some() && !self.rust().storage_library_dirty.get());
        let permission_rules = if private {
            Vec::new()
        } else {
            self.rust()
                .storage_library
                .as_ref()
                .into_iter()
                .flat_map(|library| library.permissions.iter())
                .filter(|rule| origin.as_deref().is_none_or(|origin| rule.origin == origin))
                .map(|rule| {
                    serde_json::json!({
                        "permission": rule.permission,
                        "decision": rule.decision,
                        "expires_at": rule.expires_at,
                        "updated_at": rule.updated_at
                    })
                })
                .collect::<Vec<_>>()
        };
        let mut blocking = self.ipc_blocking_status(&Value::Null)?;
        if private {
            if let Some(blocking) = blocking.as_object_mut() {
                blocking.insert("active_site_blocked_requests".into(), Value::from(0));
                blocking.insert("active_site_explanation".into(), serde_json::json!({}));
                blocking.insert("active_site_decisions".into(), serde_json::json!([]));
            }
        }
        let userscripts_available = if private || origin.is_none() {
            false
        } else {
            self.rust().userscript_roots.as_ref().is_some_and(|roots| {
                userscript::matching_page_scripts(&roots.config, &current_site_url, false)
                    .is_ok_and(|scripts| !scripts.is_empty())
            })
        };
        let userscript_inventory = if private {
            serde_json::json!({
                "status": "unavailable",
                "reason": "private-session userscript metadata is withheld",
                "provenance": "privacy-boundary"
            })
        } else if let Some(roots) = self.rust().userscript_roots.as_ref() {
            match userscript::installed_scripts(&roots.config) {
                Ok(installed) => serde_json::json!({
                    "status": "available",
                    "installed": installed,
                    "matching_active_site": userscripts_available,
                    "provenance": "profile-userscript-manifests"
                }),
                Err(reason) => serde_json::json!({
                    "status": "unavailable",
                    "reason": bounded_navigation_failure_detail(&reason),
                    "provenance": "profile-userscript-manifests"
                }),
            }
        } else {
            serde_json::json!({
                "status": "not-configured",
                "installed": [],
                "matching_active_site": false,
                "provenance": "profile-userscript-manifests"
            })
        };
        let compatibility = compatibility::diagnostic_snapshot_for_host_and_engine(
            origin.as_deref().and_then(safe_site_host).as_deref(),
            Some(qt_webengine_version().as_str()),
        );
        let mut site_doctor_experiments = Vec::new();
        if !private && self.rust().blocking_enabled && origin.is_some() {
            site_doctor_experiments.push("blocking-bypass");
        }
        if userscripts_available {
            site_doctor_experiments.push("userscripts-off");
        }
        let compiled_defaults_available = if private || origin.is_none() {
            false
        } else {
            serde_json::from_value::<Config>(self.rust().config.clone()).is_ok_and(|config| {
                matching_site_rules(&config, &current_site_url)
                    .iter()
                    .any(|rule| rule.set.keys().any(|key| setting_supports_site_scope(key)))
            })
        };
        if compiled_defaults_available {
            site_doctor_experiments.push("compiled-defaults");
        }
        if !private && origin.is_some() {
            site_doctor_experiments.push("fresh-view");
        }
        let active_site_experiment = self
            .rust()
            .active_site_experiment
            .as_ref()
            .map(|experiment| {
                serde_json::json!({
                    "id": experiment.id,
                    "kind": experiment.kind,
                    "tab": experiment.tab.to_string(),
                    "temporary_tab": experiment.temporary_tab.map(|tab| tab.to_string()),
                    "origin": experiment.origin,
                    "url": experiment.url,
                    "state": "pending",
                    "timeout_seconds": SITE_DOCTOR_EXPERIMENT_TIMEOUT.as_secs(),
                    "remaining_seconds": site_doctor_remaining_seconds(experiment.created_at, Instant::now())
                        .unwrap_or(0),
                })
            })
            .unwrap_or(Value::Null);
        let mut diagnostics = diagnostics::snapshot_with_primary_selection(Some(
            qobject::ferric_browser_primary_selection_available(),
        ));
        apply_qt_runtime_facts(&mut diagnostics);
        let facts = vec![
            serde_json::json!({
                "id": "network.blocking",
                "value": blocking,
                "provenance": "profile/runtime",
                "scope": "profile/site",
                "apply_time": "live",
                "state": "active",
                "capability": "available",
                "remediation_actions": ["blocking-toggle", "blocklist-update"]
            }),
            serde_json::json!({
                "id": "permissions",
                "value": {
                    "durable_rules": permission_rules,
                    "ready": permission_metadata_ready,
                    "session_grants": [],
                    "pending_requests": []
                },
                "provenance": "permission-store/runtime",
                "scope": "profile/origin",
                "apply_time": "live",
                "state": "active",
                "capability": "available",
                "remediation_actions": ["permission-reset"]
            }),
            serde_json::json!({
                "id": "engine.webengine",
                "value": diagnostics["capabilities"]["webengine"].clone(),
                "provenance": "engine-observation",
                "scope": "application",
                "apply_time": "compiled",
                "state": "active",
                "capability": "available",
                "remediation_actions": ["diagnostics"]
            }),
            serde_json::json!({
                "id": "capture",
                "value": {
                    "status": "unavailable",
                    "reason": "active capture is not exposed by the pinned public Qt adapter",
                    "provenance": "not-probed"
                },
                "provenance": "engine-observation",
                "scope": "tab/document",
                "apply_time": "live",
                "state": "unknown",
                "capability": "unavailable",
                "remediation_actions": []
            }),
            serde_json::json!({
                "id": "page.userscripts",
                "value": userscript_inventory,
                "provenance": "runtime",
                "scope": "profile/site",
                "apply_time": "live",
                "state": "unknown",
                "capability": "unknown",
                "remediation_actions": []
            }),
            serde_json::json!({
                "id": "compatibility.workarounds",
                "value": compatibility,
                "provenance": "compiled-reviewed-data",
                "scope": "application/site",
                "apply_time": "compiled",
                "state": "active",
                "capability": "available",
                "remediation_actions": ["diagnostics"]
            }),
        ];
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "private": private,
            "url": if private { Value::Null } else { Value::String(tab.and_then(|tab| tab.url.as_deref()).map_or_else(|| "about:blank".into(), safe_ipc_url)) },
            "origin": if private { Value::Null } else { origin.as_ref().map_or(Value::Null, |origin| Value::String(origin.clone())) },
            "title": if private { Value::Null } else { Value::String(tab.map_or_else(String::new, |tab| tab.title.replace(['\n', '\r'], " "))) },
            "capture": {
                "tab_id": tab.map(|tab| tab.id.to_string()),
                "window_id": tab.map(|tab| tab.window.to_string()),
                "document_id": tab.map(|tab| tab.document.to_string()),
                "generation": tab.map(|tab| tab.generation),
                "profile_id": if private { Value::Null } else { tab.map_or(Value::Null, |tab| Value::String(tab.profile.to_string())) }
            },
            "profile": {
                "id": if private { Value::Null } else { tab.map_or(Value::Null, |tab| Value::String(tab.profile.to_string())) },
                "label": if private { Value::String("Private".into()) } else { profile.map_or(Value::Null, |profile| Value::String(profile.label.clone())) },
                "privacy": if private { "private" } else { "normal" }
            },
            "context": window.and_then(|window| window.context.clone()).map_or(Value::Null, Value::String),
            "loading": tab.map(|tab| format!("{:?}", tab.loading).to_ascii_lowercase()),
            "renderer": tab.map(|tab| format!("{:?}", tab.renderer).to_ascii_lowercase()),
            "blocking": blocking,
            "facts": facts,
            "safe_remediation_actions": ["blocking-toggle", "blocklist-update", "permission-reset", "diagnostics"],
            "site_doctor": {
                "available": !private && origin.is_some(),
                "experiments": site_doctor_experiments,
                "active": if private {
                    Value::Null
                } else {
                    active_site_experiment
                },
                "last_result": if private {
                    Value::Null
                } else {
                    self.rust()
                        .last_site_doctor_result
                        .clone()
                        .unwrap_or(Value::Null)
                },
                "reason": if private {
                    "private sessions do not expose durable site experiments"
                } else if origin.is_none() {
                    "the active document has no eligible HTTP(S) origin"
                } else if !self.rust().blocking_enabled && !userscripts_available {
                    "one-shot fresh same-profile view is available"
                } else if !userscripts_available {
                    "one-shot blocker and fresh-view experiments are available"
                } else {
                    "one-shot blocker, userscript, and fresh-view experiments are available"
                }
            }
        }))
    }

    pub(super) fn ipc_site_report(&self, include_host: bool) -> Result<Value, String> {
        let ledger = self.ipc_site_status(&Value::Null)?;
        let private = ledger
            .get("private")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let origin_host = if include_host && !private {
            ledger
                .get("origin")
                .and_then(Value::as_str)
                .and_then(safe_site_host)
                .map_or(Value::Null, Value::String)
        } else {
            Value::Null
        };
        let facts = ledger
            .get("facts")
            .and_then(Value::as_array)
            .map(|facts| {
                facts
                    .iter()
                    .filter_map(|fact| {
                        let object = fact.as_object()?;
                        Some(serde_json::json!({
                            "id": object.get("id")?,
                            "state": object.get("state")?,
                            "capability": object.get("capability")?,
                            "provenance": object.get("provenance")?,
                            "scope": object.get("scope")?,
                            "apply_time": object.get("apply_time")?,
                            "status": object.get("value").and_then(Value::as_object).and_then(|value| value.get("status")).cloned().unwrap_or(Value::Null)
                        }))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let blocking = ledger
            .get("blocking")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let active_decisions = blocking
            .get("active_site_decisions")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let has_security_deny_decision = blocking
            .get("active_site_decisions")
            .and_then(Value::as_array)
            .is_some_and(|decisions| {
                decisions.iter().any(|decision| {
                    decision.get("reason").and_then(Value::as_str) == Some("security deny rule")
                })
            });
        let security_deny_rules = security_deny_rule_count(&self.rust().config_json.to_string());
        let mut matched_rule_ids = Vec::new();
        if active_decisions > 0 {
            matched_rule_ids.push("blocking.host-rule");
        }
        if has_security_deny_decision {
            matched_rule_ids.push("blocking.security-deny-host");
        }
        let loaded_lists = blocking
            .get("loaded_lists")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let active_experiment = ledger
            .get("site_doctor")
            .and_then(Value::as_object)
            .and_then(|doctor| doctor.get("active"))
            .filter(|active| !active.is_null())
            .cloned()
            .unwrap_or(Value::Null);
        let last_experiment_result = ledger
            .get("site_doctor")
            .and_then(Value::as_object)
            .and_then(|doctor| doctor.get("last_result"))
            .cloned()
            .unwrap_or(Value::Null);
        let mut diagnostics = diagnostics::snapshot_with_primary_selection(Some(
            qobject::ferric_browser_primary_selection_available(),
        ));
        apply_qt_runtime_facts(&mut diagnostics);
        let quirk_ids = compatibility::load()
            .ok()
            .map(|registry| {
                compatibility::active_ids(
                    &registry,
                    ledger
                        .get("origin")
                        .and_then(Value::as_str)
                        .and_then(safe_site_host)
                        .as_deref(),
                    None,
                )
            })
            .unwrap_or_default();
        Ok(serde_json::json!({
            "schema": 1,
            "correlation_id": format!("site-report-{}", Uuid::new_v4().simple()),
            "application": diagnostics["application"].clone(),
            "engine": diagnostics["capabilities"]["webengine"].clone(),
            "platform": {
                "display": diagnostics["runtime"]["display"].clone(),
                "compositor": diagnostics["runtime"]["compositor"].clone(),
                "graphics_backend": diagnostics["runtime"]["graphics_backend"].clone(),
                "software_rendering": diagnostics["runtime"]["software_rendering"].clone()
            },
            "site": {
                "private": private,
                "host": origin_host
            },
            "facts": facts,
            "blocking": {
                "enabled": blocking.get("enabled").cloned().unwrap_or(Value::Null),
                "security_deny_rules": security_deny_rules,
                "loaded_list_ids": loaded_lists,
                "list_metadata": blocking
                    .get("list_metadata")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!([])),
                "active_decision_count": active_decisions,
                "matched_rule_ids": matched_rule_ids
            },
            "quirk_ids": quirk_ids,
            "experiment": {
                "active": active_experiment,
                "last_result": last_experiment_result
            },
            "excluded": [
                "cookies", "tokens", "form_text", "dom", "full_request_urls",
                "account_identifiers", "profile_names", "private_session_data"
            ]
        }))
    }

    pub(super) fn ipc_blocking_status(&self, params: &Value) -> Result<Value, String> {
        query_empty_object_or_null(params, "blocking.status")?;
        let list_value = |value: &QStringList| {
            Value::Array(
                value
                    .iter()
                    .map(|entry| Value::String(entry.to_string()))
                    .collect(),
            )
        };
        let json_array = |value: &QString| {
            serde_json::from_str::<Value>(&value.to_string())
                .ok()
                .filter(Value::is_array)
                .unwrap_or_else(|| Value::Array(Vec::new()))
        };
        let blocked_rules = list_value(&self.rust().blocking_hosts);
        let exception_rules = list_value(&self.rust().blocking_exceptions);
        let (active_explanation, active_decisions) =
            self.rust().blocking_active_evidence.json_values();
        let security_deny_rules = security_deny_rule_count(&self.rust().config_json.to_string());
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "enabled": self.rust().blocking_enabled,
            "status": self.rust().status_text.to_string(),
            "bypass_sites": list_value(&self.rust().blocking_bypass_sites),
            "loaded_lists": json_array(&self.rust().blocking_loaded_lists),
            "list_metadata": json_array(&self.rust().blocking_list_metadata),
            "skipped_lists": json_array(&self.rust().blocking_skipped_lists),
            "blocked_rules": blocked_rules.as_array().map_or(0, Vec::len),
            "exception_rules": exception_rules.as_array().map_or(0, Vec::len),
            "blocked_requests": self.rust().blocking_blocked_count.max(0),
            "active_site_blocked_requests": self.rust().blocking_active_site_count.max(0),
            "unknown_context_requests": self.rust().blocking_unknown_context_count.max(0),
            "security_deny_rules": security_deny_rules,
            "active_site_explanation": active_explanation,
            "active_site_decisions": active_decisions
        }))
    }

    pub(super) fn blocking_status_message(&self) -> String {
        let Ok(status) = self.ipc_blocking_status(&Value::Null) else {
            return "Blocking status unavailable".into();
        };
        let enabled = status
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let loaded = status
            .get("loaded_lists")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let skipped = status
            .get("skipped_lists")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let rules = status
            .get("blocked_rules")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let blocked = status
            .get("blocked_requests")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let security_deny_rules = status
            .get("security_deny_rules")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let explanation = status
            .get("active_site_explanation")
            .and_then(Value::as_object)
            .and_then(|value| {
                Some(format!(
                    " · last blocked {} by {}",
                    value.get("resource_host")?.as_str()?,
                    value.get("matched_rule")?.as_str()?
                ))
            })
            .unwrap_or_default();
        format!(
            "Blocking {} · {rules} rules · {security_deny_rules} security-deny rules · {blocked} blocked · {loaded} lists loaded · {skipped} skipped{explanation}",
            if enabled { "on" } else { "off" }
        )
    }

    pub(super) fn ipc_operations_query(&self, params: &Value) -> Result<Value, String> {
        operations_query_value(self.rust(), params)
    }
}
