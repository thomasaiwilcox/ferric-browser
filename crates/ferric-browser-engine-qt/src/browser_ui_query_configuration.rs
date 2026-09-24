//! Configuration and runtime-fact query projections.

#[allow(clippy::wildcard_imports)]
use super::*;

impl qobject::BrowserUi {
    pub(super) fn ipc_config_get(&self, params: &Value) -> Result<Value, IpcQueryError> {
        let object = query_object(params, "config.get", &["key", "url", "explain"])
            .map_err(IpcQueryError::Invalid)?;
        let key = object
            .get("key")
            .and_then(Value::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 256 && !key.chars().any(char::is_control))
            .ok_or_else(|| IpcQueryError::Invalid("config.get requires a nonempty key".into()))?;
        if let Some(url) = object.get("url")
            && !url.is_null()
            && url.as_str().is_none()
        {
            return Err(IpcQueryError::Invalid(
                "config.get url must be a string or null".into(),
            ));
        }
        let requested_url = object
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty());
        if requested_url.is_some_and(|url| {
            url.len() > 8192
                || url.chars().any(char::is_control)
                || !url.starts_with("http://") && !url.starts_with("https://")
        }) {
            return Err(IpcQueryError::Invalid(
                "config.get url must be a bounded HTTP(S) URL".into(),
            ));
        }
        if let Some(explain) = object.get("explain")
            && explain.as_bool().is_none()
        {
            return Err(IpcQueryError::Invalid(
                "config.get explain must be a boolean".into(),
            ));
        }
        let mut value = &self.rust().config;
        for part in key.split('.') {
            value = value
                .as_object()
                .and_then(|object| object.get(part))
                .ok_or_else(|| {
                    IpcQueryError::NotFound(format!("configuration key not found: {key}"))
                })?;
        }
        if requested_url.is_some() && !setting_supports_site_scope(key) {
            return Err(IpcQueryError::Invalid(format!(
                "configuration key {key} does not support site scope"
            )));
        }
        let base_value = value.clone();
        let metadata = setting_metadata(key).ok_or_else(|| {
            IpcQueryError::Invalid(format!(
                "configuration key is not in the setting registry: {key}"
            ))
        })?;
        let pending_value = self
            .rust()
            .pending_config
            .as_ref()
            .and_then(|pending| config_value_at_path(pending, key))
            .filter(|pending| *pending != &base_value)
            .cloned();
        let mut effective_value = base_value.clone();
        let mut effective_source = self.rust().config_source.to_string();
        let mut effective_scope = "global";
        let mut contributors = vec![serde_json::json!({
            "layer": "global",
            "source": self.rust().config_source.to_string(),
            "value": config_value_at_path(&self.rust().base_config, key)
                .cloned()
                .unwrap_or_else(|| base_value.clone()),
            "status": "contributing"
        })];
        for (layer, overrides, scope) in [
            ("profile", &self.rust().profile_overrides, "profile"),
            ("runtime", &self.rust().runtime_overrides, "global"),
            ("cli", &self.rust().cli_overrides, "global"),
            ("temporary", &self.rust().temporary_overrides, "global"),
        ] {
            if let Some(layer_value) = runtime_override_value(overrides, key) {
                effective_source = layer.to_owned();
                effective_scope = scope;
                effective_value = layer_value.clone();
                contributors.push(serde_json::json!({
                    "layer": layer,
                    "value": layer_value,
                    "status": "contributing"
                }));
            }
        }
        if let Some(url) = requested_url {
            let config =
                serde_json::from_value::<Config>(self.rust().config.clone()).map_err(|error| {
                    IpcQueryError::Invalid(format!(
                        "validated configuration could not be decoded: {error}"
                    ))
                })?;
            for rule in matching_site_rules(&config, url) {
                if let Some(site_value) = rule.set.get(key)
                    && let Ok(site_value) = serde_json::to_value(site_value)
                {
                    effective_value = site_value.clone();
                    effective_source = format!("site-rule:{}", rule.id);
                    effective_scope = "site";
                    let contributor = serde_json::json!({
                        "layer": "site-rule",
                        "id": rule.id,
                        "priority": rule.priority,
                        "value": site_value,
                        "status": "contributing"
                    });
                    contributors.push(contributor);
                }
            }
        }
        let mut result = serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "key": key,
            "value": effective_value,
            "metadata": {
                "type": metadata.value_type,
                "default": metadata.default_value,
                "supported_scopes": metadata.supported_scopes,
                "apply_time": metadata.apply_time,
                "prerequisite": metadata.prerequisite,
                "sensitivity": metadata.sensitivity
            },
            "source": effective_source,
            "scope": effective_scope,
            "supported_scopes": setting_supported_scopes(key),
            "site_scope": if setting_supports_site_scope(key) {
                serde_json::json!({"supported": true, "mechanism": "validated-site-rule"})
            } else {
                serde_json::json!({
                    "supported": false,
                    "mechanism": "profile-or-global-value",
                    "reason": "changing an engine-wide value on tab focus would leak across tabs"
                })
            },
            "apply_time": metadata.apply_time,
            "restart_required": metadata.apply_time == "restart",
            "reload_required": matches!(metadata.apply_time, "reload" | "startup"),
            "pending": pending_value.is_some(),
            "pending_value": pending_value,
        });
        if object
            .get("explain")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            for contributor in &mut contributors {
                let layer = contributor.get("layer").and_then(Value::as_str);
                let source = contributor.get("source").and_then(Value::as_str);
                let site_id = contributor.get("id").and_then(Value::as_str);
                let winner = layer == Some(effective_source.as_str())
                    || source == Some(effective_source.as_str())
                    || effective_source
                        .strip_prefix("site-rule:")
                        .is_some_and(|id| layer == Some("site-rule") && site_id == Some(id));
                if winner {
                    contributor["status"] = serde_json::json!("winner");
                } else if contributor.get("status").and_then(Value::as_str) == Some("contributing")
                {
                    contributor["status"] = serde_json::json!("shadowed");
                }
            }
            result["explanation"] = serde_json::json!({
                "contributors": contributors,
                "winner": result["source"].clone(),
                "scope": result["scope"].clone()
            });
        }
        Ok(result)
    }
}
